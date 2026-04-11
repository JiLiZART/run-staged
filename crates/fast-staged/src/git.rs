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
}
