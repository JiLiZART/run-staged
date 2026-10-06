//! Lock-safe git access for run-staged, in-process through gix.
//!
//! The rules (see git's own lock protocol):
//! - The index file is `GIT_INDEX_FILE` when set, else `<git-dir>/index`.
//!   During `git commit -a` git points hooks at `.git/index.lock`.
//! - **Reads** open the index file directly: no lock, no stat refresh, so they
//!   work while another git process holds `<index>.lock`.
//! - **Writes** ([`Repo::restore`], [`Repo::stage`]) take `<index>.lock` with a
//!   bounded wait, write, and rename it over the index.
//! - The `git` executable is never started.

use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use gix::{
    bstr::{BStr, BString},
    index::entry::{Mode, Stage},
};

mod error;
mod hook;
mod snapshot;
mod write;

pub use error::GitError;
use error::git_err;
pub use gix::ObjectId;
pub use hook::{HOOK_MARKER, install_hook};
pub use snapshot::{Changes, Snapshot};

/// File name of the index backup inside the git dir.
pub const BACKUP_FILE: &str = "run-staged.index";

/// An opened repository plus the index file every operation uses.
pub struct Repo {
    repo: gix::Repository,
    work_dir: PathBuf,
    index_path: PathBuf,
    /// The index as first read, shared by all reads; [`Repo::stage`] clears it.
    index: Mutex<Option<Arc<gix::index::File>>>,
}

impl Repo {
    /// Discovers the repository from `cwd`, honoring `GIT_DIR` / `GIT_WORK_TREE`.
    /// `index_file` is the value of `GIT_INDEX_FILE`, if any (relative to `cwd`).
    pub fn open(cwd: &Path, index_file: Option<PathBuf>) -> Result<Repo, GitError> {
        let repo = gix::discover_with_environment_overrides(cwd)
            .map_err(|_| GitError::NotGitRepository)?;
        let work_dir = repo.workdir().ok_or(GitError::Bare)?.to_path_buf();
        let index_path = match index_file {
            Some(p) if !p.as_os_str().is_empty() => cwd.join(p),
            _ => repo.index_path(),
        };
        Ok(Repo {
            repo,
            work_dir,
            index_path,
            index: Mutex::new(None),
        })
    }

    pub fn git_dir(&self) -> &Path {
        self.repo.git_dir()
    }

    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    pub fn index_path(&self) -> &Path {
        &self.index_path
    }

    pub fn backup_path(&self) -> PathBuf {
        self.git_dir().join(BACKUP_FILE)
    }

    /// Reads the index without locking. A missing index reads as empty.
    fn load_index(&self) -> Result<gix::index::File, GitError> {
        if !self.index_path.exists() {
            let state = gix::index::State::new(self.repo.object_hash());
            return Ok(gix::index::File::from_state(state, self.index_path.clone()));
        }
        gix::index::File::at(
            &self.index_path,
            self.repo.object_hash(),
            false,
            Default::default(),
        )
        .map_err(git_err)
    }

    /// The cached read-only index, loaded on first use.
    fn index(&self) -> Result<Arc<gix::index::File>, GitError> {
        let mut cached = self.index.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(index) = cached.as_ref() {
            return Ok(Arc::clone(index));
        }
        let index = Arc::new(self.load_index()?);
        *cached = Some(Arc::clone(&index));
        Ok(index)
    }

    fn forget_index(&self) {
        *self.index.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// HEAD's tree as an index, or `None` when HEAD is unborn.
    fn head_index(&self) -> Result<Option<gix::index::File>, GitError> {
        let Ok(tree) = self.repo.head_tree_id() else {
            return Ok(None);
        };
        self.repo.index_from_tree(&tree).map(Some).map_err(git_err)
    }

    /// Files whose index entry differs from HEAD (added, copied, modified,
    /// renamed under the new path). Deleted files, conflicts, and submodules are
    /// excluded. With an unborn HEAD every index entry counts.
    pub fn staged_files(&self) -> Result<Vec<PathBuf>, GitError> {
        let index = self.index()?;
        let head = self.head_index()?;
        let files: Vec<PathBuf> = index
            .entries()
            .iter()
            .filter(|e| e.stage() == Stage::Unconflicted && e.mode != Mode::COMMIT)
            .filter(|e| {
                let path = e.path(&index);
                head.as_ref()
                    .and_then(|h| h.entry_by_path(path))
                    .is_none_or(|he| he.id != e.id || he.mode != e.mode)
            })
            .map(|e| to_path(e.path(&index)))
            .collect();
        if files.is_empty() {
            return Err(GitError::NoStagedFiles);
        }
        Ok(files)
    }

    /// Tracked files whose working-tree content differs from the index.
    /// Deleted and untracked files are excluded. An empty list is not an error.
    pub fn unstaged_files(&self) -> Result<Vec<PathBuf>, GitError> {
        let index = self.index()?;
        let mut hasher = Hasher::new(&self.repo, &self.work_dir)?;
        let mut files = Vec::new();
        for e in index.entries() {
            if e.stage() != Stage::Unconflicted || e.mode == Mode::COMMIT {
                continue;
            }
            let rela = e.path(&index);
            let abs = self.work_dir.join(gix::path::from_bstr(rela));
            let Ok(md) = gix::index::fs::Metadata::from_path_no_follow(&abs) else {
                continue; // deleted in the working tree
            };
            // Same shortcut git uses: an unchanged stat means unchanged content,
            // unless the entry is "racy" (written in the same tick as the index).
            let opts = Default::default();
            if !e.stat.is_racy(index.timestamp(), opts)
                && gix::index::entry::Stat::from_fs(&md).is_ok_and(|s| e.stat.matches(&s, opts))
            {
                continue;
            }
            if hasher.worktree_id(rela, &index)? != Some(e.id) {
                files.push(to_path(rela));
            }
        }
        Ok(files)
    }
}

/// Hashes working-tree files the way `git add` would, without writing objects.
struct Hasher<'r> {
    repo: &'r gix::Repository,
    work_dir: &'r Path,
    pipeline: gix::filter::Pipeline<'r>,
    buf: Vec<u8>,
}

impl<'r> Hasher<'r> {
    fn new(repo: &'r gix::Repository, work_dir: &'r Path) -> Result<Self, GitError> {
        let (pipeline, _) = repo.filter_pipeline(None).map_err(git_err)?;
        Ok(Hasher {
            repo,
            work_dir,
            pipeline,
            buf: Vec::new(),
        })
    }

    fn worktree_id(
        &mut self,
        rela: &BStr,
        index: &gix::index::State,
    ) -> Result<Option<ObjectId>, GitError> {
        use gix::filter::plumbing::pipeline::convert::ToGitOutcome;

        let rel_path = gix::path::from_bstr(rela);
        let abs = self.work_dir.join(&rel_path);
        let md = match std::fs::symlink_metadata(&abs) {
            Ok(md) => md,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        self.buf.clear();
        if md.is_symlink() {
            let target = std::fs::read_link(&abs)?;
            self.buf
                .extend_from_slice(gix::path::into_bstr(target).as_ref());
        } else if md.is_file() {
            let file = std::fs::File::open(&abs)?;
            match self
                .pipeline
                .convert_to_git(file, &rel_path, index)
                .map_err(git_err)?
            {
                ToGitOutcome::Unchanged(mut f) => {
                    f.read_to_end(&mut self.buf)?;
                }
                ToGitOutcome::Buffer(b) => self.buf.extend_from_slice(b),
                ToGitOutcome::Process(mut r) => {
                    r.read_to_end(&mut self.buf)?;
                }
            }
        } else {
            return Ok(None);
        }
        gix::objs::compute_hash(self.repo.object_hash(), gix::objs::Kind::Blob, &self.buf)
            .map(Some)
            .map_err(git_err)
    }
}

pub(crate) fn to_path(rela: &BStr) -> PathBuf {
    gix::path::from_bstr(rela).into_owned()
}

pub(crate) fn to_bstring(p: &Path) -> BString {
    gix::path::to_unix_separators_on_windows(gix::path::into_bstr(p)).into_owned()
}

#[cfg(test)]
mod tests;
