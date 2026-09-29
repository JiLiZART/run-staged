## Context

`execute_all` returns all outcomes at the end; `output::print_results` prints them afterwards. There is no progress signal during execution. Research flagged that the ratatui async event-loop pattern varies by version — verify against the version in `Cargo.lock` before implementing.

## Goals / Non-Goals

**Goals:**
- Real-time per-command status with elapsed time, grouped by config group.
- Plain-text fallback identical to today's output when not interactive.
- Terminal never left in raw mode.

**Non-Goals:**
- Scrollable command output inside the TUI (failure boxes print after the TUI closes).
- Mouse or keyboard interaction beyond Ctrl+C.

## Decisions

- **Event channel, not shared mutable state**: executor sends `Started/Finished` events over a tokio mpsc channel; the renderer owns the view model and redraws on events plus a ~100ms tick for spinners and timers. Keeps `execute_all` testable without a terminal.
- **Inline viewport, not alternate screen**: the final frame stays in scrollback, so the commit log shows what ran. Failure boxes print below it after the TUI exits.
- **Activation**: TUI iff stdout is a TTY and `--quiet` is not set. No env heuristics beyond TTY detection.
- **Panic hook** installed before entering raw mode restores the terminal, then chains to the previous hook.
- **Ctrl+C in raw mode** arrives as a key event, not a signal; the renderer maps it to the cancellation flag from `add-signal-handling`.

## Risks / Trade-offs

- Raw mode swallows SIGINT → must be handled via key events or interrupts stop working in TUI mode. Covered by a task and a scenario.
- Rendering cost must stay negligible relative to child processes; tick rate capped at ~10 fps.
