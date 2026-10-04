//! Plain-text rendering of run results. Every function returns `String`s; the
//! binary decides where they go, so the future TUI can reuse the same text.

pub mod ansi;

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

/// How a failure box is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BoxStyle {
    /// Color the border and keep the command's own colors (stderr is a terminal).
    pub colored: bool,
    /// The invocation timed out: yellow instead of red.
    pub timed_out: bool,
    /// Total box width in columns, borders included (the terminal width).
    /// Clamped to at least [`MIN_BOX_WIDTH`].
    pub width: usize,
}

/// Box width when the terminal width is unknown.
pub const DEFAULT_BOX_WIDTH: usize = 80;
/// Narrowest box drawn, whatever the terminal says.
pub const MIN_BOX_WIDTH: usize = 20;

const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

/// The full-width bordered failure box for one failed or timed-out invocation.
///
/// Every row is `│ text │`, padded to `style.width`; long lines wrap. Captured
/// output always loses cursor moves, clears, and other control sequences. With
/// `style.colored` the border and `FAILED:` line are red (yellow for timeouts),
/// command colors are kept, and every row's text ends with a reset; otherwise
/// all escape sequences are removed.
pub fn format_error_box(
    command: &str,
    exit_code: i32,
    stdout: &[u8],
    stderr: &[u8],
    style: BoxStyle,
) -> String {
    let (paint, reset) = match (style.colored, style.timed_out) {
        (false, _) => ("", ""),
        (true, false) => (RED, RESET),
        (true, true) => (YELLOW, RESET),
    };
    let sgr = if style.colored {
        ansi::Sgr::Keep
    } else {
        ansi::Sgr::Strip
    };
    let width = style.width.max(MIN_BOX_WIDTH);
    let border = "─".repeat(width - 2);
    let edge = format!("{paint}│{reset}");
    // `prefix` is an SGR to open the text with (the header's paint).
    let rows = |prefix: &str, line: &str| {
        let clean = ansi::sanitize(&line.replace('\t', "    "), sgr);
        let mut s = String::new();
        for (text, cols) in wrap(&format!("{prefix}{clean}"), width - 4) {
            let pad = " ".repeat(width - 4 - cols);
            let end = if style.colored { RESET } else { "" };
            s.push_str(&format!("{edge} {text}{end}{pad} {edge}\n"));
        }
        s
    };
    let content = |line: &str| rows("", line);

    let mut out = String::new();
    out.push_str(&format!("{paint}┌{border}┐{reset}\n"));
    out.push_str(&rows(paint, &format!("FAILED: {command}")));
    out.push_str(&content(&format!("Exit code: {exit_code}")));

    let stderr_str = String::from_utf8_lossy(stderr);
    if !stderr_str.trim().is_empty() {
        out.push_str(&format!("{paint}├{border}┤{reset}\n"));
        for line in stderr_str.lines().take(40) {
            out.push_str(&content(line));
        }
    }

    let stdout_str = String::from_utf8_lossy(stdout);
    if !stdout_str.trim().is_empty() {
        out.push_str(&format!("{paint}├{border}┤{reset}\n"));
        out.push_str(&content("stdout:"));
        for line in stdout_str.lines().take(20) {
            out.push_str(&content(line));
        }
    }

    out.push_str(&format!("{paint}└{border}┘{reset}"));
    out
}

/// Splits `line` into rows of at most `width` columns, each with its column
/// count. SGR sequences take no room; a row cut mid-color reopens that color.
fn wrap(line: &str, width: usize) -> Vec<(String, usize)> {
    let mut rows = Vec::new();
    let (mut row, mut cols, mut active) = (String::new(), 0, String::new());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // `sanitize` leaves only complete SGR sequences: ESC [ … m.
            let mut seq = String::from(c);
            for c in chars.by_ref() {
                seq.push(c);
                if c == 'm' {
                    break;
                }
            }
            if seq == "\x1b[0m" || seq == "\x1b[m" {
                active.clear();
            } else {
                active.push_str(&seq);
            }
            row.push_str(&seq);
            continue;
        }
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if cols + w > width && cols > 0 {
            rows.push((std::mem::replace(&mut row, active.clone()), cols));
            cols = 0;
        }
        row.push(c);
        cols += w;
    }
    rows.push((row, cols));
    rows
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

/// Render all outcomes. Success lines are dropped when `quiet`; failures never
/// are. `colored` and `width` shape the failure boxes (see [`format_error_box`]).
pub fn render_results(
    outcomes: &[CommandOutcome],
    quiet: bool,
    colored: bool,
    width: usize,
) -> Rendered {
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
                timed_out,
                ..
            } => {
                r.failures += 1;
                let style = BoxStyle {
                    colored,
                    timed_out: *timed_out,
                    width,
                };
                r.stderr
                    .push(format_error_box(command, *exit_code, stdout, stderr, style));
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
            timed_out: false,
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
        let out = format_error_box("eslint", 1, b"", b"oops", BoxStyle::default());
        assert!(out.contains("FAILED: eslint"), "{out}");
        assert!(out.contains("Exit code: 1"), "{out}");
        assert!(out.contains("oops"), "{out}");
    }

    #[test]
    fn error_box_handles_empty_output() {
        let out = format_error_box("foo", 2, b"", b"", BoxStyle::default());
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
        let out = format_error_box(
            "rustc",
            101,
            b"compiling foo\nlinking",
            b"E0001",
            BoxStyle::default(),
        );
        assert!(out.contains("stdout:"));
        assert!(out.contains("compiling foo"));
        assert!(out.contains("E0001"));
    }

    #[test]
    fn error_box_skips_stdout_when_empty() {
        let out = format_error_box("foo", 1, b"", b"some stderr", BoxStyle::default());
        assert!(!out.contains("stdout:"));
    }

    #[test]
    fn error_box_truncates_long_streams() {
        let stderr: String = (0..50).map(|i| format!("e{i}\n")).collect();
        let stdout: String = (0..30).map(|i| format!("o{i}\n")).collect();
        let out = format_error_box(
            "x",
            1,
            stdout.as_bytes(),
            stderr.as_bytes(),
            BoxStyle::default(),
        );
        assert!(out.contains("│ e39 ") && !out.contains("│ e40 "));
        assert!(out.contains("│ o19 ") && !out.contains("│ o20 "));
    }

    #[test]
    fn render_collects_failures_and_passes() {
        let outcomes = vec![success(1), success(2), failure()];
        let loud = render_results(&outcomes, false, false, 80);
        assert_eq!(loud.failures, 1);
        assert_eq!(loud.stdout.len(), 2);
        assert_eq!(loud.stderr.len(), 1);
        let quiet = render_results(&outcomes, true, false, 80);
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
        let r = render_results(&outcomes, true, false, 80);
        assert_eq!(r.failures, 1);
        assert_eq!(
            r.stderr,
            vec!["✗ g/*.rs: skipped (cancelled by sibling failure)"]
        );
    }

    // ── colored failure boxes ────────────────────────────────────────────────

    #[test]
    fn plain_box_is_unchanged_without_escapes() {
        let style = BoxStyle {
            width: 20,
            ..BoxStyle::default()
        };
        let out = format_error_box("lint", 1, b"o1", b"e1\ne2", style);
        let want = "\
┌──────────────────┐
│ FAILED: lint     │
│ Exit code: 1     │
├──────────────────┤
│ e1               │
│ e2               │
├──────────────────┤
│ stdout:          │
│ o1               │
└──────────────────┘";
        assert_eq!(out, want);
    }

    #[test]
    fn plain_box_strips_escapes() {
        let out = format_error_box(
            "x",
            1,
            b"",
            b"\x1b[31merror\x1b[0m\x1b[2K!",
            BoxStyle::default(),
        );
        assert!(out.contains("│ error!  "), "{out:?}");
        assert!(!out.contains('\x1b'), "{out:?}");
    }

    #[test]
    fn colored_box_is_red_and_keeps_command_colors() {
        let style = BoxStyle {
            colored: true,
            timed_out: false,
            width: 40,
        };
        let out = format_error_box("x", 1, b"", b"\x1b[31merror\x1b[0m\x1b[3A", style);
        let first = out.lines().next().unwrap();
        assert!(
            first.starts_with("\x1b[31m┌") && first.ends_with("┐\x1b[0m"),
            "{first:?}"
        );
        assert!(
            out.contains("\x1b[31m│\x1b[0m \x1b[31mFAILED: x\x1b[0m"),
            "{out:?}"
        );
        let content = out.lines().find(|l| l.contains("error")).unwrap();
        assert!(
            content.contains("\x1b[31merror\x1b[0m"),
            "command color kept: {content:?}"
        );
        assert!(
            content.contains("\x1b[0m\x1b[0m "),
            "reset before padding: {content:?}"
        );
        assert!(
            !content.contains("\x1b[3A"),
            "cursor move removed: {content:?}"
        );
    }

    #[test]
    fn timed_out_box_is_yellow() {
        let style = BoxStyle {
            colored: true,
            timed_out: true,
            width: 40,
        };
        let out = format_error_box("x", -1, b"", b"timed out", style);
        assert!(out.starts_with("\x1b[33m┌"), "{out:?}");
        assert!(!out.contains("\x1b[31m"), "{out:?}");
    }

    #[test]
    fn render_passes_color_and_timeout_through() {
        let mut timed = failure();
        if let CommandOutcome::Failure { timed_out, .. } = &mut timed {
            *timed_out = true;
        }
        let r = render_results(&[failure(), timed], false, true, 80);
        assert!(r.stderr[0].starts_with("\x1b[31m┌"));
        assert!(r.stderr[1].starts_with("\x1b[33m┌"));
    }

    // ── full-width layout ────────────────────────────────────────────────────

    #[test]
    fn box_spans_width_and_wraps_long_lines() {
        let style = BoxStyle {
            width: 30,
            ..BoxStyle::default()
        };
        let long = "x".repeat(30) + "日本";
        let out = format_error_box("c", 1, b"", long.as_bytes(), style);
        for line in out.lines() {
            assert_eq!(unicode_width::UnicodeWidthStr::width(line), 30, "{line:?}");
        }
        assert!(out.contains(&format!("│ {} │", "x".repeat(26))), "{out}");
        assert!(out.contains("│ xxxx日本"), "{out}");
    }

    #[test]
    fn wrap_reopens_color_on_next_row() {
        let rows = wrap("\x1b[31mabcd\x1b[0mef", 3);
        let texts: Vec<_> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["\x1b[31mabc", "\x1b[31md\x1b[0mef"]);
        assert!(rows.iter().all(|(_, cols)| *cols == 3));
    }
}
