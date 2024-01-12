use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub use self::error::{ProtocolError, ProtocolResult};
pub use self::message::Message;
pub use self::socket::Socket;

use self::shared_memory::{Access, SharedMemory};

mod error;
mod message;
mod message_channel;
mod shared_memory;
mod socket;

#[repr(u32)]
enum Activation {
    Inactive = 0,
    Done = 1,
    Startable = 2,
    Active = 3,
}

const MAX_FORKS: usize = 512;
const FORK_ACTIVATION_SIZE: usize = MAX_FORKS * 2 * std::mem::size_of::<u32>();

/// The server that maintains the main instance of the simulations, spawns new forks and
/// communicates with the forks give them an input and get the output.
pub struct ForkServer<const MAX_INPUT_SIZE: usize, const OUTPUT_SIZE: usize> {
    sockets_path_prefix: PathBuf,

    server: Child,
    server_socket: Socket,

    fork_activation: SharedMemory<FORK_ACTIVATION_SIZE>,

    forks: Vec<(SharedMemory<MAX_INPUT_SIZE>, SharedMemory<OUTPUT_SIZE>)>,
}

impl<const MAX_INPUT_SIZE: usize, const OUTPUT_SIZE: usize>
    ForkServer<MAX_INPUT_SIZE, OUTPUT_SIZE>
{
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

        let fork_activation = SharedMemory::new(Access::ReadWrite)?;

        let activations: *mut [u32; MAX_FORKS * 2] = fork_activation.get_mut().cast();
        let activations = unsafe { activations.as_mut() }.unwrap();
        for i in 0..MAX_FORKS {
            activations[i * 2 + 0] = Activation::Inactive as u32;
            activations[i * 2 + 1] = 0;
        }

        let fork_server = Self {
            sockets_path_prefix,

            server,
            server_socket: server_sockets,

            fork_activation,

            forks: Vec::new(),
        };

        Ok(fork_server)
    }

    pub fn server_socket(&mut self) -> &mut Socket {
        &mut self.server_socket
    }

    fn fork_activations(&self) -> *mut [u32; MAX_FORKS * 2] {
        self.fork_activation.get_mut().cast()
    }

    /// Create an initial fork returning the index of that fork
    pub fn create_fork(&mut self) -> ProtocolResult<usize> {
        if self.forks.len() == MAX_FORKS {
            return Err(ProtocolError::Other(format!(
                "Maximum number of forks ({MAX_FORKS}) reached"
            )));
        }

        let idx = self.forks.len();

        let input = SharedMemory::new(Access::ReadWrite)?;
        let output = SharedMemory::new(Access::ReadOnly)?;

        self.forks.push((input, output));

        Ok(idx)
    }

    /// Start a fork at a given index
    pub fn start_fork(&mut self, at: usize, input: &[u8]) -> ProtocolResult<()> {
        if input.len() > MAX_INPUT_SIZE {
            return Err(ProtocolError::Other(format!(
                "Invalid input. Size is too large (len={} where max length is {MAX_INPUT_SIZE})",
                input.len(),
            )));
        }

        #[cfg(debug_assertions)]
        {
            let activation = unsafe { *self.fork_activations() }[at * 2];
            assert!(
                activation == Activation::Inactive as u32 || activation == Activation::Done as u32
            );
        }

        let (input_shm, _) = &self.forks[at];

        unsafe { std::slice::from_raw_parts_mut(input_shm.get_mut(), MAX_INPUT_SIZE) }
            .copy_from_slice(input);

        let input_len = input.len() as u32;

        let fork_activations = self.fork_activations();
        let fork_activations = unsafe { fork_activations.as_mut() }.unwrap();
        fork_activations[at * 2 + 1] = input_len;
        fork_activations[at * 2 + 0] = Activation::Startable as u32;

        Ok(())
    }

    /// Try to finish a fork process
    ///
    /// When the fork finishes with `data`, this returns `Ok(Some(data))`. If the fork is not yet
    /// finished, it returns `Ok(None)`.
    pub fn try_finish(&mut self, at: usize) -> ProtocolResult<Option<&[u8; OUTPUT_SIZE]>> {
        if at >= self.forks.len() {
            return Err(ProtocolError::Other(format!(
                "Finish `at` index is out of range"
            )));
        }

        if unsafe { *self.fork_activations() }[at] > 0 {
            return Ok(None);
        }

        let output: *const [u8; OUTPUT_SIZE] = self.forks[at].1.get().cast();

        Ok(Some(unsafe { output.as_ref() }.unwrap()))
    }

    pub fn clean(mut self) -> ProtocolResult<()> {
        self.fork_activation.clean()?;

        for (input, output) in self.forks {
            input.clean()?;
            output.clean()?;
        }

        self.server_socket.send_message(&Message::Exit)?;
        self.server.wait()?;

        Ok(())
    }
}
