//! Command building and spawning primitives.
//!
//! `build_argv()` parses a command string via shlex and applies the matched files
//! using either `{files}` placeholder substitution or positional append.
//!
//! `chunk_files()` splits a file list into argv-safe chunks based on platform ARG_MAX
//! with 25% headroom for environment variables.
//!
//! `spawn_command()` runs a parsed argv via `tokio::process::Command`, mapping ENOENT
//! to `AppError::CommandNotFound`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::AppError;
use crate::config::types::{Config, PathFormat};
use crate::matcher::MatchedGroup;

/// Result of one spawned command invocation.
///
/// One [`MatchedPattern`] can produce multiple outcomes — per-file mode emits
/// one outcome per file, and batch mode emits one outcome per ARG_MAX chunk.
///
/// Outcomes are returned by [`execute_all`] regardless of whether subsequent
/// groups were skipped due to the cross-group failure gate (D-14). The caller
/// inspects each variant via [`CommandOutcome::is_failure`] to decide whether
/// to roll back the staged diff (Plan 05-05).
#[derive(Debug, Clone)]
pub enum CommandOutcome {
    /// Command exited with status 0.
    Success {
        group: String,
        pattern: String,
        command: String,
        argv: Vec<String>,
        file_count: usize,
        duration: Duration,
        stdout: Vec<u8>,
    },
    /// Command exited with non-zero status, IO error, or `CommandNotFound` (mapped to exit 127).
    Failure {
        group: String,
        pattern: String,
        command: String,
        argv: Vec<String>,
        file_count: usize,
        duration: Duration,
        exit_code: i32,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// Cancelled before spawn because the per-group cancel flag was set by a
    /// sibling failure under `Group.continue_on_error == false` (D-13).
    Cancelled {
        group: String,
        pattern: String,
        command: String,
    },
}

impl CommandOutcome {
    /// Returns true for any non-success outcome (failure or cancellation).
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            CommandOutcome::Failure { .. } | CommandOutcome::Cancelled { .. }
        )
    }
}

/// Execute every matched group end-to-end (TDD stub — replaced in GREEN).
pub async fn execute_all(
    _matched_groups: &[MatchedGroup<'_>],
    _config: &Config,
    _git_root: &Path,
) -> Vec<CommandOutcome> {
    // RED stub: returns empty so tests fail predictably.
    Vec::new()
}

/// Parse a command string and inject file paths.
///
/// - If the parsed argv contains a token equal to `"{files}"`, that ONE token is
///   replaced inline by all file paths (in order, expanded to N tokens).
/// - Otherwise, all file paths are appended positionally after the parsed argv.
/// - Paths are formatted according to `path_format`: `Relative` passes them as-is
///   (relative to `git_root`); `Absolute` joins `git_root` + path then calls
///   `std::fs::canonicalize` (best-effort — falls back to the joined path if
///   canonicalize fails, e.g. file does not yet exist).
///
/// Returns `Err(AppError::ConfigInvalid)` if shlex cannot parse the command (e.g.
/// unbalanced quotes).
pub fn build_argv(
    command: &str,
    files: &[PathBuf],
    path_format: PathFormat,
    git_root: &Path,
) -> Result<Vec<String>, AppError> {
    let parsed = shlex::split(command)
        .ok_or_else(|| AppError::ConfigInvalid(format!("invalid command syntax: {command:?}")))?;

    let formatted_files: Vec<String> = files
        .iter()
        .map(|f| match path_format {
            PathFormat::Relative => f.display().to_string(),
            PathFormat::Absolute => {
                let joined = git_root.join(f);
                std::fs::canonicalize(&joined)
                    .unwrap_or(joined)
                    .display()
                    .to_string()
            }
        })
        .collect();

    if let Some(idx) = parsed.iter().position(|t| t == "{files}") {
        let mut out = Vec::with_capacity(parsed.len() - 1 + formatted_files.len());
        out.extend_from_slice(&parsed[..idx]);
        out.extend(formatted_files.iter().cloned());
        out.extend_from_slice(&parsed[idx + 1..]);
        Ok(out)
    } else {
        let mut out = parsed;
        out.extend(formatted_files);
        Ok(out)
    }
}

/// Platform ARG_MAX with 25% safety headroom for environment variables (D-26).
#[cfg(unix)]
fn arg_max() -> usize {
    let raw = unsafe { libc::sysconf(libc::_SC_ARG_MAX) };
    let limit = if raw > 0 { raw as usize } else { 131_072 };
    (limit as f64 * 0.75) as usize
}

#[cfg(windows)]
fn arg_max() -> usize {
    (32_767.0 * 0.75) as usize
}

/// Internal helper exposed for testing — `chunk_files` calls this with
/// `arg_max() - cmd_len` so unit tests can inject a tiny budget.
pub(crate) fn chunk_files_with_budget(files: &[PathBuf], budget: usize) -> Vec<&[PathBuf]> {
    if files.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    let mut current = 0usize;
    for (i, f) in files.iter().enumerate() {
        let len = f.as_os_str().len() + 1; // +1 for separator/NUL
        if current + len > budget && start < i {
            chunks.push(&files[start..i]);
            start = i;
            current = 0;
        }
        current += len;
    }
    if start < files.len() {
        chunks.push(&files[start..]);
    }
    chunks
}

/// Split `files` into chunks where each chunk's total argv byte size (including
/// the fixed `cmd_argv` prefix) stays under the platform `ARG_MAX` minus 25%
/// headroom. Returns at least one chunk for any non-empty input; an empty
/// `files` slice yields an empty `Vec` (D-27 — caller skips empty batches).
pub fn chunk_files<'a>(cmd_argv: &[String], files: &'a [PathBuf]) -> Vec<&'a [PathBuf]> {
    if files.is_empty() {
        return Vec::new();
    }
    let cmd_len: usize = cmd_argv.iter().map(|s| s.len() + 1).sum();
    let budget = arg_max().saturating_sub(cmd_len);
    chunk_files_with_budget(files, budget)
}

/// Spawn a command and capture its full stdout+stderr.
///
/// - Returns `Ok(Output)` for both success AND non-zero exit (caller inspects
///   `output.status` to decide what is a failure).
/// - Returns `Err(AppError::CommandNotFound)` if the binary is not on PATH (ENOENT).
/// - Returns `Err(AppError::Io)` for other IO errors.
pub async fn spawn_command(
    argv: &[String],
    working_dir: &Path,
) -> Result<std::process::Output, AppError> {
    if argv.is_empty() {
        return Err(AppError::ConfigInvalid("empty argv".to_string()));
    }
    Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::CommandNotFound(argv[0].clone())
            } else {
                AppError::Io(e)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        std::env::current_dir().unwrap()
    }

    // ---- build_argv ----

    #[test]
    fn build_argv_parses_simple_command() {
        let argv = build_argv(
            "prettier --write",
            &[PathBuf::from("a.ts")],
            PathFormat::Relative,
            &root(),
        )
        .unwrap();
        assert_eq!(argv, vec!["prettier", "--write", "a.ts"]);
    }

    #[test]
    fn build_argv_handles_placeholder() {
        let argv = build_argv(
            "cmd --files {files} --strict",
            &[PathBuf::from("a.ts"), PathBuf::from("b.ts")],
            PathFormat::Relative,
            &root(),
        )
        .unwrap();
        assert_eq!(argv, vec!["cmd", "--files", "a.ts", "b.ts", "--strict"]);
    }

    #[test]
    fn build_argv_unbalanced_quotes_returns_config_invalid() {
        let result = build_argv(
            "echo \"unbalanced",
            &[PathBuf::from("a.ts")],
            PathFormat::Relative,
            &root(),
        );
        assert!(matches!(result, Err(AppError::ConfigInvalid(_))));
    }

    #[test]
    fn build_argv_path_format_absolute_canonicalizes() {
        // Use the actual project root and a real-looking relative path.
        // Even if canonicalize fails (file doesn't exist), it falls back to joined path.
        let git_root = root();
        let argv = build_argv(
            "tool",
            &[PathBuf::from("src/lib.rs")],
            PathFormat::Absolute,
            &git_root,
        )
        .unwrap();
        // argv = ["tool", "<absolute>/src/lib.rs"]
        assert_eq!(argv.len(), 2);
        let last = &argv[1];
        // Absolute paths start with "/" on unix or have a Windows drive letter / UNC prefix.
        let is_absolute = last.starts_with('/')
            || (last.len() > 2
                && last.chars().nth(1) == Some(':')
                && last.chars().nth(2) == Some('\\'));
        assert!(is_absolute, "expected absolute path, got {last}");
        assert!(last.ends_with("src/lib.rs") || last.ends_with("src\\lib.rs"));
    }

    #[test]
    fn build_argv_path_format_relative_passes_through() {
        let argv = build_argv(
            "tool",
            &[PathBuf::from("src/foo.ts")],
            PathFormat::Relative,
            &root(),
        )
        .unwrap();
        assert_eq!(argv, vec!["tool".to_string(), "src/foo.ts".to_string()]);
    }

    #[test]
    fn build_argv_no_files_with_placeholder_drops_token() {
        let argv = build_argv("cmd {files} --strict", &[], PathFormat::Relative, &root()).unwrap();
        // Empty file list means {files} is replaced by nothing.
        assert_eq!(argv, vec!["cmd", "--strict"]);
    }

    // ---- chunk_files ----

    #[test]
    fn chunk_files_returns_single_chunk_for_small_input() {
        let files = vec![PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")];
        let chunks = chunk_files(&["cmd".to_string()], &files);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 3);
    }

    #[test]
    fn chunk_files_returns_empty_vec_for_empty_input() {
        let chunks = chunk_files(&["cmd".to_string()], &[]);
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn chunk_files_splits_on_arg_max_overflow() {
        // Use a tiny synthetic budget via the test helper.
        let files: Vec<PathBuf> = vec![
            PathBuf::from("aaaaaaa"), // 7+1 = 8
            PathBuf::from("bbbbbbb"), // 8 -> running 16
            PathBuf::from("ccccccc"), // 8 -> would push over 20
            PathBuf::from("ddddddd"),
        ];
        let chunks = chunk_files_with_budget(&files, 20);
        assert!(
            chunks.len() >= 2,
            "expected at least 2 chunks, got {}",
            chunks.len()
        );
        // Reassemble and verify order preserved.
        let flat: Vec<&PathBuf> = chunks.iter().flat_map(|c| c.iter()).collect();
        assert_eq!(flat.len(), files.len());
        for (i, f) in files.iter().enumerate() {
            assert_eq!(flat[i], f);
        }
    }

    #[test]
    fn chunk_files_preserves_file_order_within_and_across_chunks() {
        let files: Vec<PathBuf> = vec![
            PathBuf::from("a"),
            PathBuf::from("b"),
            PathBuf::from("c"),
            PathBuf::from("d"),
        ];
        let chunks = chunk_files_with_budget(&files, 4); // budget 4 -> chunks of ~2
        let flat: Vec<&PathBuf> = chunks.iter().flat_map(|c| c.iter()).collect();
        assert_eq!(flat, vec![&files[0], &files[1], &files[2], &files[3]]);
    }

    #[test]
    fn chunk_files_single_oversized_file_still_emits_chunk() {
        // Pathological: one file alone exceeds budget. We still emit it as one chunk
        // (caller would otherwise skip it, breaking semantics). The argv would fail
        // at spawn — that's the OS's job to report.
        let files = vec![PathBuf::from("verylongfilename")];
        let chunks = chunk_files_with_budget(&files, 4);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 1);
    }

    // ---- spawn_command ----

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_command_success_returns_output_with_zero_exit() {
        let argv = vec!["true".to_string()];
        let cwd = std::env::current_dir().unwrap();
        let output = spawn_command(&argv, &cwd).await.unwrap();
        assert_eq!(output.status.code(), Some(0));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_command_nonzero_exit_returns_output_with_nonzero_status() {
        let argv = vec!["false".to_string()];
        let cwd = std::env::current_dir().unwrap();
        let output = spawn_command(&argv, &cwd).await.unwrap();
        // `false` always exits with code 1.
        assert_eq!(output.status.code(), Some(1));
    }

    #[tokio::test]
    async fn spawn_command_missing_binary_returns_command_not_found() {
        let argv = vec!["definitely-not-a-real-binary-xyz".to_string()];
        let cwd = std::env::current_dir().unwrap();
        let result = spawn_command(&argv, &cwd).await;
        match result {
            Err(AppError::CommandNotFound(name)) => {
                assert!(
                    name.contains("definitely-not-a-real-binary-xyz"),
                    "expected binary name in error, got {name}"
                );
            }
            other => panic!("expected CommandNotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn spawn_command_empty_argv_returns_config_invalid() {
        let argv: Vec<String> = vec![];
        let cwd = std::env::current_dir().unwrap();
        let result = spawn_command(&argv, &cwd).await;
        assert!(matches!(result, Err(AppError::ConfigInvalid(_))));
    }

    // ---- execute_all / execute_group / run_one (Plan 05-03) ----

    use crate::config::types::{Config, ConfigSource, ExecutionOrder, Group};
    use crate::matcher::{MatchedGroup, MatchedPattern};
    use std::collections::HashMap;
    use std::time::Instant;

    fn make_group(name: &str, exec: ExecutionOrder, coe: bool, per_file: bool) -> Group {
        Group {
            name: name.to_string(),
            patterns: HashMap::new(),
            timeout: None,
            execution_order: exec,
            continue_on_error: coe,
            per_file,
            path_format: PathFormat::Relative,
        }
    }

    fn make_config(continue_on_error: bool) -> Config {
        Config {
            source: ConfigSource::TomlFile(PathBuf::from("test.toml")),
            groups: Vec::new(),
            timeout: None,
            execution_order: ExecutionOrder::Parallel,
            continue_on_error,
            per_file: false,
        }
    }

    fn pattern_with_files(pattern: &str, command: &str, files: Vec<&str>) -> MatchedPattern {
        MatchedPattern {
            pattern: pattern.to_string(),
            commands: vec![command.to_string()],
            files: files.into_iter().map(PathBuf::from).collect(),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn parallel_wall_clock_under_sum() {
        let g = make_group("p", ExecutionOrder::Parallel, true, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![
                MatchedPattern {
                    pattern: "*.x".to_string(),
                    commands: vec!["sh -c \"sleep 0.2\"".to_string()],
                    files: vec![PathBuf::from("dummy.x")],
                },
                MatchedPattern {
                    pattern: "*.y".to_string(),
                    commands: vec!["sh -c \"sleep 0.2\"".to_string()],
                    files: vec![PathBuf::from("dummy.y")],
                },
            ],
        };
        let cfg = make_config(true);
        let cwd = std::env::current_dir().unwrap();
        let start = Instant::now();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        let elapsed = start.elapsed();
        assert_eq!(outcomes.len(), 2, "expected 2 outcomes, got {}", outcomes.len());
        // Two parallel sleep 0.2 → wall clock should be well under 0.35s.
        assert!(
            elapsed.as_secs_f64() < 0.35,
            "expected parallel wall clock < 0.35s, got {:?}",
            elapsed
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sequential_wall_clock_at_least_sum() {
        let g = make_group("s", ExecutionOrder::Sequential, true, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![
                MatchedPattern {
                    pattern: "*.x".to_string(),
                    commands: vec!["sh -c \"sleep 0.2\"".to_string()],
                    files: vec![PathBuf::from("dummy.x")],
                },
                MatchedPattern {
                    pattern: "*.y".to_string(),
                    commands: vec!["sh -c \"sleep 0.2\"".to_string()],
                    files: vec![PathBuf::from("dummy.y")],
                },
            ],
        };
        let cfg = make_config(true);
        let cwd = std::env::current_dir().unwrap();
        let start = Instant::now();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        let elapsed = start.elapsed();
        assert_eq!(outcomes.len(), 2);
        // Two sequential sleep 0.2 → wall clock should be ≥ 0.35s (≈ sum 0.4).
        assert!(
            elapsed.as_secs_f64() >= 0.35,
            "expected sequential wall clock >= 0.35s, got {:?}",
            elapsed
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn exit_code_propagates_when_command_fails() {
        let g = make_group("g", ExecutionOrder::Parallel, true, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![
                pattern_with_files("ok", "true", vec!["a.x"]),
                pattern_with_files("bad", "false", vec!["b.x"]),
            ],
        };
        let cfg = make_config(true);
        let cwd = std::env::current_dir().unwrap();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        assert_eq!(outcomes.len(), 2);
        let mut had_failure_with_1 = false;
        let mut had_success = false;
        for o in &outcomes {
            match o {
                CommandOutcome::Failure { exit_code, .. } => {
                    if *exit_code == 1 {
                        had_failure_with_1 = true;
                    }
                }
                CommandOutcome::Success { .. } => {
                    had_success = true;
                }
                _ => {}
            }
        }
        assert!(had_failure_with_1, "expected at least one Failure with exit_code=1");
        assert!(had_success, "expected at least one Success");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failure_in_first_group_skips_second_group_when_global_coe_false() {
        let g1 = make_group("g1", ExecutionOrder::Parallel, true, false);
        let g2 = make_group("g2", ExecutionOrder::Parallel, true, false);
        let groups = [&g1, &g2];
        let mgs = vec![
            MatchedGroup {
                group: groups[0],
                patterns: vec![pattern_with_files("p1", "false", vec!["a.x"])],
            },
            MatchedGroup {
                group: groups[1],
                patterns: vec![pattern_with_files("p2", "true", vec!["b.x"])],
            },
        ];
        let cfg = make_config(false); // global continue_on_error = false
        let cwd = std::env::current_dir().unwrap();
        let outcomes = super::execute_all(&mgs, &cfg, &cwd).await;
        // Only G1's failure should be present.
        assert_eq!(outcomes.len(), 1, "expected only G1 outcomes, got {outcomes:?}");
        for o in &outcomes {
            match o {
                CommandOutcome::Success { group, pattern, .. }
                | CommandOutcome::Failure { group, pattern, .. }
                | CommandOutcome::Cancelled { group, pattern, .. } => {
                    assert_ne!(group, "g2", "g2 should not have been spawned");
                    assert_ne!(pattern, "p2", "p2 should not have been spawned");
                }
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failure_in_first_group_runs_second_group_when_global_coe_true() {
        let g1 = make_group("g1", ExecutionOrder::Parallel, true, false);
        let g2 = make_group("g2", ExecutionOrder::Parallel, true, false);
        let mgs = vec![
            MatchedGroup {
                group: &g1,
                patterns: vec![pattern_with_files("p1", "false", vec!["a.x"])],
            },
            MatchedGroup {
                group: &g2,
                patterns: vec![pattern_with_files("p2", "true", vec!["b.x"])],
            },
        ];
        let cfg = make_config(true); // global continue_on_error = true
        let cwd = std::env::current_dir().unwrap();
        let outcomes = super::execute_all(&mgs, &cfg, &cwd).await;
        assert_eq!(outcomes.len(), 2, "expected outcomes from both groups");
        let mut saw_g1_failure = false;
        let mut saw_g2_success = false;
        for o in &outcomes {
            match o {
                CommandOutcome::Failure { group, .. } if group == "g1" => saw_g1_failure = true,
                CommandOutcome::Success { group, .. } if group == "g2" => saw_g2_success = true,
                _ => {}
            }
        }
        assert!(saw_g1_failure, "expected g1 failure: {outcomes:?}");
        assert!(saw_g2_success, "expected g2 success: {outcomes:?}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn group_cancel_flag_set_on_sibling_failure_when_group_coe_false() {
        // Group-level continue_on_error = false: when the false sibling fails,
        // remaining sleepers should observe the cancel flag (or finish as Failure).
        let g = make_group("cancellable", ExecutionOrder::Parallel, false, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![
                pattern_with_files("slow1", "sh -c \"sleep 0.5; exit 0\"", vec!["a.x"]),
                pattern_with_files("slow2", "sh -c \"sleep 0.5; exit 0\"", vec!["b.x"]),
                pattern_with_files("fail", "false", vec!["c.x"]),
            ],
        };
        let cfg = make_config(true);
        let cwd = std::env::current_dir().unwrap();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        // Must have at least one Failure outcome.
        let has_failure = outcomes
            .iter()
            .any(|o| matches!(o, CommandOutcome::Failure { .. }));
        assert!(
            has_failure,
            "expected at least one Failure outcome, got {outcomes:?}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn per_file_spawns_one_process_per_file() {
        let g = make_group("p", ExecutionOrder::Parallel, true, /* per_file */ true);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![pattern_with_files(
                "*.x",
                "echo",
                vec!["a.x", "b.x", "c.x"],
            )],
        };
        let cfg = make_config(true);
        let cwd = std::env::current_dir().unwrap();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        assert_eq!(outcomes.len(), 3, "expected 3 outcomes (per-file): {outcomes:?}");
        for o in &outcomes {
            match o {
                CommandOutcome::Success { argv, file_count, .. } => {
                    assert_eq!(*file_count, 1, "per-file → file_count=1");
                    // argv = ["echo", "<one-file>"]
                    assert_eq!(argv.len(), 2);
                }
                other => panic!("expected Success, got {other:?}"),
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn batch_spawns_one_process_with_all_files() {
        let g = make_group("b", ExecutionOrder::Parallel, true, /* per_file */ false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![pattern_with_files(
                "*.x",
                "echo",
                vec!["a.x", "b.x", "c.x"],
            )],
        };
        let cfg = make_config(true);
        let cwd = std::env::current_dir().unwrap();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        assert_eq!(outcomes.len(), 1, "expected 1 outcome (batch): {outcomes:?}");
        match &outcomes[0] {
            CommandOutcome::Success { argv, file_count, .. } => {
                assert_eq!(*file_count, 3);
                // argv = ["echo", "a.x", "b.x", "c.x"]
                assert!(argv.contains(&"a.x".to_string()));
                assert!(argv.contains(&"b.x".to_string()));
                assert!(argv.contains(&"c.x".to_string()));
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_pattern_skips_spawn() {
        let g = make_group("e", ExecutionOrder::Parallel, true, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![MatchedPattern {
                pattern: "*.x".to_string(),
                commands: vec!["true".to_string()],
                files: vec![],
            }],
        };
        let cfg = make_config(true);
        let cwd = std::env::current_dir().unwrap();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        assert_eq!(outcomes.len(), 0, "empty file list → zero outcomes");
    }

    #[tokio::test]
    async fn command_not_found_produces_failure_outcome() {
        let g = make_group("nf", ExecutionOrder::Parallel, true, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![pattern_with_files(
                "*.x",
                "definitely-not-a-real-binary-zzz",
                vec!["a.x"],
            )],
        };
        let cfg = make_config(true);
        let cwd = std::env::current_dir().unwrap();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        assert_eq!(outcomes.len(), 1);
        match &outcomes[0] {
            CommandOutcome::Failure { exit_code, stderr, .. } => {
                assert_ne!(*exit_code, 0, "missing binary → non-zero exit_code");
                let stderr_str = String::from_utf8_lossy(stderr);
                assert!(
                    stderr_str.contains("definitely-not-a-real-binary-zzz"),
                    "expected stderr to mention binary name, got: {stderr_str}"
                );
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }
}
