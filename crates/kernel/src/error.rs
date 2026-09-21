use std::fmt;
use std::io;

#[derive(Debug)]
pub enum KernelError {
    Io(io::Error),
    Json(String),
    Message(String),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::Io(e) => write!(f, "{e}"),
            KernelError::Json(e) | KernelError::Message(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for KernelError {}

impl From<io::Error> for KernelError {
    fn from(e: io::Error) -> Self {
        KernelError::Io(e)
    }
}
