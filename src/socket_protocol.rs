use std::fmt::Display;
use std::io;

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
        message.to_writer(&mut self.inner)?;
        self.inner.flush()?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum ProtocolError {
    Io(std::io::Error),
    Utf8(std::string::FromUtf8Error),
    InvalidMessageVariant(u8),
    StringOverflow,
    BytearrayOverflow,
    PlatformConvert(&'static str),
    Other(String),
}

pub type ProtocolResult<T> = Result<T, ProtocolError>;

impl ProtocolError {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

impl Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::Io(err) => write!(f, "IOError: {err}"),
            ProtocolError::Utf8(err) => write!(f, "Utf8: {err}"),
            ProtocolError::InvalidMessageVariant(b) => {
                write!(f, "Invalid message variant: 0x{b:02X}")
            }
            ProtocolError::StringOverflow => write!(f, "Given string is too large to be sent"),
            ProtocolError::BytearrayOverflow => {
                write!(f, "Given bytearray is too large to be sent")
            }
            ProtocolError::PlatformConvert(t) => {
                write!(f, "Platform is unable to convert {t} to usize")
            }
            ProtocolError::Other(s) => f.write_str(&s),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<std::io::Error> for ProtocolError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<std::string::FromUtf8Error> for ProtocolError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::Utf8(value)
    }
}

impl From<&'static str> for ProtocolError {
    fn from(value: &'static str) -> Self {
        Self::Other(value.to_string())
    }
}

impl From<String> for ProtocolError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

pub trait FromReader: Sized {
    fn from_reader(reader: &mut impl std::io::Read) -> ProtocolResult<Self>;
}

pub trait ToWriter: Sized {
    fn to_writer(&self, writer: &mut impl std::io::Write) -> ProtocolResult<()>;
}

macro_rules! define_messages {
    (
    $($repr:literal = $name:ident$({ $($field_name:ident: $field:ty),+ $(,) ?})? ),+ $(,)?
    ) => {
        #[repr(u8)]
        #[derive(Clone, Copy)]
        enum MessageVariant {
            $(
            $name = $repr,
            )+
        }

        #[repr(u8)]
        pub enum Message {
            $(
            $name$({ $($field_name: $field),+ })? = $repr,
            )+
        }

        impl TryFrom<u8> for MessageVariant {
            type Error = u8;
            fn try_from(v: u8) -> Result<Self, Self::Error> {
                match v {
                    $(
                    $repr => Ok(Self::$name),
                    )+
                    _ => Err(v),
                }
            }
        }

        impl FromReader for Message {
            fn from_reader(reader: &mut impl std::io::Read) -> ProtocolResult<Self> {
                let mut variant = 0u8;
                reader.read_exact(std::slice::from_mut(&mut variant))?;

                let variant = MessageVariant::try_from(variant)
                    .map_err(|v| {
                        ProtocolError::InvalidMessageVariant(v)
                    })?;

                match variant {
                    $(
                    MessageVariant::$name => {
                        Ok(Self::$name$({
                            $(
                            $field_name: <$field>::from_reader(reader)?,
                            )+
                        })?)
                    }
                    )+
                }
            }
        }

        impl ToWriter for MessageVariant {
            fn to_writer(&self, writer: &mut impl std::io::Write) -> ProtocolResult<()> {
                let variant = *self as u8;
                writer.write_all(&[variant])?;
                Ok(())
            }
        }

        impl ToWriter for Message {
            fn to_writer(&self, writer: &mut impl std::io::Write) -> ProtocolResult<()> {
                match self {
                    $(
                    Self::$name$({ $($field_name),+ })? => {
                        MessageVariant::$name.to_writer(writer)?;
                        $($(
                        $field_name.to_writer(writer)?;
                        )+)?
                    }
                    )+
                }

                Ok(())
            }
        }
    };
}

macro_rules! impl_io_for_nums {
    ($($numty:ty),+ $(,)?) => {
        $(
        impl FromReader for $numty {
            fn from_reader(reader: &mut impl std::io::Read) -> ProtocolResult<Self> {
                let mut buf = [0u8; std::mem::size_of::<$numty>()];
                reader.read_exact(&mut buf)?;
                Ok(<$numty>::from_le_bytes(buf))
            }
        }

        impl ToWriter for $numty {
            fn to_writer(&self, writer: &mut impl std::io::Write) -> ProtocolResult<()> {
                Ok(writer.write_all(&self.to_le_bytes())?)
            }
        }
        )+
    };
}

impl_io_for_nums! {
    u8,
    u16,
    u32,
    u64,
}

impl FromReader for String {
    fn from_reader(reader: &mut impl std::io::Read) -> ProtocolResult<Self> {
        let len = u16::from_reader(reader)?;
        let mut buf = vec![0; len.into()];
        reader.read_exact(&mut buf)?;
        Ok(String::from_utf8(buf)?)
    }
}

impl ToWriter for String {
    fn to_writer(&self, writer: &mut impl std::io::Write) -> ProtocolResult<()> {
        let len: u16 = self
            .len()
            .try_into()
            .map_err(|_| ProtocolError::StringOverflow)?;
        len.to_writer(writer)?;
        writer.write_all(self.as_bytes())?;

        Ok(())
    }
}

impl FromReader for Vec<u8> {
    fn from_reader(reader: &mut impl std::io::Read) -> ProtocolResult<Self> {
        let len = u32::from_reader(reader)?;
        let len = usize::try_from(len).map_err(|_| ProtocolError::PlatformConvert("u32"))?;
        let mut buf = vec![0; len];
        reader.read_exact(&mut buf)?;
        Ok(buf)
    }
}

impl ToWriter for Vec<u8> {
    fn to_writer(&self, writer: &mut impl std::io::Write) -> ProtocolResult<()> {
        let len: u32 = self
            .len()
            .try_into()
            .map_err(|_| ProtocolError::BytearrayOverflow)?;
        len.to_writer(writer)?;
        writer.write_all(self)?;

        Ok(())
    }
}

define_messages! {
    0 = Fail { msg: String },
    1 = Ack,
    2 = Exit,

    16 = Fork { socket_path: String },
    17 = Data { content: Vec<u8> },
    18 = InputWidth { width: u32 },
}
