use std::collections::HashMap;
use std::time::Duration;

use crate::AppError;

use super::types::{ExecutionOrder, Group};

/// Resolve a group's settings using cascading defaults: hardcoded -> root -> group.
///
/// - `timeout`: group wins over root; falls back to None if neither is set
/// - `execution_order`: group wins over root; falls back to Parallel
/// - `continue_on_error`: group wins over root; falls back to false
/// - `per_file`: group wins over root; falls back to false
pub(crate) fn resolve_group(
    raw_patterns: HashMap<String, Vec<String>>,
    name: String,
    raw_timeout: Option<String>,
    raw_exec_order: Option<String>,
    raw_continue: Option<bool>,
    raw_per_file: Option<bool>,
    root_timeout: &Option<String>,
    root_exec_order: &Option<String>,
    root_continue: &Option<bool>,
    root_per_file: &Option<bool>,
) -> Result<Group, AppError> {
    let timeout = raw_timeout
        .or_else(|| root_timeout.clone())
        .and_then(|s| parse_duration::parse(&s).ok());

    let execution_order = match raw_exec_order
        .or_else(|| root_exec_order.clone())
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

    let continue_on_error = raw_continue.or(*root_continue).unwrap_or(false);
    let per_file = raw_per_file.or(*root_per_file).unwrap_or(false);

    Ok(Group {
        name,
        patterns: raw_patterns,
        timeout,
        execution_order,
        continue_on_error,
        per_file,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_patterns() -> HashMap<String, Vec<String>> {
        HashMap::new()
    }

    #[test]
    fn test_all_none_yields_hardcoded_defaults() {
        let group = resolve_group(
            empty_patterns(),
            "test".to_string(),
            None,
            None,
            None,
            None,
            &None,
            &None,
            &None,
            &None,
        )
        .unwrap();

        assert_eq!(group.execution_order, ExecutionOrder::Parallel);
        assert!(!group.continue_on_error);
        assert!(!group.per_file);
        assert!(group.timeout.is_none());
    }

    #[test]
    fn test_group_timeout_none_inherits_root_timeout() {
        let group = resolve_group(
            empty_patterns(),
            "test".to_string(),
            None,
            None,
            None,
            None,
            &Some("5s".to_string()),
            &None,
            &None,
            &None,
        )
        .unwrap();

        assert_eq!(group.timeout, Some(Duration::from_secs(5)));
    }

    #[test]
    fn test_group_timeout_wins_over_root_timeout() {
        let group = resolve_group(
            empty_patterns(),
            "test".to_string(),
            Some("10s".to_string()),
            None,
            None,
            None,
            &Some("5s".to_string()),
            &None,
            &None,
            &None,
        )
        .unwrap();

        assert_eq!(group.timeout, Some(Duration::from_secs(10)));
    }

    #[test]
    fn test_group_continue_on_error_inherits_root() {
        let group = resolve_group(
            empty_patterns(),
            "test".to_string(),
            None,
            None,
            None,
            None,
            &None,
            &None,
            &Some(true),
            &None,
        )
        .unwrap();

        assert!(group.continue_on_error);
    }

    #[test]
    fn test_group_continue_on_error_wins_over_root() {
        let group = resolve_group(
            empty_patterns(),
            "test".to_string(),
            None,
            None,
            Some(false),
            None,
            &None,
            &None,
            &Some(true),
            &None,
        )
        .unwrap();

        assert!(!group.continue_on_error);
    }

    #[test]
    fn test_invalid_execution_order_returns_error() {
        let result = resolve_group(
            empty_patterns(),
            "test".to_string(),
            None,
            Some("invalid_order".to_string()),
            None,
            None,
            &None,
            &None,
            &None,
            &None,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("execution_order"));
    }

    #[test]
    fn test_sequential_execution_order_parsed() {
        let group = resolve_group(
            empty_patterns(),
            "test".to_string(),
            None,
            Some("sequential".to_string()),
            None,
            None,
            &None,
            &None,
            &None,
            &None,
        )
        .unwrap();

        assert_eq!(group.execution_order, ExecutionOrder::Sequential);
    }
}
