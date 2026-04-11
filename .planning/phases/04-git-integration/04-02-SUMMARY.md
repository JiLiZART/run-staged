---
phase: 04-git-integration
plan: 02
subsystem: git
tags: [gix, git-diff, git-apply, patch, backup, restore]

# Dependency graph
requires:
  - phase: 04-01
    provides: GitContext struct with open(), staged_files(), unstaged_files()
provides:
  - backup() method on GitContext — creates .git/fast-staged.patch via git diff --cached
  - restore() method on GitContext — applies patch via git apply --cached
  - cleanup() method on GitContext — removes patch file silently if absent
  - lib.rs::run() pipeline wired: config -> git open -> staged files -> backup -> cleanup
  - integration tests use real git repos (git init + git add) instead of fake .git/ stubs
affects: [05-file-matching, 06-cli-flags, 08-tui]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "git diff --cached via std::process::Command for patch generation (pragmatic exception to no-git-CLI rule)"
    - "git apply --cached via std::process::Command for patch restore (gix has no apply API)"
    - "Idempotent cleanup: check file exists before remove_file"

key-files:
  created: []
  modified:
    - crates/fast-staged/src/git.rs
    - crates/fast-staged/src/lib.rs
    - crates/fast-staged/tests/integration_test.rs

key-decisions:
  - "Use git diff --cached shell-out for backup() — gix blob-level diff API requires complex assembly per research, fallback explicitly allowed"
  - "workdir() vs work_dir() — clippy flagged deprecated work_dir(), switched to workdir()"
  - "Integration tests migrated to real git repos — make_git_root (fake .git/) replaced with make_real_git_repo (git init)"

patterns-established:
  - "Pattern: backup/restore/cleanup as three symmetric methods on GitContext"
  - "Pattern: restore() is a no-op when no patch file exists (silent success)"
  - "Pattern: cleanup() is a no-op when no patch file exists (silent success)"

requirements-completed: [GIT-02, GIT-03, GIT-04]

# Metrics
duration: 8min
completed: 2026-04-11
---

# Phase 4 Plan 02: Git Backup/Restore Pipeline Summary

**Patch backup/restore/cleanup added to GitContext with git diff --cached + git apply --cached, wired into lib.rs::run() pipeline with real-git-repo integration tests**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-04-11T19:00:00Z
- **Completed:** 2026-04-11T19:08:43Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- `backup()` creates `.git/fast-staged.patch` via `git diff --cached`, handling unborn HEAD correctly
- `restore()` applies patch back via `git apply --cached` (no-op when patch absent)
- `cleanup()` removes patch file silently (no-op when patch absent)
- `lib.rs::run()` pipeline: config load -> git open -> staged files -> backup -> (future: execute) -> cleanup
- 8 new unit tests covering all backup/restore/cleanup scenarios
- Integration tests migrated from fake `.git/` stubs to real git repos with actual `git init` + `git add`
- Added `test_binary_exits_nonzero_with_no_staged_files` integration test
- 72 total tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add backup(), restore(), cleanup() to GitContext with tests** - `fea761f` (feat)
2. **Task 2: Wire git module into lib.rs::run() pipeline** - `832dd30` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `crates/fast-staged/src/git.rs` - Added backup(), restore(), cleanup() methods + 8 unit tests
- `crates/fast-staged/src/lib.rs` - Updated run() to wire full git pipeline
- `crates/fast-staged/tests/integration_test.rs` - Migrated to real git repos, added no-staged-files test

## Decisions Made
- Used `git diff --cached` shell-out for `backup()` — the research notes the gix blob-level diff API requires complex assembly (status iterator + blob OID extraction + UnifiedDiffBuilder per file) and explicitly allows this fallback. This is a pragmatic exception documented alongside the `restore()` git apply exception.
- Fixed deprecated `work_dir()` → `workdir()` discovered via clippy (Rule 1 auto-fix)
- Integration tests migrated from fake `.git/` stubs to real `git init` repos — required by the new `git::GitContext::open()` and `staged_files()` calls in `run()` that need a real index file.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed deprecated `work_dir()` method**
- **Found during:** Task 1 (backup/restore implementation)
- **Issue:** gix 0.81.0 deprecated `work_dir()`, clippy -D warnings flagged it as error
- **Fix:** Replaced both occurrences with `workdir()` per clippy suggestion
- **Files modified:** crates/fast-staged/src/git.rs
- **Verification:** `cargo clippy -- -D warnings` exits 0
- **Committed in:** fea761f (Task 1 commit)

**2. [Plan note] backup() uses git diff --cached instead of gix blob diff**
- **Found during:** Task 1 implementation
- **Issue:** gix status Item::TreeIndex change field access for blob OIDs requires runtime verification; plan explicitly permits `git diff --cached` fallback
- **Fix:** Used `std::process::Command::new("git").args(["diff", "--cached"])` — consistent with `restore()`'s `git apply --cached` approach
- **Files modified:** crates/fast-staged/src/git.rs
- **Verification:** 8 unit tests pass including backup creates patch, non-empty content, overwrite stale, unborn HEAD
- **Committed in:** fea761f (Task 1 commit)

---

**Total deviations:** 2 (1 auto-fixed bug, 1 planned fallback from research)
**Impact on plan:** Both handled correctly. Patch generation works as specified; gix blob-level diff can be explored as optimization post-v1.

## Issues Encountered
None — implementation matched plan intent. The `git diff --cached` fallback was explicitly planned in the research as a valid approach.

## Known Stubs
None — backup/restore/cleanup are fully implemented. The future `restore()` call on command failure is noted as a comment in `lib.rs::run()` and will be wired in Phase 5 when execution logic is added.

## Next Phase Readiness
- Git pipeline fully wired: config -> git open -> staged files -> backup -> cleanup
- Phase 5 (file-matching) can call `git_ctx.restore()` + `git_ctx.cleanup()` on command failure
- All 72 tests green, clippy clean, rustfmt clean

---
*Phase: 04-git-integration*
*Completed: 2026-04-11*
