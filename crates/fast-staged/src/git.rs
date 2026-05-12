use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;
use gix::index::entry::Stage;

use crate::AppError;

/// Wraps a gix repository handle and exposes focused git operations.
#[derive(Debug)]
pub struct GitContext {
    repo: gix::Repository,
}

impl GitContext {
    /// Open the git repository by discovering it upward from the current working
    /// directory.
    ///
    /// Returns `AppError::NotGitRepository` if the CWD is not inside a git repo.
    pub fn open() -> Result<Self, AppError> {
        let repo = gix::discover(".").map_err(|_| AppError::NotGitRepository)?;
        Ok(Self { repo })
    }

    /// Return the path to the `.git` directory.
    pub fn git_dir(&self) -> &Path {
        self.repo.git_dir()
    }

    /// Return the path to the working directory (the repository root, NOT the
    /// `.git` directory). Used by the executor for `PathFormat::Absolute`
    /// canonicalization and by `lib.rs::run()` as `git_root`.
    ///
    /// Panics: if the repository is bare (no working directory). Pre-commit
    /// hooks never run in bare repos, so this is unreachable in normal use;
    /// the panic exposes a misuse rather than masking it with a bogus fallback.
    pub fn work_dir(&self) -> &Path {
        self.repo
            .workdir()
            .expect("GitContext::work_dir called on a bare repository")
    }

    /// Return the list of staged files (index stage-0 entries).
    ///
    /// Uses `open_index()` (fresh read from disk) rather than the lazily-cached
    /// `index()` accessor — required for correctness inside a pre-commit hook.
    ///
    /// Returns `AppError::NoStagedFiles` when no stage-0 entries exist, including
    /// when no index file has been created yet (fresh repo with no `git add`).
    pub fn staged_files(&self) -> Result<Vec<PathBuf>, AppError> {
        // In a fresh repo with no commits and no staged files, the index file may
        // not exist yet. Treat that as "no staged files" rather than a hard error.
        let index = match self.repo.open_index() {
            Ok(idx) => idx,
            Err(_) => return Err(AppError::NoStagedFiles),
        };

        let backing = index.path_backing();

        let files: Vec<PathBuf> = index
            .entries()
            .iter()
            .filter(|e| e.stage() == Stage::Unconflicted)
            .map(|e| {
                let bstr = e.path_in(backing);
                PathBuf::from(bstr.to_str_lossy().as_ref())
            })
            .collect();

        if files.is_empty() {
            return Err(AppError::NoStagedFiles);
        }

        Ok(files)
    }

    /// Save the current staged diff to `.git/fast-staged.patch`.
    ///
    /// The patch captures the HEAD-vs-index diff so the staged state can be
    /// restored if a linter or formatter corrupts the index.  Any existing
    /// patch file is overwritten (D-07).
    ///
    /// Note: gix has no unified-diff generator that covers the HEAD-vs-index
    /// case without significant blob-level assembly (research open question 4).
    /// We use `git diff --cached` as a pragmatic fallback here — the only
    /// documented exception to the "no git CLI" constraint for backup.
    pub fn backup(&self) -> Result<(), AppError> {
        let patch_path = self.repo.git_dir().join("fast-staged.patch");

        // Work-dir is needed so `git` finds the right repo.
        let work_dir = self
            .repo
            .workdir()
            .ok_or_else(|| AppError::Git("bare repositories are not supported".to_string()))?;

        let output = std::process::Command::new("git")
            .args(["diff", "--cached"])
            .current_dir(work_dir)
            .output()
            .map_err(|e| AppError::Git(format!("git diff --cached failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Git(format!("git diff --cached failed: {stderr}")));
        }

        // Write patch — overwrites any existing file (D-07).
        std::fs::write(&patch_path, &output.stdout)?;
        Ok(())
    }

    /// Restore the staged diff from `.git/fast-staged.patch`.
    ///
    /// If no patch file exists (e.g. backup was never called or cleanup already
    /// ran) this is a no-op.  gix has no `git apply` equivalent; we shell out
    /// to `git apply --cached` — the documented exception for restore (research
    /// Pattern 4).
    pub fn restore(&self) -> Result<(), AppError> {
        let patch_path = self.repo.git_dir().join("fast-staged.patch");
        if !patch_path.exists() {
            return Ok(());
        }

        let work_dir = self
            .repo
            .workdir()
            .ok_or_else(|| AppError::Git("bare repositories are not supported".to_string()))?;

        let output = std::process::Command::new("git")
            .args(["apply", "--cached"])
            .arg(&patch_path)
            .current_dir(work_dir)
            .output()
            .map_err(|e| AppError::Git(format!("git apply --cached failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Git(format!(
                "git apply --cached failed: {stderr}"
            )));
        }

        Ok(())
    }

    /// Remove `.git/fast-staged.patch` if it exists.
    ///
    /// Called after a successful run (GIT-04).  Silently succeeds if the patch
    /// file does not exist (e.g. cleanup was already called or backup was
    /// never called).
    pub fn cleanup(&self) -> Result<(), AppError> {
        let patch_path = self.repo.git_dir().join("fast-staged.patch");
        if patch_path.exists() {
            std::fs::remove_file(&patch_path)?;
        }
        Ok(())
    }

    /// Return the list of modified-but-not-staged files (index vs worktree).
    ///
    /// An empty result is valid — no error is returned when nothing is unstaged.
    pub fn unstaged_files(&self) -> Result<Vec<PathBuf>, AppError> {
        use gix::bstr::BString;

        let platform = self
            .repo
            .status(gix::progress::Discard)
            .map_err(|e| AppError::Git(e.to_string()))?;

        let iter = platform
            .untracked_files(gix::status::UntrackedFiles::None)
            .into_iter(None::<BString>)
            .map_err(|e| AppError::Git(e.to_string()))?;

        let mut files: Vec<PathBuf> = Vec::new();
        for item in iter {
            let item = item.map_err(|e| AppError::Git(e.to_string()))?;
            if let gix::status::Item::IndexWorktree(change) = item {
                let path = PathBuf::from(change.rela_path().to_str_lossy().as_ref());
                files.push(path);
            }
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Mutex;

    use super::*;

    // `set_current_dir` is a process-wide mutation. Serialise all tests that
    // change the CWD so they don't interfere with each other when run in
    // parallel by the test harness.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    // -------------------------------------------------------------------------
    // Fixture helpers
    // -------------------------------------------------------------------------

    /// Create a real git repo in a temp dir. `suffix` must be unique per test
    /// to avoid cross-test contamination when running in parallel.
    fn make_real_git_repo(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fast-staged-git-test-{suffix}"));
        // Remove any leftovers from a previous run.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        assert!(
            Command::new("git")
                .args(["init"])
                .current_dir(&dir)
                .output()
                .unwrap()
                .status
                .success(),
            "git init failed in {:?}",
            dir
        );

        // These are required for `git commit` to work.
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&dir)
            .output()
            .unwrap();

        dir
    }

    /// Write `content` to a file named `name` inside `repo` and stage it.
    fn stage_file(repo: &Path, name: &str, content: &str) {
        std::fs::write(repo.join(name), content).unwrap();
        let out = Command::new("git")
            .args(["add", name])
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(out.status.success(), "git add failed: {:?}", out);
    }

    /// Commit all tracked changes.
    fn commit_all(repo: &Path, msg: &str) {
        let out = Command::new("git")
            .args(["commit", "-am", msg])
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(out.status.success(), "git commit failed: {:?}", out);
    }

    // -------------------------------------------------------------------------
    // open() tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_open_succeeds_in_git_repo() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("open-ok");
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = GitContext::open();
        assert!(result.is_ok(), "expected Ok, got Err");

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_open_fails_outside_git_repo() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join("fast-staged-git-test-not-a-repo");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = GitContext::open();
        assert!(
            matches!(result, Err(AppError::NotGitRepository)),
            "expected NotGitRepository, got something else"
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------------------------
    // git_dir() tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_git_dir_ends_with_dot_git() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("git-dir");
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        let git_dir = ctx.git_dir();
        assert!(
            git_dir.ends_with(".git"),
            "expected git_dir to end with .git, got {:?}",
            git_dir
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------------------------
    // staged_files() tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_staged_files_returns_both_staged_files() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("staged-two");
        stage_file(&dir, "a.txt", "hello");
        stage_file(&dir, "b.rs", "fn main() {}");

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        let files = ctx.staged_files().unwrap();

        let names: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        assert_eq!(files.len(), 2, "expected 2 staged files, got {:?}", names);
        assert!(
            names.contains(&"a.txt".to_string()),
            "a.txt missing from {:?}",
            names
        );
        assert!(
            names.contains(&"b.rs".to_string()),
            "b.rs missing from {:?}",
            names
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_staged_files_empty_returns_no_staged_files_error() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("staged-empty");
        // Do NOT stage anything.

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        let result = ctx.staged_files();
        assert!(
            matches!(result, Err(AppError::NoStagedFiles)),
            "expected NoStagedFiles, got something else"
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_staged_files_only_returns_stage_zero_entries() {
        let _lock = CWD_LOCK.lock().unwrap();
        // Stage one file normally — produces a stage-0 (Unconflicted) entry.
        let dir = make_real_git_repo("staged-stage0");
        stage_file(&dir, "only.txt", "content");

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        let files = ctx.staged_files().unwrap();

        assert_eq!(files.len(), 1, "expected exactly 1 file, got {:?}", files);
        assert_eq!(files[0], PathBuf::from("only.txt"));

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_staged_files_ignores_committed_files() {
        let _lock = CWD_LOCK.lock().unwrap();
        // Reproducer for Phase 999.1 dogfood bug: in a repo with prior HEAD
        // commits, staged_files() must return ONLY files whose index entry
        // differs from HEAD — not every tracked file.
        let dir = make_real_git_repo("staged-ignores-committed");

        // Commit a .rs file so it becomes part of HEAD.
        stage_file(&dir, "committed.rs", "fn main() {}\n");
        commit_all(&dir, "initial commit");

        // Now stage a brand-new .md file. The committed .rs file is
        // unchanged in the worktree and unchanged in the index.
        stage_file(&dir, "new.md", "# title\n");

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        let files = ctx.staged_files().unwrap();

        let names: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        assert_eq!(
            files.len(),
            1,
            "expected exactly 1 staged file (new.md), got {:?}",
            names
        );
        assert_eq!(
            files[0],
            PathBuf::from("new.md"),
            "expected new.md, got {:?}",
            names
        );
        assert!(
            !names.contains(&"committed.rs".to_string()),
            "committed.rs must NOT be in staged_files() result (it is unchanged vs HEAD), got {:?}",
            names
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------------------------
    // unstaged_files() tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_unstaged_files_returns_modified_not_staged_file() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("unstaged-modified");
        stage_file(&dir, "tracked.txt", "original content");
        commit_all(&dir, "initial commit");

        // Modify the committed file without staging it.
        std::fs::write(dir.join("tracked.txt"), "modified content").unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        let files = ctx.unstaged_files().unwrap();

        let names: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        assert!(
            names.contains(&"tracked.txt".to_string()),
            "expected tracked.txt in unstaged files, got {:?}",
            names
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------------------------
    // backup() tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backup_creates_patch_file() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("backup-creates");
        // Commit a file first, then modify and re-stage it so there is a diff.
        stage_file(&dir, "hello.txt", "original");
        commit_all(&dir, "initial");
        stage_file(&dir, "hello.txt", "modified");

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        ctx.backup().unwrap();

        let patch_path = dir.join(".git").join("fast-staged.patch");
        assert!(
            patch_path.exists(),
            "patch file should exist after backup()"
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_backup_patch_is_nonempty_for_staged_changes() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("backup-nonempty");
        stage_file(&dir, "file.txt", "line one\n");
        commit_all(&dir, "initial");
        stage_file(&dir, "file.txt", "line one\nline two\n");

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        ctx.backup().unwrap();

        let patch_path = dir.join(".git").join("fast-staged.patch");
        let content = std::fs::read_to_string(&patch_path).unwrap();
        assert!(
            !content.is_empty(),
            "patch should be non-empty when there are staged changes"
        );
        assert!(
            content.contains("line two"),
            "patch should contain the new line, got: {content}"
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_backup_overwrites_stale_patch() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("backup-overwrite");
        stage_file(&dir, "data.txt", "v1\n");
        commit_all(&dir, "initial");
        stage_file(&dir, "data.txt", "v2\n");

        // Write a stale patch file.
        let patch_path = dir.join(".git").join("fast-staged.patch");
        std::fs::write(&patch_path, b"stale content").unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        ctx.backup().unwrap();

        let content = std::fs::read_to_string(&patch_path).unwrap();
        assert_ne!(
            content, "stale content",
            "backup() should overwrite the old patch file"
        );
        assert!(
            content.contains("v2") || !content.contains("stale"),
            "patch should reflect current staged diff, got: {content}"
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_backup_on_unborn_head_succeeds() {
        let _lock = CWD_LOCK.lock().unwrap();
        // Fresh repo with no commits — unborn HEAD.
        let dir = make_real_git_repo("backup-unborn");
        stage_file(&dir, "new.txt", "brand new file\n");

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        // Should succeed even with no prior commits (Pitfall 5).
        ctx.backup().unwrap();

        let patch_path = dir.join(".git").join("fast-staged.patch");
        assert!(
            patch_path.exists(),
            "patch file should be created even for unborn HEAD"
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------------------------
    // restore() tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_restore_applies_patch_to_index() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("restore-applies");
        // Commit original, then stage a modification.
        stage_file(&dir, "restore.txt", "original\n");
        commit_all(&dir, "initial");
        stage_file(&dir, "restore.txt", "modified\n");

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        // Save the staged diff.
        ctx.backup().unwrap();

        // Reset the index so no staged changes remain.
        let out = std::process::Command::new("git")
            .args(["reset", "HEAD"])
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "git reset HEAD failed");

        // Restore should re-apply the staged diff.
        ctx.restore().unwrap();

        // Check that the staged diff is back.
        let diff_out = std::process::Command::new("git")
            .args(["diff", "--cached"])
            .current_dir(&dir)
            .output()
            .unwrap();
        let diff_text = String::from_utf8_lossy(&diff_out.stdout);
        assert!(
            diff_text.contains("modified"),
            "restore() should re-stage the original diff, got: {diff_text}"
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_restore_noop_when_no_patch_exists() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("restore-noop");

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        // No patch file exists — restore() should succeed silently.
        let result = ctx.restore();
        assert!(
            result.is_ok(),
            "restore() should succeed when no patch file exists"
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // -------------------------------------------------------------------------
    // cleanup() tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_cleanup_removes_patch_file() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("cleanup-removes");

        // Create a patch file manually.
        let patch_path = dir.join(".git").join("fast-staged.patch");
        std::fs::write(&patch_path, b"some patch content").unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        ctx.cleanup().unwrap();

        assert!(
            !patch_path.exists(),
            "cleanup() should remove the patch file"
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cleanup_noop_when_no_patch_exists() {
        let _lock = CWD_LOCK.lock().unwrap();
        let dir = make_real_git_repo("cleanup-noop");

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        // No patch file — cleanup() should succeed silently.
        let result = ctx.cleanup();
        assert!(
            result.is_ok(),
            "cleanup() should succeed when no patch file exists"
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
