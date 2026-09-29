# git-staging Specification

## Purpose
Determine which files to operate on from the git repository and protect the user's staged changes when commands fail.

## Requirements

### Requirement: Repository detection
The tool SHALL locate the git repository by searching upward from the current working directory. Outside a repository it SHALL print `error: not a git repository` and exit with code 1. Bare repositories SHALL be rejected with a git error.

#### Scenario: Outside a repository
- **WHEN** the tool runs in a directory that is not inside a git repository
- **THEN** it prints `error: not a git repository` and exits with code 1

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
With `--unstaged` the tool SHALL operate on tracked files modified in the working tree but not staged. Untracked files SHALL be excluded. Backup, restore, and cleanup SHALL be skipped in this mode.

#### Scenario: Unstaged files used
- **WHEN** `a.rs` is modified but not staged and the tool runs with `--unstaged`
- **THEN** commands receive `a.rs`

#### Scenario: No patch in unstaged mode
- **WHEN** the tool runs with `--unstaged` and a command fails
- **THEN** no `.git/fast-staged.patch` is created or applied

### Requirement: Staged diff backup
Before running any command in staged mode, the tool SHALL write the staged diff (index vs HEAD) to `.git/fast-staged.patch`, overwriting any existing file. The backup SHALL only be written after file matching succeeds, so a run that matches nothing never creates the patch.

#### Scenario: Patch written before commands
- **WHEN** staged files match a pattern
- **THEN** `.git/fast-staged.patch` exists while commands run

#### Scenario: No match, no patch
- **WHEN** no staged file matches any pattern
- **THEN** `.git/fast-staged.patch` is not created

### Requirement: Restore on failure
When any command fails, times out, or is cancelled in staged mode, the tool SHALL re-apply the patch to the index and then delete the patch file. If re-applying fails, the tool SHALL keep the patch file and exit with an error that includes the patch path and the manual recovery command `git apply --cached <patch>`.

#### Scenario: Failure restores and cleans up
- **WHEN** a command exits non-zero
- **THEN** the staged diff is re-applied and `.git/fast-staged.patch` is removed

#### Scenario: Restore failure keeps patch
- **WHEN** re-applying the patch fails
- **THEN** the patch file remains and the error tells the user to run `git apply --cached <patch>`

### Requirement: Cleanup on success
When all commands succeed in staged mode, the tool SHALL delete `.git/fast-staged.patch` without modifying the index.

#### Scenario: Success cleans up
- **WHEN** all commands exit 0
- **THEN** `.git/fast-staged.patch` no longer exists
