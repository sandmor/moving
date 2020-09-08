use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OSError {
    #[error("unknown error")]
    Unknown,
    #[error("error while parsing some data")]
    Parse,
    #[error("out of memory")]
    InsufficientMemory,
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("`{0}`")]
    Other(String),
}
