//! Spec: command-execution.

use std::time::{Duration, Instant};

mod common;
use common::*;

fn repo(config: &str, staged: &[&str]) -> Repo {
    let mut b = Repo::builder().file("run-staged.toml", config);
    for f in staged {
        b = b.staged(f, "x");
    }
    b.build()
}

#[test]
fn quoted_arguments() {
    let repo = repo(
        r#"patterns = { "*.js" = ['fixture-tool "my file"'] }"#,
        &["a.js"],
    );
    assert_ok(&run_staged(&repo).run());
    assert_eq!(calls(&repo), vec![vec!["my file", "a.js"]]);
}

#[test]
fn no_shell_features() {
    let repo = repo(
        r#"patterns = { "*.js" = ["fixture-tool && echo hi"] }"#,
        &["a.js"],
    );
    assert_ok(&run_staged(&repo).run());
    assert_eq!(calls(&repo), vec![vec!["&&", "echo", "hi", "a.js"]]);
}

#[test]
fn files_appended() {
    let repo = repo(
        r#"patterns = { "*.ts" = ["fixture-tool --stdout fix"] }"#,
        &["a.ts"],
    );
    assert_ok(&run_staged(&repo).run());
    assert_eq!(calls(&repo), vec![vec!["--stdout", "fix", "a.ts"]]);
}

#[test]
fn files_placeholder() {
    let repo = repo(
        r#"patterns = { "*.ts" = ["fixture-tool {files} --stdout check"] }"#,
        &["a.ts"],
    );
    assert_ok(&run_staged(&repo).run());
    assert_eq!(calls(&repo), vec![vec!["a.ts", "--stdout", "check"]]);
}

#[test]
fn batch_mode() {
    let repo = repo(
        r#"patterns = { "*.rs" = ["fixture-tool"] }"#,
        &["a.rs", "b.rs"],
    );
    assert_ok(&run_staged(&repo).run());
    assert_eq!(calls(&repo), vec![vec!["a.rs", "b.rs"]]);
}

#[test]
fn per_file_mode() {
    let repo = repo(
        "per_file = true\npatterns = { \"*.rs\" = [\"fixture-tool\"] }",
        &["a.rs", "b.rs"],
    );
    assert_ok(&run_staged(&repo).run());
    let mut c = calls(&repo);
    c.sort();
    assert_eq!(c, vec![vec!["a.rs"], vec!["b.rs"]]);
}

#[test]
fn oversized_file_list() {
    // Enough long paths to exceed 75% of ARG_MAX on Linux (2 MiB), macOS (1 MiB),
    // and Windows (32 KiB command line).
    let dir = "d".repeat(100);
    let names: Vec<String> = (0..9000)
        .map(|i| format!("{dir}/{}-{i:05}.txt", "f".repeat(120)))
        .collect();
    let mut b = Repo::builder().file(
        "run-staged.toml",
        "patterns = { \"*.txt\" = [\"fixture-tool\"] }",
    );
    for n in &names {
        b = b.staged(n, "");
    }
    let repo = b.build();
    let r = run_staged(&repo)
        .args(["--quiet"])
        .timeout(Duration::from_secs(120))
        .run();
    assert_ok(&r);
    let c = calls(&repo);
    assert!(c.len() > 1, "expected several invocations, got {}", c.len());
    let mut seen: Vec<String> = c.into_iter().flatten().collect();
    seen.sort();
    let mut want = names.clone();
    want.sort();
    assert_eq!(seen, want, "every file exactly once");
}

#[test]
fn parallel_group() {
    let repo = repo(
        r#"patterns = { "*.a" = ["fixture-tool --sleep 1000"], "*.b" = ["fixture-tool --sleep 1000"] }"#,
        &["x.a", "x.b"],
    );
    let start = Instant::now();
    assert_ok(&run_staged(&repo).run());
    assert!(
        start.elapsed() < Duration::from_millis(1800),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn groups_do_not_overlap() {
    let repo = Repo::builder()
        .committed("m.txt", "old")
        .staged("m.txt", "")
        .file(
            "run-staged.toml",
            "[a]\npatterns = { \"m.txt\" = [\"fixture-tool --sleep 300 --append a\"] }\n\
             [b]\npatterns = { \"m.txt\" = [\"fixture-tool --append b\"] }\n",
        )
        .build();
    assert_ok(&run_staged(&repo).run());
    assert_eq!(repo.worktree_content("m.txt").as_deref(), Some("ab"));
}

#[test]
fn later_group_skipped() {
    let repo = repo(
        "[a]\npatterns = { \"*\" = [\"fixture-tool --exit 1\"] }\n[b]\npatterns = { \"*\" = [\"fixture-tool --stdout b-ran\"] }\n",
        &["x"],
    );
    assert_fails(&run_staged(&repo).run(), 1);
    assert_eq!(calls(&repo).len(), 1, "group b must not run");
}

#[test]
fn continue_on_error() {
    let repo = repo(
        "continue_on_error = true\n[a]\npatterns = { \"*\" = [\"fixture-tool --exit 1\"] }\n[b]\npatterns = { \"*\" = [\"fixture-tool\"] }\n",
        &["x"],
    );
    assert_fails(&run_staged(&repo).run(), 1);
    assert_eq!(calls(&repo).len(), 2, "group b still runs");
}

#[test]
fn command_exceeds_timeout() {
    let repo = repo(
        r#"timeout = "1s"
patterns = { "*" = ["fixture-tool --sleep 10000"] }"#,
        &["x"],
    );
    let start = Instant::now();
    let r = run_staged(&repo).run();
    let stderr = assert_fails(&r, 1);
    assert!(
        start.elapsed() < Duration::from_secs(6),
        "{:?}",
        start.elapsed()
    );
    assert!(stderr.contains("timed out after"), "{stderr}");
}

#[test]
fn missing_executable() {
    let repo = repo(r#"patterns = { "*" = ["nonexistent-tool"] }"#, &["x"]);
    let stderr = assert_fails(&run_staged(&repo).run(), 1).to_string();
    assert!(stderr.contains("Exit code: 127"), "{stderr}");
    assert!(
        stderr.contains("command not found: nonexistent-tool"),
        "{stderr}"
    );
}

#[test]
fn absolute_paths() {
    let repo = repo(
        "path_format = \"absolute\"\npatterns = { \"*.rs\" = [\"fixture-tool\"] }",
        &["src/a.rs"],
    );
    assert_ok(&run_staged(&repo).run());
    let arg = std::path::PathBuf::from(&calls(&repo)[0][0]);
    assert!(arg.is_absolute(), "{arg:?}");
    assert_eq!(
        std::fs::canonicalize(&arg).unwrap(),
        std::fs::canonicalize(repo.root().join("src/a.rs")).unwrap()
    );
}

#[test]
fn one_of_several_fails() {
    let repo = repo(
        r#"patterns = { "*.a" = ["fixture-tool --exit 2"], "*.b" = ["fixture-tool"] }"#,
        &["x.a", "x.b"],
    );
    let r = run_staged(&repo).run();
    let stderr = assert_fails(&r, 1);
    assert!(stderr.contains("failed with exit code 2"), "{stderr}");
}

#[test]
fn configured_chunk_size() {
    let repo = repo(
        "chunk_size = 2\nexecution_order = \"sequential\"\npatterns = { \"*.js\" = [\"fixture-tool\"] }",
        &["a.js", "b.js", "c.js"],
    );
    assert_ok(&run_staged(&repo).run());
    assert_eq!(calls(&repo), vec![vec!["a.js", "b.js"], vec!["c.js"]]);
}

#[test]
fn zero_timeout_means_no_limit() {
    let repo = repo(
        "timeout = \"0sec\"\n[g]\npatterns = { \"*\" = [\"fixture-tool --sleep 1000\"] }",
        &["x"],
    );
    assert_ok(&run_staged(&repo).run());
}
