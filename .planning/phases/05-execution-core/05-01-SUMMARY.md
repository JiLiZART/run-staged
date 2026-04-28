---
phase: 05-execution-core
plan: 01
subsystem: matcher
tags: [matcher, globset, matchbase, negation, case-sensitive]
requirements: [EXEC-06, EXEC-07]
dependency_graph:
  requires:
    - "config::types::Config"
    - "config::types::Group"
    - "AppError::ConfigInvalid"
    - "AppError::NoFilesMatched"
    - "globset 0.4"
  provides:
    - "matcher::match_files"
    - "matcher::MatchedGroup"
    - "matcher::MatchedPattern"
  affects:
    - "Plan 05-05 will consume Vec<MatchedGroup> in run() to drive executor"
    - "Plan 05-05 wires --allow-empty to suppress AppError::NoFilesMatched"
tech_stack:
  added: []
  patterns:
    - "matchBase glob normalization (auto-prefix **/ for slashless patterns)"
    - "negation via !-prefix with separate include/exclude GlobSets"
    - "GlobBuilder::case_insensitive(false) for case-sensitive matching default"
key_files:
  created:
    - crates/fast-staged/src/matcher.rs
  modified:
    - crates/fast-staged/src/lib.rs
decisions:
  - "Each HashMap key in Group.patterns is one pattern string; include + negation per group requires two separate keys (D-03 honored)"
  - "Pattern with zero matches is skipped at pattern level (D-27); group with no surviving patterns is skipped at group level"
  - "ConfigInvalid raised when a single pattern entry is negation-only (Pitfall 4 from research)"
metrics:
  duration_minutes: 4
  completed_date: "2026-04-28"
  tasks_completed: 2
  tests_added: 10
  tests_passing_total: 77
---

# Phase 05 Plan 01: Glob Matcher Summary

**One-liner:** Pure-function glob matcher with matchBase auto-recursion, negation, case-sensitive defaults, returning structured `Vec<MatchedGroup>` for the executor.

## API Surface

```rust
// crates/fast-staged/src/matcher.rs

#[derive(Debug, Clone)]
pub struct MatchedPattern {
    pub pattern: String,         // original pattern from config
    pub commands: Vec<String>,   // command strings to run
    pub files: Vec<PathBuf>,     // files matching this pattern
}

#[derive(Debug, Clone)]
pub struct MatchedGroup<'a> {
    pub group: &'a Group,                // borrowed for execution settings
    pub patterns: Vec<MatchedPattern>,   // only patterns with >=1 file (D-27)
}

pub fn match_files<'a>(
    config: &'a Config,
    files: &[PathBuf],
) -> Result<Vec<MatchedGroup<'a>>, AppError>;
```

## matchBase Rules (D-02 / D-03 / D-04)

| Pattern         | Behavior                                                          |
|-----------------|-------------------------------------------------------------------|
| `*.ts`          | matchBase: matches `top.ts`, `src/foo/bar.ts`, `deep/nest/x.ts`   |
| `src/*.ts`      | Literal: matches `src/bar.ts`, NOT `nested/src/bar.ts`            |
| `!*.snap`       | Negation: excludes paths whose basename ends in `.snap`           |
| `*.TS`          | Case-sensitive: does NOT match `foo.ts`                           |
| `[unclosed`     | `AppError::ConfigInvalid` with offending pattern in message       |

Implementation detail: per pattern, helper `build_globsets` strips a leading `!`, then auto-prefixes `**/` to bodies containing no `/`, then builds with `GlobBuilder::case_insensitive(false)`.

## Empty-match Semantics

- **Per pattern (D-27):** zero matches → that pattern is skipped (no `MatchedPattern` emitted). Tools like `eslint` invoked with empty args lint the whole repo, which is catastrophic — skipping is mandatory.
- **Per group:** if no surviving patterns, the group is omitted entirely.
- **Globally:** if zero patterns match across ALL groups → `Err(AppError::NoFilesMatched)`. Plan 05-05 will suppress this under `--allow-empty` (D-21).

## Negation-only Pattern Rejection

A single HashMap entry whose key is just `!something` (no include) returns `Err(AppError::ConfigInvalid)` mentioning the group name and pattern. This catches Pitfall 4 from the research: an all-negation group would compile to an empty include set that matches nothing silently. Surfacing the error early is more user-friendly. Users wanting include + negation must declare two HashMap entries (one include, one `!`-prefixed).

## Tests Added (10)

1. `matchbase_slashless_pattern_recurses` — `*.ts` matches at any depth
2. `slashed_pattern_is_literal` — `src/*.ts` does not auto-recurse
3. `negation_excludes_matched_file` — `!*.snap` excludes `.snap` files (verified at globset layer with both keys present in same set)
4. `case_sensitive_default` — `*.TS` ≠ `foo.ts`; `*.ts` ≠ `FOO.TS`
5. `multiple_patterns_collect_all_matches` — group with `*.ts` + `*.rs` produces two `MatchedPattern` entries
6. `pattern_with_zero_matches_is_skipped_at_pattern_level` — D-27 enforcement
7. `match_files_returns_no_files_matched_when_zero_matches_globally` — `Err(NoFilesMatched)` when all groups empty
8. `match_files_returns_ok_when_at_least_one_pattern_matches` — happy path
9. `only_negations_in_group_returns_config_invalid` — Pitfall 4 guard
10. `invalid_glob_syntax_returns_config_invalid` — `[unclosed` rejected with pattern in message

All 10 pass. Crate test suite total: **77 passing** (67 prior + 10 new).

## Verification

- `cargo test -p fast-staged --lib matcher::tests` — 10/10 pass
- `cargo build -p fast-staged` — clean build
- `cargo clippy -p fast-staged --lib -- -D warnings` — clean
- `cargo fmt --check` — clean

## Deferred to Plan 05-05

- `--allow-empty` flag suppression of `AppError::NoFilesMatched` (D-21).
- Wiring `match_files()` into `lib.rs::run()` between `git_ctx.backup()` and the existing debug-print block.
- Calling matcher with `config` and the staged/unstaged file list returned by `git_ctx`.

## Deviations from Plan

### Auto-fixed Issues

None — all deviations are scope-only ordering adjustments (below).

### Plan Ordering Adjustment (informational)

**Issue:** Task 2 ("register `pub mod matcher;` in lib.rs") had to happen before Task 1's TDD RED tests could be reached by `cargo test --lib matcher::tests`. The Rust crate model requires the module to be declared in `lib.rs` for tests inside `matcher.rs` to compile and run.

**Resolution:** The `pub mod matcher;` line was added to `lib.rs` as part of the RED commit (`test(05-01): add failing tests for glob matcher (RED)`). Task 2 reflects state that already exists; no separate commit was needed. Final acceptance check `grep -n "pub mod matcher;" crates/fast-staged/src/lib.rs` returns the expected line.

**Impact:** None. Plan 05-02/03/04/05 all see the registered module as expected.

## Commits

| # | Hash      | Type | Message                                                            |
|---|-----------|------|--------------------------------------------------------------------|
| 1 | `03e00dc` | test | test(05-01): add failing tests for glob matcher (RED)              |
| 2 | `f9bd6f9` | feat | feat(05-01): implement glob matcher with matchBase + negation (GREEN) |

## Self-Check: PASSED

- File `crates/fast-staged/src/matcher.rs` — FOUND
- File `crates/fast-staged/src/lib.rs` updated with `pub mod matcher;` — FOUND
- Commit `03e00dc` — FOUND in `git log`
- Commit `f9bd6f9` — FOUND in `git log`
- All 10 acceptance test names present in matcher.rs
- All 10 tests pass under `cargo test -p fast-staged --lib matcher::tests`
- `cargo clippy -p fast-staged --lib -- -D warnings` exits 0
- `cargo fmt --check` exits 0
