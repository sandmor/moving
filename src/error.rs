use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum ConnectError {
    #[error("unknown error")]
    UnknownError,
    #[error("error while parsing some data")]
    ParseError,
    #[error("out of memory")]
    InsufficientMemory,
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("the connection was rejected")]
    Authenticate,
}

#[derive(Debug, Error)]
pub enum OSError {
    #[error("reply or id error")]
    ReplyOrId(#[from] x11rb::errors::ReplyOrIdError),
    #[error("connection error")]
    ConnectionError(#[from] x11rb::errors::ConnectionError),
}