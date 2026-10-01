//! Spec: cli.

mod common;
use common::*;

#[test]
fn flags_accepted() {
    let repo = Repo::builder()
        .committed("a", "a")
        .file(
            "run-staged.toml",
            "patterns = { \"*\" = [\"fixture-tool\"] }",
        )
        .build();
    let r = run_staged(&repo).args(["--quiet", "--allow-empty"]).run();
    assert_ok(&r);
}

#[test]
fn print_version() {
    let repo = Repo::builder().build();
    let r = run_staged(&repo).args(["--version"]).run();
    assert_ok(&r);
    assert_eq!(
        r.stdout.trim(),
        format!("run-staged {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn print_help() {
    let repo = Repo::builder().build();
    let r = run_staged(&repo).args(["--help"]).run();
    assert_ok(&r);
    for flag in [
        "--config",
        "--quiet",
        "--unstaged",
        "--allow-empty",
        "--install",
    ] {
        assert!(
            r.stdout.contains(flag),
            "missing {flag} in help:\n{}",
            r.stdout
        );
    }
}

#[test]
fn typo_in_flag() {
    let repo = Repo::builder().build();
    let r = run_staged(&repo).args(["--quet"]).run();
    let stderr = assert_fails(&r, 2);
    assert!(stderr.contains("--quiet"), "{stderr}");
}

#[test]
fn conflicting_install_flags() {
    let repo = Repo::builder().build();
    for extra in ["--unstaged", "--allow-empty"] {
        let r = run_staged(&repo).args(["--install", extra]).run();
        assert_fails(&r, 2);
    }
}

#[test]
fn runtime_error_format() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repo::builder().build();
    let r = run_staged(&repo).dir(dir.path()).run();
    let stderr = assert_fails(&r, 1);
    assert!(stderr.starts_with("error:"), "{stderr}");
}

#[test]
fn binary_delegates_to_core() {
    // A core error (missing config) surfaces as `error: <message>` with exit 1.
    let repo = Repo::builder().staged("a.js", "a").build();
    let r = run_staged(&repo).run();
    assert_eq!(
        assert_fails(&r, 1).trim(),
        "error: configuration file not found"
    );
}
