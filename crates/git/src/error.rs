use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("not a git repository")]
    NotGitRepository,

    #[error("bare repositories are not supported")]
    Bare,

    #[error("no staged files — run 'git add' to stage files")]
    NoStagedFiles,

    #[error(
        "index is locked: {}\nanother git process is running, or a stale lock must be removed",
        lock.display()
    )]
    LockBusy { lock: PathBuf },

    #[error(
        "failed to restore the index from {backup}: {reason}\nrecover manually by copying {backup} over {index}",
        backup = backup.display(),
        index = index.display()
    )]
    RestoreFailed {
        backup: PathBuf,
        index: PathBuf,
        reason: String,
    },

    #[error("pre-commit hook already exists at {}\nadd `run-staged` to it by hand", path.display())]
    HookExists { path: PathBuf },

    #[error("git error: {0}")]
    Git(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Wraps any gix error as [`GitError::Git`].
pub(crate) fn git_err(e: impl std::fmt::Display) -> GitError {
    GitError::Git(e.to_string())
}
