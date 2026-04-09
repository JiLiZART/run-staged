use std::collections::HashMap;
use std::path::Path;

use crate::AppError;

use super::defaults::{RawGroupSettings, RootDefaults, resolve_group};
use super::types::{Config, ConfigSource, ExecutionOrder, RawJsonConfig};

/// Config field names used to detect structured vs flat lint-staged JSON.
/// If the JSON object contains any of these keys, it is treated as structured.
const CONFIG_FIELD_NAMES: &[&str] = &[
    "timeout",
    "execution_order",
    "continue_on_error",
    "per_file",
    "patterns",
];

/// Parse a JSON config file from the filesystem.
pub fn parse_json_file(path: &Path) -> Result<Config, AppError> {
    let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
    let source = ConfigSource::JsonFile(path.to_path_buf());
    parse_json_str(&content, source, &path.display().to_string())
}

/// Parse a JSON config from a string.
/// Accepts a `source` for tracking origin and `path_display` for error messages.
pub fn parse_json_str(
    content: &str,
    source: ConfigSource,
    path_display: &str,
) -> Result<Config, AppError> {
    let raw: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| AppError::ConfigInvalid(format!("{path_display}: {e}")))?;

    parse_json_value(raw, source, path_display)
}

/// Parse a `serde_json::Value` as a fast-staged config.
/// Used internally and by `parse_package_json_value` for the extracted "fast-staged" key.
fn parse_json_value(
    raw: serde_json::Value,
    source: ConfigSource,
    path_display: &str,
) -> Result<Config, AppError> {
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
            .map_err(|e| AppError::ConfigInvalid(format!("{path_display}: {e}")))?;
        build_config_from_json(structured, source, path_display)
    } else {
        // No config field keys present — attempt flat lint-staged detection.
        let flat_result: Result<HashMap<String, Vec<String>>, _> =
            serde_json::from_value(raw.clone());
        if let Ok(flat) = flat_result {
            if flat.is_empty() {
                return Err(AppError::ConfigInvalid(format!(
                    "{path_display}: no patterns defined"
                )));
            }
            let group = resolve_group(
                RawGroupSettings {
                    patterns: flat,
                    name: "default".to_string(),
                    timeout: None,
                    execution_order: None,
                    continue_on_error: None,
                    per_file: None,
                },
                RootDefaults {
                    timeout: &None,
                    execution_order: &None,
                    continue_on_error: &None,
                    per_file: &None,
                },
            )
            .map_err(|e| AppError::ConfigInvalid(format!("{path_display}: {e}")))?;
            return Ok(Config {
                source,
                groups: vec![group],
                ..Config::default()
            });
        }

        // Flat detection failed — try structured parse (named groups without config field names).
        match serde_json::from_value::<RawJsonConfig>(raw) {
            Ok(structured) => build_config_from_json(structured, source, path_display),
            Err(structured_err) => Err(AppError::ConfigInvalid(format!(
                "{path_display}: expected structured config or flat glob-to-commands mapping: {structured_err}"
            ))),
        }
    }
}

/// Build a Config from a deserialized RawJsonConfig.
fn build_config_from_json(
    raw: RawJsonConfig,
    source: ConfigSource,
    path_display: &str,
) -> Result<Config, AppError> {
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
            patterns: Option<HashMap<String, Vec<String>>>,
        }

        let raw_group: RawJsonGroup = serde_json::from_value(value.clone()).map_err(|e| {
            AppError::ConfigInvalid(format!("{path_display}: group \"{group_name}\": {e}"))
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

    Ok(Config {
        source,
        groups,
        timeout,
        execution_order,
        continue_on_error: raw.continue_on_error.unwrap_or(false),
        per_file: raw.per_file.unwrap_or(false),
    })
}

/// Parse the already-extracted `"fast-staged"` value from package.json.
/// Used by config discovery (Plan 02) when `package.json["fast-staged"]` is found.
pub fn parse_package_json_value(
    value: serde_json::Value,
    source: ConfigSource,
) -> Result<Config, AppError> {
    parse_json_value(value, source, "package.json[fast-staged]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_source() -> ConfigSource {
        ConfigSource::JsonFile(std::path::PathBuf::from("test.json"))
    }

    fn parse(content: &str) -> Result<Config, AppError> {
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
}
