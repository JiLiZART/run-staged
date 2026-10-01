# git-staging Specification

## Purpose
Determine which files to operate on from the git repository and protect the user's staged changes when commands fail.

## Requirements

### Requirement: Repository detection
The tool SHALL locate the git repository by searching upward from the current working directory. It SHALL honor `GIT_DIR` and `GIT_WORK_TREE` when they are set (as they are inside git hooks). The index file SHALL be `GIT_INDEX_FILE` when set, otherwise `<git-dir>/index`. Outside a repository the tool SHALL print `error: not a git repository` and exit with code 1. Bare repositories SHALL be rejected with a git error.

#### Scenario: Outside a repository
- **WHEN** the tool runs in a directory that is not inside a git repository
- **THEN** it prints `error: not a git repository` and exits with code 1

#### Scenario: Alternate index from hook environment
- **WHEN** `GIT_INDEX_FILE` points at `.git/index.lock` (as during `git commit -a`) and that file stages `a.js`
- **THEN** the tool lists `a.js` as staged, reading from `.git/index.lock`

### Requirement: Staged file listing
By default the tool SHALL operate on files whose index entry differs from HEAD: added, copied, modified, and renamed files (renames under their new path). Deleted files and committed-but-unchanged files SHALL be excluded. In a repository with no commits, all non-conflicted index entries SHALL be used. Paths SHALL be relative to the repository root.

#### Scenario: Committed files are not re-linted
- **WHEN** the repository has committed `.rs` files and only `README.md` is staged
- **THEN** the file list contains only `README.md`

#### Scenario: Deleted file excluded
- **WHEN** a staged change deletes `old.rs`
- **THEN** `old.rs` is not in the file list

#### Scenario: Fresh repository
- **WHEN** HEAD is unborn and `a.txt` is staged
- **THEN** the file list contains `a.txt`

### Requirement: No staged files
When there are no staged files the tool SHALL print `error: no staged files — run 'git add' to stage files` and exit with code 1, unless `--allow-empty` is given, in which case it SHALL exit 0 without output.

#### Scenario: Nothing staged
- **WHEN** the tool runs with nothing staged and without `--allow-empty`
- **THEN** it exits with code 1 and the no-staged-files message

#### Scenario: Nothing staged with allow-empty
- **WHEN** the tool runs with nothing staged and `--allow-empty`
- **THEN** it exits 0 silently

### Requirement: Unstaged mode
With `--unstaged` the tool SHALL operate on tracked files whose working-tree content differs from the index. Untracked files and files deleted from the working tree SHALL be excluded. Backup, restore, staging, and cleanup SHALL be skipped in this mode, and the index SHALL NOT be written.

#### Scenario: Unstaged files used
- **WHEN** `a.rs` is modified but not staged and the tool runs with `--unstaged`
- **THEN** commands receive `a.rs`

#### Scenario: No patch in unstaged mode
- **WHEN** the tool runs with `--unstaged` and a command fails
- **THEN** no `<git-dir>/run-staged.index` backup is created and the index is unchanged

### Requirement: Staged diff backup
Before running any command in staged mode, the tool SHALL save a byte-for-byte copy of the index file to `<git-dir>/run-staged.index`, overwriting any existing copy. The backup SHALL only be written after file matching succeeds, so a run that matches nothing never creates it.

#### Scenario: Patch written before commands
- **WHEN** staged files match a pattern
- **THEN** `<git-dir>/run-staged.index` exists while commands run and equals the index as it was before the run

#### Scenario: No match, no patch
- **WHEN** no staged file matches any pattern
- **THEN** `<git-dir>/run-staged.index` is not created

### Requirement: Restore on failure
When any command fails, times out, or is cancelled in staged mode, the tool SHALL replace the index with the backup, under the index lock (see "Locked index writes"), and then delete the backup. If restoring fails, the tool SHALL keep the backup and exit with an error that includes the backup path, the index path, and an instruction to copy the backup over the index manually.

#### Scenario: Failure restores and cleans up
- **WHEN** a command exits non-zero after another command ran `git add` on an extra file
- **THEN** the index equals its state before the run and `<git-dir>/run-staged.index` is removed

#### Scenario: Restore failure keeps patch
- **WHEN** the index lock cannot be acquired during restore
- **THEN** the backup file remains, and the error names both paths and explains the manual copy

### Requirement: Cleanup on success
When all commands succeed in staged mode, the tool SHALL first stage task modifications (see "Stage task modifications"). It SHALL then delete `<git-dir>/run-staged.index`. If staging fails, the tool SHALL keep the backup, print the error with the backup path, and exit with code 1.

#### Scenario: Success cleans up
- **WHEN** all commands exit 0
- **THEN** `<git-dir>/run-staged.index` no longer exists

### Requirement: No git executable
All repository reads and writes (listing, backup, restore, staging, reading config for hook install) SHALL be done in-process. The tool SHALL NOT start a `git` process.

#### Scenario: git missing from PATH
- **WHEN** a staged-mode run with a formatter that changes a file runs with no `git` on PATH
- **THEN** the run succeeds and the formatted content is staged

### Requirement: Lock-free reads
Reading the repository (discovery, listing staged or unstaged files, backup, hashing working-tree files) SHALL NOT create, wait for, or fail on any lock file. Reads SHALL also NOT update the index's cached file stats. A run that needs no index write SHALL succeed while another process holds the index lock.

#### Scenario: Index locked by another git process
- **WHEN** `.git/index.lock` exists (held by another process) and every matched group is read-only
- **THEN** the tool lists the staged files, runs the commands, and exits 0 without touching `.git/index.lock`

#### Scenario: Commands may use git during the run
- **WHEN** a configured command itself runs `git status` or `git add`
- **THEN** that command is not blocked by any lock held by run-staged

### Requirement: Locked index writes
Every index write (restore and staging) SHALL:
- acquire `<index-file>.lock` with exclusive-create semantics;
- write the new index to the lock file;
- atomically rename it over the index file.

If the lock is held, the tool SHALL retry for up to 5 seconds. If it still cannot get the lock, it SHALL fail with an error naming the lock path, and the hint that another git process is running or a stale lock must be removed. The lock SHALL be released on every exit path.

#### Scenario: Busy lock released in time
- **WHEN** the index lock is held when staging starts and is released 1 second later
- **THEN** staging completes and the tool exits 0

#### Scenario: Lock never released
- **WHEN** the index lock stays held for more than 5 seconds during staging
- **THEN** the tool exits 1 with an error naming the lock path, and the backup is kept

#### Scenario: Hook-provided index
- **WHEN** `GIT_INDEX_FILE` is `.git/index.lock` and staging is needed
- **THEN** the tool locks `.git/index.lock.lock` and writes the updated `.git/index.lock`, so the commit includes the staged edits

### Requirement: Stage task modifications
After a successful staged-mode run, the tool SHALL write the current working-tree content of every file that meets all of these conditions to the object database and the index:
- it was matched by at least one group that is not read-only;
- a command changed it;
- it was fully staged before the run.

Files that a command deleted SHALL be removed from the index. Files that no command changed SHALL NOT be touched. In `--unstaged` mode nothing SHALL be staged.

#### Scenario: Formatter fix is committed
- **WHEN** `a.js` is staged, and a command in a group that is not read-only rewrites `a.js` and exits 0
- **THEN** the index contains the rewritten `a.js`

#### Scenario: Read-only group does not stage
- **WHEN** a command in a `read_only = true` group modifies `a.js`
- **THEN** the index entry for `a.js` is unchanged

#### Scenario: Failed run does not stage
- **WHEN** a command modifies `a.js` and another command fails
- **THEN** the index is restored from the backup and the modification is not staged

### Requirement: Partially staged files
A file is partially staged when, before the run, its working-tree content differs from its index content. The tool SHALL NOT re-stage such a file. Instead, after a successful run, it SHALL print a warning to stderr naming each such file that a command modified, and it SHALL still exit 0.

#### Scenario: Unstaged hunk protected
- **WHEN** `a.js` has one staged hunk and one unstaged hunk, and a formatter rewrites it
- **THEN** the index entry for `a.js` is unchanged and stderr contains a warning naming `a.js`
