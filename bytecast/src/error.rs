use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BytesError {
    BufferTooSmall { needed: usize, available: usize },
    InvalidData { message: &'static str },
    UnexpectedEof { needed: usize, available: usize },
    Custom { message: &'static str },
}

impl fmt::Display for BytesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { needed, available } => {
                write!(
                    f,
                    "buffer too small: needed {needed} bytes, only {available} available"
                )
            }
            Self::InvalidData { message } => write!(f, "invalid data: {message}"),
            Self::UnexpectedEof { needed, available } => {
                write!(
                    f,
                    "unexpected end of input: needed {needed} bytes, only {available} available"
                )
            }
            Self::Custom { message } => write!(f, "{message}"),
        }
    }
}

// Rust 1.81+
impl core::error::Error for BytesError {}

pub type Result<T> = core::result::Result<T, BytesError>;
