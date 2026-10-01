//! Spec: hook-install, plus real `git commit` runs through the installed hook.

use std::path::Path;

mod common;
use common::*;

const MARKER: &str = "# run-staged";

fn install(repo: &Repo) -> RunResult {
    run_staged(repo).args(["--install"]).run()
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).unwrap().permissions().mode() & 0o111 != 0
}

#[test]
fn fresh_install() {
    let repo = Repo::builder().build();
    let r = install(&repo);
    assert_ok(&r);
    let hook = repo.git_dir().join("hooks/pre-commit");
    assert!(r.stdout.contains("pre-commit"), "{}", r.stdout);
    assert!(std::fs::read_to_string(&hook).unwrap().contains(MARKER));
    #[cfg(unix)]
    assert!(is_executable(&hook));
}

#[test]
fn custom_hooks_path() {
    let repo = Repo::builder()
        .git_config("core.hooksPath", ".githooks")
        .build();
    assert_ok(&install(&repo));
    assert!(repo.root().join(".githooks/pre-commit").is_file());
    assert!(!repo.git_dir().join("hooks/pre-commit").exists());
}

#[test]
fn reinstall_is_idempotent() {
    let repo = Repo::builder().build();
    assert_ok(&install(&repo));
    let hook = repo.git_dir().join("hooks/pre-commit");
    let first = std::fs::read(&hook).unwrap();
    assert_ok(&install(&repo));
    assert_eq!(std::fs::read(&hook).unwrap(), first);
}

#[test]
fn foreign_hook_kept() {
    let repo = Repo::builder()
        .file(".git/hooks/pre-commit", "#!/bin/sh\nnpx lint-staged\n")
        .build();
    let r = install(&repo);
    let stderr = assert_fails(&r, 1);
    assert!(
        stderr.contains("pre-commit hook already exists at"),
        "{stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(repo.git_dir().join("hooks/pre-commit")).unwrap(),
        "#!/bin/sh\nnpx lint-staged\n"
    );
}

#[test]
fn install_outside_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repo::builder().build();
    let r = run_staged(&repo).dir(dir.path()).args(["--install"]).run();
    assert_eq!(assert_fails(&r, 1).trim(), "error: not a git repository");
}

// ── through git itself (hooks need a POSIX sh) ───────────────────────────────

#[cfg(unix)]
fn git_with_run_staged(repo: &Repo, args: &[&str]) -> RunResult {
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_run-staged"))
        .parent()
        .unwrap();
    repo.runner()
        .program("git")
        .args(args.iter().copied())
        .path_prepend(bin_dir)
        .run()
}

#[cfg(unix)]
#[test]
fn hook_runs_run_staged_on_commit() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.toml",
            "[lint]\npatterns = { \"*.js\" = [\"fixture-tool --stderr nope --exit 1\"] }\n",
        )
        .build();
    assert_ok(&install(&repo));
    let r = git_with_run_staged(&repo, &["commit", "-q", "-m", "try"]);
    assert_ne!(r.code, Some(0), "commit must be aborted:\n{}", r.stderr);
    assert!(r.stderr.contains("nope"), "{}", r.stderr);
    assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]).trim(), "1");
}

#[cfg(unix)]
#[test]
fn commit_includes_formatter_edits() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .staged("a.js", "new")
        .file(
            "run-staged.toml",
            "[fmt]\npatterns = { \"*.js\" = [\"fixture-tool --append ' // fmt'\"] }\n",
        )
        .build();
    assert_ok(&install(&repo));
    let r = git_with_run_staged(&repo, &["commit", "-q", "-m", "fmt"]);
    assert_ok(&r);
    assert_eq!(repo.git(&["show", "HEAD:a.js"]), "new // fmt");
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=no"]),
        ""
    );
}

#[cfg(unix)]
#[test]
fn commit_all_includes_formatter_edits() {
    let repo = Repo::builder()
        .committed("a.js", "old")
        .file(
            "run-staged.toml",
            "[fmt]\npatterns = { \"*.js\" = [\"fixture-tool --append ' // fmt'\"] }\n",
        )
        .build();
    assert_ok(&install(&repo));
    // Changed but not staged: `git commit -a` stages it into .git/index.lock.
    repo.write("a.js", "new");
    let r = git_with_run_staged(&repo, &["commit", "-a", "-q", "-m", "fmt"]);
    assert_ok(&r);
    assert_eq!(repo.git(&["show", "HEAD:a.js"]), "new // fmt");
    assert_eq!(
        repo.git(&["status", "--porcelain", "--untracked-files=no"]),
        ""
    );
}
