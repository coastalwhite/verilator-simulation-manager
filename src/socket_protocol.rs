use std::fmt::Display;

#[derive(Debug)]
pub enum FromReaderError {
    Io(std::io::Error),
    Utf8(std::string::FromUtf8Error),
    Other(String),
}

#[derive(Debug)]
pub enum ToWriterError {
    Io(std::io::Error),
    Other(String),
}

impl Display for ToWriterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToWriterError::Io(err) => write!(f, "IOError: {err}"),
            ToWriterError::Other(s) => f.write_str(&s),
        }
    }
}

impl std::error::Error for ToWriterError {}

impl From<std::io::Error> for FromReaderError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<std::io::Error> for ToWriterError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<std::string::FromUtf8Error> for FromReaderError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::Utf8(value)
    }
}

impl From<&'static str> for ToWriterError {
    fn from(value: &'static str) -> Self {
        Self::Other(value.to_string())
    }
}

impl From<&'static str> for FromReaderError {
    fn from(value: &'static str) -> Self {
        Self::Other(value.to_string())
    }
}

impl From<String> for ToWriterError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

impl From<String> for FromReaderError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

pub trait FromReader: Sized {
    fn from_reader(reader: &mut impl std::io::Read) -> Result<Self, FromReaderError>;
}

pub trait ToWriter: Sized {
    fn to_writer(&self, writer: &mut impl std::io::Write) -> Result<(), ToWriterError>;
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
            type Error = ();
            fn try_from(v: u8) -> Result<Self, Self::Error> {
                match v {
                    $(
                    $repr => Ok(Self::$name),
                    )+
                    _ => Err(()),
                }
            }
        }

        impl FromReader for Message {
            fn from_reader(reader: &mut impl std::io::Read) -> Result<Self, FromReaderError> {
                let mut variant = 0u8;
                reader.read_exact(std::slice::from_mut(&mut variant))?;

                let variant = MessageVariant::try_from(variant).map_err(|_| format!("Invalid message variant: {variant}"))?;

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
            fn to_writer(&self, writer: &mut impl std::io::Write) -> Result<(), ToWriterError> {
                let variant = *self as u8;
                writer.write_all(&[variant])?;
                Ok(())
            }
        }

        impl ToWriter for Message {
            fn to_writer(&self, writer: &mut impl std::io::Write) -> Result<(), ToWriterError> {
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
            fn from_reader(reader: &mut impl std::io::Read) -> Result<Self, FromReaderError> {
                let mut buf = [0u8; std::mem::size_of::<$numty>()];
                reader.read_exact(&mut buf)?;
                Ok(<$numty>::from_le_bytes(buf))
            }
        }

        impl ToWriter for $numty {
            fn to_writer(&self, writer: &mut impl std::io::Write) -> Result<(), ToWriterError> {
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
    fn from_reader(reader: &mut impl std::io::Read) -> Result<Self, FromReaderError> {
        let len = u16::from_reader(reader)?;
        let mut buf = vec![0; len.into()];
        reader.read_exact(&mut buf)?;
        Ok(String::from_utf8(buf)?)
    }
}

impl ToWriter for String {
    fn to_writer(&self, writer: &mut impl std::io::Write) -> Result<(), ToWriterError> {
        let len: u16 = self.len().try_into().map_err(|_| "String length is too big to send")?;
        len.to_writer(writer)?;
        writer.write_all(self.as_bytes())?;

        Ok(())
    }
}

impl FromReader for Vec<u8> {
    fn from_reader(reader: &mut impl std::io::Read) -> Result<Self, FromReaderError> {
        let len = u32::from_reader(reader)?;
        let mut buf = vec![0; len.try_into().map_err(|_| "Platform does not support this vector size")?];
        reader.read_exact(&mut buf)?;
        Ok(buf)
    }
}

impl ToWriter for Vec<u8> {
    fn to_writer(&self, writer: &mut impl std::io::Write) -> Result<(), ToWriterError> {
        let len: u32 = self.len().try_into().map_err(|_| "Vector length is too big to send")?;
        len.to_writer(writer)?;
        writer.write_all(self)?;

        Ok(())
    }
}

define_messages! {
    0 = Fail { msg: String },
    1 = Ack,
    2 = StatusCheck,
    3 = Exit,

    16 = Fork { socket_path: String },
    17 = Data { content: Vec<u8> },
}
