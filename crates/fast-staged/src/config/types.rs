use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum ConfigSource {
    TomlFile(PathBuf),
    JsonFile(PathBuf),
    PackageJson(PathBuf),
    Explicit(PathBuf),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum ExecutionOrder {
    #[default]
    Parallel,
    Sequential,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub name: String,
    pub patterns: HashMap<String, Vec<String>>,
    pub timeout: Option<Duration>,
    pub execution_order: ExecutionOrder,
    pub continue_on_error: bool,
    pub per_file: bool,
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
    pub patterns: Option<HashMap<String, Vec<String>>>,
    #[serde(flatten)]
    pub groups: HashMap<String, toml::Value>,
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
    pub patterns: Option<HashMap<String, Vec<String>>>,
}

/// Raw intermediate struct for JSON deserialization.
/// Uses `#[serde(flatten)]` with `serde_json::Value` for named groups.
#[derive(Debug, Deserialize)]
pub(crate) struct RawJsonConfig {
    pub timeout: Option<String>,
    pub execution_order: Option<String>,
    pub continue_on_error: Option<bool>,
    pub per_file: Option<bool>,
    pub patterns: Option<HashMap<String, Vec<String>>>,
    #[serde(flatten)]
    pub groups: HashMap<String, serde_json::Value>,
}
