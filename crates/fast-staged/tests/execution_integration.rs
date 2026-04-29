// Plan 05-05 end-to-end execution integration tests.
//
// Integration tests assume git, sh, echo, false on PATH (Unix or WSL/Git Bash
// on Windows). Each test uses a uniquely-suffixed temp dir to allow parallel
// execution without contention; tests do not mutate the test process's CWD —
// they pass `current_dir(&dir)` to `Command` instead.

use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_fast-staged")
}

fn make_real_git_repo(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fast-staged-exec-test-{suffix}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(
        Command::new("git")
            .args(["init"])
            .current_dir(&dir)
            .output()
            .unwrap()
            .status
            .success()
    );
    Command::new("git")
        .args(["config", "user.email", "t@t.com"])
        .current_dir(&dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "T"])
        .current_dir(&dir)
        .output()
        .unwrap();
    dir
}

fn stage_file(repo: &Path, name: &str, content: &str) {
    std::fs::write(repo.join(name), content).unwrap();
    let out = Command::new("git")
        .args(["add", name])
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(out.status.success(), "git add failed: {out:?}");
}

fn write_config(repo: &Path, name: &str, content: &str) {
    std::fs::write(repo.join(name), content).unwrap();
}

fn run_binary(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new(binary())
        .args(args)
        .current_dir(repo)
        .output()
        .expect("failed to run fast-staged binary")
}

fn patch_exists(repo: &Path) -> bool {
    repo.join(".git").join("fast-staged.patch").exists()
}

#[test]
fn end_to_end_success_runs_cleanup() {
    let dir = make_real_git_repo("e2e-success");
    write_config(
        &dir,
        ".fast-staged.toml",
        r#"
            [group1.patterns]
            "*.txt" = ["echo"]
        "#,
    );
    stage_file(&dir, "a.txt", "hello");

    let out = run_binary(&dir, &[]);
    assert!(
        out.status.success(),
        "expected exit 0, got {:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !patch_exists(&dir),
        "cleanup should have removed .git/fast-staged.patch"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_failure_runs_restore_and_cleanup() {
    let dir = make_real_git_repo("e2e-failure");
    write_config(
        &dir,
        ".fast-staged.toml",
        r#"
            [group1.patterns]
            "*.txt" = ["false"]
        "#,
    );
    stage_file(&dir, "a.txt", "hello");

    let out = run_binary(&dir, &[]);
    assert!(
        !out.status.success(),
        "expected non-zero exit on failing command"
    );

    // Plan 05 contract on the failure path (D-30, D-31): EITHER restore()
    // succeeded → cleanup ran → no patch file remains; OR restore() failed →
    // AppError::RestoreFailed surfaced and cleanup was SKIPPED so the patch
    // remains for manual recovery. Both branches are valid Plan 05 behaviour;
    // the binary must exit non-zero in both cases. (Phase 4's restore() does
    // not always succeed on fresh-add patches — that scope was deferred per
    // D-29 — so we accept either branch and assert the invariants instead.)
    let stderr = String::from_utf8_lossy(&out.stderr);
    if patch_exists(&dir) {
        // RestoreFailed branch: error chain must mention restore-failure
        // recovery so the user knows how to recover (D-30 contract).
        assert!(
            stderr.contains("failed to restore staged diff")
                || stderr.contains("git apply --cached"),
            "patch remains → expected RestoreFailed message in stderr, got: {stderr}"
        );
    }
    // else: restore + cleanup both ran cleanly — the happy failure path.

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn end_to_end_unstaged_failure_skips_backup_and_cleanup() {
    // Regression guard for Phase 4 D-12 + Plan 05 D-31: in --unstaged mode
    // we never write a backup patch and never call restore/cleanup, even when
    // commands fail. This is the only failure-path test that can assert
    // exact patch state without depending on Phase 4 restore() succeeding.
    let dir = make_real_git_repo("e2e-unstaged-failure");
    write_config(
        &dir,
        ".fast-staged.toml",
        r#"
            [group1.patterns]
            "*.txt" = ["false"]
        "#,
    );
    // Commit a baseline so unstaged_files() can find a modified-but-not-staged file.
    stage_file(&dir, "a.txt", "baseline\n");
    let out = Command::new("git")
        .args(["commit", "-m", "baseline"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git commit failed: {out:?}");
    // Modify the worktree without staging.
    std::fs::write(dir.join("a.txt"), "modified\n").unwrap();

    let out = run_binary(&dir, &["--unstaged"]);
    assert!(
        !out.status.success(),
        "expected non-zero exit on failing command in --unstaged mode"
    );
    assert!(
        !patch_exists(&dir),
        "no patch file should ever exist in --unstaged mode (Phase 4 D-12)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn allow_empty_clean_repo_exits_zero_silently() {
    let dir = make_real_git_repo("ae-clean");
    // Need a config so config-not-found doesn't bail early.
    write_config(
        &dir,
        ".fast-staged.toml",
        r#"
            [group1.patterns]
            "*.txt" = ["echo"]
        "#,
    );
    // No staged files.

    let out = run_binary(&dir, &["--allow-empty"]);
    assert!(
        out.status.success(),
        "expected exit 0 with --allow-empty on clean repo, got {:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !patch_exists(&dir),
        "no backup should exist on no-op exit (D-23)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn allow_empty_no_pattern_match_exits_zero_silently() {
    let dir = make_real_git_repo("ae-no-match");
    write_config(
        &dir,
        ".fast-staged.toml",
        r#"
            [group1.patterns]
            "*.bar" = ["echo"]
        "#,
    );
    stage_file(&dir, "a.foo", "x");

    let out = run_binary(&dir, &["--allow-empty"]);
    assert!(
        out.status.success(),
        "expected exit 0 with --allow-empty on no-match, got {:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !patch_exists(&dir),
        "patch must NEVER be written on no-match early exit (D-23: match runs before backup)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_allow_empty_clean_repo_exits_nonzero() {
    let dir = make_real_git_repo("noae-clean");
    write_config(
        &dir,
        ".fast-staged.toml",
        r#"
            [group1.patterns]
            "*.txt" = ["echo"]
        "#,
    );

    let out = run_binary(&dir, &[]);
    assert!(
        !out.status.success(),
        "expected non-zero exit without --allow-empty on clean repo (regression guard for Phase 4 NoStagedFiles)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
