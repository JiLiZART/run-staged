use std::collections::HashMap;

use crate::AppError;

use super::types::{ExecutionOrder, Group};

/// Raw settings for a group before defaults are applied.
pub(crate) struct RawGroupSettings {
    pub patterns: HashMap<String, Vec<String>>,
    pub name: String,
    pub timeout: Option<String>,
    pub execution_order: Option<String>,
    pub continue_on_error: Option<bool>,
    pub per_file: Option<bool>,
}

/// Root-level defaults from the config file (before group-level overrides).
pub(crate) struct RootDefaults<'a> {
    pub timeout: &'a Option<String>,
    pub execution_order: &'a Option<String>,
    pub continue_on_error: &'a Option<bool>,
    pub per_file: &'a Option<bool>,
}

/// Resolve a group's settings using cascading defaults: hardcoded -> root -> group.
///
/// - `timeout`: group wins over root; falls back to None if neither is set
/// - `execution_order`: group wins over root; falls back to Parallel
/// - `continue_on_error`: group wins over root; falls back to false
/// - `per_file`: group wins over root; falls back to false
pub(crate) fn resolve_group(
    group: RawGroupSettings,
    root: RootDefaults<'_>,
) -> Result<Group, AppError> {
    let timeout = group
        .timeout
        .or_else(|| root.timeout.clone())
        .and_then(|s| parse_duration::parse(&s).ok());

    let execution_order = match group
        .execution_order
        .or_else(|| root.execution_order.clone())
        .as_deref()
    {
        None | Some("parallel") => ExecutionOrder::Parallel,
        Some("sequential") => ExecutionOrder::Sequential,
        Some(other) => {
            return Err(AppError::ConfigInvalid(format!(
                "unknown execution_order: {other:?}, expected \"parallel\" or \"sequential\""
            )));
        }
    };

    let continue_on_error = group
        .continue_on_error
        .or(*root.continue_on_error)
        .unwrap_or(false);
    let per_file = group.per_file.or(*root.per_file).unwrap_or(false);

    Ok(Group {
        name: group.name,
        patterns: group.patterns,
        timeout,
        execution_order,
        continue_on_error,
        per_file,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn empty_patterns() -> HashMap<String, Vec<String>> {
        HashMap::new()
    }

    fn make_group(
        timeout: Option<&str>,
        exec_order: Option<&str>,
        cont: Option<bool>,
        per_file: Option<bool>,
    ) -> RawGroupSettings {
        RawGroupSettings {
            patterns: empty_patterns(),
            name: "test".to_string(),
            timeout: timeout.map(String::from),
            execution_order: exec_order.map(String::from),
            continue_on_error: cont,
            per_file,
        }
    }

    fn make_root<'a>(
        timeout: &'a Option<String>,
        exec_order: &'a Option<String>,
        cont: &'a Option<bool>,
        per_file: &'a Option<bool>,
    ) -> RootDefaults<'a> {
        RootDefaults {
            timeout,
            execution_order: exec_order,
            continue_on_error: cont,
            per_file,
        }
    }

    #[test]
    fn test_all_none_yields_hardcoded_defaults() {
        let group = resolve_group(
            make_group(None, None, None, None),
            make_root(&None, &None, &None, &None),
        )
        .unwrap();

        assert_eq!(group.execution_order, ExecutionOrder::Parallel);
        assert!(!group.continue_on_error);
        assert!(!group.per_file);
        assert!(group.timeout.is_none());
    }

    #[test]
    fn test_group_timeout_none_inherits_root_timeout() {
        let root_timeout = Some("5s".to_string());
        let group = resolve_group(
            make_group(None, None, None, None),
            make_root(&root_timeout, &None, &None, &None),
        )
        .unwrap();

        assert_eq!(group.timeout, Some(Duration::from_secs(5)));
    }

    #[test]
    fn test_group_timeout_wins_over_root_timeout() {
        let root_timeout = Some("5s".to_string());
        let group = resolve_group(
            make_group(Some("10s"), None, None, None),
            make_root(&root_timeout, &None, &None, &None),
        )
        .unwrap();

        assert_eq!(group.timeout, Some(Duration::from_secs(10)));
    }

    #[test]
    fn test_group_continue_on_error_inherits_root() {
        let root_cont = Some(true);
        let group = resolve_group(
            make_group(None, None, None, None),
            make_root(&None, &None, &root_cont, &None),
        )
        .unwrap();

        assert!(group.continue_on_error);
    }

    #[test]
    fn test_group_continue_on_error_wins_over_root() {
        let root_cont = Some(true);
        let group = resolve_group(
            make_group(None, None, Some(false), None),
            make_root(&None, &None, &root_cont, &None),
        )
        .unwrap();

        assert!(!group.continue_on_error);
    }

    #[test]
    fn test_invalid_execution_order_returns_error() {
        let result = resolve_group(
            make_group(None, Some("invalid_order"), None, None),
            make_root(&None, &None, &None, &None),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("execution_order"));
    }

    #[test]
    fn test_sequential_execution_order_parsed() {
        let group = resolve_group(
            make_group(None, Some("sequential"), None, None),
            make_root(&None, &None, &None, &None),
        )
        .unwrap();

        assert_eq!(group.execution_order, ExecutionOrder::Sequential);
    }
}
