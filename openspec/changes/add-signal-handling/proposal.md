## Why

Pressing Ctrl+C during a pre-commit run today kills fast-staged but can leave linters running as orphans and skips restoring the staged diff. Errors also say what went wrong but rarely how to fix it. This is roadmap Phase 7 (EXEC-04, ERR-02).

## What Changes

- Handle SIGINT/SIGTERM (Unix) and Ctrl+C / Ctrl+Break (Windows): stop starting commands, terminate running ones, restore the staged diff, exit 130.
- A second interrupt skips the grace period and kills remaining children immediately.
- Errors carry an actionable hint where a remediation exists.
- `main.rs` no longer panics when the working directory is unavailable (06-REVIEW WR-01).

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `command-execution`: adds interrupt handling requirements.
- `output-reporting`: adds actionable error hints.

## Impact

- Code: `lib.rs` (signal listener, restore on cancel), `commands.rs` (kill in-flight children on cancel), `error.rs` (hints), `main.rs` (exit code 130, no `expect`).
- Dependencies: none new (tokio `signal` feature already enabled).
- CI: adds a Windows job so the Windows termination path actually runs.
- Terminal restore on panic (ERR-03) moves to `add-terminal-ui`, where raw mode is introduced.
