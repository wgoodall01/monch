use crate::exe::Exit;
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] io::Error),

    #[error("execution of child process failed: {0}")]
    ExecutionFailed(io::Error),

    #[error("{cmd}: command not found: {source}")]
    ResolveBinary { cmd: String, source: which::Error },

    #[error("invalid working directory '{0}'")]
    BadWorkingDirectory(String),
}

impl Error {
    pub fn as_exit(&self) -> Exit {
        match self {
            Error::Io(_) => Exit::FAILURE,
            Error::ExecutionFailed(_) => Exit::COULD_NOT_EXECUTE,
            Error::ResolveBinary { .. } => Exit::COMMAND_NOT_FOUND,
            Error::BadWorkingDirectory(_) => Exit::FAILURE,
        }
    }
}
