//! Shared helpers for the end-to-end suite.
#![allow(dead_code)]

pub use run_staged_testkit::{Repo, RunResult};

/// Arguments of every `fixture-tool` invocation, in completion order, with the
/// tool's own options left in.
pub fn calls(repo: &Repo) -> Vec<Vec<String>> {
    repo.invocations().into_iter().map(|i| i.args).collect()
}

/// Asserts a failed run and returns its stderr.
#[track_caller]
pub fn assert_fails(r: &RunResult, code: i32) -> &str {
    assert_eq!(
        r.code,
        Some(code),
        "stdout:\n{}\nstderr:\n{}",
        r.stdout,
        r.stderr
    );
    &r.stderr
}

/// Asserts a successful run.
#[track_caller]
pub fn assert_ok(r: &RunResult) {
    assert_eq!(
        r.code,
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        r.stdout,
        r.stderr
    );
}

/// A runner for the `run-staged` binary built for this test crate.
pub fn run_staged(repo: &Repo) -> run_staged_testkit::Runner<'_> {
    repo.runner().program(env!("CARGO_BIN_EXE_run-staged"))
}
