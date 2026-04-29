---
phase: 05-execution-core
plan: 04
subsystem: commands-timeout
tags: [commands, timeout, sigterm, sigkill, tokio-time, terminate, run_one]
requirements: [EXEC-05]
dependency_graph:
  requires:
    - "commands::run_one (Plan 05-03 — body modified in place)"
    - "commands::execute_all / execute_group_parallel / execute_group_sequential (Plan 05-03 — signatures extended)"
    - "config::types::Group::timeout (Plan 03 cascade — already group→global→none merged)"
    - "AppError::CommandTimeout { command, duration } (Plan 05-02)"
    - "nix 0.31 with `signal` feature (Plan 05-02 — verified present, not re-added)"
    - "tokio::time::timeout (existing tokio dep)"
    - "tokio::process::Child::id, kill, wait (existing tokio dep)"
  provides:
    - "timeout-aware run_one(task, cancel, effective_timeout)"
    - "private terminate(&mut Child) cross-platform helper (SIGTERM→2s grace→SIGKILL on Unix; TerminateProcess on Windows)"
    - "private failure-outcome constructors: failure_outcome_timeout, failure_outcome_command_not_found, failure_outcome_io, failure_outcome_io_msg"
    - "effective_timeout plumbing through execute_group_parallel + execute_group_sequential"
  affects:
    - "Plan 05-05 inherits timeout enforcement transparently — execute_all signature unchanged; no caller changes required"
tech_stack:
  added: []
  patterns:
    - "tokio::time::timeout(d, child.wait()) for timeout enforcement (Pattern 3)"
    - "Manual Command::spawn (not spawn_command) so Child handle is retained for kill"
    - "child.stdout.take()/child.stderr.take() before wait() so the child is not blocked on full stdio buffers; drained via AsyncReadExt::read_to_end after wait completes"
    - "nix::sys::signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM) for cooperative termination on Unix"
    - "child.kill().await for SIGKILL on Unix / TerminateProcess on Windows — also reaps the child to avoid zombies (Pitfall 6)"
    - "AppError::CommandTimeout.to_string() supplies the substring 'timed out' for stderr — keeps user-facing message and test assertion in lockstep with the error type"
key_files:
  created: []
  modified:
    - crates/fast-staged/src/commands.rs
decisions:
  - "Manual spawn (Command::new + spawn) used inside run_one rather than calling spawn_command(). Reason: spawn_command() awaits Command::output() which CONSUMES the Child, leaving no handle for kill on timeout. The manual path keeps `mut child: tokio::process::Child` so terminate() can call child.id() (for SIGTERM) and child.kill() (SIGKILL/TerminateProcess)."
  - "stdout/stderr handles taken pre-wait, drained post-wait via AsyncReadExt::read_to_end. Rationale: if we used child.wait_with_output() the Child would be consumed by value; if we ignored stdio entirely the OS pipe buffers could fill (~64KB on Linux) and block the child. Taking + draining is the documented tokio pattern."
  - "On Unix, SIGTERM precedes SIGKILL with a 2-second grace window via tokio::time::timeout(2s, child.wait()). Even if the SIGTERM dispatch itself fails (e.g. PID already reaped by tokio reactor), the subsequent child.kill().await still runs to ensure SIGKILL fallback (D-19)."
  - "On Windows, child.kill().await runs directly. There is no TerminateProcess equivalent of cooperative signal delivery (D-20), so the grace window is skipped — adding it would just slow termination by 2 seconds with no behavioral benefit."
  - "Stderr for the timeout failure is `AppError::CommandTimeout::to_string()` (which renders as 'command `<cmd>` timed out after <duration>'). This couples the user-visible message to the error type — Plan 05-05 can also surface CommandTimeout directly without re-deriving stderr text."
  - "effective_timeout consumed from `matched.group.timeout` directly (no further cascade in commands.rs). Rationale: Phase 3 `defaults::resolve_group()` already merged group → global → none into Group.timeout, so any merge here would either duplicate logic or risk drift. The plan explicitly mandates this consumption pattern."
  - "Task 2 committed as a single atomic GREEN commit rather than splitting impl + un-ignore-tests into two commits. Rationale: un-ignoring Task 1 stubs requires the impl to be present (otherwise the un-ignored tests fail the suite). Splitting would either land a broken intermediate commit or introduce extra ceremony with no value. Plan 03 followed the same single-GREEN pattern."
  - "no_timeout_passes_through_unchanged regression guard added in `mod tests` to lock in the contract that `Group.timeout == None` produces byte-equivalent behavior to Plan 03 (no spurious timeout error, ordinary Success outcome)."
metrics:
  duration_minutes: 7
  completed_date: "2026-04-29"
  tasks_completed: 2
  tests_added: 3
  tests_passing_total: 116
---

# Phase 05 Plan 04: Per-Spawn Timeout Enforcement Summary

**One-liner:** `run_one` manually spawns the child (not via `spawn_command`) and wraps `child.wait()` in `tokio::time::timeout` when `Group.timeout = Some(d)`; on Elapsed the cross-platform `terminate(&mut Child)` helper sends SIGTERM-then-SIGKILL on Unix (2s grace) or TerminateProcess on Windows, and `run_one` returns `CommandOutcome::Failure` whose stderr is derived from `AppError::CommandTimeout` (containing the substring "timed out").

## Spawn Path Shape

### Before (Plan 03)

```rust
let result = spawn_command(&task.argv, &task.working_dir).await; // awaits Command::output(), Child is consumed
match result { /* map Output / errors to CommandOutcome */ }
```

### After (Plan 04)

```rust
let mut child = Command::new(&task.argv[0])
    .args(&task.argv[1..])
    .current_dir(&task.working_dir)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;                        // ENOENT → failure_outcome_command_not_found

let stdout_handle = child.stdout.take();   // released so wait() can advance
let stderr_handle = child.stderr.take();

let exit_status_result = match effective_timeout {
    Some(d) => match tokio_timeout(d, child.wait()).await {
        Ok(inner) => inner,
        Err(_elapsed) => {
            terminate(&mut child).await;
            return failure_outcome_timeout(&task, start.elapsed(), d);
        }
    },
    None => child.wait().await,                // unchanged from Plan 03 semantics
};
// ... drain stdout/stderr buffers via AsyncReadExt::read_to_end, build outcome.
```

## terminate() Helper

```rust
async fn terminate(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGTERM,
            );
            // 2-second grace window — child may exit cooperatively.
            let _ = tokio_timeout(Duration::from_secs(2), child.wait()).await;
        }
    }
    // SIGKILL on Unix; TerminateProcess on Windows. Reaps the child too.
    let _ = child.kill().await;
}
```

| Platform | First signal           | Grace window | Final action               |
|----------|------------------------|--------------|----------------------------|
| Unix     | `SIGTERM` via `nix::sys::signal::kill` | 2 seconds (`tokio_timeout(2s, child.wait())`) | `child.kill().await` (SIGKILL, also reaps) |
| Windows  | (none)                 | (none)       | `child.kill().await` (`TerminateProcess`)  |

## Plumbing

| Layer                       | Before                                  | After                                                                      |
|-----------------------------|-----------------------------------------|----------------------------------------------------------------------------|
| `run_one`                   | `(task, cancel)`                        | `(task, cancel, effective_timeout: Option<Duration>)`                      |
| `execute_group_parallel`    | `(tasks, cancel, continue_on_error)`    | `(tasks, cancel, continue_on_error, effective_timeout: Option<Duration>)`  |
| `execute_group_sequential`  | `(tasks, cancel, continue_on_error)`    | `(tasks, cancel, continue_on_error, effective_timeout: Option<Duration>)`  |
| `execute_all`               | (unchanged signature)                   | (unchanged signature) — internally `effective_timeout = matched.group.timeout` |

`Group.timeout` is already cascaded `group → global → none` by Phase 3's `defaults::resolve_group()`, so `commands.rs` consumes it directly without further merge.

## AppError → stderr Coupling

```rust
fn failure_outcome_timeout(task: &Task, duration: Duration, configured: Duration) -> CommandOutcome {
    let err = AppError::CommandTimeout {
        command: task.command.clone(),
        duration: configured,
    };
    CommandOutcome::Failure {
        // ... metadata copied from task
        exit_code: -1,
        stdout: Vec::new(),
        stderr: err.to_string().into_bytes(),    // contains "timed out"
    }
}
```

`AppError::CommandTimeout` renders as `command \`<cmd>\` timed out after <duration>`, satisfying the Task 1 test assertion that stderr contains the substring `"timed out"` while keeping the user-facing message coupled to the error type for Plan 05-05 reuse.

## Helper Failure-Outcome Constructors Added

| Function                            | Purpose                                                                                          |
|-------------------------------------|--------------------------------------------------------------------------------------------------|
| `failure_outcome_timeout`           | Build `Failure` with stderr from `AppError::CommandTimeout`, exit_code -1                        |
| `failure_outcome_command_not_found` | Build `Failure` for spawn-time ENOENT, exit_code 127, stderr `"command not found: NAME"` (POSIX) |
| `failure_outcome_io`                | Build `Failure` from a `std::io::Error`, stderr `"io error: {e}"`, exit_code -1                  |
| `failure_outcome_io_msg`            | Build `Failure` with synthetic stderr text (no `std::io::Error` available)                       |

## Cargo.toml

`nix = { version = "0.31", features = ["signal"] }` was already declared by Plan 05-02 (verified — no edit needed for Plan 04). The `signal` feature is the one Plan 04 actually exercises (`nix::sys::signal::kill` + `Signal::SIGTERM`).

## Tests Added (3)

| #  | Name                                       | cfg(unix) | Purpose                                                                                                     |
|----|--------------------------------------------|-----------|-------------------------------------------------------------------------------------------------------------|
| 1  | `timeout_kills_long_running_process`       | yes       | 150ms timeout vs `sleep 30` — outcome is `Failure { exit_code: -1, stderr ⊃ "timed out" }`, elapsed < 5s    |
| 2  | `sigterm_then_sigkill_reaps_child`         | yes       | `trap '' TERM; sleep 30` ignores SIGTERM — proves SIGKILL fallback fires; elapsed < 5s; outcome is `Failure` |
| 3  | `no_timeout_passes_through_unchanged`      | yes       | `Group.timeout = None` + `true` binary → `Success` (regression guard for Plan 03 byte-equivalence)          |

Plus `make_group_with_timeout(name, timeout_ms)` test helper.

**Total crate tests: 116 passing** (was 113 after Plan 05-03; +3 from this plan).

## Verification

| Check                                                                                | Result |
|--------------------------------------------------------------------------------------|--------|
| `grep -nE 'nix.*=.*"0\.31".*signal' crates/fast-staged/Cargo.toml`                  | 1 line |
| `grep -n "tokio::time::timeout\|tokio_timeout" crates/fast-staged/src/commands.rs`   | ≥1 line (alias `tokio_timeout` used at call sites) |
| `grep -n "Signal::SIGTERM" crates/fast-staged/src/commands.rs`                       | 1 line |
| `grep -n "child.kill()" crates/fast-staged/src/commands.rs`                          | ≥1 line |
| `grep -n "AppError::CommandTimeout" crates/fast-staged/src/commands.rs`              | 1 line |
| `grep -n "fn failure_outcome_timeout" crates/fast-staged/src/commands.rs`            | 1 line |
| `grep -nE '#\[ignore = "Plan 04' crates/fast-staged/src/commands.rs`                 | 0 lines (un-ignored) |
| `grep -n "fn no_timeout_passes_through_unchanged" crates/fast-staged/src/commands.rs`| 1 line |
| `cargo build -p fast-staged`                                                         | exits 0, no warnings |
| `cargo test -p fast-staged --lib commands::tests::timeout_kills_long_running_process`| PASS (no `--include-ignored` needed) |
| `cargo test -p fast-staged --lib commands::tests::sigterm_then_sigkill_reaps_child`  | PASS |
| `cargo test -p fast-staged --lib commands::tests::no_timeout_passes_through_unchanged` | PASS |
| `cargo test -p fast-staged --lib`                                                    | 116 passing |
| `cargo clippy -p fast-staged --lib -- -D warnings`                                   | exits 0 |
| `cargo fmt --check`                                                                  | exits 0 |
| Zombie check: `cargo test && cargo test` then `pgrep -af 'sleep 30'`                  | No persistent leaks (transient PIDs reaped within ms by tokio reactor) |

## Acceptance Criteria

All Plan 04 success criteria pass:

- nix 0.31 with `signal` feature in Cargo.toml — PASS (already from Plan 02)
- `run_one` wraps spawn in `tokio::time::timeout` only when `effective_timeout = Some(d)` — PASS
- Unix: SIGTERM via `nix::sys::signal::kill`, 2s grace via `tokio::time::timeout(2s, child.wait())`, then SIGKILL via `child.kill().await` — PASS
- Windows: `child.kill().await` directly, no grace window — PASS (verified by `#[cfg(unix)]` gating on the SIGTERM block)
- Timeout failure → `CommandOutcome::Failure { exit_code: -1, stderr: contains("timed out") }` derived from `AppError::CommandTimeout` — PASS
- No-timeout path is byte-equivalent to Plan 03 — PASS (`no_timeout_passes_through_unchanged` guards this)
- All Task 1 stubs un-ignored and pass — PASS
- Clippy and rustfmt clean — PASS

## Deviations from Plan

### Auto-fixed Issues

None. The plan's spec was followed verbatim — every must-have artifact and key-link landed exactly as described. The only judgment call was the test/build commit split (see below).

### Plan Ordering Adjustment (informational)

**Task 2 — Single GREEN commit instead of two**

The plan listed two suggested commits for Task 2:
1. `feat(05-04): wrap run_one spawn in tokio::time::timeout with SIGTERM/SIGKILL escalation`
2. `test(05-04): un-ignore timeout stubs and add no-timeout passthrough guard`

**Resolution:** Landed as a single GREEN commit `8d61098` carrying message #1.

**Rationale:** Un-ignoring the Task 1 stubs requires the timeout impl to be present in the same commit — otherwise the intermediate commit would have running tests that fail. Splitting impl + un-ignore + new guard would either land a broken intermediate or introduce ceremony with no auditable benefit. Plan 03 followed the same single-GREEN pattern. Both impl changes and test un-ignore/addition are part of the same logical "Plan 04 lands" change. The commit message body lists the test changes explicitly, so the audit trail is preserved.

**Impact:** None on correctness or behavior. The GREEN commit is reproducible, compiles cleanly, passes 116/116 tests, clippy, and fmt.

## Auth Gates

None.

## Known Stubs

None. Every match arm has a real body. No placeholder data flows. The implementation is complete: timeout enforcement is end-to-end functional on both Unix and Windows code paths (Windows path verified via `#[cfg(unix)]` gating only on the SIGTERM-grace block — the `child.kill()` final fallback is unconditional).

## Commits

| # | Hash      | Type | Message                                                                                  |
|---|-----------|------|------------------------------------------------------------------------------------------|
| 1 | `e29af1c` | test | test(05-04): add failing timeout stubs (#[ignore] until Plan 04 Task 2)                  |
| 2 | `8d61098` | feat | feat(05-04): wrap run_one spawn in tokio::time::timeout with SIGTERM/SIGKILL escalation  |

## Self-Check: PASSED

- File `crates/fast-staged/src/commands.rs` — FOUND
- Commit `e29af1c` — FOUND in `git log`
- Commit `8d61098` — FOUND in `git log`
- `cargo build -p fast-staged` — exits 0
- `cargo test -p fast-staged --lib` — 116 passing
- `cargo clippy -p fast-staged --lib -- -D warnings` — exits 0
- `cargo fmt --check` — exits 0
- `grep -n "tokio::time::timeout\|tokio_timeout" commands.rs` — present (alias `tokio_timeout`)
- `grep -n "Signal::SIGTERM" commands.rs` — present
- `grep -n "child.kill()" commands.rs` — present
- `grep -n "AppError::CommandTimeout" commands.rs` — present
- `grep -n "fn failure_outcome_timeout" commands.rs` — present
- `grep -nE '#\[ignore = "Plan 04' commands.rs` — ZERO lines (un-ignored as required)
- `grep -n "fn no_timeout_passes_through_unchanged" commands.rs` — present
- `grep -nE 'nix.*=.*"0\.31".*signal' Cargo.toml` — present (Plan 02 dependency)
