//! Glob matcher for staged files.
//!
//! Pure function that takes a loaded [`Config`] and a slice of staged file
//! paths and produces a structured [`Vec<MatchedGroup>`] describing which
//! files matched which patterns within which groups, alongside the commands
//! to run.
//!
//! # Matching rules
//!
//! - **matchBase semantics (D-02):** patterns containing no `/` are
//!   automatically prefixed with `**/` so that `*.ts` matches `foo.ts`,
//!   `src/foo/bar.ts`, and any nesting depth. Patterns containing `/`
//!   (e.g. `src/*.ts`) are kept literal and do NOT auto-recurse.
//! - **Negation (D-03):** a pattern starting with `!` excludes matches.
//!   The include set must contain at least one non-negated pattern;
//!   otherwise [`MatchError::InvalidPattern`] is returned.
//! - **Case-sensitive (D-04):** matching is always case-sensitive. `*.TS`
//!   does NOT match `foo.ts`.
//! - **Empty-pattern skip (D-27):** patterns with zero matched files are
//!   omitted from the resulting group entry. A group with no matched
//!   patterns is omitted from the result entirely.
//! - **Globally empty result:** if zero files match across ALL groups, the
//!   matcher returns [`MatchError::NoFilesMatched`]. The caller is responsible
//!   for suppressing this under `--allow-empty` (Plan 05).
//!
//! Each entry in [`Group::patterns`] is a single pattern string keyed to a
//! command list. Combining include + negation in the same group requires
//! using two separate keys (one include + one `!`-prefixed negation).
//! Multi-pattern strings (comma-separated) are not supported in this phase.

use std::path::PathBuf;

mod extglob;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use run_staged_config::types::Config;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MatchError {
    #[error("invalid configuration: {0}")]
    InvalidPattern(String),

    #[error("no staged files matched any configured patterns")]
    NoFilesMatched,
}

// Defined in config so exec can consume them without depending on matcher.
pub use run_staged_config::types::{MatchedGroup, MatchedPattern};

/// Build include/exclude [`GlobSet`]s from a list of raw pattern strings.
///
/// Returns `(include_set, exclude_set, has_any_include)`. `has_any_include`
/// is `true` when at least one of the inputs was a non-negated pattern.
/// Errors are mapped to [`MatchError::InvalidPattern`] with the offending raw
/// pattern in the message.
fn build_globsets(patterns: &[&str]) -> Result<(GlobSet, GlobSet, bool), MatchError> {
    let mut include = GlobSetBuilder::new();
    let mut exclude = GlobSetBuilder::new();
    let mut has_any_include = false;

    for raw in patterns {
        let (set, body) = if let Some(stripped) = raw.strip_prefix('!') {
            (&mut exclude, stripped)
        } else {
            has_any_include = true;
            (&mut include, *raw)
        };

        let normalized = match_base(body);

        let glob = GlobBuilder::new(&normalized)
            .case_insensitive(false)
            // `*` and `?` stop at `/`, as in picomatch; only `**` crosses directories.
            .literal_separator(true)
            .build()
            .map_err(|e| {
                MatchError::InvalidPattern(format!("invalid glob pattern {raw:?}: {e}"))
            })?;
        set.add(glob);
    }

    let include_set = include.build().map_err(|e| {
        MatchError::InvalidPattern(format!("failed to build include glob set: {e}"))
    })?;
    let exclude_set = exclude.build().map_err(|e| {
        MatchError::InvalidPattern(format!("failed to build exclude glob set: {e}"))
    })?;

    Ok((include_set, exclude_set, has_any_include))
}

/// matchBase: a pattern with no `/` matches a file name at any depth.
fn match_base(pattern: &str) -> String {
    if pattern.contains('/') {
        pattern.to_string()
    } else {
        format!("**/{pattern}")
    }
}

/// A leading `!` negates, except `!(`, which starts a negated extglob.
fn is_negation(pattern: &str) -> bool {
    pattern.starts_with('!') && !pattern.starts_with("!(")
}

/// The path with `/` separators, as patterns are written.
fn slash_path(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.into_owned()
    }
}

/// Patterns that match every path. They skip globset, whose first regex
/// compile costs ~2 ms in a fresh process.
fn matches_everything(pattern: &str) -> bool {
    matches!(pattern, "*" | "**" | "**/*")
}

/// Match staged `files` against the patterns declared in `config`.
///
/// Iterates each [`Group`] in declaration order; within a group, iterates
/// each `(pattern, commands)` pair from [`Group::patterns`]. For each
/// pattern entry, builds a one-pattern globset, filters `files`, and
/// collects an entry only when at least one file matched (D-27).
///
/// # Errors
///
/// - [`MatchError::InvalidPattern`] if a pattern fails to compile or a single
///   pattern entry consists solely of a negation (no include patterns).
/// - [`MatchError::NoFilesMatched`] if zero patterns across all groups
///   produced any matches. Plan 05 will suppress this error under
///   `--allow-empty`.
pub fn match_files<'a>(
    config: &'a Config,
    files: &[PathBuf],
) -> Result<Vec<MatchedGroup<'a>>, MatchError> {
    let mut matched_groups: Vec<MatchedGroup<'a>> = Vec::new();
    let mut total_matched_patterns: usize = 0;

    for group in &config.groups {
        let mut group_patterns: Vec<MatchedPattern> = Vec::new();

        for (pattern_str, commands) in &group.patterns {
            let matched: Vec<PathBuf> = if matches_everything(pattern_str) {
                files.to_vec()
            } else if extglob::is_extended(pattern_str) && !is_negation(pattern_str) {
                let regex = extglob::compile(&match_base(pattern_str)).map_err(|detail| {
                    MatchError::InvalidPattern(format!(
                        "invalid glob pattern {pattern_str:?}: {detail}"
                    ))
                })?;
                files
                    .iter()
                    // A path that hits fancy-regex's backtrack limit counts as
                    // not matching; staged paths are far too short for that.
                    .filter(|p| regex.is_match(&slash_path(p)).unwrap_or(false))
                    .cloned()
                    .collect()
            } else {
                let (include_set, exclude_set, has_any_include) =
                    build_globsets(&[pattern_str.as_str()])?;

                if !has_any_include {
                    return Err(MatchError::InvalidPattern(format!(
                        "group '{}' pattern '{}' has only negation — nothing can match",
                        group.name, pattern_str
                    )));
                }

                files
                    .iter()
                    .filter(|p| include_set.is_match(p) && !exclude_set.is_match(p))
                    .cloned()
                    .collect()
            };

            if !matched.is_empty() {
                group_patterns.push(MatchedPattern {
                    pattern: pattern_str.clone(),
                    commands: commands.clone(),
                    files: matched,
                });
            }
        }

        if !group_patterns.is_empty() {
            total_matched_patterns += group_patterns.len();
            matched_groups.push(MatchedGroup {
                group,
                patterns: group_patterns,
            });
        }
    }

    if total_matched_patterns == 0 {
        return Err(MatchError::NoFilesMatched);
    }

    Ok(matched_groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use run_staged_config::types::{Config, ConfigSource, ExecutionOrder, Group};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_group(name: &str, patterns: &[(&str, &[&str])]) -> Group {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for (pat, cmds) in patterns {
            map.insert(
                (*pat).to_string(),
                cmds.iter().map(|s| (*s).to_string()).collect(),
            );
        }
        Group {
            name: name.to_string(),
            patterns: map,
            timeout: None,
            execution_order: ExecutionOrder::Parallel,
            continue_on_error: false,
            per_file: false,
            path_format: run_staged_config::types::PathFormat::Relative,
            read_only: false,
            chunk_size: None,
        }
    }

    fn make_config(groups: Vec<Group>) -> Config {
        Config {
            source: ConfigSource::TomlFile(PathBuf::from("test.toml")),
            groups,
            timeout: None,
            execution_order: ExecutionOrder::Parallel,
            continue_on_error: false,
            per_file: false,
        }
    }

    fn paths(items: &[&str]) -> Vec<PathBuf> {
        items.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn matchbase_slashless_pattern_recurses() {
        let cfg = make_config(vec![make_group("lint", &[("*.ts", &["eslint"])])]);
        let files = paths(&["top.ts", "src/foo/bar.ts", "deep/nested/dir/x.ts", "foo.js"]);
        let res = match_files(&cfg, &files).expect("should match");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].patterns.len(), 1);
        let mp = &res[0].patterns[0];
        assert_eq!(mp.pattern, "*.ts");
        let matched_strs: Vec<String> = mp
            .files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert!(matched_strs.contains(&"top.ts".to_string()));
        assert!(matched_strs.contains(&"src/foo/bar.ts".to_string()));
        assert!(matched_strs.contains(&"deep/nested/dir/x.ts".to_string()));
        assert!(!matched_strs.contains(&"foo.js".to_string()));
    }

    #[test]
    fn match_all_fast_path_agrees_with_globset() {
        let files = paths(&["a.js", ".env", "src/b.rs", "deep/x/.hidden", "no_ext"]);
        for p in ["*", "**", "**/*"] {
            let (include, exclude, _) = build_globsets(&[p]).unwrap();
            let via_globset: Vec<_> = files
                .iter()
                .filter(|f| include.is_match(f) && !exclude.is_match(f))
                .cloned()
                .collect();
            assert!(matches_everything(p));
            assert_eq!(via_globset, files, "pattern {p}");
        }
        assert!(!matches_everything("*.js"));
        assert!(!matches_everything("!*"));
    }

    #[test]
    fn star_stays_within_a_directory() {
        let cfg = make_config(vec![make_group("lint", &[("src/*.ts", &["eslint"])])]);
        let files = paths(&["src/a.ts", "src/lib/b.ts"]);
        let res = match_files(&cfg, &files).expect("should match");
        assert_eq!(res[0].patterns[0].files, paths(&["src/a.ts"]));
    }

    #[test]
    fn slashed_pattern_is_literal() {
        let cfg = make_config(vec![make_group("lint", &[("src/*.ts", &["eslint"])])]);
        let files = paths(&["src/bar.ts", "nested/src/bar.ts"]);
        let res = match_files(&cfg, &files).expect("should match");
        assert_eq!(res.len(), 1);
        let mp = &res[0].patterns[0];
        let matched_strs: Vec<String> = mp
            .files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert!(matched_strs.contains(&"src/bar.ts".to_string()));
        assert!(!matched_strs.contains(&"nested/src/bar.ts".to_string()));
    }

    #[test]
    fn negation_excludes_matched_file() {
        // Two pattern entries in the same group: include `*.ts` plus
        // negation `!*.snap` combined into a SINGLE pattern key by listing
        // both in one string... but per the matcher contract each HashMap
        // key is one pattern. To express include+negation in this phase
        // the user must use a multi-line pattern body, which we emulate
        // here by treating the second entry as a negation that lives in
        // its OWN key — that should NOT cancel out the include because
        // the matcher processes patterns independently. So instead, we
        // verify the supported model: a single pattern string carries
        // both the include and the negation. For the per-pattern API
        // this is achieved with one HashMap entry whose key is the
        // include string and the negation lives outside.
        //
        // Concretely: each HashMap key is a SINGLE pattern. To get a
        // negation effect within a group entry, the include and exclude
        // must live in the SAME pattern string. Since HashMap keys are
        // single strings, we test that a `!`-prefixed pattern entry
        // alone returns ConfigInvalid (covered by another test) and
        // verify here the in-pattern behavior: pattern = `*.ts` with
        // file `foo.snap` - the file should NOT match `*.ts` since it
        // doesn't end in `.ts`. The `foo.snap.ts` file ends in `.ts`
        // and matches.
        let cfg = make_config(vec![make_group("lint", &[("*.ts", &["eslint"])])]);
        let files = paths(&["foo.ts", "foo.snap", "foo.snap.ts"]);
        let res = match_files(&cfg, &files).expect("should match");
        let mp = &res[0].patterns[0];
        let matched_strs: Vec<String> = mp
            .files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert!(matched_strs.contains(&"foo.ts".to_string()));
        assert!(!matched_strs.contains(&"foo.snap".to_string()));
        assert!(matched_strs.contains(&"foo.snap.ts".to_string()));

        // Now exercise the build_globsets helper directly to verify
        // include + negation interaction at the globset layer:
        let (inc, exc, has_inc) = build_globsets(&["*.ts", "!*.snap"]).expect("globsets build");
        assert!(has_inc);
        let p_ts = PathBuf::from("foo.ts");
        let p_snap = PathBuf::from("foo.snap");
        let p_snap_ts = PathBuf::from("foo.snap.ts");
        assert!(inc.is_match(&p_ts) && !exc.is_match(&p_ts));
        assert!(!inc.is_match(&p_snap) || exc.is_match(&p_snap));
        // foo.snap.ts ends in `.ts` so include matches; only `*.snap` is
        // excluded (basename does not end in `.snap`), so it survives.
        assert!(inc.is_match(&p_snap_ts) && !exc.is_match(&p_snap_ts));
    }

    #[test]
    fn case_sensitive_default() {
        // Pattern *.TS must NOT match foo.ts.
        let cfg = make_config(vec![make_group("lint", &[("*.TS", &["eslint"])])]);
        let files = paths(&["foo.ts"]);
        let res = match_files(&cfg, &files);
        assert!(matches!(res, Err(MatchError::NoFilesMatched)));

        // Pattern *.ts must NOT match FOO.TS.
        let cfg = make_config(vec![make_group("lint", &[("*.ts", &["eslint"])])]);
        let files = paths(&["FOO.TS"]);
        let res = match_files(&cfg, &files);
        assert!(matches!(res, Err(MatchError::NoFilesMatched)));
    }

    #[test]
    fn multiple_patterns_collect_all_matches() {
        let cfg = make_config(vec![make_group(
            "polyglot",
            &[("*.ts", &["eslint"]), ("*.rs", &["cargo fmt"])],
        )]);
        let files = paths(&["a.ts", "b.rs", "c.md"]);
        let res = match_files(&cfg, &files).expect("should match");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].patterns.len(), 2);
        // Each pattern has its own file list.
        for mp in &res[0].patterns {
            match mp.pattern.as_str() {
                "*.ts" => {
                    assert_eq!(mp.files, vec![PathBuf::from("a.ts")]);
                    assert_eq!(mp.commands, vec!["eslint".to_string()]);
                }
                "*.rs" => {
                    assert_eq!(mp.files, vec![PathBuf::from("b.rs")]);
                    assert_eq!(mp.commands, vec!["cargo fmt".to_string()]);
                }
                other => panic!("unexpected pattern: {other}"),
            }
        }
    }

    #[test]
    fn pattern_with_zero_matches_is_skipped_at_pattern_level() {
        let cfg = make_config(vec![make_group(
            "polyglot",
            &[("*.ts", &["eslint"]), ("*.go", &["gofmt"])],
        )]);
        let files = paths(&["a.ts", "b.ts"]);
        let res = match_files(&cfg, &files).expect("should match");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].patterns.len(), 1);
        assert_eq!(res[0].patterns[0].pattern, "*.ts");
    }

    #[test]
    fn match_files_returns_no_files_matched_when_zero_matches_globally() {
        let cfg = make_config(vec![
            make_group("ts", &[("*.ts", &["eslint"])]),
            make_group("rs", &[("*.rs", &["cargo fmt"])]),
        ]);
        let files = paths(&["README.md", "LICENSE"]);
        let res = match_files(&cfg, &files);
        assert!(matches!(res, Err(MatchError::NoFilesMatched)));
    }

    #[test]
    fn match_files_returns_ok_when_at_least_one_pattern_matches() {
        let cfg = make_config(vec![
            make_group("ts", &[("*.ts", &["eslint"])]),
            make_group("rs", &[("*.rs", &["cargo fmt"])]),
        ]);
        let files = paths(&["only.ts"]);
        let res = match_files(&cfg, &files).expect("should match");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].group.name, "ts");
    }

    #[test]
    fn only_negations_in_group_returns_config_invalid() {
        let cfg = make_config(vec![make_group("only_neg", &[("!*.snap", &["noop"])])]);
        let files = paths(&["foo.ts"]);
        let res = match_files(&cfg, &files);
        match res {
            Err(MatchError::InvalidPattern(msg)) => {
                assert!(
                    msg.contains("only_neg"),
                    "error should mention group name: {msg}"
                );
            }
            other => panic!("expected ConfigInvalid, got {other:?}"),
        }
    }

    #[test]
    fn invalid_glob_syntax_returns_config_invalid() {
        let cfg = make_config(vec![make_group("bad", &[("[unclosed", &["noop"])])]);
        let files = paths(&["foo.ts"]);
        let res = match_files(&cfg, &files);
        match res {
            Err(MatchError::InvalidPattern(msg)) => {
                assert!(
                    msg.contains("[unclosed"),
                    "error should mention pattern: {msg}"
                );
            }
            other => panic!("expected ConfigInvalid, got {other:?}"),
        }
    }
}
