//! Spec: command-execution — "Color environment for commands".
//! Own test binary: the scenarios set and clear process environment variables,
//! which is only safe with a single test per process.

use std::path::PathBuf;

use run_staged_config::types::{Config, Group, MatchedGroup, MatchedPattern};
use run_staged_exec::{execute_all_with, CommandOutcome, ExecOptions};

fn env_seen(force_color: bool) -> String {
    let tool = run_staged_testkit::fixture_tool_path()
        .to_string_lossy()
        .into_owned();
    let command = format!(
        "{} --print-env FORCE_COLOR --print-env CLICOLOR_FORCE --print-env NO_COLOR",
        shlex::try_quote(&tool).unwrap()
    );
    let group = Group::default();
    let matched = MatchedGroup {
        group: &group,
        patterns: vec![MatchedPattern {
            pattern: "*".into(),
            commands: vec![command],
            files: vec![PathBuf::from("x")],
        }],
    };
    let opts = ExecOptions {
        events: None,
        force_color,
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let outcomes = rt.block_on(execute_all_with(
        &[matched],
        &Config::default(),
        &std::env::temp_dir(),
        &opts,
    ));
    match &outcomes[0] {
        CommandOutcome::Success { stdout, .. } => String::from_utf8_lossy(stdout).into_owned(),
        other => panic!("expected Success, got {other:?}"),
    }
}

fn clear() {
    // SAFETY: single test in this binary; no other thread reads the env.
    unsafe {
        for v in ["FORCE_COLOR", "CLICOLOR_FORCE", "NO_COLOR"] {
            std::env::remove_var(v);
        }
    }
}

#[test]
fn color_environment_scenarios() {
    // Colors forced in a terminal.
    clear();
    let out = env_seen(true);
    assert!(
        out.contains("FORCE_COLOR=1\n") && out.contains("CLICOLOR_FORCE=1\n"),
        "{out}"
    );

    // User setting wins.
    clear();
    unsafe { std::env::set_var("FORCE_COLOR", "0") };
    let out = env_seen(true);
    assert!(out.contains("FORCE_COLOR=0\n"), "{out}");
    assert!(out.contains("CLICOLOR_FORCE=<unset>"), "{out}");

    // Piped run (color not allowed) adds nothing.
    clear();
    let out = env_seen(false);
    assert!(
        out.contains("FORCE_COLOR=<unset>") && out.contains("CLICOLOR_FORCE=<unset>"),
        "{out}"
    );

    // NO_COLOR disables forcing.
    clear();
    unsafe { std::env::set_var("NO_COLOR", "1") };
    let out = env_seen(true);
    assert!(out.contains("NO_COLOR=1\n"), "{out}");
    assert!(
        out.contains("FORCE_COLOR=<unset>") && out.contains("CLICOLOR_FORCE=<unset>"),
        "{out}"
    );
    clear();
}
