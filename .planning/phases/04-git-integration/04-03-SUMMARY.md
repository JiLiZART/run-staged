---
phase: 04-git-integration
plan: 03
subsystem: git-integration
tags: [cli, git, --unstaged, gap-closure, tdd]
requirements: [GIT-05]
dependency_graph:
  requires: [04-01, 04-02]
  provides: ["--unstaged CLI flag", "run(unstaged: bool) signature"]
  affects: [crates/fast-staged/src/main.rs, crates/fast-staged/src/lib.rs]
tech_stack:
  added: []
  patterns: ["std::env::args() minimal flag detection (pre-clap)", "conditional branch between staged_files() and unstaged_files()"]
key_files:
  created: []
  modified:
    - crates/fast-staged/src/main.rs
    - crates/fast-staged/src/lib.rs
    - crates/fast-staged/tests/integration_test.rs
    - crates/fast-staged/src/config/discover.rs
decisions:
  - "Use std::env::args().any() for flag detection, not clap — Phase 6 adds full CLI parsing per plan constraint"
  - "Skip backup()/cleanup() calls when --unstaged is active — they target the HEAD-vs-index diff, irrelevant for worktree-only mode"
  - "unstaged_files() empty result is NOT an error (unlike staged_files()) — matches git workflow expectation of clean worktree being valid"
metrics:
  duration_minutes: 4.4
  completed: "2026-04-24"
  tests_added: 2
  total_tests: 74
---

# Phase 04 Plan 03: --unstaged Flag Wiring Summary

**One-liner:** Wire the --unstaged CLI flag through `main.rs` to `lib.rs::run()` so it branches between `staged_files()` and `unstaged_files()`, closing the GIT-05 verification gap with a minimal 3-file change.

## What Was Built

- `main.rs` now detects the `--unstaged` flag via `std::env::args().any(|a| a == "--unstaged")` and passes a bool to `fast_staged::run(unstaged)`.
- `lib.rs::run()` signature changed to `pub async fn run(unstaged: bool) -> Result<(), AppError>`.
- `run()` conditionally calls `git_ctx.unstaged_files()?` when the flag is set, otherwise `git_ctx.staged_files()?`.
- In `--unstaged` mode, `backup()` and `cleanup()` are skipped — those operate on the HEAD-vs-index diff which is irrelevant when targeting worktree-only changes.
- Debug output label is now conditional: "Unstaged files: N" vs "Staged files: N" — this label is the primary signal that the flag was honored.
- Two new integration tests: one proves `--unstaged` flows through to `unstaged_files()`, the other guards against regression of the default staged path.

## Verification Evidence

| Check | Result |
| ----- | ------ |
| `cargo test` | 74 passed, 0 failed (67 unit + 7 integration + 0 doc) |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt -- --check` | clean |
| `grep '\-\-unstaged' crates/fast-staged/src/main.rs` | match at line 6 |
| `grep 'unstaged_files' crates/fast-staged/src/lib.rs` | 2 matches (comment + call) |
| `grep 'run(unstaged' crates/fast-staged/src/main.rs` | match at line 7 |
| `grep 'test_binary_with_unstaged_flag' tests/integration_test.rs` | match at line 208 |

## Commits

| Phase | Commit | Message |
| ----- | ------ | ------- |
| RED | `ed298cc` | test(04-03): add failing tests for --unstaged flag wiring |
| GREEN | `dc2b058` | feat(04-03): wire --unstaged flag through run() to unstaged_files() |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing clippy needless-borrow in discover.rs:277**

- **Found during:** Clippy gate on Task 1 (GREEN verification)
- **Issue:** `cargo clippy -- -D warnings` failed on `&path.parent().unwrap()` in a test helper added during Phase 02-02. Failure predates this plan (confirmed by `git stash` + clippy on clean baseline) — caused by a newer clippy version promoting `needless_borrow` to warning.
- **Why fixed here:** The plan's acceptance criteria include `cargo clippy -- -D warnings passes clean`. Without this one-character fix, the acceptance gate could not be met. Single-line change, zero behavioral impact.
- **Fix:** Removed the leading `&` — `path.parent().unwrap()` is already `Option<&Path>`.
- **Files modified:** `crates/fast-staged/src/config/discover.rs`
- **Commit:** `dc2b058` (folded into GREEN commit)

### Intentional Test Strengthening

The plan suggested the integration test assert on the filename appearing in stderr. Direct observation showed `tracked.txt` appears in stderr in BOTH modes after `git commit` (committed files remain in the index at stage-0). To make the test discriminate between modes, the assertion was strengthened to require the literal label `Unstaged files` — which only appears when `--unstaged` actually triggered the `unstaged_files()` branch. This is a stronger proof of correctness than the plan's original suggestion.

## Known Stubs

None. Per plan scope, Phase 6 will replace `std::env::args().any()` with `clap`-based parsing — this is documented in the `main.rs` comment (`// Minimal flag detection — Phase 6 replaces this with full clap parsing.`) and is not a stub but a deliberate phase boundary.

## Self-Check: PASSED

- FOUND: `crates/fast-staged/src/main.rs` (modified)
- FOUND: `crates/fast-staged/src/lib.rs` (modified)
- FOUND: `crates/fast-staged/tests/integration_test.rs` (modified, 2 new tests)
- FOUND: `crates/fast-staged/src/config/discover.rs` (modified, clippy fix)
- FOUND: commit `ed298cc` (RED: failing tests)
- FOUND: commit `dc2b058` (GREEN: implementation + clippy fix)
- All 74 tests pass.
- Clippy and fmt gates clean.
- GIT-05 requirement satisfied end-to-end.
