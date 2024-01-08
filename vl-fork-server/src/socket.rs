use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::ProtocolResult;
use crate::message::Message;
use crate::message_channel::MessageChannel;

#[derive(Debug)]
pub struct Socket {
    channel: MessageChannel<UnixStream>,
}

impl Socket {
    pub(crate) fn new(channel: MessageChannel<UnixStream>) -> Self {
        Self { channel }
    }

    /// Generate the path for a socket with given `idx`
    pub(crate) fn new_path(path_prefix: &Path, idx: usize) -> PathBuf {
        // @Note: This can be compile-time generated to a certain extend. Maybe that is a better
        // idea than this.

        let socket_file_name = format!("sock-{idx}");
        let socket_path = path_prefix.join(socket_file_name);

        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }

        socket_path
    }

    pub(crate) fn new_server(
        socket_path: &Path,
        read_timeout: Option<Duration>,
        write_timeout: Option<Duration>,
    ) -> ProtocolResult<Socket> {
        let socket_listener = UnixListener::bind(socket_path)?;
        let (stream, _socket_addr) = socket_listener.accept()?;

        stream.set_read_timeout(read_timeout)?;
        stream.set_write_timeout(write_timeout)?;

        let channel = MessageChannel::new(stream);
        let socket = Self { channel };

        Ok(socket)
    }

    pub(crate) fn new_fork(
        server: &mut Self,
        path_prefix: &Path,
        idx: usize,
    ) -> ProtocolResult<UnixStream> {
        let socket_path = Self::new_path(path_prefix, idx + 1);

        let socket_listener = UnixListener::bind(&socket_path)?;
        server.send_message(&Message::Fork { socket_path })?;
        let (stream, _socket_addr) = socket_listener.accept()?;

        stream.set_nonblocking(true)?;
        Ok(stream)
    }

    pub(crate) fn replace_socket(&mut self, new_socket: UnixStream) -> UnixStream {
        self.channel.replace(new_socket)
    }

    pub fn read_message(&mut self) -> ProtocolResult<Message> {
        self.channel.recv()
    }

    pub fn try_read_message(&mut self) -> ProtocolResult<Option<Message>> {
        self.channel.try_recv()
    }

    pub fn send_message(&mut self, msg: &Message) -> ProtocolResult<()> {
        self.channel.send(msg)
    }
}
