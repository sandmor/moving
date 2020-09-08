use crate::error::*;
use x11rb::errors::*;

impl From<ConnectionError> for OSError {
    fn from(e: ConnectionError) -> Self {
        match e {
            ConnectionError::UnknownError => OSError::UnknownError,
            ConnectionError::UnsupportedExtension => unreachable!(),
            ConnectionError::MaximumRequestLengthExceeded => OSError::Other("a request larger than the maximum X11 server request length was sent".to_owned()),
            ConnectionError::FDPassingFailed => OSError::Other("failed to pass file descriptor to the X11 server".to_owned()),
            ConnectionError::ParseError => OSError::ParseError,
            ConnectionError::InsufficientMemory => OSError::InsufficientMemory,
            ConnectionError::IOError(io) => OSError::IO(io)
        }
    }
}

impl From<ReplyError> for OSError {
    fn from(e: ReplyError) -> Self {
        match e {
            ReplyError::ConnectionError(e) => Self::from(e),
            ReplyError::X11Error(e) => OSError::Other(format!("{:?}", e)),
        }
    }
}

impl From<ReplyOrIdError> for OSError {
    fn from(e: ReplyOrIdError) -> Self {
        match e {
            ReplyOrIdError::ConnectionError(e) => Self::from(e),
            e => {
                OSError::Other(format!("{:?}", e))
            }
        }
    }
}