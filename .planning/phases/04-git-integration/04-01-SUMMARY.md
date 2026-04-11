---
phase: 04-git-integration
plan: "01"
subsystem: git
tags: [git, gix, staged-files, unstaged-files, index]
dependency_graph:
  requires: [error.rs AppError variants]
  provides: [GitContext::open, GitContext::staged_files, GitContext::unstaged_files, GitContext::git_dir]
  affects: [lib.rs, future execution layer]
tech_stack:
  added: []
  patterns: [gix::discover for repo discovery, open_index for fresh index read, status iterator for unstaged changes, Mutex-guarded CWD in tests]
key_files:
  created: [crates/fast-staged/src/git.rs]
  modified: [crates/fast-staged/src/lib.rs]
decisions:
  - "Use open_index() not index() for fresh pre-commit hook reads (Pitfall 2)"
  - "Map open_index() failure to NoStagedFiles rather than Git error — handles fresh repos with no index file"
  - "Serialise CWD-mutating tests with a static Mutex to avoid parallel contamination"
  - "Use entry.path_in(backing) not entry.path(state) — path_in takes PathStorageRef slice directly"
  - "Use entry.stage() == Stage::Unconflicted not == 0 — stage() returns enum not integer"
metrics:
  duration_seconds: 426
  completed_date: "2026-04-11"
  tasks_completed: 1
  files_changed: 2
---

# Phase 04 Plan 01: Git Integration — GitContext Summary

GitContext struct wrapping gix 0.81.0 with open/staged_files/unstaged_files/git_dir, all unit-tested with real git repos.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create GitContext with open(), staged_files(), unstaged_files() | 3dcf5a3 | crates/fast-staged/src/git.rs, crates/fast-staged/src/lib.rs |

## What Was Built

`crates/fast-staged/src/git.rs` implements `GitContext`:

- `open()` — calls `gix::discover(".")`, maps any error to `AppError::NotGitRepository`
- `git_dir()` — returns `self.repo.git_dir()` as `&Path`
- `staged_files()` — calls `repo.open_index()`, iterates entries filtering `Stage::Unconflicted`, returns `AppError::NoStagedFiles` when empty (including when no index file exists in a fresh repo)
- `unstaged_files()` — uses `repo.status(gix::progress::Discard)` with `UntrackedFiles::None`, collects `Item::IndexWorktree` items from the status iterator

7 unit tests cover: open success, open outside repo, git_dir path, two staged files, empty index, stage-0 filter, and modified-but-unstaged file detection. Tests use real `git init` repos in temp dirs and serialize CWD mutations with a `static Mutex`.

`lib.rs` updated: added `pub mod git;` declaration.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] gix API shape differed from research notes**
- **Found during:** Task 1 implementation
- **Issue:** Research documented `entry.path(&backing)` taking `&[u8]` and `entry.stage() == 0`, but actual gix 0.49.0 index API uses `entry.path_in(backing: &PathStorageRef)` and `entry.stage()` returns `Stage` enum not `u32`
- **Fix:** Used `entry.path_in(backing)` and `Stage::Unconflicted` comparison; imported `gix::index::entry::Stage` via the `gix::index` re-export
- **Files modified:** `crates/fast-staged/src/git.rs`
- **Commit:** 3dcf5a3

**2. [Rule 1 - Bug] open_index() errors on no-index fresh repo rather than returning empty**
- **Found during:** Task 1 testing (RED phase failed with wrong error type)
- **Issue:** Research assumed `open_index()` would return an Ok with empty entries on a fresh repo. In practice, a freshly-initialized git repo has no index file yet; `open_index()` returns `Err`, not `Ok([])`.
- **Fix:** Added match arm to convert any `open_index()` error to `AppError::NoStagedFiles` — semantically correct since "no index file" means "no staged files"
- **Files modified:** `crates/fast-staged/src/git.rs`
- **Commit:** 3dcf5a3

**3. [Rule 2 - Missing critical functionality] CWD mutex for parallel test safety**
- **Found during:** Task 1 testing (GREEN phase — 3 tests flaked when run in parallel)
- **Issue:** `std::env::set_current_dir` is process-wide; parallel test execution caused races where one test's CWD change broke another test's `GitContext::open()`
- **Fix:** Added `static CWD_LOCK: Mutex<()>` in test module; each test acquires the lock before mutating CWD
- **Files modified:** `crates/fast-staged/src/git.rs`
- **Commit:** 3dcf5a3

## Known Stubs

None — all public functions are fully implemented and tested.

## Self-Check: PASSED

- `crates/fast-staged/src/git.rs` — FOUND
- commit `3dcf5a3` — FOUND
- `pub struct GitContext` in git.rs — FOUND
- `cargo test --lib -- git::tests` — 7/7 PASSED
- `cargo test` full suite — 63/63 PASSED
- `cargo clippy -- -D warnings` — CLEAN
- `cargo fmt -- --check` — CLEAN
