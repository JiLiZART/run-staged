use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration file not found")]
    ConfigNotFound,

    #[error("invalid configuration: {0}")]
    ConfigInvalid(String),

    #[error("not a git repository")]
    NotGitRepository,

    #[error("no staged files — run 'git add' to stage files")]
    NoStagedFiles,

    #[error("no staged files matched any configured patterns")]
    NoFilesMatched,

    #[error("command not found: {0}")]
    CommandNotFound(String),

    #[error("command failed with exit code {0}")]
    CommandFailed(i32),

    #[error("command timed out after {0}")]
    Timeout(String),

    #[error("execution cancelled")]
    Cancelled,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("git error: {0}")]
    Git(String),
}
