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
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Write a file into a directory.
fn write_file(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).expect("write file");
}

/// Create a `.git/` directory so config discovery stops here.
fn make_git_root(dir: &Path) {
    fs::create_dir_all(dir.join(".git")).expect("create .git");
}

// ── tests ────────────────────────────────────────────────────────────────────

#[test]
fn test_binary_exits_nonzero_without_config() {
    let dir = make_temp_dir("no-config");
    make_git_root(&dir); // stop discovery walk here

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
    make_git_root(&dir);
    write_file(
        &dir,
        ".fast-staged.toml",
        "[patterns]\n\"*.rs\" = [\"cargo fmt\"]\n",
    );

    let output = Command::new(binary())
        .current_dir(&dir)
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "Binary should exit 0 when valid TOML config found. stderr: {}",
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
    make_git_root(&dir);
    write_file(
        &dir,
        ".fast-staged.json",
        "{\"patterns\": {\"*.js\": [\"eslint\"]}}",
    );

    let output = Command::new(binary())
        .current_dir(&dir)
        .output()
        .expect("run binary");

    assert!(
        output.status.success(),
        "Binary should exit 0 when valid JSON config found. stderr: {}",
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
    make_git_root(&dir);
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
