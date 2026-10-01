//! Command building and spawning primitives.
//!
//! `build_argv()` parses a command string via shlex and applies the matched files
//! using either `{files}` placeholder substitution or positional append.
//!
//! `chunk_files()` splits a file list into argv-safe chunks based on platform ARG_MAX
//! with 25% headroom for environment variables.
//!
//! `spawn_command()` runs a parsed argv via `tokio::process::Command`, mapping ENOENT
//! to `ExecError::CommandNotFound`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::task::JoinSet;
use tokio::time::timeout as tokio_timeout;

mod error;
pub use error::ExecError;
use run_staged_config::types::MatchedGroup;
use run_staged_config::types::{Config, ExecutionOrder, PathFormat};

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

/// One unit of work: a fully-built argv plus the metadata needed to construct
/// a [`CommandOutcome`] after spawn completes. One `MatchedPattern` may expand
/// into many `Task`s under per-file mode or ARG_MAX chunking.
#[derive(Debug, Clone)]
struct Task {
    group_name: String,
    pattern: String,
    /// Original command string from config (for reporting in CommandOutcome).
    command: String,
    /// Pre-built argv (after build_argv + chunk).
    argv: Vec<String>,
    /// How many files this task represents (1 in per-file mode; chunk size in batch mode).
    file_count: usize,
    working_dir: PathBuf,
}

/// Drive a single [`Task`] through a manual spawn (so the [`Child`] handle is
/// retained for `kill()`), mapping the result to a [`CommandOutcome`].
///
/// **Plan 04 timeout enforcement (D-17, D-18, D-19, D-20):**
/// When `effective_timeout = Some(d)`, `child.wait()` is wrapped in
/// `tokio::time::timeout(d, ...)`. On `Elapsed`:
/// - **Unix:** send SIGTERM via `nix::sys::signal::kill`, await `child.wait()`
///   for up to 2 seconds (grace window for cooperative cleanup), then call
///   `child.kill().await` (SIGKILL) which also reaps the child to avoid zombies.
/// - **Windows:** call `child.kill().await` directly — `TerminateProcess`
///   has no graceful-exit equivalent, so no SIGTERM grace window.
///
/// When `effective_timeout = None`, no timeout wrapping is applied — the spawn
/// awaits `child.wait()` directly, behavior-equivalent to the Plan 03 path.
///
/// **Cooperative cancellation (D-13):** if the per-group flag is already set
/// when `run_one` starts, the function returns `CommandOutcome::Cancelled`
/// without spawning. Once the child is in flight we do not interrupt it from
/// this layer (only the timeout path kills running children).
async fn run_one(
    task: Task,
    cancel: Arc<AtomicBool>,
    effective_timeout: Option<Duration>,
) -> CommandOutcome {
    if cancel.load(Ordering::Relaxed) {
        return CommandOutcome::Cancelled {
            group: task.group_name,
            pattern: task.pattern,
            command: task.command,
        };
    }

    let start = Instant::now();

    if task.argv.is_empty() {
        // Defensive: spawn_command also rejects empty argv, but keep the
        // shape parallel for the manual spawn path.
        return failure_outcome_io_msg(&task, start.elapsed(), "empty argv".to_string());
    }

    // Manual spawn — keep Child so we can kill on timeout.
    let mut child = match Command::new(&task.argv[0])
        .args(&task.argv[1..])
        .current_dir(&task.working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return failure_outcome_command_not_found(&task, start.elapsed());
        }
        Err(e) => {
            return failure_outcome_io(&task, start.elapsed(), e);
        }
    };

    // Take stdout/stderr handles so `child.wait()` can advance. We drain
    // them *after* the process exits — child output is small (lint/format
    // stderr-summary scale) so a deferred read is acceptable.
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let exit_status_result = match effective_timeout {
        Some(d) => match tokio_timeout(d, child.wait()).await {
            Ok(inner) => inner,
            Err(_elapsed) => {
                // Timeout fired — terminate the child (SIGTERM-then-SIGKILL on
                // Unix; TerminateProcess on Windows). After this, `child` has
                // been reaped by `kill().await`, so we can drop the handles.
                terminate(&mut child).await;
                return failure_outcome_timeout(&task, start.elapsed(), d);
            }
        },
        None => child.wait().await,
    };

    let exit_status = match exit_status_result {
        Ok(s) => s,
        Err(e) => {
            return failure_outcome_io(&task, start.elapsed(), e);
        }
    };

    // Drain captured stdout / stderr.
    let mut stdout_buf = Vec::new();
    if let Some(mut h) = stdout_handle {
        let _ = h.read_to_end(&mut stdout_buf).await;
    }
    let mut stderr_buf = Vec::new();
    if let Some(mut h) = stderr_handle {
        let _ = h.read_to_end(&mut stderr_buf).await;
    }

    let duration = start.elapsed();
    if exit_status.success() {
        CommandOutcome::Success {
            group: task.group_name,
            pattern: task.pattern,
            command: task.command,
            argv: task.argv,
            file_count: task.file_count,
            duration,
            stdout: stdout_buf,
        }
    } else {
        CommandOutcome::Failure {
            group: task.group_name,
            pattern: task.pattern,
            command: task.command,
            argv: task.argv,
            file_count: task.file_count,
            duration,
            exit_code: exit_status.code().unwrap_or(-1),
            stdout: stdout_buf,
            stderr: stderr_buf,
        }
    }
}

/// Cross-platform child termination after a timeout fired.
///
/// **Unix:** send SIGTERM via `nix::sys::signal::kill`, then await
/// `child.wait()` with a 2-second grace timeout. After the grace window
/// (or if SIGTERM dispatch failed), call `child.kill().await` which sends
/// SIGKILL and reaps the child (avoiding zombies — Pitfall 6).
///
/// **Windows:** there is no graceful-termination equivalent, so call
/// `child.kill().await` directly (`TerminateProcess`).
async fn terminate(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGTERM,
            );
            // Grace window for cooperative cleanup before SIGKILL.
            let _ = tokio_timeout(Duration::from_secs(2), child.wait()).await;
        }
    }
    // SIGKILL on Unix; TerminateProcess on Windows. Also reaps the child.
    let _ = child.kill().await;
}

/// Build a [`CommandOutcome::Failure`] for the timeout path, with a stderr body
/// derived from `ExecError::CommandTimeout` so it always contains `"timed out"`.
fn failure_outcome_timeout(
    task: &Task,
    duration: Duration,
    configured: Duration,
) -> CommandOutcome {
    let err = ExecError::CommandTimeout {
        command: task.command.clone(),
        duration: configured,
    };
    CommandOutcome::Failure {
        group: task.group_name.clone(),
        pattern: task.pattern.clone(),
        command: task.command.clone(),
        argv: task.argv.clone(),
        file_count: task.file_count,
        duration,
        exit_code: -1,
        stdout: Vec::new(),
        stderr: err.to_string().into_bytes(),
    }
}

/// Build a [`CommandOutcome::Failure`] for the ENOENT path (POSIX convention:
/// shell exit 127, stderr `"command not found: NAME"`).
fn failure_outcome_command_not_found(task: &Task, duration: Duration) -> CommandOutcome {
    CommandOutcome::Failure {
        group: task.group_name.clone(),
        pattern: task.pattern.clone(),
        command: task.command.clone(),
        argv: task.argv.clone(),
        file_count: task.file_count,
        duration,
        exit_code: 127,
        stdout: Vec::new(),
        stderr: format!("command not found: {}", task.argv[0]).into_bytes(),
    }
}

/// Build a [`CommandOutcome::Failure`] for an arbitrary `std::io::Error`.
fn failure_outcome_io(task: &Task, duration: Duration, e: std::io::Error) -> CommandOutcome {
    failure_outcome_io_msg(task, duration, format!("io error: {e}"))
}

/// Build a [`CommandOutcome::Failure`] with a synthetic stderr message
/// (no `std::io::Error` instance available).
fn failure_outcome_io_msg(task: &Task, duration: Duration, msg: String) -> CommandOutcome {
    CommandOutcome::Failure {
        group: task.group_name.clone(),
        pattern: task.pattern.clone(),
        command: task.command.clone(),
        argv: task.argv.clone(),
        file_count: task.file_count,
        duration,
        exit_code: -1,
        stdout: Vec::new(),
        stderr: msg.into_bytes(),
    }
}

/// Expand a `MatchedGroup` into a flat list of `Task`s. Honors per-file vs
/// batch mode, runs `build_argv` once per chunk, and chunk-splits batch mode
/// via `chunk_files`. Patterns with empty file lists are skipped (D-27 — the
/// matcher already drops these but this is a defensive guard for direct
/// callers that construct `MatchedGroup` literally).
fn tasks_for_group(matched: &MatchedGroup<'_>, git_root: &Path) -> Result<Vec<Task>, ExecError> {
    let mut tasks = Vec::new();
    let group = matched.group;
    let working_dir = git_root.to_path_buf();

    for pattern in &matched.patterns {
        if pattern.files.is_empty() {
            continue;
        }
        for command_str in &pattern.commands {
            if group.per_file {
                // One process per file — build_argv called per file, no chunking.
                for file in &pattern.files {
                    let file_slice = std::slice::from_ref(file);
                    let argv = build_argv(command_str, file_slice, group.path_format, git_root)?;
                    tasks.push(Task {
                        group_name: group.name.clone(),
                        pattern: pattern.pattern.clone(),
                        command: command_str.clone(),
                        argv,
                        file_count: 1,
                        working_dir: working_dir.clone(),
                    });
                }
            } else {
                // Batch mode: build the bare argv once for chunk-budgeting,
                // then build_argv per chunk so {files} substitution / positional
                // append happens per-chunk against the chosen file slice.
                let bare_argv = build_argv(command_str, &[], group.path_format, git_root)?;
                // Arg-length budget first, then the configured chunk_size, so each
                // invocation respects whichever limit is smaller. Order is preserved.
                let max_files = group.chunk_size.unwrap_or(usize::MAX);
                let chunks = chunk_files(&bare_argv, &pattern.files)
                    .into_iter()
                    .flat_map(|c| c.chunks(max_files));
                for chunk in chunks {
                    let argv = build_argv(command_str, chunk, group.path_format, git_root)?;
                    tasks.push(Task {
                        group_name: group.name.clone(),
                        pattern: pattern.pattern.clone(),
                        command: command_str.clone(),
                        argv,
                        file_count: chunk.len(),
                        working_dir: working_dir.clone(),
                    });
                }
            }
        }
    }
    Ok(tasks)
}

/// Run all `tasks` concurrently via [`JoinSet`]. On any failing outcome under
/// `continue_on_error == false`, set the per-group cancel flag so already-spawned
/// siblings can bail out cooperatively; new tasks check the flag at start (D-13).
async fn execute_group_parallel(
    tasks: Vec<Task>,
    cancel: Arc<AtomicBool>,
    continue_on_error: bool,
    effective_timeout: Option<Duration>,
) -> Vec<CommandOutcome> {
    let mut set: JoinSet<CommandOutcome> = JoinSet::new();
    for task in tasks {
        let cancel_for_task = Arc::clone(&cancel);
        set.spawn(async move { run_one(task, cancel_for_task, effective_timeout).await });
    }

    let mut outcomes = Vec::new();
    while let Some(joined) = set.join_next().await {
        let outcome = match joined {
            Ok(o) => o,
            Err(e) => CommandOutcome::Failure {
                group: String::new(),
                pattern: String::new(),
                command: String::new(),
                argv: vec![],
                file_count: 0,
                duration: Duration::ZERO,
                exit_code: -1,
                stdout: vec![],
                stderr: format!("task panicked: {e}").into_bytes(),
            },
        };
        if outcome.is_failure() && !continue_on_error {
            cancel.store(true, Ordering::Relaxed);
        }
        outcomes.push(outcome);
    }
    outcomes
}

/// Run all `tasks` one after another in declared order. After the first
/// failing outcome under `continue_on_error == false`, set the cancel flag
/// and emit `CommandOutcome::Cancelled` for every remaining task without
/// spawning them.
async fn execute_group_sequential(
    tasks: Vec<Task>,
    cancel: Arc<AtomicBool>,
    continue_on_error: bool,
    effective_timeout: Option<Duration>,
) -> Vec<CommandOutcome> {
    let mut outcomes = Vec::with_capacity(tasks.len());
    for task in tasks {
        let outcome = run_one(task, Arc::clone(&cancel), effective_timeout).await;
        if outcome.is_failure() && !continue_on_error {
            cancel.store(true, Ordering::Relaxed);
        }
        outcomes.push(outcome);
    }
    outcomes
}

/// Execute every matched group end-to-end.
///
/// **Outer loop:** strictly sequential across groups (D-14 — no two groups
/// ever overlap). **Inner dispatch:** obeys each group's `execution_order`
/// — `Parallel` spawns all of the group's `Task`s into a [`JoinSet`];
/// `Sequential` runs them one at a time in declared order.
///
/// **Per-group cancellation (D-13):** each group gets a fresh `Arc<AtomicBool>`.
/// When any task in the group fails AND `Group.continue_on_error == false`,
/// the flag is set; not-yet-spawned tasks observe the flag at start and emit
/// `CommandOutcome::Cancelled`. Already-running tasks finish normally — we
/// never SIGTERM in-flight processes from this layer (Plan 04 layers timeout
/// on top, which DOES terminate the child process when the timer fires).
///
/// **Cross-group gate (D-14):** after a group finishes, if any of its outcomes
/// is a failure AND `Config.continue_on_error == false`, subsequent groups
/// are not started. Their patterns will be absent from the returned `Vec`.
///
/// **Per-file vs batch (D-24/D-25):** when `Group.per_file == true`, one
/// process spawns per file (one outcome per file). Otherwise files are
/// passed as args to a single invocation, chunked via `chunk_files` to stay
/// under ARG_MAX.
///
/// **Empty patterns (D-27):** patterns whose `files` is empty are skipped
/// before spawn. Defensive — the matcher already drops these.
///
/// **Failure semantics (D-15):** the function ALWAYS returns the full outcome
/// list rather than short-circuiting via `Result`. The caller (Plan 05-05's
/// `run()`) iterates and calls `is_failure()` to decide whether to roll back
/// the staged diff.
///
/// **CommandNotFound (D-08):** mapped to `CommandOutcome::Failure` with
/// `exit_code = 127` and `stderr = "command not found: NAME"`, matching the
/// POSIX shell convention.
pub async fn execute_all(
    matched_groups: &[MatchedGroup<'_>],
    config: &Config,
    git_root: &Path,
) -> Vec<CommandOutcome> {
    let mut all_outcomes: Vec<CommandOutcome> = Vec::new();

    for matched in matched_groups {
        // Per-group cancel flag — fresh for each group so a previous group's
        // failure doesn't leak cancellation into the next one (when global
        // continue_on_error allows continuation).
        let cancel = Arc::new(AtomicBool::new(false));

        let tasks = match tasks_for_group(matched, git_root) {
            Ok(t) => t,
            Err(e) => {
                // build_argv shlex failure — surface as a synthetic Failure
                // so the caller still sees a non-zero outcome for this group.
                all_outcomes.push(CommandOutcome::Failure {
                    group: matched.group.name.clone(),
                    pattern: String::new(),
                    command: String::new(),
                    argv: vec![],
                    file_count: 0,
                    duration: Duration::ZERO,
                    exit_code: -1,
                    stdout: vec![],
                    stderr: format!("argv build failed: {e}").into_bytes(),
                });
                if !config.continue_on_error {
                    break;
                }
                continue;
            }
        };

        // Defensive D-27: empty pattern files → no tasks → skip group entirely.
        if tasks.is_empty() {
            continue;
        }

        // Plan 04 (D-17): `Group.timeout` already reflects the
        // group→global→none cascade per `defaults::resolve_group()`, so we
        // consume it directly here with no further merge.
        let effective_timeout = matched.group.timeout;

        let group_outcomes = match matched.group.execution_order {
            ExecutionOrder::Parallel => {
                execute_group_parallel(
                    tasks,
                    cancel,
                    matched.group.continue_on_error,
                    effective_timeout,
                )
                .await
            }
            ExecutionOrder::Sequential => {
                execute_group_sequential(
                    tasks,
                    cancel,
                    matched.group.continue_on_error,
                    effective_timeout,
                )
                .await
            }
        };

        let group_had_failure = group_outcomes.iter().any(CommandOutcome::is_failure);
        all_outcomes.extend(group_outcomes);
        if group_had_failure && !config.continue_on_error {
            // D-14: skip subsequent groups.
            break;
        }
    }

    all_outcomes
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
/// Returns `Err(ExecError::InvalidCommand)` if shlex cannot parse the command (e.g.
/// unbalanced quotes).
pub fn build_argv(
    command: &str,
    files: &[PathBuf],
    path_format: PathFormat,
    git_root: &Path,
) -> Result<Vec<String>, ExecError> {
    let parsed = shlex::split(command)
        .ok_or_else(|| ExecError::InvalidCommand(format!("invalid command syntax: {command:?}")))?;

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
/// - Returns `Err(ExecError::CommandNotFound)` if the binary is not on PATH (ENOENT).
/// - Returns `Err(ExecError::Io)` for other IO errors.
pub async fn spawn_command(
    argv: &[String],
    working_dir: &Path,
) -> Result<std::process::Output, ExecError> {
    if argv.is_empty() {
        return Err(ExecError::InvalidCommand("empty argv".to_string()));
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
                ExecError::CommandNotFound(argv[0].clone())
            } else {
                ExecError::Io(e)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Working directory for tests: outside any git repository, so
    /// fixture-tool never writes an invocation log into this repo's `.git`.
    fn root() -> PathBuf {
        std::env::temp_dir()
    }

    /// Absolute path of the portable fake command.
    fn tool() -> String {
        run_staged_testkit::fixture_tool_path()
            .to_string_lossy()
            .into_owned()
    }

    /// A command string running fixture-tool with `args`.
    fn ft(args: &str) -> String {
        let quoted = shlex::try_quote(&tool()).unwrap().into_owned();
        format!("{quoted} {args}").trim_end().to_string()
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
        assert!(matches!(result, Err(ExecError::InvalidCommand(_))));
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

    #[tokio::test]
    async fn spawn_command_success_returns_output_with_zero_exit() {
        let argv = vec![tool()];
        let cwd = root();
        let output = spawn_command(&argv, &cwd).await.unwrap();
        assert_eq!(output.status.code(), Some(0));
    }

    #[tokio::test]
    async fn spawn_command_nonzero_exit_returns_output_with_nonzero_status() {
        let argv = vec![tool(), "--exit".to_string(), "1".to_string()];
        let cwd = root();
        let output = spawn_command(&argv, &cwd).await.unwrap();
        // `false` always exits with code 1.
        assert_eq!(output.status.code(), Some(1));
    }

    #[tokio::test]
    async fn spawn_command_missing_binary_returns_command_not_found() {
        let argv = vec!["definitely-not-a-real-binary-xyz".to_string()];
        let cwd = root();
        let result = spawn_command(&argv, &cwd).await;
        match result {
            Err(ExecError::CommandNotFound(name)) => {
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
        let cwd = root();
        let result = spawn_command(&argv, &cwd).await;
        assert!(matches!(result, Err(ExecError::InvalidCommand(_))));
    }

    // ---- execute_all / execute_group / run_one (Plan 05-03) ----

    use run_staged_config::types::{Config, ConfigSource, ExecutionOrder, Group};
    use run_staged_config::types::{MatchedGroup, MatchedPattern};
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
            read_only: false,
            chunk_size: None,
        }
    }

    /// Plan 04 helper: like `make_group` but pre-sets `timeout = Some(Duration::from_millis(timeout_ms))`.
    fn make_group_with_timeout(name: &str, timeout_ms: u64) -> Group {
        Group {
            name: name.to_string(),
            patterns: HashMap::new(),
            timeout: Some(Duration::from_millis(timeout_ms)),
            execution_order: ExecutionOrder::Parallel,
            continue_on_error: false,
            per_file: false,
            path_format: PathFormat::Relative,
            read_only: false,
            chunk_size: None,
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn parallel_wall_clock_under_sum() {
        let g = make_group("p", ExecutionOrder::Parallel, true, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![
                MatchedPattern {
                    pattern: "*.x".to_string(),
                    commands: vec![ft("--sleep 1000")],
                    files: vec![PathBuf::from("dummy.x")],
                },
                MatchedPattern {
                    pattern: "*.y".to_string(),
                    commands: vec![ft("--sleep 1000")],
                    files: vec![PathBuf::from("dummy.y")],
                },
            ],
        };
        let cfg = make_config(true);
        let cwd = root();
        let start = Instant::now();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        let elapsed = start.elapsed();
        assert_eq!(
            outcomes.len(),
            2,
            "expected 2 outcomes, got {}",
            outcomes.len()
        );
        // Two parallel 1s sleeps → sum is 2s. Parallel wall clock must stay well under
        // it; the 0.8s margin absorbs spawn overhead on a loaded machine.
        assert!(
            elapsed.as_secs_f64() < 1.8,
            "expected parallel wall clock < 1.8s (sum is 2s), got {:?}",
            elapsed
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn sequential_wall_clock_at_least_sum() {
        let g = make_group("s", ExecutionOrder::Sequential, true, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![
                MatchedPattern {
                    pattern: "*.x".to_string(),
                    commands: vec![ft("--sleep 200")],
                    files: vec![PathBuf::from("dummy.x")],
                },
                MatchedPattern {
                    pattern: "*.y".to_string(),
                    commands: vec![ft("--sleep 200")],
                    files: vec![PathBuf::from("dummy.y")],
                },
            ],
        };
        let cfg = make_config(true);
        let cwd = root();
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

    #[tokio::test]
    async fn exit_code_propagates_when_command_fails() {
        let g = make_group("g", ExecutionOrder::Parallel, true, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![
                pattern_with_files("ok", &ft(""), vec!["a.x"]),
                pattern_with_files("bad", &ft("--exit 1"), vec!["b.x"]),
            ],
        };
        let cfg = make_config(true);
        let cwd = root();
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
        assert!(
            had_failure_with_1,
            "expected at least one Failure with exit_code=1"
        );
        assert!(had_success, "expected at least one Success");
    }

    #[tokio::test]
    async fn failure_in_first_group_skips_second_group_when_global_coe_false() {
        let g1 = make_group("g1", ExecutionOrder::Parallel, true, false);
        let g2 = make_group("g2", ExecutionOrder::Parallel, true, false);
        let groups = [&g1, &g2];
        let mgs = vec![
            MatchedGroup {
                group: groups[0],
                patterns: vec![pattern_with_files("p1", &ft("--exit 1"), vec!["a.x"])],
            },
            MatchedGroup {
                group: groups[1],
                patterns: vec![pattern_with_files("p2", &ft(""), vec!["b.x"])],
            },
        ];
        let cfg = make_config(false); // global continue_on_error = false
        let cwd = root();
        let outcomes = super::execute_all(&mgs, &cfg, &cwd).await;
        // Only G1's failure should be present.
        assert_eq!(
            outcomes.len(),
            1,
            "expected only G1 outcomes, got {outcomes:?}"
        );
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

    #[tokio::test]
    async fn failure_in_first_group_runs_second_group_when_global_coe_true() {
        let g1 = make_group("g1", ExecutionOrder::Parallel, true, false);
        let g2 = make_group("g2", ExecutionOrder::Parallel, true, false);
        let mgs = vec![
            MatchedGroup {
                group: &g1,
                patterns: vec![pattern_with_files("p1", &ft("--exit 1"), vec!["a.x"])],
            },
            MatchedGroup {
                group: &g2,
                patterns: vec![pattern_with_files("p2", &ft(""), vec!["b.x"])],
            },
        ];
        let cfg = make_config(true); // global continue_on_error = true
        let cwd = root();
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

    #[tokio::test]
    async fn group_cancel_flag_set_on_sibling_failure_when_group_coe_false() {
        // Group-level continue_on_error = false: when the false sibling fails,
        // remaining sleepers should observe the cancel flag (or finish as Failure).
        let g = make_group("cancellable", ExecutionOrder::Parallel, false, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![
                pattern_with_files("slow1", &ft("--sleep 500"), vec!["a.x"]),
                pattern_with_files("slow2", &ft("--sleep 500"), vec!["b.x"]),
                pattern_with_files("fail", &ft("--exit 1"), vec!["c.x"]),
            ],
        };
        let cfg = make_config(true);
        let cwd = root();
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

    #[tokio::test]
    async fn per_file_spawns_one_process_per_file() {
        let g = make_group(
            "p",
            ExecutionOrder::Parallel,
            true,
            /* per_file */ true,
        );
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![pattern_with_files(
                "*.x",
                &ft(""),
                vec!["a.x", "b.x", "c.x"],
            )],
        };
        let cfg = make_config(true);
        let cwd = root();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        assert_eq!(
            outcomes.len(),
            3,
            "expected 3 outcomes (per-file): {outcomes:?}"
        );
        for o in &outcomes {
            match o {
                CommandOutcome::Success {
                    argv, file_count, ..
                } => {
                    assert_eq!(*file_count, 1, "per-file → file_count=1");
                    // argv = [fixture-tool, "<one-file>"]
                    assert_eq!(argv.len(), 2);
                }
                other => panic!("expected Success, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn batch_spawns_one_process_with_all_files() {
        let g = make_group(
            "b",
            ExecutionOrder::Parallel,
            true,
            /* per_file */ false,
        );
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![pattern_with_files(
                "*.x",
                &ft(""),
                vec!["a.x", "b.x", "c.x"],
            )],
        };
        let cfg = make_config(true);
        let cwd = root();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        assert_eq!(
            outcomes.len(),
            1,
            "expected 1 outcome (batch): {outcomes:?}"
        );
        match &outcomes[0] {
            CommandOutcome::Success {
                argv, file_count, ..
            } => {
                assert_eq!(*file_count, 3);
                // argv = [fixture-tool, "a.x", "b.x", "c.x"]
                assert!(argv.contains(&"a.x".to_string()));
                assert!(argv.contains(&"b.x".to_string()));
                assert!(argv.contains(&"c.x".to_string()));
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn chunk_size_splits_batch_in_order() {
        let mut g = make_group("c", ExecutionOrder::Sequential, true, false);
        g.chunk_size = Some(2);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![pattern_with_files(
                "*.x",
                &ft(""),
                vec!["a.x", "b.x", "c.x"],
            )],
        };
        let outcomes = super::execute_all(&[mg], &make_config(true), &root()).await;
        let files: Vec<Vec<String>> = outcomes
            .iter()
            .map(|o| match o {
                CommandOutcome::Success { argv, .. } => argv[1..].to_vec(),
                other => panic!("expected Success, got {other:?}"),
            })
            .collect();
        assert_eq!(files, vec![vec!["a.x", "b.x"], vec!["c.x"]]);
    }

    #[tokio::test]
    async fn chunk_size_ignored_in_per_file_mode() {
        let mut g = make_group("c", ExecutionOrder::Parallel, true, true);
        g.chunk_size = Some(2);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![pattern_with_files(
                "*.x",
                &ft(""),
                vec!["a.x", "b.x", "c.x"],
            )],
        };
        let outcomes = super::execute_all(&[mg], &make_config(true), &root()).await;
        assert_eq!(outcomes.len(), 3);
    }

    #[tokio::test]
    async fn empty_pattern_skips_spawn() {
        let g = make_group("e", ExecutionOrder::Parallel, true, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![MatchedPattern {
                pattern: "*.x".to_string(),
                commands: vec![ft("")],
                files: vec![],
            }],
        };
        let cfg = make_config(true);
        let cwd = root();
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
        let cwd = root();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        assert_eq!(outcomes.len(), 1);
        match &outcomes[0] {
            CommandOutcome::Failure {
                exit_code, stderr, ..
            } => {
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

    // ---- Plan 05-04: timeout enforcement (TDD RED stubs) ----

    #[tokio::test]
    async fn timeout_kills_long_running_process() {
        let group = make_group_with_timeout("slow", 150);
        let pattern = MatchedPattern {
            pattern: "*.x".to_string(),
            commands: vec![ft("--sleep 30000")],
            files: vec![PathBuf::from("dummy.x")],
        };
        let matched = MatchedGroup {
            group: &group,
            patterns: vec![pattern],
        };
        let config = make_config(true);
        let started = Instant::now();
        let outcomes = super::execute_all(&[matched], &config, &root()).await;
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_secs(5),
            "timeout did not fire within 5s — actual elapsed={elapsed:?}"
        );
        assert_eq!(
            outcomes.len(),
            1,
            "expected exactly one outcome, got {outcomes:?}"
        );
        match &outcomes[0] {
            CommandOutcome::Failure {
                exit_code, stderr, ..
            } => {
                let s = String::from_utf8_lossy(stderr);
                assert!(
                    s.contains("timed out"),
                    "stderr should mention 'timed out', got {s:?}"
                );
                assert_eq!(*exit_code, -1, "timeout failure must use exit_code -1");
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sigterm_then_sigkill_reaps_child() {
        let group = make_group_with_timeout("trap-term", 150);
        // Process traps SIGTERM and ignores it — only SIGKILL will end it.
        let pattern = MatchedPattern {
            pattern: "*.x".to_string(),
            commands: vec!["sh -c \"trap '' TERM; sleep 30\"".to_string()],
            files: vec![PathBuf::from("dummy.x")],
        };
        let matched = MatchedGroup {
            group: &group,
            patterns: vec![pattern],
        };
        let config = make_config(true);
        let started = Instant::now();
        let outcomes = super::execute_all(&[matched], &config, &root()).await;
        let elapsed = started.elapsed();

        // 150ms timeout + 2s SIGTERM grace + small SIGKILL latency must finish well under 5s.
        assert!(
            elapsed < Duration::from_secs(5),
            "SIGKILL fallback didn't fire within 5s — elapsed={elapsed:?}"
        );
        assert!(
            matches!(&outcomes[0], CommandOutcome::Failure { .. }),
            "expected Failure outcome, got {:?}",
            outcomes[0]
        );
    }

    /// Regression guard for Plan 04: when `Group.timeout == None`, the spawn
    /// path must behave byte-equivalent to Plan 03 — no spurious timeout
    /// errors, no SIGTERM/SIGKILL escalation, ordinary `Success` outcome.
    #[tokio::test]
    async fn no_timeout_passes_through_unchanged() {
        // make_group sets timeout: None.
        let g = make_group("notimeout", ExecutionOrder::Parallel, true, false);
        let mg = MatchedGroup {
            group: &g,
            patterns: vec![pattern_with_files("*.x", &ft(""), vec!["a.x"])],
        };
        let cfg = make_config(true);
        let cwd = root();
        let outcomes = super::execute_all(&[mg], &cfg, &cwd).await;
        assert_eq!(outcomes.len(), 1, "expected 1 outcome, got {outcomes:?}");
        assert!(
            matches!(&outcomes[0], CommandOutcome::Success { .. }),
            "expected Success when timeout is None, got {:?}",
            outcomes[0]
        );
    }
}
