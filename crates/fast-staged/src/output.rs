//! Pre-TUI output rendering. Phase 8 will replace this with a ratatui live view;
//! the capture-then-print model stays. `print_error_box` is intentionally
//! isolated so that Phase 8's swap is mechanical.
//!
//! Decisions D-09 (capture stdout/stderr), D-10 (success line shape), D-11
//! (bordered failure box), D-12 (quiet semantics — failures always print).

use std::time::Duration;

use crate::commands::CommandOutcome;

/// Format a success line. Pure function — testable without IO.
/// Shape per D-10: `"✓ {group}/{pattern}: {cmd} ({duration}, {N} file(s))"`.
pub fn format_success_line(
    group: &str,
    pattern: &str,
    command: &str,
    duration: Duration,
    file_count: usize,
) -> String {
    let dur = format_duration(duration);
    let suffix = if file_count == 1 { "file" } else { "files" };
    format!("✓ {group}/{pattern}: {command} ({dur}, {file_count} {suffix})")
}

/// Print a success line to stdout unless `quiet` is set (D-12).
pub fn print_success_line(
    group: &str,
    pattern: &str,
    command: &str,
    duration: Duration,
    file_count: usize,
    quiet: bool,
) {
    if quiet {
        return;
    }
    println!(
        "{}",
        format_success_line(group, pattern, command, duration, file_count)
    );
}

/// Format the bordered failure box for one Failure outcome. Pure — testable.
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

/// Print the bordered failure box to stderr (D-11, D-12 — failures always print).
pub fn print_error_box(command: &str, exit_code: i32, stdout: &[u8], stderr: &[u8]) {
    eprintln!("{}", format_error_box(command, exit_code, stdout, stderr));
}

/// Walk all outcomes; emit success summaries (suppressed by quiet) and failure
/// boxes (always printed). Returns the count of failures so callers can branch.
pub fn print_results(outcomes: &[CommandOutcome], quiet: bool) -> usize {
    let mut failures = 0usize;
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
                print_success_line(group, pattern, command, *duration, *file_count, quiet);
            }
            CommandOutcome::Failure {
                command,
                exit_code,
                stdout,
                stderr,
                ..
            } => {
                failures += 1;
                print_error_box(command, *exit_code, stdout, stderr);
            }
            CommandOutcome::Cancelled {
                group,
                pattern,
                command,
            } => {
                failures += 1;
                eprintln!("✗ {group}/{pattern}: {command} (cancelled by sibling failure)");
            }
        }
    }
    failures
}

/// Render a [`Duration`] as either `"NNNms"` (sub-second) or `"X.XXs"`.
fn format_duration(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // format_success_line / print_success_line
    // -------------------------------------------------------------------------

    #[test]
    fn print_success_line_format() {
        let s =
            format_success_line("tools", "*.rs", "cargo fmt", Duration::from_millis(120), 3);
        // Whitespace-tolerant — assert each fragment in the output.
        assert!(s.contains("✓"), "expected ✓ glyph in {s}");
        assert!(s.contains("tools/*.rs"), "expected group/pattern in {s}");
        assert!(s.contains("cargo fmt"), "expected command in {s}");
        assert!(s.contains("120ms"), "expected duration in {s}");
        assert!(s.contains("3 file"), "expected file count in {s}");
    }

    #[test]
    fn print_success_line_quiet_suppresses_output() {
        // The quiet branch is a side-effect-only path. We assert behaviour by
        // ensuring the helper does not panic and that the *format* helper still
        // returns a non-empty string for the same inputs (so the suppression is
        // happening at the print layer, not the format layer).
        let formatted = format_success_line("g", "p", "c", Duration::from_millis(1), 1);
        assert!(!formatted.is_empty());
        // Calling print_success_line with quiet=true should be a no-op — no
        // panic and no observable side-effect we can capture here. The
        // print_results test below covers the fuller behaviour.
        print_success_line("g", "p", "c", Duration::from_millis(1), 1, true);
    }

    #[test]
    fn print_success_line_singular_vs_plural() {
        let single = format_success_line("g", "p", "c", Duration::from_millis(1), 1);
        let multi = format_success_line("g", "p", "c", Duration::from_millis(1), 5);
        assert!(single.contains("1 file"), "expected '1 file' in {single}");
        assert!(!single.contains("1 files"), "expected singular in {single}");
        assert!(multi.contains("5 files"), "expected plural in {multi}");
    }

    #[test]
    fn print_success_line_seconds_for_long_duration() {
        let s = format_success_line("g", "p", "c", Duration::from_millis(1500), 2);
        assert!(s.contains("1.50s"), "expected 1.50s in {s}");
    }

    // -------------------------------------------------------------------------
    // format_error_box / print_error_box
    // -------------------------------------------------------------------------

    #[test]
    fn print_error_box_includes_command_and_exit_code() {
        let out = format_error_box("eslint", 1, b"", b"oops");
        assert!(out.contains("FAILED"), "expected FAILED in {out}");
        assert!(out.contains("eslint"), "expected command in {out}");
        assert!(out.contains("Exit code: 1"), "expected exit-code in {out}");
        assert!(out.contains("oops"), "expected stderr content in {out}");
    }

    #[test]
    fn print_error_box_handles_empty_stderr_without_panic() {
        let out = format_error_box("foo", 2, b"", b"");
        assert!(out.contains("FAILED"), "expected FAILED in {out}");
        assert!(out.contains("Exit code: 2"), "expected exit-code in {out}");
        // No panic above is the main behaviour. The output should still render
        // top + middle + bottom borders.
        assert!(out.contains("┌"), "expected top border");
        assert!(out.contains("└"), "expected bottom border");
    }

    #[test]
    fn print_error_box_includes_stdout_when_nonempty() {
        let out = format_error_box("rustc", 101, b"compiling foo\nlinking", b"E0001");
        assert!(out.contains("stdout:"), "expected stdout: header in {out}");
        assert!(out.contains("compiling foo"), "expected stdout content");
        assert!(out.contains("E0001"), "expected stderr content");
    }

    #[test]
    fn print_error_box_skips_stdout_when_empty() {
        let out = format_error_box("foo", 1, b"", b"some stderr");
        assert!(!out.contains("stdout:"), "expected NO stdout: header");
    }

    // -------------------------------------------------------------------------
    // print_results
    // -------------------------------------------------------------------------

    #[test]
    fn print_results_collects_failures_and_passes() {
        let outcomes = vec![
            CommandOutcome::Success {
                group: "g".into(),
                pattern: "*.rs".into(),
                command: "echo".into(),
                argv: vec!["echo".into()],
                file_count: 1,
                duration: Duration::from_millis(10),
                stdout: Vec::new(),
            },
            CommandOutcome::Success {
                group: "g".into(),
                pattern: "*.rs".into(),
                command: "echo".into(),
                argv: vec!["echo".into()],
                file_count: 1,
                duration: Duration::from_millis(10),
                stdout: Vec::new(),
            },
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
            },
        ];
        let failures_quiet = print_results(&outcomes, true);
        let failures_loud = print_results(&outcomes, false);
        assert_eq!(failures_quiet, 1);
        assert_eq!(failures_loud, 1);
    }

    #[test]
    fn print_results_counts_cancelled_as_failure() {
        let outcomes = vec![CommandOutcome::Cancelled {
            group: "g".into(),
            pattern: "*.rs".into(),
            command: "skipped".into(),
        }];
        let failures = print_results(&outcomes, false);
        assert_eq!(failures, 1, "Cancelled should count as a failure");
    }
}
