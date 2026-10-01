use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use run_staged_testkit::Repo as Fixture;

use crate::{GitError, Repo};

fn open(fx: &Fixture) -> Repo {
    Repo::open(fx.root(), None).expect("open")
}

fn paths(v: &[&str]) -> Vec<PathBuf> {
    v.iter().map(PathBuf::from).collect()
}

// ── open ─────────────────────────────────────────────────────────────────────

#[test]
fn outside_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        Repo::open(dir.path(), None),
        Err(GitError::NotGitRepository)
    ));
}

#[test]
fn opens_from_subdirectory() {
    let fx = Fixture::builder().staged("pkg/web/a.js", "a").build();
    let repo = Repo::open(&fx.root().join("pkg/web"), None).unwrap();
    assert_eq!(repo.work_dir(), fx.root());
}

#[test]
fn index_file_override_is_resolved_against_cwd() {
    let fx = Fixture::builder().staged("a.js", "a").build();
    let repo = Repo::open(fx.root(), Some(PathBuf::from(".git/index.lock"))).unwrap();
    assert_eq!(repo.index_path(), fx.root().join(".git/index.lock"));
    let repo = Repo::open(fx.root(), Some(PathBuf::new())).unwrap();
    assert_eq!(repo.index_path(), fx.git_dir().join("index"));
}

// ── listing ──────────────────────────────────────────────────────────────────

#[test]
fn committed_files_are_not_relinted() {
    let fx = Fixture::builder()
        .committed("src/main.rs", "fn main() {}")
        .committed("src/lib.rs", "")
        .staged("README.md", "hi")
        .build();
    assert_eq!(open(&fx).staged_files().unwrap(), paths(&["README.md"]));
}

#[test]
fn deleted_file_excluded() {
    let fx = Fixture::builder()
        .committed("old.rs", "x")
        .committed("keep.rs", "y")
        .build();
    fx.git(&["rm", "-q", "old.rs"]);
    fx.write("new.rs", "z");
    fx.git(&["add", "new.rs"]);
    assert_eq!(open(&fx).staged_files().unwrap(), paths(&["new.rs"]));
}

#[test]
fn fresh_repository() {
    let fx = Fixture::builder().staged("a.txt", "a").build();
    assert_eq!(open(&fx).staged_files().unwrap(), paths(&["a.txt"]));
}

#[test]
fn modified_and_mode_change_are_staged() {
    let fx = Fixture::builder()
        .committed("a.sh", "echo")
        .committed("b.txt", "b")
        .staged("b.txt", "b2")
        .build();
    fx.git(&["update-index", "--chmod=+x", "a.sh"]);
    let mut files = open(&fx).staged_files().unwrap();
    files.sort();
    assert_eq!(files, paths(&["a.sh", "b.txt"]));
}

#[test]
fn nothing_staged() {
    let fx = Fixture::builder().committed("a", "a").build();
    assert!(matches!(
        open(&fx).staged_files(),
        Err(GitError::NoStagedFiles)
    ));
}

#[test]
fn reads_ignore_a_held_index_lock() {
    let fx = Fixture::builder()
        .committed("a.js", "1")
        .staged("a.js", "2")
        .unstaged("a.js", "3")
        .build();
    let lock = fx.git_dir().join("index.lock");
    std::fs::write(&lock, b"held").unwrap();
    let before = std::fs::metadata(&lock).unwrap().modified().unwrap();
    let repo = open(&fx);
    assert_eq!(repo.staged_files().unwrap(), paths(&["a.js"]));
    assert_eq!(repo.unstaged_files().unwrap(), paths(&["a.js"]));
    assert_eq!(
        repo.partially_staged(&paths(&["a.js"])).unwrap(),
        paths(&["a.js"])
    );
    repo.backup().unwrap();
    assert_eq!(std::fs::read(&lock).unwrap(), b"held");
    assert_eq!(
        std::fs::metadata(&lock).unwrap().modified().unwrap(),
        before
    );
}

#[test]
fn staged_files_read_from_index_file_override() {
    let mut fx = Fixture::builder()
        .committed("a.js", "1")
        .committed("b.js", "1")
        .build();
    fx.enable_commit_a_index();
    // Stage b.js into the alternate index only (as `git commit -a` would).
    fx.write("b.js", "2");
    fx.git(&["add", "b.js"]);
    let repo = Repo::open(fx.root(), Some(fx.index_path())).unwrap();
    assert_eq!(repo.staged_files().unwrap(), paths(&["b.js"]));
    let default = Repo::open(fx.root(), None).unwrap();
    assert!(matches!(
        default.staged_files(),
        Err(GitError::NoStagedFiles)
    ));
}

#[test]
fn unstaged_lists_modified_tracked_files_only() {
    let fx = Fixture::builder()
        .committed("a.rs", "a")
        .committed("b.rs", "b")
        .committed("gone.rs", "g")
        .build();
    fx.write("a.rs", "changed");
    fx.write("untracked.rs", "u");
    std::fs::remove_file(fx.root().join("gone.rs")).unwrap();
    assert_eq!(open(&fx).unstaged_files().unwrap(), paths(&["a.rs"]));
}

// ── hashing ──────────────────────────────────────────────────────────────────

#[test]
fn crlf_with_autocrlf_is_not_a_change() {
    let fx = Fixture::builder()
        .git_config("core.autocrlf", "true")
        .committed("a.txt", "line1\r\nline2\r\n")
        .build();
    // The blob in git is LF; the working tree has CRLF.
    assert_eq!(fx.index_content("a.txt").unwrap(), "line1\nline2\n");
    let repo = open(&fx);
    assert!(
        repo.partially_staged(&paths(&["a.txt"]))
            .unwrap()
            .is_empty()
    );
    assert!(repo.unstaged_files().unwrap().is_empty());
}

#[test]
fn worktree_ids_detect_edits_and_deletions() {
    let fx = Fixture::builder()
        .staged("a.js", "a")
        .staged("b.js", "b")
        .build();
    let repo = open(&fx);
    let files = paths(&["a.js", "b.js"]);
    let before = repo.worktree_ids(&files).unwrap();
    assert!(before.iter().all(Option::is_some));
    fx.write("a.js", "formatted");
    std::fs::remove_file(fx.root().join("b.js")).unwrap();
    let after = repo.worktree_ids(&files).unwrap();
    assert_ne!(before[0], after[0]);
    assert_eq!(after[1], None);
    // Hashing writes no objects: the new content is not in the odb.
    let id = after[0].unwrap().to_string();
    assert!(
        !fx.git_dir()
            .join("objects")
            .join(&id[..2])
            .join(&id[2..])
            .exists()
    );
}

#[test]
fn partially_staged_detection() {
    let fx = Fixture::builder()
        .committed("p.js", "1")
        .staged("p.js", "2")
        .unstaged("p.js", "3")
        .staged("full.js", "x")
        .build();
    assert_eq!(
        open(&fx)
            .partially_staged(&paths(&["p.js", "full.js"]))
            .unwrap(),
        paths(&["p.js"])
    );
}

// ── backup / restore ─────────────────────────────────────────────────────────

#[test]
fn backup_then_restore_undoes_index_changes() {
    let fx = Fixture::builder()
        .committed("a", "a")
        .staged("a", "b")
        .build();
    let repo = open(&fx);
    let before = std::fs::read(fx.git_dir().join("index")).unwrap();
    let backup = repo.backup().unwrap();
    assert_eq!(std::fs::read(&backup).unwrap(), before);
    // Something stages an extra file during the run.
    fx.write("extra", "x");
    fx.git(&["add", "extra"]);
    repo.restore().unwrap();
    assert_eq!(std::fs::read(fx.git_dir().join("index")).unwrap(), before);
    assert!(!backup.exists());
    assert_eq!(fx.staged_paths(), vec!["a"]);
}

#[test]
fn restore_waits_for_a_lock_released_in_time() {
    let fx = Fixture::builder().staged("a", "a").build();
    let repo = open(&fx);
    repo.backup().unwrap();
    let lock = fx.git_dir().join("index.lock");
    std::fs::write(&lock, b"busy").unwrap();
    let releaser = thread::spawn(move || {
        thread::sleep(Duration::from_secs(1));
        std::fs::remove_file(lock).unwrap();
    });
    repo.restore().unwrap();
    releaser.join().unwrap();
    assert!(!fx.git_dir_has("run-staged.index"));
}

#[test]
fn restore_keeps_backup_when_lock_never_released() {
    let fx = Fixture::builder().staged("a", "a").build();
    let repo = open(&fx);
    repo.backup().unwrap();
    std::fs::write(fx.git_dir().join("index.lock"), b"busy").unwrap();
    let start = Instant::now();
    let err = repo.restore().unwrap_err();
    assert!(start.elapsed() >= Duration::from_secs(4), "should wait ~5s");
    let msg = err.to_string();
    assert!(matches!(err, GitError::RestoreFailed { .. }), "{msg}");
    assert!(msg.contains("run-staged.index"), "{msg}");
    assert!(msg.contains("index.lock"), "{msg}");
    assert!(fx.git_dir_has("run-staged.index"));
    assert_eq!(
        std::fs::read(fx.git_dir().join("index.lock")).unwrap(),
        b"busy"
    );
}

#[test]
fn cleanup_is_idempotent() {
    let fx = Fixture::builder().staged("a", "a").build();
    let repo = open(&fx);
    repo.backup().unwrap();
    repo.cleanup().unwrap();
    repo.cleanup().unwrap();
    assert!(!fx.git_dir_has("run-staged.index"));
}

// ── staging ──────────────────────────────────────────────────────────────────

#[test]
fn stage_writes_blobs_and_leaves_status_clean() {
    let fx = Fixture::builder()
        .committed("a.js", "old")
        .committed("gone.js", "g")
        .staged("a.js", "ugly")
        .staged("new.js", "n")
        .build();
    fx.write("a.js", "pretty");
    fx.write("new.js", "n2");
    std::fs::remove_file(fx.root().join("gone.js")).unwrap();
    open(&fx)
        .stage(&paths(&["a.js", "new.js"]), &paths(&["gone.js"]))
        .unwrap();
    assert_eq!(fx.index_content("a.js").as_deref(), Some("pretty"));
    assert_eq!(fx.index_content("new.js").as_deref(), Some("n2"));
    assert_eq!(fx.index_content("gone.js"), None);
    let diff = fx.git(&["diff", "--cached", "--name-status"]);
    assert!(
        diff.contains("M\ta.js") && diff.contains("A\tnew.js") && diff.contains("D\tgone.js"),
        "{diff}"
    );
    let status = fx.git(&["status", "--porcelain"]);
    assert_eq!(
        status, "M  a.js\nD  gone.js\nA  new.js\n",
        "worktree must be clean"
    );
    assert!(!fx.git_dir_has("index.lock"));
}

#[test]
fn stage_new_entry_keeps_index_sorted() {
    let fx = Fixture::builder()
        .staged("b.js", "b")
        .staged("d.js", "d")
        .build();
    fx.write("a.js", "a");
    fx.write("c.js", "c");
    open(&fx).stage(&paths(&["c.js", "a.js"]), &[]).unwrap();
    assert_eq!(fx.git(&["ls-files"]), "a.js\nb.js\nc.js\nd.js\n");
    fx.git(&["fsck", "--no-progress"]);
}

#[test]
fn stage_into_index_file_override() {
    let mut fx = Fixture::builder().committed("a.js", "1").build();
    fx.enable_commit_a_index();
    fx.write("a.js", "2");
    let repo = Repo::open(fx.root(), Some(fx.index_path())).unwrap();
    repo.stage(&paths(&["a.js"]), &[]).unwrap();
    assert_eq!(fx.index_content("a.js").as_deref(), Some("2"));
    assert!(!fx.git_dir().join("index.lock.lock").exists());
    // The default index is untouched.
    let default = std::process::Command::new("git")
        .args(["show", ":a.js"])
        .current_dir(fx.root())
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&default.stdout), "1");
}

#[test]
fn stage_fails_when_lock_is_held() {
    let fx = Fixture::builder().staged("a.js", "a").build();
    fx.write("a.js", "b");
    std::fs::write(fx.git_dir().join("index.lock"), b"busy").unwrap();
    let err = open(&fx).stage(&paths(&["a.js"]), &[]).unwrap_err();
    assert!(matches!(err, GitError::LockBusy { .. }), "{err}");
    assert!(err.to_string().contains("index.lock"));
}
