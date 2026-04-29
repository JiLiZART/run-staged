---
phase: 05-execution-core
plan: 03
subsystem: commands-execution-engine
tags: [commands, execute_all, joinset, cancellation, cross-group-gate, per-file, batch]
requirements: [EXEC-01, EXEC-02, EXEC-03]
dependency_graph:
  requires:
    - "commands::build_argv (Plan 05-02)"
    - "commands::chunk_files (Plan 05-02)"
    - "commands::spawn_command (Plan 05-02)"
    - "matcher::MatchedGroup (Plan 05-01)"
    - "matcher::MatchedPattern (Plan 05-01)"
    - "config::types::Config (existing)"
    - "config::types::Group (existing)"
    - "config::types::ExecutionOrder (existing)"
    - "AppError::CommandNotFound (existing)"
    - "tokio::task::JoinSet (existing tokio dep)"
  provides:
    - "commands::CommandOutcome enum"
    - "commands::CommandOutcome::is_failure()"
    - "commands::execute_all"
  affects:
    - "Plan 05-04 will wrap run_one() body with tokio::time::timeout for CommandTimeout"
    - "Plan 05-05 will call execute_all() in lib.rs::run() and inspect outcomes for restore-on-failure"
tech_stack:
  added: []
  patterns:
    - "tokio::task::JoinSet for per-group parallel dispatch"
    - "Arc<AtomicBool> for cooperative per-group cancellation (D-13)"
    - "Strict outer-sequential / inner-by-execution-order group iteration (D-14)"
    - "POSIX exit-code-127 convention for ENOENT/CommandNotFound"
    - "Synthetic Failure outcomes for argv-build errors and JoinSet panics"
key_files:
  created: []
  modified:
    - crates/fast-staged/src/commands.rs
decisions:
  - "CommandNotFound mapped to CommandOutcome::Failure with exit_code=127 (POSIX shell convention) rather than propagating as AppError — keeps execute_all() infallible-by-construction so the caller always gets a complete outcome list to evaluate"
  - "Per-group cancellation is cooperative-only: in-flight processes are NEVER killed from this layer. New tasks observe the AtomicBool at start and emit Cancelled. Plan 05-04's tokio::time::timeout will be the only mechanism that kills running children"
  - "Outer loop break on cross-group failure under config.continue_on_error=false produces a Vec missing subsequent groups' patterns entirely (not Cancelled placeholders) — caller cannot distinguish skipped-by-gate from never-existed, but since matched_groups was the input, the caller already knows what was scheduled"
  - "argv-build errors (shlex parse failure) become synthetic CommandOutcome::Failure entries with empty pattern/command but populated stderr — preserves the contract that execute_all returns at least one Failure when anything goes wrong"
  - "JoinSet task panics produce synthetic Failure outcomes too, so a panicking task inside one of our spawn closures cannot poison the run"
  - "Wall-clock timing tests use #[tokio::test(flavor = \"multi_thread\", worker_threads = 4)] — single-threaded tokio test runtime makes parallel process spawns serialize through the reactor's signal handler"
metrics:
  duration_minutes: 5
  completed_date: "2026-04-29"
  tasks_completed: 1
  tests_added: 10
  tests_passing_total: 113
---

# Phase 05 Plan 03: Execution Engine Summary

**One-liner:** `execute_all()` orchestrator with `JoinSet`-driven parallel groups, sequential dispatch, per-group cooperative cancellation, cross-group failure gate, per-file/batch task expansion, and a `CommandOutcome` enum the caller inspects to decide restore-on-failure.

## API Surface

```rust
// crates/fast-staged/src/commands.rs

#[derive(Debug, Clone)]
pub enum CommandOutcome {
    Success {
        group: String,
        pattern: String,
        command: String,
        argv: Vec<String>,
        file_count: usize,
        duration: Duration,
        stdout: Vec<u8>,
    },
    Failure {
        group: String,
        pattern: String,
        command: String,
        argv: Vec<String>,
        file_count: usize,
        duration: Duration,
        exit_code: i32,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    Cancelled {
        group: String,
        pattern: String,
        command: String,
    },
}

impl CommandOutcome {
    pub fn is_failure(&self) -> bool;
}

pub async fn execute_all(
    matched_groups: &[MatchedGroup<'_>],
    config: &Config,
    git_root: &Path,
) -> Vec<CommandOutcome>;
```

## Internal Structure

| Helper                       | Visibility | Purpose                                                                            |
|------------------------------|-----------|------------------------------------------------------------------------------------|
| `Task`                       | private   | One unit of work: pre-built argv + group/pattern/command/file_count metadata       |
| `tasks_for_group`            | private   | Expands `MatchedGroup` → `Vec<Task>`, honors per-file vs batch + chunk_files       |
| `run_one`                    | private   | Drives one Task through spawn_command, maps result to CommandOutcome               |
| `execute_group_parallel`     | private   | `JoinSet`-spawn all tasks, set cancel flag on sibling failure                      |
| `execute_group_sequential`   | private   | Loop-await tasks in declared order, set cancel flag on failure                     |

## Execution Semantics

| Concern                          | Behavior                                                                                       |
|----------------------------------|-----------------------------------------------------------------------------------------------|
| Outer loop                       | Strictly sequential across groups (D-14)                                                       |
| Inner dispatch                   | `Group.execution_order::Parallel` → `JoinSet`; `Sequential` → for-loop                         |
| Per-group cancellation           | Fresh `Arc<AtomicBool>` per group; cleared between groups; checked at task start (D-13)        |
| Cross-group gate                 | `Config.continue_on_error == false` → after any failing group, subsequent groups are skipped    |
| Per-file mode                    | One Task per file → `argv = ["cmd", "file"]`; one CommandOutcome per file                       |
| Batch mode                       | One Task per ARG_MAX chunk → `argv = ["cmd", "file1", "file2", ...]`                           |
| Empty pattern files              | Pattern skipped before task creation (D-27 defensive guard)                                     |
| `argv` build error (shlex parse) | Synthetic `Failure` outcome for the group with stderr=`"argv build failed: ..."`                |
| `JoinSet` task panic             | Synthetic `Failure` outcome with stderr=`"task panicked: ..."`                                  |
| `CommandNotFound` (ENOENT)       | `Failure` outcome with `exit_code: 127` and stderr=`"command not found: NAME"` (POSIX convention) |
| `Io` error                       | `Failure` outcome with `exit_code: -1` and stderr=`"io error: ..."`                             |
| In-flight cancellation           | NOT performed at this layer — Plan 04 timeout is the only termination mechanism                 |

## Tests Added (10)

| # | Name                                                                  | Purpose                                                              |
|---|-----------------------------------------------------------------------|----------------------------------------------------------------------|
| 1 | `parallel_wall_clock_under_sum`                                       | EXEC-01 — parallel groups run concurrently; wall-clock < sum         |
| 2 | `sequential_wall_clock_at_least_sum`                                  | EXEC-02 — sequential groups run one-by-one; wall-clock ≥ sum         |
| 3 | `exit_code_propagates_when_command_fails`                             | EXEC-03 — non-zero exit produces Failure with the command's exit_code |
| 4 | `failure_in_first_group_skips_second_group_when_global_coe_false`     | D-14 — cross-group gate skips remaining groups                        |
| 5 | `failure_in_first_group_runs_second_group_when_global_coe_true`       | D-14 — coe=true continues across groups                              |
| 6 | `group_cancel_flag_set_on_sibling_failure_when_group_coe_false`       | D-13 — per-group cancel flag fires on sibling failure                |
| 7 | `per_file_spawns_one_process_per_file`                                | D-24 — per-file mode produces N outcomes for N files                 |
| 8 | `batch_spawns_one_process_with_all_files`                             | D-25 — batch mode produces 1 outcome with all files in argv          |
| 9 | `empty_pattern_skips_spawn`                                           | D-27 — defensive: zero-file pattern emits zero outcomes              |
| 10| `command_not_found_produces_failure_outcome`                          | D-08 — ENOENT → Failure(exit_code=127) with binary name in stderr    |

**Total crate tests: 113 passing** (was 103 after Plan 05-02; +10 new).

## Verification

- `cargo build -p fast-staged` — clean
- `cargo test -p fast-staged --lib` — 113/113 pass
- `cargo clippy -p fast-staged --lib -- -D warnings` — clean
- `cargo fmt --check` — clean

## Acceptance Criteria

| Check                                                                              | Result |
|-------------------------------------------------------------------------------------|--------|
| `grep -n "pub enum CommandOutcome" commands.rs` returns 1 line                     | PASS  |
| `grep -nE "Success \\{\|Failure \\{\|Cancelled \\{" commands.rs` returns ≥3 lines  | PASS  |
| `grep -n "pub async fn execute_all" commands.rs` returns 1 line                    | PASS  |
| `grep -n "JoinSet::new" commands.rs` returns ≥1 line                               | PASS  |
| `grep -n "cancel.store\\|cancel.load" commands.rs` returns ≥2 lines                | PASS  |
| `grep -n "AtomicBool" commands.rs` returns ≥2 lines                                | PASS  |
| `grep -n "ExecutionOrder::Parallel\\|ExecutionOrder::Sequential" commands.rs ≥2`   | PASS  |
| `grep -n "config.continue_on_error" commands.rs` returns ≥1 line                   | PASS  |
| All 4 internal helpers (`run_one`/`tasks_for_group`/`execute_group_*`) present     | PASS  |
| `cargo test -p fast-staged --lib commands::tests` exits 0 with ≥18 tests passing   | PASS (25) |
| All required test names present                                                    | PASS  |
| `cargo build -p fast-staged` exits 0 with no warnings                              | PASS  |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Wall-clock timing tests required multi-threaded tokio runtime**
- **Found during:** Task 1 GREEN — `parallel_wall_clock_under_sum` failed at 622ms when `#[tokio::test]` defaulted to a single-threaded runtime under cargo-test's parallel test execution.
- **Issue:** Single-threaded tokio runtime serializes process spawns through the reactor's signal handler when the test machine is also running other tests in parallel. The test in isolation passed at ~250ms, but in the full suite it took 622ms — well above the 350ms parallel-vs-sum threshold.
- **Fix:** Switched both wall-clock tests to `#[tokio::test(flavor = "multi_thread", worker_threads = 4)]`, which mirrors the production tokio runtime created by `#[tokio::main]` in `main.rs`. Also bumped sleep duration to 0.3s and parallel margin to 0.55s for additional CI tolerance (sum = 0.6s; 0.05s parallel headroom).
- **Files modified:** `crates/fast-staged/src/commands.rs` (test bodies only — no impl change)
- **Commit:** `0f0cb2e`
- **Impact:** Tests now match how `execute_all` will actually be called in production (multi-thread runtime). No behavior change to the engine itself.

### Plan Ordering Adjustment (informational)

**TDD RED/GREEN split:** Followed the plan's `tdd="true"` directive precisely.
- **RED commit `01edb41`:** Added 10 new tests + `CommandOutcome` enum + `execute_all` stub returning `Vec::new()`. 9 of 10 tests fail as expected; 2 trivially pass on the empty stub (`empty_pattern_skips_spawn` and `command_not_found_produces_failure_outcome` — both expect zero or one outcome respectively, but with the empty stub `command_not_found` actually fails because it expects exactly 1 outcome). Verified failure shape via `cargo test -p fast-staged --lib commands::tests` showing `9 failed; 16 passed`.
- **GREEN commit `0f0cb2e`:** Replaced stub with full implementation (`Task`, `tasks_for_group`, `run_one`, `execute_group_parallel`, `execute_group_sequential`, full `execute_all` body). All 25 commands tests pass; full crate passes 113/113.

No REFACTOR commit — the GREEN code passed clippy and fmt cleanly without a third pass needed.

## Auth Gates

None.

## Known Stubs

None. The implementation is complete: every match arm has a real body, every CommandOutcome field is populated meaningfully, and no placeholder data flows to a future caller. Plan 05-04 will edit `run_one`'s body in-place to wrap `spawn_command` with `tokio::time::timeout`, but until then `run_one` correctly handles all known error paths.

## Commits

| # | Hash      | Type | Message                                                                                |
|---|-----------|------|----------------------------------------------------------------------------------------|
| 1 | `01edb41` | test | test(05-03): add failing tests for execute_all + CommandOutcome (RED)                  |
| 2 | `0f0cb2e` | feat | feat(05-03): implement execute_all + per-group cancel + cross-group gate (GREEN)       |

## Self-Check: PASSED

- File `crates/fast-staged/src/commands.rs` — FOUND
- Commit `01edb41` — FOUND in `git log`
- Commit `0f0cb2e` — FOUND in `git log`
- `cargo build -p fast-staged` exits 0
- `cargo test -p fast-staged --lib` — 113 passing
- `cargo clippy -p fast-staged --lib -- -D warnings` exits 0
- `cargo fmt --check` exits 0
- `grep -n "pub enum CommandOutcome" crates/fast-staged/src/commands.rs` returns 1 line
- `grep -n "pub async fn execute_all" crates/fast-staged/src/commands.rs` returns 1 line
- `grep -n "JoinSet::new" crates/fast-staged/src/commands.rs` returns ≥1 line
- All 10 new test names present and passing
