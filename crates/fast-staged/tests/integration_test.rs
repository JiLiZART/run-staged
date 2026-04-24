use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Path to the compiled binary (set by cargo test harness).
fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_fast-staged")
}

/// Create a temporary directory with a unique suffix.
fn make_temp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fast-staged-inttest-{suffix}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Write a file into a directory.
fn write_file(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).expect("write file");
}

/// Create a real git repository in `dir` so `gix::discover()` succeeds.
///
/// This replaces the old `make_git_root()` stub (which only created a `.git/`
/// directory) now that `run()` calls `git::GitContext::open()` and
/// `staged_files()` which require a real index file.
fn make_real_git_repo(dir: &Path) {
    let out = Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("git init");
    assert!(out.status.success(), "git init failed: {:?}", out);

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .expect("git config email");
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .expect("git config name");
}

/// Stage a file so `staged_files()` returns at least one entry.
fn stage_file(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).expect("write file for staging");
    let out = Command::new("git")
        .args(["add", name])
        .current_dir(dir)
        .output()
        .expect("git add");
    assert!(out.status.success(), "git add failed: {:?}", out);
}

// ── tests ────────────────────────────────────────────────────────────────────

#[test]
fn test_binary_exits_nonzero_without_config() {
    let dir = make_temp_dir("no-config");
    // Real git repo — gix::discover() succeeds; config error fires first.
    make_real_git_repo(&dir);
    // Stage a dummy file so staged_files() would succeed if reached.
    stage_file(&dir, "dummy.txt", "hello");

    let output = Command::new(binary())
        .current_dir(&dir)
        .output()
        .expect("run binary");

    assert!(
        !output.status.success(),
        "Binary should exit non-zero when no config found"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("configuration file not found"),
        "Expected 'configuration file not found' in stderr, got: {stderr}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_binary_loads_toml_config() {
    let dir = make_temp_dir("toml-config");
    make_real_git_repo(&dir);
    write_file(
        &dir,
        ".fast-staged.toml",
        "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
    );
    // Stage a file so staged_files() returns a non-empty list.
    stage_file(&dir, "main.rs", "fn main() {}");

    let output = Command::new(binary())
        .current_dir(&dir)
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "Binary should exit 0 when valid TOML config found and files are staged. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Loaded config from"),
        "Expected 'Loaded config from' in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("1 pattern(s)"),
        "Expected '1 pattern(s)' in stderr, got: {stderr}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_binary_loads_json_config() {
    let dir = make_temp_dir("json-config");
    make_real_git_repo(&dir);
    write_file(
        &dir,
        ".fast-staged.json",
        "{\"patterns\": {\"*.js\": [\"eslint\"]}}",
    );
    // Stage a file so staged_files() succeeds.
    stage_file(&dir, "index.js", "console.log('hi');");

    let output = Command::new(binary())
        .current_dir(&dir)
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "Binary should exit 0 when valid JSON config found and files are staged. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Loaded config from"),
        "Expected 'Loaded config from' in stderr, got: {stderr}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_binary_reports_invalid_config() {
    let dir = make_temp_dir("invalid-config");
    // Real git repo — gix::discover() succeeds; config error fires first.
    make_real_git_repo(&dir);
    write_file(&dir, ".fast-staged.toml", "this is not valid toml [[[");

    let output = Command::new(binary())
        .current_dir(&dir)
        .output()
        .expect("run binary");

    assert!(
        !output.status.success(),
        "Binary should exit non-zero when config is invalid"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid configuration"),
        "Expected 'invalid configuration' in stderr, got: {stderr}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_binary_exits_nonzero_with_no_staged_files() {
    let dir = make_temp_dir("no-staged");
    make_real_git_repo(&dir);
    // Valid config but nothing staged.
    write_file(
        &dir,
        ".fast-staged.toml",
        "[patterns]\n\"*.txt\" = [\"echo\"]\n",
    );

    let output = Command::new(binary())
        .current_dir(&dir)
        .output()
        .expect("run binary");

    assert!(
        !output.status.success(),
        "Binary should exit non-zero when no staged files"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no staged files"),
        "Expected 'no staged files' in stderr, got: {stderr}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_binary_with_unstaged_flag_shows_unstaged_files() {
    let dir = make_temp_dir("unstaged-flag");
    make_real_git_repo(&dir);
    // Need a valid config so we reach git branch.
    write_file(
        &dir,
        ".fast-staged.toml",
        "[patterns]\n\"*.txt\" = [\"echo\"]\n",
    );

    // Commit a file, then modify it without staging.
    stage_file(&dir, "tracked.txt", "original content");
    let out = Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&dir)
        .output()
        .expect("git commit");
    assert!(out.status.success(), "git commit failed: {:?}", out);

    // Modify the file in the worktree but do NOT stage.
    fs::write(dir.join("tracked.txt"), "modified content").expect("write modified");

    let output = Command::new(binary())
        .args(["--unstaged"])
        .current_dir(&dir)
        .output()
        .expect("run binary with --unstaged");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Binary should exit 0 when --unstaged finds modified files. stderr: {stderr}"
    );
    // With --unstaged, the label must reflect unstaged mode, proving the flag
    // was detected and the unstaged_files() branch was taken.
    assert!(
        stderr.contains("Unstaged files"),
        "Expected 'Unstaged files' label in stderr with --unstaged, got: {stderr}"
    );
    assert!(
        stderr.contains("tracked.txt"),
        "Expected 'tracked.txt' in stderr with --unstaged, got: {stderr}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_binary_without_unstaged_flag_shows_staged_files() {
    let dir = make_temp_dir("staged-no-flag");
    make_real_git_repo(&dir);
    write_file(
        &dir,
        ".fast-staged.toml",
        "[patterns]\n\"*.txt\" = [\"echo\"]\n",
    );

    // Stage a file — no commit needed.
    stage_file(&dir, "staged.txt", "content");

    let output = Command::new(binary())
        .current_dir(&dir)
        .output()
        .expect("run binary without --unstaged");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Binary should exit 0 with staged files. stderr: {stderr}"
    );
    assert!(
        stderr.contains("Staged files"),
        "Expected 'Staged files' label in stderr without --unstaged, got: {stderr}"
    );
    assert!(
        stderr.contains("staged.txt"),
        "Expected 'staged.txt' in stderr, got: {stderr}"
    );

    fs::remove_dir_all(&dir).ok();
}
