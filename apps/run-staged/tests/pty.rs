//! Spec: terminal-ui — the live panel, run under a real pseudo-terminal.
//!
//! The harness answers cursor-position queries (`ESC[6n`) the way a terminal
//! emulator would, since ratatui's inline viewport asks for it.

use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

mod common;
use common::*;

struct PtyRun {
    /// Raw bytes as written to the terminal (lossy UTF-8).
    output: String,
    /// What a 30x100 terminal shows at the end (vt100 emulation).
    screen: String,
    code: u32,
}

/// Runs `run-staged args` in `repo` attached to a 30x100 pseudo-terminal.
/// `during` gets the child's pid and the live output (for sending signals).
fn run_in_pty(
    repo: &Repo,
    args: &[&str],
    env: &[(&str, &str)],
    during: impl FnOnce(Option<u32>, &Mutex<Vec<u8>>),
) -> PtyRun {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_run-staged"));
    cmd.cwd(repo.root());
    for (k, v) in repo.isolated_env_vars() {
        cmd.env(k, v);
    }
    cmd.env("TERM", "xterm-256color");
    cmd.env_remove("NO_COLOR");
    for (k, v) in env {
        cmd.env(k, v);
    }
    for a in args {
        cmd.arg(a);
    }
    let mut child = pty.slave.spawn_command(cmd).expect("spawn in pty");
    drop(pty.slave);

    let output = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut reader = pty.master.try_clone_reader().unwrap();
    let mut writer = pty.master.take_writer().unwrap();
    let sink = Arc::clone(&output);
    let pump = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        // Emulate the terminal so each cursor-position query gets the answer a
        // real terminal would give at that moment.
        let mut term = vt100::Parser::new(30, 100, 0);
        const QUERY: &[u8] = b"\x1b[6n";
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            sink.lock().unwrap().extend_from_slice(&buf[..n]);
            let mut rest = &buf[..n];
            while let Some(i) = rest.windows(QUERY.len()).position(|w| w == QUERY) {
                term.process(&rest[..i + QUERY.len()]);
                let (row, col) = term.screen().cursor_position();
                let _ = write!(writer, "\x1b[{};{}R", row + 1, col + 1);
                rest = &rest[i + QUERY.len()..];
            }
            term.process(rest);
        }
    });

    during(child.process_id(), &output);
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "run-staged hung in the pty"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    drop(pty.master);
    let _ = pump.join();
    let bytes = output.lock().unwrap().clone();
    let mut parser = vt100::Parser::new(30, 100, 0);
    parser.process(&bytes);
    PtyRun {
        output: String::from_utf8_lossy(&bytes).into_owned(),
        screen: parser.screen().contents(),
        code: status.exit_code(),
    }
}

fn formatter_repo(command: &str) -> Repo {
    Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.toml",
            format!("[fmt]\npatterns = {{ \"*.js\" = [\"{command}\"] }}\n"),
        )
        .build()
}

const HIDE: &str = "\x1b[?25l";
const SHOW: &str = "\x1b[?25h";

#[test]
fn successful_run_shows_the_panel() {
    let repo = formatter_repo("fixture-tool --sleep 300 --append x");
    let r = run_in_pty(&repo, &[], &[], |_, _| {});
    assert_eq!(r.code, 0, "{}", r.output);
    assert!(
        r.output.contains(HIDE) && r.output.contains(SHOW),
        "cursor hidden then shown"
    );
    let o = &r.screen;
    assert!(o.contains('╭') && o.contains('╯'), "rounded border:\n{o}");
    for step in [
        "Backed up original state",
        "Ran tasks for staged files",
        "Staged changes from tasks",
        "Cleaned up temporary files",
    ] {
        assert!(o.contains(step), "missing {step:?}");
    }
    assert!(o.contains("1/1"), "gauge");
    assert!(
        !o.contains("✓ fmt/*.js"),
        "no plain success line under the panel"
    );
    assert!(
        !o.contains("⋯ Backing up"),
        "no plain step lines under the panel"
    );
    assert!(o.contains("1 file, "), "summary after the panel");
    assert_eq!(repo.index_content("a.js").as_deref(), Some("newx"));
}

#[test]
fn failing_run_shows_red_border_and_colored_box_below() {
    let repo = formatter_repo("fixture-tool --stderr 'lint error' --exit 1");
    let r = run_in_pty(&repo, &[], &[], |_, _| {});
    assert_eq!(r.code, 1, "{}", r.output);
    assert!(r.screen.contains("Tasks failed"), "{}", r.screen);
    let o = &r.output;
    // The failure box starts with a red border after the panel's last frame.
    let box_at = o.rfind("\x1b[31m┌").expect("colored failure box");
    let panel_end = o.rfind('╯').expect("panel");
    assert!(panel_end < box_at, "box printed below the panel");
    assert!(o[box_at..].contains("lint error"));
    assert!(
        o.contains("\x1b[38;5;9m") || o.contains("\x1b[31m"),
        "red in the panel"
    );
}

#[test]
fn quiet_in_a_terminal_prints_nothing() {
    let repo = formatter_repo("fixture-tool");
    let r = run_in_pty(&repo, &["--quiet"], &[], |_, _| {});
    assert_eq!(r.code, 0);
    assert!(!r.output.contains('╭'), "no panel: {:?}", r.output);
    assert_eq!(r.output.trim(), "");
}

#[test]
fn dumb_terminal_uses_plain_output() {
    let repo = formatter_repo("fixture-tool");
    let r = run_in_pty(&repo, &[], &[("TERM", "dumb")], |_, _| {});
    assert_eq!(r.code, 0);
    assert!(!r.output.contains('╭'), "no panel");
    assert!(
        r.output.contains("⋯ Backing up original state…"),
        "{}",
        r.output
    );
    assert!(
        !r.output.contains("\x1b["),
        "no escape sequences: {:?}",
        r.output
    );
}

#[test]
fn no_color_animates_without_colors_and_forces_nothing() {
    let repo = formatter_repo("fixture-tool --sleep 300 --print-env FORCE_COLOR");
    std::fs::write(
        repo.root().join("run-staged.toml"),
        "[fmt]\npatterns = { \"*.js\" = [\"fixture-tool --sleep 300 --print-env FORCE_COLOR --exit 1\"] }\n",
    )
    .unwrap();
    let r = run_in_pty(&repo, &[], &[("NO_COLOR", "1")], |_, _| {});
    assert!(r.output.contains('╭'), "panel still drawn");
    assert!(
        !has_color_sgr(&r.output),
        "no foreground colors: {:?}",
        r.output
    );
    assert!(r.output.contains("FORCE_COLOR=<unset>"), "{}", r.output);
}

#[test]
fn colors_forced_for_commands_in_a_terminal() {
    let repo =
        formatter_repo("fixture-tool --print-env FORCE_COLOR --print-env CLICOLOR_FORCE --exit 1");
    let r = run_in_pty(&repo, &[], &[], |_, _| {});
    assert!(r.output.contains("FORCE_COLOR=1"), "{}", r.output);
    assert!(r.output.contains("CLICOLOR_FORCE=1"), "{}", r.output);
}

#[cfg(unix)]
#[test]
fn ctrl_c_restores_terminal_and_exits_130() {
    let repo = formatter_repo("fixture-tool --sleep 5000");
    let r = run_in_pty(&repo, &[], &[], |pid, out| {
        // Wait until the panel shows the running command, then press Ctrl+C.
        let start = Instant::now();
        while !String::from_utf8_lossy(&out.lock().unwrap()).contains("fixture-tool --sleep") {
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "panel never appeared"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(200));
        unsafe { libc_kill(pid.expect("pid") as i32, 2) };
    });
    assert_eq!(r.code, 130, "{}", r.screen);
    let hide = r.output.find(HIDE).expect("cursor hidden");
    assert!(
        r.output[hide..].contains(SHOW),
        "cursor shown again after Ctrl+C"
    );
}

/// True if `s` contains an SGR sequence that sets a foreground or background color.
fn has_color_sgr(s: &str) -> bool {
    let mut rest = s;
    while let Some(i) = rest.find("\x1b[") {
        rest = &rest[i + 2..];
        let end = rest
            .find(|c: char| c.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        if rest[end..].starts_with('m') {
            let colored = rest[..end].split(';').any(|p| {
                matches!(
                    p.parse::<u16>(),
                    Ok(30..=38 | 40..=48 | 90..=97 | 100..=107)
                )
            });
            if colored {
                return true;
            }
        }
        rest = &rest[end..];
    }
    false
}

#[cfg(unix)]
unsafe extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}
