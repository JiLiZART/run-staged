//! Checks `match_files` against picomatch's own verdicts (with lint-staged
//! 17's options), recorded in `picomatch_cases.json`.

use std::{collections::HashMap, path::PathBuf};

use run_staged_config::types::{Config, Group};
use run_staged_matcher::{MatchError, match_files};

fn config_for(pattern: &str) -> Config {
    Config {
        groups: vec![Group {
            name: "default".into(),
            patterns: HashMap::from([(pattern.to_string(), vec!["true".to_string()])]),
            ..Group::default()
        }],
        ..Config::default()
    }
}

#[test]
fn matches_agree_with_picomatch() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("picomatch_cases.json")).unwrap();
    let paths: Vec<PathBuf> = data["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| PathBuf::from(p.as_str().unwrap()))
        .collect();
    let mut failures = Vec::new();
    for (pattern, expected) in data["matches"].as_object().unwrap() {
        let mut expected: Vec<PathBuf> = expected
            .as_array()
            .unwrap()
            .iter()
            .map(|p| PathBuf::from(p.as_str().unwrap()))
            .collect();
        let config = config_for(pattern);
        let mut actual = match match_files(&config, &paths) {
            Ok(groups) => groups[0].patterns[0].files.clone(),
            Err(MatchError::NoFilesMatched) => vec![],
            Err(e) => panic!("{pattern}: {e}"),
        };
        expected.sort();
        actual.sort();
        if actual != expected {
            failures.push(format!(
                "{pattern}\n  picomatch: {expected:?}\n  run-staged: {actual:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
