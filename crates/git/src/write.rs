//! Index backup, restore, and staging. Every write goes through `<index>.lock`.

use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use gix::{
    bstr::BString,
    index::entry::{Flags, Mode, Stage, Stat},
    objs::tree::EntryKind,
};

use crate::{GitError, Repo, error::git_err, to_bstring};

/// How long a write waits for another process to release the index lock.
const LOCK_WAIT: Duration = Duration::from_secs(5);

impl Repo {
    /// Copies the index byte-for-byte to `<git-dir>/run-staged.index`.
    /// git replaces the index by atomic rename, so the copy is always consistent.
    pub fn backup(&self) -> Result<PathBuf, GitError> {
        let backup = self.backup_path();
        std::fs::copy(self.index_path(), &backup)?;
        Ok(backup)
    }

    /// Replaces the index with the backup under the index lock, then deletes the
    /// backup. On failure the backup is kept and the error explains the manual copy.
    pub fn restore(&self) -> Result<(), GitError> {
        let backup = self.backup_path();
        let attempt = || -> Result<(), GitError> {
            let bytes = std::fs::read(&backup)?;
            let mut lock = self.lock_index()?;
            lock.write_all(&bytes)?;
            lock.commit().map_err(|e| git_err(e.error))?;
            self.forget_index();
            Ok(())
        };
        match attempt() {
            Ok(()) => {
                std::fs::remove_file(&backup)?;
                Ok(())
            }
            Err(e) => Err(GitError::RestoreFailed {
                backup,
                index: self.index_path().to_path_buf(),
                reason: e.to_string(),
            }),
        }
    }

    /// Deletes the backup if it exists.
    pub fn cleanup(&self) -> Result<(), GitError> {
        match std::fs::remove_file(self.backup_path()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    /// Stages the working-tree content of `changed` (writing blobs, like
    /// `git add`) and removes `deleted` from the index, in one locked write.
    /// A path in `changed` that no longer exists is removed as well.
    pub fn stage(&self, changed: &[PathBuf], deleted: &[PathBuf]) -> Result<(), GitError> {
        if changed.is_empty() && deleted.is_empty() {
            return Ok(());
        }
        let mut lock = self.lock_index()?;
        // Load after locking so no concurrent write is lost.
        let mut index = self.load_index()?;
        let (mut pipeline, _) = self.repo.filter_pipeline(None).map_err(git_err)?;

        let mut remove: Vec<BString> = deleted.iter().map(|p| to_bstring(p)).collect();
        let mut pushed = false;
        for path in changed {
            let rela = to_bstring(path);
            let Some((id, kind, _)) = pipeline
                .worktree_file_to_object(rela.as_ref(), &index)
                .map_err(git_err)?
            else {
                remove.push(rela);
                continue;
            };
            let mode = match kind {
                EntryKind::Blob => Mode::FILE,
                EntryKind::BlobExecutable => Mode::FILE_EXECUTABLE,
                EntryKind::Link => Mode::SYMLINK,
                EntryKind::Tree | EntryKind::Commit => continue,
            };
            let stat = fresh_stat(&self.work_dir().join(path))?;
            match index.entry_mut_by_path_and_stage(rela.as_ref(), Stage::Unconflicted) {
                Some(entry) => {
                    entry.id = id;
                    entry.mode = mode;
                    entry.stat = stat;
                }
                None => {
                    index.dangerously_push_entry(stat, id, Flags::empty(), mode, rela.as_ref());
                    pushed = true;
                }
            }
        }
        if !remove.is_empty() {
            index.remove_entries(|_, path, e| {
                e.stage() == Stage::Unconflicted && remove.iter().any(|r| r == path)
            });
        }
        if pushed {
            index.sort_entries();
        }
        // The cached tree no longer matches the entries; git rebuilds it.
        index.remove_tree();
        index
            .write_to(&mut lock, Default::default())
            .map_err(git_err)?;
        lock.commit().map_err(|e| git_err(e.error))?;
        self.forget_index();
        Ok(())
    }

    /// Acquires `<index>.lock`, retrying for up to [`LOCK_WAIT`].
    fn lock_index(&self) -> Result<gix::lock::File, GitError> {
        gix::lock::File::acquire_to_update_resource(
            self.index_path(),
            gix::lock::acquire::Fail::AfterDurationWithBackoff(LOCK_WAIT),
            None,
        )
        .map_err(|_| GitError::LockBusy {
            lock: lock_path(self.index_path()),
        })
    }
}

fn lock_path(index: &Path) -> PathBuf {
    let mut p = index.as_os_str().to_owned();
    p.push(".lock");
    PathBuf::from(p)
}

fn fresh_stat(abs: &Path) -> Result<Stat, GitError> {
    let md = gix::index::fs::Metadata::from_path_no_follow(abs)?;
    Stat::from_fs(&md).map_err(git_err)
}
