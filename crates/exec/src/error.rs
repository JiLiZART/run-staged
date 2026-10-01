use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecError {
    #[error("invalid configuration: {0}")]
    InvalidCommand(String),

    #[error("command not found: {0}")]
    CommandNotFound(String),

    #[error("command `{command}` timed out after {duration:?}")]
    CommandTimeout {
        command: String,
        duration: std::time::Duration,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
