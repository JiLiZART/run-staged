## Why

Output is only printed after every command finishes, so a slow pre-commit run looks hung. A live view of what is running, for how long, and what failed is the main UX promise of the project. This is roadmap Phase 8 (UI-01..05) plus ERR-03 (terminal restore on panic).

## What Changes

- Live TUI while commands run: per-command spinner, label, elapsed time, grouped by config group, final status per command.
- TUI only when stdout is a TTY and `--quiet` is not set; otherwise the existing plain-text output is used.
- Terminal restored on normal exit, error, interrupt, and panic.

## Capabilities

### New Capabilities
- `terminal-ui`: live progress rendering and terminal lifecycle.

### Modified Capabilities
- `output-reporting`: quiet mode explicitly disables the TUI.

## Impact

- Code: new `ui` module; executor emits progress events; `lib.rs` chooses TUI vs plain output.
- Dependencies: `ratatui` (already declared) with its crossterm backend.
- Depends on `add-signal-handling` for the cancellation path; raw mode must translate Ctrl+C key events into the same cancellation.
