//! Plain-text rendering of run results. Every function returns `String`s; the
//! binary decides where they go, so the future TUI can reuse the same text.

use std::time::Duration;

use run_staged_exec::CommandOutcome;

/// `"✓ {group}/{pattern}: {cmd} ({duration}, {N} file(s))"`.
pub fn format_success_line(
    group: &str,
    pattern: &str,
    command: &str,
    duration: Duration,
    file_count: usize,
) -> String {
    let dur = format_duration(duration);
    format!(
        "✓ {group}/{pattern}: {command} ({dur}, {})",
        plural_files(file_count)
    )
}

/// The bordered failure box for one failed or timed-out invocation.
pub fn format_error_box(command: &str, exit_code: i32, stdout: &[u8], stderr: &[u8]) -> String {
    let width = 60;
    let border = "─".repeat(width);
    let mut out = String::new();
    out.push_str(&format!("┌{border}┐\n"));
    out.push_str(&format!("│ FAILED: {command}\n"));
    out.push_str(&format!("│ Exit code: {exit_code}\n"));

    let stderr_str = String::from_utf8_lossy(stderr);
    if !stderr_str.trim().is_empty() {
        out.push_str(&format!("├{border}┤\n"));
        for line in stderr_str.lines().take(40) {
            out.push_str(&format!("│ {line}\n"));
        }
    }

    let stdout_str = String::from_utf8_lossy(stdout);
    if !stdout_str.trim().is_empty() {
        out.push_str(&format!("├{border}┤\n"));
        out.push_str("│ stdout:\n");
        for line in stdout_str.lines().take(20) {
            out.push_str(&format!("│ {line}\n"));
        }
    }

    out.push_str(&format!("└{border}┘"));
    out
}

/// `"✗ {group}/{pattern}: {cmd} (cancelled by sibling failure)"`.
pub fn format_cancelled_line(group: &str, pattern: &str, command: &str) -> String {
    format!("✗ {group}/{pattern}: {command} (cancelled by sibling failure)")
}

/// Text to print for a set of outcomes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Rendered {
    /// Success lines (empty in quiet mode).
    pub stdout: Vec<String>,
    /// Failure boxes and cancelled lines (always present).
    pub stderr: Vec<String>,
    /// Failed, timed-out, and cancelled invocations.
    pub failures: usize,
}

/// Render all outcomes. Success lines are dropped when `quiet`; failures never are.
pub fn render_results(outcomes: &[CommandOutcome], quiet: bool) -> Rendered {
    let mut r = Rendered::default();
    for outcome in outcomes {
        match outcome {
            CommandOutcome::Success {
                group,
                pattern,
                command,
                duration,
                file_count,
                ..
            } => {
                if !quiet {
                    r.stdout.push(format_success_line(
                        group,
                        pattern,
                        command,
                        *duration,
                        *file_count,
                    ));
                }
            }
            CommandOutcome::Failure {
                command,
                exit_code,
                stdout,
                stderr,
                ..
            } => {
                r.failures += 1;
                r.stderr
                    .push(format_error_box(command, *exit_code, stdout, stderr));
            }
            CommandOutcome::Cancelled {
                group,
                pattern,
                command,
            } => {
                r.failures += 1;
                r.stderr
                    .push(format_cancelled_line(group, pattern, command));
            }
        }
    }
    r
}

/// Where a run step is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Started,
    Finished,
    Failed,
}

/// A step line. `what` is lower-case, e.g. `"backing up original state"`:
/// `⋯ Backing up original state…`, `✔ Done backing up original state!`,
/// `✗ Failed backing up original state`.
pub fn format_step(state: StepState, what: &str) -> String {
    match state {
        StepState::Started => {
            let mut chars = what.chars();
            let capitalized: String = chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
                .unwrap_or_default();
            format!("⋯ {capitalized}…")
        }
        StepState::Finished => format!("✔ Done {what}!"),
        StepState::Failed => format!("✗ Failed {what}"),
    }
}

/// The closing line: `"<N> file|files, <duration>"`.
pub fn format_summary(file_count: usize, elapsed: Duration) -> String {
    format!("{}, {}", plural_files(file_count), format_duration(elapsed))
}

/// Warning for a partially staged file that a command modified.
pub fn format_partial_warning(path: &std::path::Path) -> String {
    format!(
        "⚠ {} has unstaged changes, so task edits to it were not staged",
        path.display()
    )
}

/// `"NNNms"` under one second, `"X.XXs"` otherwise.
pub fn format_duration(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

fn plural_files(n: usize) -> String {
    format!("{n} {}", if n == 1 { "file" } else { "files" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn success(file_count: usize) -> CommandOutcome {
        CommandOutcome::Success {
            group: "g".into(),
            pattern: "*.rs".into(),
            command: "echo".into(),
            argv: vec!["echo".into()],
            file_count,
            duration: Duration::from_millis(10),
            stdout: Vec::new(),
        }
    }

    fn failure() -> CommandOutcome {
        CommandOutcome::Failure {
            group: "g".into(),
            pattern: "*.rs".into(),
            command: "false".into(),
            argv: vec!["false".into()],
            file_count: 1,
            duration: Duration::from_millis(5),
            exit_code: 1,
            stdout: Vec::new(),
            stderr: b"err".to_vec(),
        }
    }

    #[test]
    fn success_line_format() {
        let s = format_success_line("tools", "*.rs", "cargo fmt", Duration::from_millis(120), 3);
        assert_eq!(s, "✓ tools/*.rs: cargo fmt (120ms, 3 files)");
    }

    #[test]
    fn success_line_singular_vs_plural() {
        let single = format_success_line("g", "p", "c", Duration::from_millis(1), 1);
        let multi = format_success_line("g", "p", "c", Duration::from_millis(1), 5);
        assert!(single.ends_with("(1ms, 1 file)"), "{single}");
        assert!(multi.ends_with("(1ms, 5 files)"), "{multi}");
    }

    #[test]
    fn success_line_seconds_for_long_duration() {
        let s = format_success_line(
            "default",
            "*.rs",
            "cargo fmt",
            Duration::from_millis(1500),
            2,
        );
        assert_eq!(s, "✓ default/*.rs: cargo fmt (1.50s, 2 files)");
    }

    #[test]
    fn error_box_includes_command_and_exit_code() {
        let out = format_error_box("eslint", 1, b"", b"oops");
        assert!(out.contains("FAILED: eslint"), "{out}");
        assert!(out.contains("Exit code: 1"), "{out}");
        assert!(out.contains("oops"), "{out}");
    }

    #[test]
    fn error_box_handles_empty_output() {
        let out = format_error_box("foo", 2, b"", b"");
        assert!(out.contains("Exit code: 2"));
        assert!(out.starts_with('┌'));
        assert!(out.ends_with('┘'));
        assert!(
            !out.contains('├'),
            "no sections when both streams are empty"
        );
    }

    #[test]
    fn error_box_includes_stdout_when_nonempty() {
        let out = format_error_box("rustc", 101, b"compiling foo\nlinking", b"E0001");
        assert!(out.contains("stdout:"));
        assert!(out.contains("compiling foo"));
        assert!(out.contains("E0001"));
    }

    #[test]
    fn error_box_skips_stdout_when_empty() {
        let out = format_error_box("foo", 1, b"", b"some stderr");
        assert!(!out.contains("stdout:"));
    }

    #[test]
    fn error_box_truncates_long_streams() {
        let stderr: String = (0..50).map(|i| format!("e{i}\n")).collect();
        let stdout: String = (0..30).map(|i| format!("o{i}\n")).collect();
        let out = format_error_box("x", 1, stdout.as_bytes(), stderr.as_bytes());
        assert!(out.contains("│ e39\n") && !out.contains("│ e40\n"));
        assert!(out.contains("│ o19\n") && !out.contains("│ o20\n"));
    }

    #[test]
    fn render_collects_failures_and_passes() {
        let outcomes = vec![success(1), success(2), failure()];
        let loud = render_results(&outcomes, false);
        assert_eq!(loud.failures, 1);
        assert_eq!(loud.stdout.len(), 2);
        assert_eq!(loud.stderr.len(), 1);
        let quiet = render_results(&outcomes, true);
        assert_eq!(quiet.failures, 1);
        assert!(quiet.stdout.is_empty(), "quiet drops success lines");
        assert_eq!(quiet.stderr, loud.stderr, "quiet keeps failure boxes");
    }

    #[test]
    fn step_lines() {
        let what = "backing up original state";
        assert_eq!(
            format_step(StepState::Started, what),
            "⋯ Backing up original state…"
        );
        assert_eq!(
            format_step(StepState::Finished, what),
            "✔ Done backing up original state!"
        );
        assert_eq!(
            format_step(StepState::Failed, what),
            "✗ Failed backing up original state"
        );
    }

    #[test]
    fn summary_line() {
        assert_eq!(
            format_summary(3, Duration::from_millis(1500)),
            "3 files, 1.50s"
        );
        assert_eq!(format_summary(1, Duration::from_millis(12)), "1 file, 12ms");
    }

    #[test]
    fn render_counts_cancelled_as_failure() {
        let outcomes = vec![CommandOutcome::Cancelled {
            group: "g".into(),
            pattern: "*.rs".into(),
            command: "skipped".into(),
        }];
        let r = render_results(&outcomes, true);
        assert_eq!(r.failures, 1);
        assert_eq!(
            r.stderr,
            vec!["✗ g/*.rs: skipped (cancelled by sibling failure)"]
        );
    }
}
