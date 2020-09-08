use thiserror::Error;
use std::io;

#[derive(Error, Debug)]
pub enum OSError {
    #[error("unknown error")]
    UnknownError,
    #[error("error while parsing some data")]
    ParseError,
    #[error("out of memory")]
    InsufficientMemory,
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("`{0}`")]
    Other(String)
}