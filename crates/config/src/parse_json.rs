use std::collections::HashMap;
use std::path::Path;

use crate::ConfigError;

use super::defaults::{RawGroupSettings, RootDefaults, resolve_group};
use super::types::{Config, ConfigSource, ExecutionOrder, RawJsonConfig};

/// Config field names used to detect structured vs flat lint-staged JSON.
/// If the JSON object contains any of these keys, it is treated as structured.
const CONFIG_FIELD_NAMES: &[&str] = &[
    "timeout",
    "execution_order",
    "continue_on_error",
    "per_file",
    "path_format",
    "read_only",
    "chunk_size",
    "patterns",
];

/// Parse a JSON config file from the filesystem.
pub fn parse_json_file(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
    let source = ConfigSource::JsonFile(path.to_path_buf());
    parse_json_str(&content, source, &path.display().to_string())
}

/// Parse a JSON config from a string.
/// Accepts a `source` for tracking origin and `path_display` for error messages.
pub fn parse_json_str(
    content: &str,
    source: ConfigSource,
    path_display: &str,
) -> Result<Config, ConfigError> {
    let raw: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| ConfigError::Invalid(format!("{path_display}: {e}")))?;

    parse_json_value(raw, source, path_display)
}

/// Parse a `serde_json::Value` as a run-staged config.
/// Used internally and by `parse_package_json_value` for the extracted "run-staged" key.
fn parse_json_value(
    raw: serde_json::Value,
    source: ConfigSource,
    path_display: &str,
) -> Result<Config, ConfigError> {
    // Check if any config field names are present — if so, this is structured format.
    // Per Pitfall 7: only attempt flat detection when NO config field names are present.
    let has_config_key = if let serde_json::Value::Object(ref obj) = raw {
        obj.keys().any(|k| CONFIG_FIELD_NAMES.contains(&k.as_str()))
    } else {
        false
    };

    if has_config_key {
        // Try structured parse — "patterns" or other config keys were present.
        let structured: RawJsonConfig = serde_json::from_value(raw)
            .map_err(|e| ConfigError::Invalid(format!("{path_display}: {e}")))?;
        build_config_from_json(structured, source, path_display)
    } else {
        // No config field keys present — attempt flat lint-staged detection.
        let flat_result: Result<HashMap<String, Vec<String>>, _> =
            serde_json::from_value(raw.clone());
        if let Ok(flat) = flat_result {
            return flat_config(flat, source, path_display);
        }

        // Flat detection failed — try structured parse (named groups without config field names).
        match serde_json::from_value::<RawJsonConfig>(raw) {
            Ok(structured) => build_config_from_json(structured, source, path_display),
            Err(structured_err) => Err(ConfigError::Invalid(format!(
                "{path_display}: expected structured config or flat glob-to-commands mapping: {structured_err}"
            ))),
        }
    }
}

/// A config with one `default` group built from a flat glob-to-commands map.
fn flat_config(
    patterns: HashMap<String, Vec<String>>,
    source: ConfigSource,
    path_display: &str,
) -> Result<Config, ConfigError> {
    if patterns.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "{path_display}: no patterns defined"
        )));
    }
    let group = resolve_group(
        RawGroupSettings {
            patterns,
            name: "default".to_string(),
            timeout: None,
            execution_order: None,
            continue_on_error: None,
            per_file: None,
            read_only: None,
            chunk_size: None,
            path_format: None,
        },
        RootDefaults {
            timeout: &None,
            execution_order: &None,
            continue_on_error: &None,
            per_file: &None,
            read_only: &None,
            chunk_size: &None,
            path_format: &None,
        },
    )
    .map_err(|e| ConfigError::Invalid(format!("{path_display}: {e}")))?;
    Ok(Config {
        source,
        groups: vec![group],
        ..Config::default()
    })
}

/// Parse a lint-staged config value (from JSON or YAML): a flat mapping of
/// glob to a command string or an array of command strings.
pub fn parse_lint_staged_value(
    value: serde_json::Value,
    source: ConfigSource,
    path_display: &str,
) -> Result<Config, ConfigError> {
    use serde_json::Value;
    let Value::Object(map) = value else {
        return Err(ConfigError::Invalid(format!(
            "{path_display}: expected a mapping of glob patterns to commands"
        )));
    };
    let bad_value = |glob: &str| {
        ConfigError::Invalid(format!(
            "{path_display}: \"{glob}\": expected a command string or an array of command strings"
        ))
    };
    let mut patterns = HashMap::with_capacity(map.len());
    for (glob, commands) in map {
        let commands = match commands {
            Value::String(command) => vec![command],
            Value::Array(items) => items
                .into_iter()
                .map(|item| match item {
                    Value::String(command) => Ok(command),
                    _ => Err(bad_value(&glob)),
                })
                .collect::<Result<_, _>>()?,
            _ => return Err(bad_value(&glob)),
        };
        patterns.insert(glob, commands);
    }
    flat_config(patterns, source, path_display)
}

/// Build a Config from a deserialized RawJsonConfig.
fn build_config_from_json(
    raw: RawJsonConfig,
    source: ConfigSource,
    path_display: &str,
) -> Result<Config, ConfigError> {
    let mut groups = Vec::new();

    // If top-level `patterns` is present, create a "default" group from it.
    if let Some(patterns) = raw.patterns.clone() {
        let group = resolve_group(
            RawGroupSettings {
                patterns,
                name: "default".to_string(),
                timeout: None,
                execution_order: None,
                continue_on_error: None,
                per_file: None,
                read_only: None,
                chunk_size: None,
                path_format: None,
            },
            RootDefaults {
                timeout: &raw.timeout,
                execution_order: &raw.execution_order,
                continue_on_error: &raw.continue_on_error,
                per_file: &raw.per_file,
                read_only: &raw.read_only,
                chunk_size: &raw.chunk_size,
                path_format: &raw.path_format,
            },
        )
        .map_err(|e| ConfigError::Invalid(format!("{path_display}: {e}")))?;
        groups.push(group);
    }

    // Process named groups from the flattened map (insertion order preserved via preserve_order feature).
    for (group_name, value) in &raw.groups {
        // Only process object values (skip non-object entries)
        if !value.is_object() {
            continue;
        }

        #[derive(serde::Deserialize)]
        struct RawJsonGroup {
            timeout: Option<String>,
            execution_order: Option<String>,
            continue_on_error: Option<bool>,
            per_file: Option<bool>,
            read_only: Option<bool>,
            chunk_size: Option<usize>,
            patterns: Option<HashMap<String, Vec<String>>>,
            path_format: Option<String>,
        }

        let raw_group: RawJsonGroup = serde_json::from_value(value.clone()).map_err(|e| {
            ConfigError::Invalid(format!("{path_display}: group \"{group_name}\": {e}"))
        })?;

        let patterns = raw_group.patterns.unwrap_or_default();
        if patterns.is_empty() {
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
                read_only: raw_group.read_only,
                chunk_size: raw_group.chunk_size,
                path_format: raw_group.path_format,
            },
            RootDefaults {
                timeout: &raw.timeout,
                execution_order: &raw.execution_order,
                continue_on_error: &raw.continue_on_error,
                per_file: &raw.per_file,
                read_only: &raw.read_only,
                chunk_size: &raw.chunk_size,
                path_format: &raw.path_format,
            },
        )
        .map_err(|e| ConfigError::Invalid(format!("{path_display}: {e}")))?;

        groups.push(group);
    }

    if groups.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "{path_display}: no patterns defined"
        )));
    }

    let timeout = raw
        .timeout
        .as_ref()
        .and_then(|s| parse_duration::parse(s).ok());
    let execution_order = match raw.execution_order.as_deref() {
        None | Some("parallel") => ExecutionOrder::Parallel,
        Some("sequential") => ExecutionOrder::Sequential,
        Some(other) => {
            return Err(ConfigError::Invalid(format!(
                "{path_display}: unknown execution_order: {other:?}, expected \"parallel\" or \"sequential\""
            )));
        }
    };

    Ok(Config {
        source,
        groups,
        timeout,
        execution_order,
        continue_on_error: raw.continue_on_error.unwrap_or(false),
        per_file: raw.per_file.unwrap_or(false),
    })
}

/// Parse the already-extracted `"run-staged"` value from package.json.
/// Used by config discovery (Plan 02) when `package.json["run-staged"]` is found.
pub fn parse_package_json_value(
    value: serde_json::Value,
    source: ConfigSource,
) -> Result<Config, ConfigError> {
    parse_json_value(value, source, "package.json[run-staged]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_source() -> ConfigSource {
        ConfigSource::JsonFile(std::path::PathBuf::from("test.json"))
    }

    fn parse(content: &str) -> Result<Config, ConfigError> {
        parse_json_str(content, dummy_source(), "test.json")
    }

    #[test]
    fn test_structured_json_with_patterns_key() {
        let json = r#"{"patterns": {"*.rs": ["cargo fmt"]}}"#;
        let config = parse(json).unwrap();
        assert_eq!(config.groups.len(), 1);
        assert_eq!(config.groups[0].name, "default");
        assert!(config.groups[0].patterns.contains_key("*.rs"));
    }

    #[test]
    fn test_flat_lint_staged_json_two_patterns() {
        let json = r#"{"*.rs": ["cargo fmt"], "*.ts": ["eslint"]}"#;
        let config = parse(json).unwrap();
        assert_eq!(config.groups.len(), 1);
        assert_eq!(config.groups[0].name, "default");
        assert_eq!(config.groups[0].patterns.len(), 2);
        assert!(config.groups[0].patterns.contains_key("*.rs"));
        assert!(config.groups[0].patterns.contains_key("*.ts"));
    }

    #[test]
    fn test_structured_json_with_timeout() {
        let json = r#"{"timeout": "5s", "patterns": {"*.rs": ["cargo fmt"]}}"#;
        let config = parse(json).unwrap();
        assert_eq!(
            config.groups[0].timeout,
            Some(std::time::Duration::from_secs(5))
        );
    }

    #[test]
    fn test_invalid_json_syntax_returns_config_invalid() {
        let json = "{not valid json}";
        let result = parse(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid configuration"),
            "Expected config invalid, got: {err}"
        );
    }

    #[test]
    fn test_patterns_key_present_but_invalid_value_returns_structured_error() {
        // `patterns` key is present so flat detection is NOT attempted.
        // The value is invalid (string instead of object), so structured parse fails.
        let json = r#"{"patterns": "not an object"}"#;
        let result = parse(json);
        assert!(result.is_err());
        // Should NOT attempt flat detection — error is structured parse error
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid configuration"),
            "Expected structured error, got: {err}"
        );
    }

    #[test]
    fn test_empty_json_object_returns_no_patterns_error() {
        // Empty object: no config keys, no valid flat patterns
        let json = "{}";
        let result = parse(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_with_timeout_but_no_patterns_returns_error() {
        // Has config field names but no patterns — should fail as structured
        let json = r#"{"timeout": "5s"}"#;
        let result = parse(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("no patterns"),
            "Expected no patterns error, got: {err}"
        );
    }

    #[test]
    fn test_parse_package_json_value() {
        let value = serde_json::json!({"patterns": {"*.rs": ["cargo fmt"]}});
        let source = ConfigSource::PackageJson(std::path::PathBuf::from("package.json"));
        let config = parse_package_json_value(value, source).unwrap();
        assert_eq!(config.groups.len(), 1);
        assert!(config.groups[0].patterns.contains_key("*.rs"));
    }

    #[test]
    fn test_flat_detection_not_attempted_when_config_field_present() {
        // "timeout" is a config field name — so flat detection is NOT attempted.
        // With "timeout" key present and no "patterns", parse fails as structured
        // with "no patterns defined" error.
        let json = r#"{"timeout": "5s"}"#;
        let result = parse(json);
        assert!(result.is_err());
        // Should be structured parse error (no patterns), not flat detection
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("no patterns"),
            "Expected no patterns error (structured parse), got: {err}"
        );
    }

    #[test]
    fn test_json_group_execution_order_overrides_root() {
        let json = r#"{
            "execution_order": "parallel",
            "lint": {
                "execution_order": "sequential",
                "patterns": { "*.rs": ["cargo clippy"] }
            }
        }"#;
        let config = parse(json).unwrap();
        assert_eq!(config.execution_order, ExecutionOrder::Parallel);
        assert_eq!(config.groups[0].execution_order, ExecutionOrder::Sequential);
    }

    #[test]
    fn test_json_group_timeout_override_and_inheritance() {
        let json = r#"{
            "timeout": "30s",
            "lint": {
                "timeout": "10s",
                "patterns": { "*.rs": ["cargo clippy"] }
            },
            "format": {
                "patterns": { "*.rs": ["cargo fmt"] }
            }
        }"#;
        let config = parse(json).unwrap();
        assert_eq!(config.groups[0].name, "lint");
        assert_eq!(
            config.groups[0].timeout,
            Some(std::time::Duration::from_secs(10))
        );
        assert_eq!(config.groups[1].name, "format");
        assert_eq!(
            config.groups[1].timeout,
            Some(std::time::Duration::from_secs(30))
        );
    }

    #[test]
    fn test_json_group_continue_on_error_and_per_file() {
        let json = r#"{
            "format": {
                "continue_on_error": true,
                "per_file": true,
                "patterns": { "*.rs": ["cargo fmt"] }
            }
        }"#;
        let config = parse(json).unwrap();
        assert!(config.groups[0].continue_on_error);
        assert!(config.groups[0].per_file);
    }

    #[test]
    fn test_json_multi_group_mixed_overrides() {
        let json = r#"{
            "execution_order": "parallel",
            "timeout": "30s",
            "lint": {
                "execution_order": "sequential",
                "timeout": "10s",
                "patterns": { "*.rs": ["cargo clippy"] }
            },
            "format": {
                "continue_on_error": true,
                "per_file": true,
                "patterns": { "*.rs": ["cargo fmt"], "*.ts": ["prettier --write"] }
            }
        }"#;
        let config = parse(json).unwrap();
        assert_eq!(config.groups.len(), 2);
        assert_eq!(config.groups[0].name, "lint");
        assert_eq!(config.groups[0].execution_order, ExecutionOrder::Sequential);
        assert_eq!(
            config.groups[0].timeout,
            Some(std::time::Duration::from_secs(10))
        );
        assert_eq!(config.groups[1].name, "format");
        assert_eq!(config.groups[1].execution_order, ExecutionOrder::Parallel);
        assert_eq!(
            config.groups[1].timeout,
            Some(std::time::Duration::from_secs(30))
        );
        assert!(config.groups[1].continue_on_error);
        assert!(config.groups[1].per_file);
    }

    #[test]
    fn test_json_named_groups_preserve_declaration_order() {
        let json = r#"{
            "zebra": { "patterns": { "*.ts": ["eslint"] } },
            "alpha": { "patterns": { "*.rs": ["cargo fmt"] } },
            "middle": { "patterns": { "*.go": ["gofmt"] } }
        }"#;
        let config = parse(json).unwrap();
        assert_eq!(config.groups.len(), 3);
        assert_eq!(config.groups[0].name, "zebra");
        assert_eq!(config.groups[1].name, "alpha");
        assert_eq!(config.groups[2].name, "middle");
    }

    #[test]
    fn test_json_default_group_comes_first_before_named() {
        let json = r#"{
            "patterns": { "*.md": ["prettier"] },
            "beta": { "patterns": { "*.ts": ["eslint"] } },
            "alpha": { "patterns": { "*.rs": ["cargo fmt"] } }
        }"#;
        let config = parse(json).unwrap();
        assert_eq!(config.groups.len(), 3);
        assert_eq!(config.groups[0].name, "default");
        assert_eq!(config.groups[1].name, "beta");
        assert_eq!(config.groups[2].name, "alpha");
    }

    #[test]
    fn test_json_path_format_named_group_parses() {
        use crate::types::PathFormat;
        let json = r#"{
            "tools": {
                "path_format": "absolute",
                "patterns": { "*.rs": ["cargo fmt"] }
            }
        }"#;
        let config = parse(json).unwrap();
        assert_eq!(config.groups.len(), 1);
        assert_eq!(config.groups[0].path_format, PathFormat::Absolute);
    }

    #[test]
    fn test_json_path_format_invalid_value_returns_error() {
        let json = r#"{
            "tools": {
                "path_format": "nope",
                "patterns": { "*.rs": ["cargo fmt"] }
            }
        }"#;
        let result = parse(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("path_format"),
            "Expected path_format error, got: {err}"
        );
    }

    #[test]
    fn test_read_only_and_chunk_size_keys() {
        let json =
            r#"{"read_only": true, "lint": {"chunk_size": 3, "patterns": {"*.js": ["eslint"]}}}"#;
        let config = parse(json).unwrap();
        assert!(config.groups[0].read_only);
        assert_eq!(config.groups[0].chunk_size, Some(3));
    }

    fn lint_staged(json: &str) -> Result<Config, ConfigError> {
        parse_lint_staged_value(
            serde_json::from_str(json).unwrap(),
            dummy_source(),
            ".lintstagedrc.json",
        )
    }

    #[test]
    fn test_lint_staged_string_value_is_one_command() {
        let config = lint_staged(r#"{"*.ts": "eslint --fix"}"#).unwrap();
        assert_eq!(config.groups[0].patterns["*.ts"], ["eslint --fix"]);
    }

    #[test]
    fn test_lint_staged_array_value() {
        let config = lint_staged(r#"{"*.css": ["stylelint", "prettier --write"]}"#).unwrap();
        assert_eq!(
            config.groups[0].patterns["*.css"],
            ["stylelint", "prettier --write"]
        );
    }

    #[test]
    fn test_lint_staged_other_value_types_name_the_key() {
        for json in [
            r#"{"*.ts": 1}"#,
            r#"{"*.ts": ["ok", 2]}"#,
            r#"{"*.ts": {"a": "b"}}"#,
        ] {
            let err = lint_staged(json).unwrap_err();
            assert!(
                matches!(err, ConfigError::Invalid(ref m) if m.contains("\"*.ts\"") && m.contains("command string")),
                "{json}: {err:?}"
            );
        }
    }

    #[test]
    fn test_lint_staged_empty_mapping_has_no_patterns() {
        let err = lint_staged("{}").unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(ref m) if m.contains("no patterns defined")));
    }
}
