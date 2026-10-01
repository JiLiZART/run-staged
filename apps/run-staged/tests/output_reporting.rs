//! Spec: output-reporting.

mod common;
use common::*;

fn repo(config: &str) -> Repo {
    Repo::builder()
        .staged("a.rs", "a")
        .staged("b.rs", "b")
        .file("run-staged.toml", config)
        .build()
}

#[test]
fn success_output() {
    let repo = repo(r#"patterns = { "*.rs" = ["fixture-tool"] }"#);
    let r = run_staged(&repo).run();
    assert_ok(&r);
    let out = run_staged_testkit::normalize(&r.stdout, repo.root());
    assert!(
        out.contains("✓ default/*.rs: fixture-tool (<duration>, 2 files)"),
        "{out}"
    );
}

#[test]
fn failure_box() {
    let repo = repo(r#"patterns = { "*.rs" = ["fixture-tool --stderr 'lint error' --exit 1"] }"#);
    let stderr = assert_fails(&run_staged(&repo).run(), 1).to_string();
    for part in ["FAILED:", "Exit code: 1", "lint error"] {
        assert!(stderr.contains(part), "missing {part}:\n{stderr}");
    }
}

#[test]
fn sibling_cancelled() {
    let repo = repo(
        "[g]\nexecution_order = \"sequential\"\n\
         patterns = { \"*.rs\" = [\"fixture-tool --exit 1\", \"fixture-tool --stdout second\"] }\n",
    );
    let stderr = assert_fails(&run_staged(&repo).run(), 1).to_string();
    assert!(
        stderr.contains("✗ g/*.rs: fixture-tool --stdout second (cancelled by sibling failure)"),
        "{stderr}"
    );
}

#[test]
fn quiet_success() {
    let repo = repo(r#"patterns = { "*.rs" = ["fixture-tool"] }"#);
    let r = run_staged(&repo).args(["--quiet"]).run();
    assert_ok(&r);
    assert_eq!(r.stdout, "");
}

#[test]
fn quiet_failure() {
    let repo = repo(r#"patterns = { "*.rs" = ["fixture-tool --exit 1"] }"#);
    let r = run_staged(&repo).args(["--quiet"]).run();
    let stderr = assert_fails(&r, 1);
    assert!(stderr.contains("FAILED:"), "{stderr}");
    assert_eq!(r.stdout, "");
}

// ── step lines and summary ───────────────────────────────────────────────────

#[test]
fn step_lines_failed_run() {
    let repo = repo(r#"patterns = { "*.rs" = ["fixture-tool --stderr boom --exit 1"] }"#);
    let r = run_staged(&repo).run();
    assert_fails(&r, 1);
    assert!(
        !r.stdout.contains("Staging"),
        "no staging step after a failure"
    );
    insta::assert_snapshot!(r.snapshot(&repo));
}

#[test]
fn step_lines_unstaged_run() {
    let repo = Repo::builder()
        .committed("a.rs", "a")
        .file(
            "run-staged.toml",
            r#"patterns = { "*.rs" = ["fixture-tool"] }"#,
        )
        .build();
    repo.write("a.rs", "changed");
    let r = run_staged(&repo).args(["--unstaged"]).run();
    assert_ok(&r);
    insta::assert_snapshot!(r.snapshot(&repo));
}

#[test]
fn summary_after_success() {
    let repo = Repo::builder()
        .staged("a.rs", "a")
        .staged("b.rs", "b")
        .staged("c.md", "c")
        .file(
            "run-staged.toml",
            r#"patterns = { "*.rs" = ["fixture-tool"], "*.{rs,md}" = ["fixture-tool"] }"#,
        )
        .build();
    let r = run_staged(&repo).run();
    assert_ok(&r);
    let last = run_staged_testkit::normalize(r.stdout.lines().last().unwrap(), repo.root());
    assert_eq!(last, "3 files, <duration>");
}

#[test]
fn summary_after_failure() {
    let repo = repo(r#"patterns = { "*.rs" = ["fixture-tool --exit 1"] }"#);
    let r = run_staged(&repo).run();
    assert_fails(&r, 1);
    let last = run_staged_testkit::normalize(r.stdout.lines().last().unwrap(), repo.root());
    assert_eq!(last, "2 files, <duration>");
}

#[test]
fn no_summary_when_quiet_or_early_exit() {
    let repo = repo(r#"patterns = { "*.rs" = ["fixture-tool"] }"#);
    assert_eq!(run_staged(&repo).args(["--quiet"]).run().stdout, "");
    let empty = Repo::builder()
        .committed("a", "a")
        .file(
            "run-staged.toml",
            r#"patterns = { "*" = ["fixture-tool"] }"#,
        )
        .build();
    assert_eq!(run_staged(&empty).args(["--allow-empty"]).run().stdout, "");
}
