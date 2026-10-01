use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("configuration file not found")]
    NotFound,

    #[error("invalid configuration: {0}")]
    Invalid(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
