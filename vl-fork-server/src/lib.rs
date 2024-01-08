use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub use self::error::{ProtocolError, ProtocolResult};
pub use self::message::Message;
use self::message_channel::MessageChannel;
pub use self::socket::Socket;

mod error;
mod message;
mod message_channel;
mod socket;

/// The server that maintains the main instance of the simulations, spawns new forks and
/// communicates with the forks give them an input and get the output.
pub struct ForkServer {
    sockets_path_prefix: PathBuf,

    server: Child,
    server_socket: Socket,

    forks: Vec<Socket>,
}

impl ForkServer {
    /// Spawn a new fork server from a given `bin_path`.
    ///
    /// This binary path should point the compiled testbench that was instrumented to communicate
    /// with this server.
    pub fn new(
        bin_path: &Path,
        sockets_basedir: &Path,
        server_read_timeout: Option<Duration>,
        server_write_timeout: Option<Duration>,
    ) -> ProtocolResult<Self> {
        // Generate a random folder to put all the sockets in so that we know sockets will never
        // conflict.
        let dir_name = format!("vsm-{:016X}", rand::random::<u64>());
        let sockets_path_prefix = sockets_basedir.join(dir_name);
        std::fs::create_dir(&sockets_path_prefix)?;

        let socket_path = Socket::new_path(&sockets_path_prefix, 0);

        let mut server = Command::new(bin_path);

        server.arg(&socket_path);

        server.stdin(Stdio::null());
        server.stdout(Stdio::inherit());
        server.stderr(Stdio::inherit());

        let server = server.spawn()?;
        let server_sockets =
            Socket::new_server(&socket_path, server_read_timeout, server_write_timeout)?;

        let fork_server = Self {
            sockets_path_prefix,

            server,
            server_socket: server_sockets,

            forks: Vec::new(),
        };

        Ok(fork_server)
    }

    pub fn server_socket(&mut self) -> &mut Socket {
        &mut self.server_socket
    }

    pub fn get_fork(&mut self, at: usize) -> Option<&mut Socket> {
        self.forks.get_mut(at)
    }

    /// Create an initial fork returning the index of that fork
    pub fn create_fork(&mut self) -> ProtocolResult<usize> {
        let idx = self.forks.len();
        let stream = Socket::new_fork(&mut self.server_socket, &self.sockets_path_prefix, idx)?;
        let channel = MessageChannel::new(stream);
        self.forks.push(Socket::new(channel));

        Ok(idx)
    }

    /// Replace a fork at a given index
    pub fn replace_fork(&mut self, at: usize) -> ProtocolResult<()> {
        let stream = Socket::new_fork(&mut self.server_socket, &self.sockets_path_prefix, at)?;
        let old_stream = self.forks[at].replace_socket(stream);

        // @Q: Is this only proper to do or actually needed?
        old_stream.shutdown(std::net::Shutdown::Both)?;

        Ok(())
    }

    /// Try to finish a fork process
    ///
    /// When the fork finishes with `data`, this returns `Ok(Some(data))`. If the fork is not yet
    /// finished, it returns `Ok(None)`.
    pub fn try_finish(&mut self, at: usize) -> ProtocolResult<Option<Vec<u8>>> {
        let Some(message) = self.forks[at].try_read_message()? else {
            return Ok(None);
        };

        let Message::Data { content } = message else {
            return Err(ProtocolError::UnexpectedMessage(message));
        };

        Ok(Some(content))
    }

    pub fn clean(&mut self) -> ProtocolResult<()> {
        self.server_socket.send_message(&Message::Exit)?;
        self.server.wait()?;

        Ok(())
    }
}
