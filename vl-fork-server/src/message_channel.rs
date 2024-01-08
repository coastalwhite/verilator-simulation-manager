//! Definition of the `MessageChannel` struct

use std::io;

use crate::error::{ProtocolResult, ProtocolError};
use crate::message::Message;

/// A bidirectional UNIX-socket connection that reads whole messages or nothing
///
/// This channel is needed because it allows for polling of the UNIX-socket when messages might not
/// be fulling formed yet. For example, if a data message is sent, the data bytes might get flushed
/// before being fully and read buffer might not be able to read all bytes at once. In this case,
/// we need to save what we already read and continue from there next time.
///
/// Because nothing of this functionality is necessarily coupled to a UNIX-socket, we make it
/// generic over `T`. Where for any reading and writing functionality, the `T` needs to implement
/// [`std::io::Read`] and [`std::io::Write`].
#[derive(Debug)]
pub struct MessageChannel<T> {
    inner: T,
    buffer: Vec<u8>,
}

impl<T> MessageChannel<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
        }
    }

    /// Replace the old underlying socket with a new socket, returning the old socket.
    ///
    /// This clears the internal buffer, but keeps the allocated memory for the buffer.
    pub fn replace(&mut self, new_inner: T) -> T {
        let old_inner = std::mem::replace(&mut self.inner, new_inner);
        self.buffer.clear();
        old_inner
    }
}

impl<T: io::Read> MessageChannel<T> {
    /// Read a full message from the channel.
    pub fn recv(&mut self) -> ProtocolResult<Message> {
        use crate::message::FromReader;

        if let Err(err) = self.inner.read_to_end(&mut self.buffer) {
            if err.kind() != io::ErrorKind::WouldBlock {
                return Err(err.into());
            }
        }

        let mut reader = self.buffer.as_slice();
        let message = Message::from_reader(&mut reader)?;

        if reader.is_empty() {
            // If a message is successfully read, this should be the common case as messages are sent
            // synchronously.
            self.buffer.clear();
        } else {
            // This is more costly than just clearing the buffer, since we are copying the data,
            // still we can avoid new allocations.

            let message_length = self.buffer.len() - reader.len();
            let remaining_byte_range = message_length..self.buffer.len();

            self.buffer.copy_within(remaining_byte_range, 0);
            self.buffer.truncate(message_length);
        }

        Ok(message)
    }

    /// Attempt to read a full message from the channel.
    ///
    /// This returns `Ok(None)` if:
    /// - the UNIX-socket is non-blocking and reading a message would block
    /// - a message is not fully ready to be read yet
    pub fn try_recv(&mut self) -> ProtocolResult<Option<Message>> {
        use io::ErrorKind as K;

        self.recv().map(Option::Some).or_else(|err| match err {
            ProtocolError::Io(ref err)
                if matches!(err.kind(), K::WouldBlock | K::UnexpectedEof) =>
            {
                Ok(None)
            }
            err => Err(err),
        })
    }
}

impl<T: io::Write> MessageChannel<T> {
    // Write a message to the channel.
    pub fn send(&mut self, message: &Message) -> ProtocolResult<()> {
        use crate::message::ToWriter;

        message.to_writer(&mut self.inner)?;
        self.inner.flush()?;

        Ok(())
    }
}
