## Context

`execute_all` already has a shared `Arc<AtomicBool>` cancellation flag, but it is cooperative only: it stops *new* invocations; running children are only killed by the timeout path (`terminate()`: SIGTERM → 2s grace → SIGKILL on Unix, TerminateProcess on Windows). Nothing listens for signals. CI runs on Linux only, so the Windows branch of `terminate()` has never executed.

## Goals / Non-Goals

**Goals:**
- No orphaned child processes after an interrupt.
- Index restored to its pre-run state after an interrupt.
- Hints on errors users hit in practice.

**Non-Goals:**
- Process-group management for grandchildren spawned by commands (e.g. `sh -c` spawning more processes). Revisit if it shows up in practice.
- TUI cancel indicator (belongs to `add-terminal-ui`).

## Decisions

- **Reuse the existing cancellation flag and `terminate()`** instead of a new mechanism. The signal listener sets the flag; each `run_one` selects on child exit vs flag and calls `terminate()` on the flag. One kill path for timeout and interrupt.
- **Exit code 130** on interrupt (128 + SIGINT), matching shell convention and what git hooks expect for "aborted by user". Introduce via `AppError::Cancelled` mapped to 130 in `main.rs`.
- **Restore before exit**: interrupt follows the existing failure path (restore → cleanup, keep patch if restore fails).
- **Second signal** sets a "force" flag so `terminate()` skips the grace period.
- **Hints live in `AppError` Display text** (`<message>\nhint: <hint>`), keeping `main.rs` output as `error: {e}`.

## Risks / Trade-offs

- Grandchildren may survive if a command does not forward SIGTERM → documented non-goal.
- Signal tests are timing-sensitive; test uses a long `sleep` and polls for process exit rather than fixed delays.
