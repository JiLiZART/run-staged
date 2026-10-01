## Why

Pressing Ctrl+C during a pre-commit run today kills run-staged but can leave linters running as orphans and skips restoring the staged diff. Errors also say what went wrong but rarely how to fix it. This is roadmap Phase 7 (EXEC-04, ERR-02).

## What Changes

- Handle SIGINT/SIGTERM (Unix) and Ctrl+C / Ctrl+Break (Windows): stop starting commands, terminate running ones, restore the staged diff, exit 130.
- A second interrupt skips the grace period and kills remaining children immediately.
- Errors carry an actionable hint where a remediation exists.
- `apps/run-staged/src/main.rs` no longer panics when the working directory is unavailable (06-REVIEW WR-01). `build-run-staged-cli` already prints `error: cannot determine current directory: …` here; this change only adds the regression test.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `command-execution`: adds interrupt handling requirements.
- `output-reporting`: adds actionable error hints.

## Impact

- Code: `crates/core` (signal listener, restore on cancel), `crates/exec` (kill in-flight children on cancel), each crate's `error.rs` plus `core::AppError` (hints), `apps/run-staged/src/main.rs` (exit code 130).
- Dependencies: none new (tokio `signal` feature already enabled).
- CI: adds a Windows job so the Windows termination path actually runs.
- Terminal restore on panic (ERR-03) moves to `add-terminal-ui`, where raw mode is introduced.
