//! What commands changed: a snapshot of the candidate files before they run,
//! compared after. Both sides trust git's stat shortcut and read only files
//! whose stat says they may differ; that hashing fans out across threads.

use std::{
    path::{Path, PathBuf},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use gix::index::entry::{Stage, Stat};

use crate::{GitError, Hasher, ObjectId, Repo, to_bstring};

/// Below this many files to hash, spawning threads costs more than it saves.
const PARALLEL_MIN: usize = 64;

/// Candidate files as they were before commands ran.
pub struct Snapshot {
    /// When the snapshot started, in whole seconds since the epoch. A file
    /// whose stat is unchanged but whose mtime is not older than this may have
    /// been rewritten within the timestamp granularity, so it is hashed.
    taken: u64,
    files: Vec<FileState>,
}

struct FileState {
    path: PathBuf,
    exists: bool,
    /// `None` when the file is missing or its stat could not be read.
    stat: Option<Stat>,
    /// Blob id of the working-tree content, `None` when missing.
    id: Option<ObjectId>,
    /// The working tree differs from the index entry (unstaged edits).
    partial: bool,
}

/// Files commands changed since a [`Snapshot`], in candidate order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub changed: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
    /// Partially staged files a command modified; they must not be re-staged.
    pub partial_modified: Vec<PathBuf>,
}

impl Snapshot {
    /// Candidates whose working-tree content differs from their index entry.
    pub fn partially_staged(&self) -> Vec<&Path> {
        self.files
            .iter()
            .filter(|f| f.partial)
            .map(|f| f.path.as_path())
            .collect()
    }
}

impl Repo {
    /// Records the state of `paths` before commands run. A file whose stat
    /// matches its (non-racy) index entry has the entry's content and is not read.
    pub fn snapshot(&self, paths: &[PathBuf]) -> Result<Snapshot, GitError> {
        let taken = now_secs();
        let index = self.index()?;
        let opts = Default::default();
        let mut files = Vec::with_capacity(paths.len());
        let mut entry_ids = Vec::with_capacity(paths.len());
        let mut to_hash = Vec::new();
        for path in paths {
            let (exists, stat) = fs_stat(&self.work_dir.join(path));
            let entry =
                index.entry_by_path_and_stage(to_bstring(path).as_ref(), Stage::Unconflicted);
            let entry_id = entry.map(|e| e.id);
            let clean = matches!((entry, stat), (Some(e), Some(s))
                if !e.stat.is_racy(index.timestamp(), opts) && e.stat.matches(&s, opts));
            let (id, partial) = if clean {
                (entry_id, false)
            } else if !exists {
                (None, entry_id.is_some())
            } else {
                to_hash.push(files.len());
                (None, false)
            };
            files.push(FileState {
                path: path.clone(),
                exists,
                stat,
                id,
                partial,
            });
            entry_ids.push(entry_id);
        }
        let hashed = self.hash_paths(&index, &select(&files, &to_hash), threads())?;
        for (&i, id) in to_hash.iter().zip(hashed) {
            files[i].partial = entry_ids[i].is_some_and(|e| id != Some(e));
            files[i].id = id;
        }
        Ok(Snapshot { taken, files })
    }

    /// Compares the working tree with `snap`. Files whose stat is unchanged
    /// (and not racy against the snapshot time) are not read.
    pub fn changes(&self, snap: &Snapshot) -> Result<Changes, GitError> {
        let index = self.index()?;
        let mut to_hash = Vec::new();
        for (i, f) in snap.files.iter().enumerate() {
            let (exists, stat) = fs_stat(&self.work_dir.join(&f.path));
            let unchanged = match (f.exists, exists) {
                (false, false) => true,
                (true, true) => {
                    stat.is_some_and(|s| Some(s) == f.stat && u64::from(s.mtime.secs) < snap.taken)
                }
                _ => false,
            };
            if !unchanged {
                to_hash.push(i);
            }
        }
        let hashed = self.hash_paths(&index, &select(&snap.files, &to_hash), threads())?;
        let mut changes = Changes::default();
        for (&i, id) in to_hash.iter().zip(hashed) {
            let f = &snap.files[i];
            if id == f.id {
                continue;
            }
            let bucket = if f.partial {
                &mut changes.partial_modified
            } else if id.is_some() {
                &mut changes.changed
            } else {
                &mut changes.deleted
            };
            bucket.push(f.path.clone());
        }
        Ok(changes)
    }

    /// Blob ids of `paths`' working-tree content, in order, using up to
    /// `threads` threads (each with its own repository handle and filter pipeline).
    pub(crate) fn hash_paths(
        &self,
        index: &gix::index::State,
        paths: &[&Path],
        threads: usize,
    ) -> Result<Vec<Option<ObjectId>>, GitError> {
        let work_dir = self.work_dir.as_path();
        if threads <= 1 || paths.len() < PARALLEL_MIN {
            let mut hasher = Hasher::new(&self.repo, work_dir)?;
            return paths
                .iter()
                .map(|p| hasher.worktree_id(to_bstring(p).as_ref(), index))
                .collect();
        }
        let shared = self.repo.clone().into_sync();
        let shared = &shared;
        thread::scope(|s| {
            let handles: Vec<_> = paths
                .chunks(paths.len().div_ceil(threads))
                .map(|part| {
                    s.spawn(move || {
                        let repo = shared.to_thread_local();
                        let mut hasher = Hasher::new(&repo, work_dir)?;
                        part.iter()
                            .map(|p| hasher.worktree_id(to_bstring(p).as_ref(), index))
                            .collect::<Result<Vec<_>, GitError>>()
                    })
                })
                .collect();
            let mut ids = Vec::with_capacity(paths.len());
            for handle in handles {
                ids.extend(handle.join().expect("hashing thread panicked")?);
            }
            Ok(ids)
        })
    }
}

fn select<'f>(files: &'f [FileState], indices: &[usize]) -> Vec<&'f Path> {
    indices.iter().map(|&i| files[i].path.as_path()).collect()
}

/// Whether `abs` exists, and its stat. Errors other than "not found" count as
/// existing with an unknown stat, so the file is hashed and the error surfaces.
fn fs_stat(abs: &Path) -> (bool, Option<Stat>) {
    match gix::index::fs::Metadata::from_path_no_follow(abs) {
        Ok(md) => (true, Stat::from_fs(&md).ok()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (false, None),
        Err(_) => (true, None),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub(crate) fn threads() -> usize {
    // ponytail: hashing through gix's filter pipeline stops scaling at ~4
    // threads (measured on an M3 Max: 1→565 ms, 4→150 ms, 14→1 s for 10k
    // files); find the contended lock before raising this.
    const MAX_HASH_THREADS: usize = 4;
    thread::available_parallelism().map_or(1, |n| n.get().min(MAX_HASH_THREADS))
}
