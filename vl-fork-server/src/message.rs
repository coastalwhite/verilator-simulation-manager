//! Definition of messages, serialization and deserialization

use std::ffi::OsString;
use std::os::unix::prelude::OsStrExt;
use std::path::PathBuf;

use crate::error::{ProtocolError, ProtocolResult};

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
        #[derive(Debug, Clone, Copy)]
        pub enum MessageVariant {
            $(
            $name = $repr,
            )+
        }

        #[repr(u8)]
        #[derive(Debug)]
        pub enum Message {
            $(
            $name$({ $($field_name: $field),+ })? = $repr,
            )+
        }

        impl Message {
            pub fn variant(&self) -> MessageVariant {
                match self {
                    $(
                    Self::$name$({ $($field_name: _),+ })? => MessageVariant::$name,
                    )+
                }
            }
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
        let vec = Vec::from_reader(reader)?;
        Ok(String::from_utf8(vec)?)
    }
}

impl ToWriter for String {
    fn to_writer(&self, writer: &mut impl std::io::Write) -> ProtocolResult<()> {
        self.as_bytes().to_writer(writer)
    }
}

impl FromReader for PathBuf {
    fn from_reader(reader: &mut impl std::io::Read) -> ProtocolResult<Self> {
        use std::os::unix::ffi::OsStringExt;

        let len = u16::from_reader(reader)?;
        let mut buf = vec![0; len.into()];
        reader.read_exact(&mut buf)?;
        Ok(PathBuf::from(OsString::from_vec(buf)))
    }
}

impl ToWriter for PathBuf {
    fn to_writer(&self, writer: &mut impl std::io::Write) -> ProtocolResult<()> {
        self.as_os_str().as_bytes().to_writer(writer)
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
        self.as_slice().to_writer(writer)
    }
}

impl ToWriter for &[u8] {
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

    16 = Fork { input: String, output: String },
    17 = Data { content: Vec<u8> },
    18 = InputWidth { width: u32 },
}
