use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum ConfigSource {
    TomlFile(PathBuf),
    JsonFile(PathBuf),
    PackageJson(PathBuf),
    /// A lint-staged config file or `lint-staged` key in a package manifest.
    LintStaged(PathBuf),
    Explicit(PathBuf),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum ExecutionOrder {
    #[default]
    Parallel,
    Sequential,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum PathFormat {
    #[default]
    Relative,
    Absolute,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub name: String,
    pub patterns: HashMap<String, Vec<String>>,
    pub timeout: Option<Duration>,
    pub execution_order: ExecutionOrder,
    pub continue_on_error: bool,
    pub per_file: bool,
    pub path_format: PathFormat,
    /// Commands in this group never have their edits re-staged.
    pub read_only: bool,
    /// Maximum files per batch invocation; `None` means unlimited.
    pub chunk_size: Option<usize>,
}

impl Default for Group {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            patterns: HashMap::new(),
            timeout: None,
            execution_order: ExecutionOrder::Parallel,
            continue_on_error: false,
            per_file: false,
            path_format: PathFormat::Relative,
            read_only: false,
            chunk_size: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub source: ConfigSource,
    pub groups: Vec<Group>,
    pub timeout: Option<Duration>,
    pub execution_order: ExecutionOrder,
    pub continue_on_error: bool,
    pub per_file: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            source: ConfigSource::TomlFile(PathBuf::new()),
            groups: Vec::new(),
            timeout: None,
            execution_order: ExecutionOrder::Parallel,
            continue_on_error: false,
            per_file: false,
        }
    }
}

/// Raw intermediate struct for TOML deserialization.
/// Uses `#[serde(flatten)]` to capture named group sub-tables.
/// TOML correctly separates scalar fields from sub-tables with flatten.
#[derive(Debug, Deserialize)]
pub(crate) struct RawConfig {
    pub timeout: Option<String>,
    pub execution_order: Option<String>,
    pub continue_on_error: Option<bool>,
    pub per_file: Option<bool>,
    pub read_only: Option<bool>,
    pub chunk_size: Option<usize>,
    pub patterns: Option<HashMap<String, Vec<String>>>,
    pub path_format: Option<String>,
    #[serde(flatten)]
    pub groups: toml::map::Map<String, toml::Value>,
}

/// Raw group config for TOML deserialization.
/// `deny_unknown_fields` ensures typos in group settings are caught.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawGroupConfig {
    pub timeout: Option<String>,
    pub execution_order: Option<String>,
    pub continue_on_error: Option<bool>,
    pub per_file: Option<bool>,
    pub read_only: Option<bool>,
    pub chunk_size: Option<usize>,
    pub patterns: Option<HashMap<String, Vec<String>>>,
    pub path_format: Option<String>,
}

/// Raw intermediate struct for JSON deserialization.
/// Uses `#[serde(flatten)]` with `serde_json::Value` for named groups.
#[derive(Debug, Deserialize)]
pub(crate) struct RawJsonConfig {
    pub timeout: Option<String>,
    pub execution_order: Option<String>,
    pub continue_on_error: Option<bool>,
    pub per_file: Option<bool>,
    pub read_only: Option<bool>,
    pub chunk_size: Option<usize>,
    pub patterns: Option<HashMap<String, Vec<String>>>,
    pub path_format: Option<String>,
    #[serde(flatten)]
    pub groups: serde_json::Map<String, serde_json::Value>,
}

/// A single pattern within a group plus the files that matched it and the
/// commands to run. Produced by `run-staged-matcher`, consumed by `run-staged-exec`.
#[derive(Debug, Clone)]
pub struct MatchedPattern {
    /// Original pattern string from config (e.g. `"*.ts"`).
    pub pattern: String,
    /// Command strings from config (e.g. `["eslint --fix"]`).
    pub commands: Vec<String>,
    /// Files that matched this pattern.
    pub files: Vec<PathBuf>,
}

/// A group's matched patterns plus a borrowed reference to the source [`Group`]
/// for execution settings (timeout, execution_order, continue_on_error, ...).
#[derive(Debug, Clone)]
pub struct MatchedGroup<'a> {
    /// Source group; executor reads timeout/exec_order/etc. from here.
    pub group: &'a Group,
    /// Only patterns with at least one matched file.
    pub patterns: Vec<MatchedPattern>,
}
