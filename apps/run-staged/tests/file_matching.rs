//! Spec: file-matching.

mod common;
use common::*;

fn repo_with(pattern: &str, staged: &[&str]) -> Repo {
    let mut b = Repo::builder().file(
        "run-staged.toml",
        format!("[g]\npatterns = {{ \"{pattern}\" = [\"fixture-tool\"] }}\n"),
    );
    for f in staged {
        b = b.staged(f, "x");
    }
    b.build()
}

fn matched(repo: &Repo) -> Vec<String> {
    let mut v: Vec<String> = calls(repo).into_iter().flatten().collect();
    v.sort();
    v
}

#[test]
fn slashless_pattern_matches_nested_file() {
    let repo = repo_with("*.ts", &["src/foo/bar.ts"]);
    assert_ok(&run_staged(&repo).run());
    assert_eq!(matched(&repo), vec!["src/foo/bar.ts"]);
}

#[test]
fn slash_pattern_anchored_at_root() {
    let repo = repo_with("src/*.ts", &["nested/src/bar.ts", "src/top.ts"]);
    assert_ok(&run_staged(&repo).run());
    assert_eq!(matched(&repo), vec!["src/top.ts"]);
}

#[test]
fn alternation() {
    let repo = repo_with("*.{js,ts}", &["a.js", "b.ts", "c.md"]);
    assert_ok(&run_staged(&repo).run());
    assert_eq!(matched(&repo), vec!["a.js", "b.ts"]);
}

#[test]
fn case_sensitive() {
    let repo = repo_with("*.ts", &["FOO.TS", "ok.ts"]);
    assert_ok(&run_staged(&repo).run());
    assert_eq!(matched(&repo), vec!["ok.ts"]);
}

#[test]
fn negation_only_key() {
    let repo = repo_with("!*.md", &["a.md"]);
    let stderr = assert_fails(&run_staged(&repo).run(), 1).to_string();
    assert!(stderr.contains("invalid configuration"), "{stderr}");
    assert!(stderr.contains("!*.md") && stderr.contains('g'), "{stderr}");
}

#[test]
fn only_non_matching_files_staged() {
    let repo = repo_with("*.rs", &["README.md"]);
    let stderr = assert_fails(&run_staged(&repo).run(), 1).to_string();
    assert!(stderr.contains("no staged files matched"), "{stderr}");
    assert!(calls(&repo).is_empty());
}

#[test]
fn no_match_with_allow_empty() {
    let repo = repo_with("*.rs", &["README.md"]);
    let r = run_staged(&repo).args(["--allow-empty"]).run();
    assert_ok(&r);
    assert_eq!(r.stdout, "");
    assert_eq!(r.stderr, "");
    assert!(calls(&repo).is_empty());
    assert!(!repo.git_dir_has("run-staged.index"));
}
