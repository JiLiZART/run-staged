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

    #[error("command `{command}` failed with exit code {exit_code}")]
    CommandFailed { command: String, exit_code: i32 },

    #[error("command `{command}` timed out after {duration:?}")]
    CommandTimeout {
        command: String,
        duration: std::time::Duration,
    },

    #[error("execution cancelled")]
    Cancelled,

    #[error(
        "failed to restore staged diff from patch at {patch_path}: {reason}\nrun `git apply --cached {patch_path}` to recover manually"
    )]
    RestoreFailed {
        patch_path: std::path::PathBuf,
        #[source]
        reason: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("git error: {0}")]
    Git(String),
}
