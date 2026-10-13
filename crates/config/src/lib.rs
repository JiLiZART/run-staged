//! Config discovery, parsing, and resolved per-group settings for run-staged.
//!
//! Entry point: [`load_config`]. Discovery walks from the working directory to
//! the git root checking `.run-staged.toml`, `.run-staged.json`,
//! `run-staged.toml`, `run-staged.json`, `package.json["run-staged"]`, then
//! lint-staged's static configs (see [`discover`]).

pub(crate) mod defaults;
pub mod discover;
mod error;
pub mod parse_json;
pub mod parse_toml;
pub mod types;

pub use discover::{find_config_file, load_config};
pub use error::ConfigError;
pub use parse_json::{parse_json_file, parse_lint_staged_value, parse_package_json_value};
pub use parse_toml::parse_toml_file;
pub use types::{Config, ConfigSource, ExecutionOrder, Group, PathFormat};
