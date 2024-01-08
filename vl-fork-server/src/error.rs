use std::fmt;

use crate::message::Message;

#[derive(Debug)]
pub enum ProtocolError {
    Io(std::io::Error),
    Utf8(std::string::FromUtf8Error),
    InvalidMessageVariant(u8),
    StringOverflow,
    BytearrayOverflow,
    PlatformConvert(&'static str),
    UnexpectedMessage(Message),
    Other(String),
}

pub type ProtocolResult<T> = Result<T, ProtocolError>;

impl ProtocolError {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
            ProtocolError::UnexpectedMessage(m) => {
                write!(f, "Message {v:?} was not expected", v = m.variant())
            }
            ProtocolError::Other(s) => f.write_str(s),
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
