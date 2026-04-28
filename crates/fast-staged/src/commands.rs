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

use tokio::process::Command;

use crate::AppError;
use crate::config::types::PathFormat;

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
}
