use std::path::Path;

use crate::AppError;

use super::defaults::{RawGroupSettings, RootDefaults, resolve_group};
use super::types::{Config, ConfigSource, ExecutionOrder, RawConfig, RawGroupConfig};

/// Parse a TOML config file from the filesystem.
pub fn parse_toml_file(path: &Path) -> Result<Config, AppError> {
    let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
    let source = ConfigSource::TomlFile(path.to_path_buf());
    parse_toml_str(&content, source, &path.display().to_string())
}

/// Parse TOML config content from a string.
/// Accepts a `source` for tracking origin and `path_display` for error messages.
pub fn parse_toml_str(
    content: &str,
    source: ConfigSource,
    path_display: &str,
) -> Result<Config, AppError> {
    let raw: RawConfig = toml::from_str(content)
        .map_err(|e| AppError::ConfigInvalid(format!("{path_display}: {e}")))?;

    let root = RootDefaults {
        timeout: &raw.timeout,
        execution_order: &raw.execution_order,
        continue_on_error: &raw.continue_on_error,
        per_file: &raw.per_file,
    };

    let mut groups = Vec::new();

    // If top-level `[patterns]` is present, create a "default" group from it.
    if let Some(patterns) = raw.patterns.clone() {
        let group = resolve_group(
            RawGroupSettings {
                patterns,
                name: "default".to_string(),
                timeout: None,
                execution_order: None,
                continue_on_error: None,
                per_file: None,
            },
            RootDefaults {
                timeout: &raw.timeout,
                execution_order: &raw.execution_order,
                continue_on_error: &raw.continue_on_error,
                per_file: &raw.per_file,
            },
        )
        .map_err(|e| AppError::ConfigInvalid(format!("{path_display}: {e}")))?;
        groups.push(group);
    }

    // Process named groups from the flattened HashMap.
    // Each entry in raw.groups is a named sub-table (e.g., [group1]).
    // We attempt to deserialize each entry as RawGroupConfig, skipping non-table values.
    for (group_name, value) in &raw.groups {
        // Skip entries that are not tables (e.g., scalar values leaked via flatten)
        if !value.is_table() {
            continue;
        }
        let raw_group: RawGroupConfig =
            value.clone().try_into().map_err(|e: toml::de::Error| {
                AppError::ConfigInvalid(format!("{path_display}: group [{group_name}]: {e}"))
            })?;

        let patterns = raw_group.patterns.unwrap_or_default();
        if patterns.is_empty() {
            // Skip groups with no patterns
            continue;
        }

        let group = resolve_group(
            RawGroupSettings {
                patterns,
                name: group_name.clone(),
                timeout: raw_group.timeout,
                execution_order: raw_group.execution_order,
                continue_on_error: raw_group.continue_on_error,
                per_file: raw_group.per_file,
            },
            RootDefaults {
                timeout: &raw.timeout,
                execution_order: &raw.execution_order,
                continue_on_error: &raw.continue_on_error,
                per_file: &raw.per_file,
            },
        )
        .map_err(|e| AppError::ConfigInvalid(format!("{path_display}: {e}")))?;

        groups.push(group);
    }

    if groups.is_empty() {
        return Err(AppError::ConfigInvalid(format!(
            "{path_display}: no patterns defined"
        )));
    }

    // Resolve top-level defaults for the Config struct
    let timeout = raw
        .timeout
        .as_ref()
        .and_then(|s| parse_duration::parse(s).ok());
    let execution_order = match raw.execution_order.as_deref() {
        None | Some("parallel") => ExecutionOrder::Parallel,
        Some("sequential") => ExecutionOrder::Sequential,
        Some(other) => {
            return Err(AppError::ConfigInvalid(format!(
                "{path_display}: unknown execution_order: {other:?}, expected \"parallel\" or \"sequential\""
            )));
        }
    };

    // Suppress unused variable warning when root is not used after groups loop
    let _ = root;

    Ok(Config {
        source,
        groups,
        timeout,
        execution_order,
        continue_on_error: raw.continue_on_error.unwrap_or(false),
        per_file: raw.per_file.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_source() -> ConfigSource {
        ConfigSource::TomlFile(std::path::PathBuf::from("test.toml"))
    }

    fn parse(content: &str) -> Result<Config, AppError> {
        parse_toml_str(content, dummy_source(), "test.toml")
    }

    #[test]
    fn test_simple_patterns_table_creates_default_group() {
        let toml = r#"
[patterns]
"*.rs" = ["cargo fmt"]
"#;
        let config = parse(toml).unwrap();
        assert_eq!(config.groups.len(), 1);
        assert_eq!(config.groups[0].name, "default");
        let cmds = config.groups[0].patterns.get("*.rs").unwrap();
        assert_eq!(cmds, &vec!["cargo fmt".to_string()]);
    }

    #[test]
    fn test_named_group_parsed_correctly() {
        let toml = r#"
[lint]
[lint.patterns]
"*.ts" = ["eslint --fix"]
"#;
        let config = parse(toml).unwrap();
        assert_eq!(config.groups.len(), 1);
        assert_eq!(config.groups[0].name, "lint");
        assert!(config.groups[0].patterns.contains_key("*.ts"));
    }

    #[test]
    fn test_root_timeout_inherited_by_default_group() {
        let toml = r#"
timeout = "5s"
[patterns]
"*.rs" = ["cargo fmt"]
"#;
        let config = parse(toml).unwrap();
        assert_eq!(
            config.groups[0].timeout,
            Some(std::time::Duration::from_secs(5))
        );
    }

    #[test]
    fn test_sequential_execution_order_parsed() {
        let toml = r#"
execution_order = "sequential"
[patterns]
"*.rs" = ["cargo fmt"]
"#;
        let config = parse(toml).unwrap();
        assert_eq!(config.execution_order, ExecutionOrder::Sequential);
    }

    #[test]
    fn test_unknown_top_level_scalar_field_is_ignored() {
        // Note: RawConfig uses #[serde(flatten)] for named groups.
        // Top-level scalar unknown fields (like foobar=true) get captured in the
        // groups HashMap but are NOT tables, so they are silently skipped.
        // Unknown fields in GROUP structs are caught by deny_unknown_fields on RawGroupConfig.
        // This is the expected behavior per RESEARCH Pitfall 1 analysis.
        let toml = r#"
foobar = true
[patterns]
"*.rs" = ["cargo fmt"]
"#;
        // Parse succeeds - foobar is ignored, patterns group is created
        let result = parse(toml);
        assert!(
            result.is_ok(),
            "Unknown scalar fields at root should be silently ignored"
        );
        assert_eq!(result.unwrap().groups.len(), 1);
    }

    #[test]
    fn test_unknown_field_in_group_returns_config_invalid() {
        let toml = r#"
[mygroup]
unknown_field = true
[mygroup.patterns]
"*.rs" = ["cargo fmt"]
"#;
        let result = parse(toml);
        // RawGroupConfig has deny_unknown_fields, so this should fail
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("unknown") || err.contains("mygroup"),
            "Expected error about unknown field, got: {err}"
        );
    }

    #[test]
    fn test_empty_toml_returns_no_patterns_error() {
        let toml = "";
        let result = parse(toml);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("no patterns"),
            "Expected 'no patterns' error, got: {err}"
        );
    }

    #[test]
    fn test_malformed_toml_returns_config_invalid() {
        let toml = "this is not valid toml = = =";
        let result = parse(toml);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid configuration"),
            "Expected config invalid error, got: {err}"
        );
    }

    #[test]
    fn test_group_timeout_wins_over_root() {
        let toml = r#"
timeout = "5s"

[fast]
timeout = "10s"
[fast.patterns]
"*.rs" = ["cargo fmt"]
"#;
        let config = parse(toml).unwrap();
        assert_eq!(
            config.groups[0].timeout,
            Some(std::time::Duration::from_secs(10))
        );
    }

    #[test]
    fn test_named_groups_preserve_declaration_order() {
        let toml = r#"
[zebra]
[zebra.patterns]
"*.ts" = ["eslint"]

[alpha]
[alpha.patterns]
"*.rs" = ["cargo fmt"]

[middle]
[middle.patterns]
"*.go" = ["gofmt"]
"#;
        let config = parse(toml).unwrap();
        assert_eq!(config.groups.len(), 3);
        assert_eq!(config.groups[0].name, "zebra");
        assert_eq!(config.groups[1].name, "alpha");
        assert_eq!(config.groups[2].name, "middle");
    }

    #[test]
    fn test_default_group_comes_first_before_named_groups() {
        let toml = r#"
[patterns]
"*.md" = ["prettier"]

[beta]
[beta.patterns]
"*.ts" = ["eslint"]

[alpha]
[alpha.patterns]
"*.rs" = ["cargo fmt"]
"#;
        let config = parse(toml).unwrap();
        assert_eq!(config.groups.len(), 3);
        assert_eq!(config.groups[0].name, "default");
        assert_eq!(config.groups[1].name, "beta");
        assert_eq!(config.groups[2].name, "alpha");
    }

    #[test]
    fn test_all_named_groups_empty_patterns_returns_error() {
        let toml = r#"
[lint]
[lint.patterns]

[format]
[format.patterns]
"#;
        let result = parse(toml);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("no patterns"),
            "Expected 'no patterns' error, got: {err}"
        );
    }
}
