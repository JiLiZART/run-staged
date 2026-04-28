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
//!
//! TDD RED stage: signatures only, bodies unimplemented. Tests defined below
//! will fail until Task 2 GREEN lands real bodies.

use std::path::{Path, PathBuf};

use crate::AppError;
use crate::config::types::PathFormat;

pub fn build_argv(
    _command: &str,
    _files: &[PathBuf],
    _path_format: PathFormat,
    _git_root: &Path,
) -> Result<Vec<String>, AppError> {
    unimplemented!("build_argv: TDD RED — implement in GREEN commit")
}

pub(crate) fn chunk_files_with_budget<'a>(
    _files: &'a [PathBuf],
    _budget: usize,
) -> Vec<&'a [PathBuf]> {
    unimplemented!("chunk_files_with_budget: TDD RED — implement in GREEN commit")
}

pub fn chunk_files<'a>(_cmd_argv: &[String], _files: &'a [PathBuf]) -> Vec<&'a [PathBuf]> {
    unimplemented!("chunk_files: TDD RED — implement in GREEN commit")
}

pub async fn spawn_command(
    _argv: &[String],
    _working_dir: &Path,
) -> Result<std::process::Output, AppError> {
    unimplemented!("spawn_command: TDD RED — implement in GREEN commit")
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
        let git_root = root();
        let argv = build_argv(
            "tool",
            &[PathBuf::from("src/lib.rs")],
            PathFormat::Absolute,
            &git_root,
        )
        .unwrap();
        assert_eq!(argv.len(), 2);
        let last = &argv[1];
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
        let argv = build_argv(
            "cmd {files} --strict",
            &[],
            PathFormat::Relative,
            &root(),
        )
        .unwrap();
        assert_eq!(argv, vec!["cmd", "--strict"]);
    }

    // ---- chunk_files ----

    #[test]
    fn chunk_files_returns_single_chunk_for_small_input() {
        let files = vec![
            PathBuf::from("a"),
            PathBuf::from("b"),
            PathBuf::from("c"),
        ];
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
        let files: Vec<PathBuf> = vec![
            PathBuf::from("aaaaaaa"),
            PathBuf::from("bbbbbbb"),
            PathBuf::from("ccccccc"),
            PathBuf::from("ddddddd"),
        ];
        let chunks = chunk_files_with_budget(&files, 20);
        assert!(
            chunks.len() >= 2,
            "expected at least 2 chunks, got {}",
            chunks.len()
        );
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
        let chunks = chunk_files_with_budget(&files, 4);
        let flat: Vec<&PathBuf> = chunks.iter().flat_map(|c| c.iter()).collect();
        assert_eq!(flat, vec![&files[0], &files[1], &files[2], &files[3]]);
    }

    #[test]
    fn chunk_files_single_oversized_file_still_emits_chunk() {
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
