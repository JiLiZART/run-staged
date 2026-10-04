## Why

Today run-staged prints plain lines, and command results only appear after every command has finished. A slow pre-commit run looks hung, and nothing shows which step is happening (backup, tasks, staging, cleanup), which command is still running, or what it is printing. When something fails, the failure box is monochrome, and most tools' own colors are lost, because their output is captured rather than sent to a terminal. A live, animated, colorful view of the run is the main UX promise of the project (roadmap: rich log of what is going on, display running commands by groups, colored error blocks, total and per-command time).

## What Changes

- In an interactive terminal, the run is shown as a live **step tree** inside a titled, bordered panel (ratatui `Block` + `List`):
  - The four steps (back up, run tasks, stage changes, clean up) are top-level bullets.
  - A step's label types itself in when the step starts. While running, the step shows an animated spinner and a pulsing color.
  - A finished step settles to `✔` (or `✗`) with its duration, with a short highlight so the completion is visible. This includes the backup step.
  - Under the tasks step, each group is a nested bullet (`▸ <group>  <pattern>  <N files>`). Under it, each command has its own spinner, a live timer, a final status (success, failed, timed out, or cancelled), and, while it runs, the last line of its output, dimmed.
  - A progress gauge (`LineGauge`) at the bottom of the panel shows finished / total commands.
  - The panel border and gauge are colored by the overall state: running, all succeeded, or something failed.
- The view is drawn inline, below the shell prompt (not full-screen). The final frame stays in the scrollback, with failure boxes, warnings, and the summary printed under it.
- **Colored failure boxes:** when stderr is a terminal and colors are allowed, the failure box border is red (yellow for timeouts), and the command's own colored output is kept inside the box. To make that possible, commands are run with `FORCE_COLOR=1` and `CLICOLOR_FORCE=1`, unless the user already set those or `NO_COLOR`. When colors are off, escape sequences in captured output are stripped, so the box stays clean.
- The live view is used only when stdout is a terminal, `--quiet` is not set, and `TERM` is not `dumb`. Otherwise, today's plain-text output is printed, unchanged for non-terminal stderr.
- `NO_COLOR` disables every color (panel, pulse, failure boxes, forced color for commands), but the view is still animated.
- The terminal is restored on success, error, Ctrl+C, and panic.
- **Replaces the earlier drafts of this change.** Compared with the last draft, this adds the panel, the gauge, the pulse and type-in animation, the output tails, colored failure boxes, and forced color for commands.

## Capabilities

### New Capabilities
- `terminal-ui`: the live panel (step tree, animation, output tails, gauge), activation rules, sizing, and terminal lifecycle.

### Modified Capabilities
- `output-reporting`: success lines and step lines are plain-mode only; quiet mode never renders the live view; failure boxes are colored and keep the command's colors when allowed.
- `command-execution`: commands get forced-color environment variables when colored output is allowed.

## Impact

- Code:
  - New `crates/tui` crate (package `run-staged-tui`), which holds the view model, the renderer, and the terminal guard.
  - `crates/exec` streams command output line by line, while still capturing all of it, and emits started, output-line, and finished events. It also sets the color environment.
  - `crates/core` emits a `Planned` event with the group and command tree before the backup starts.
  - `crates/output` gains colored failure boxes and an ANSI sanitizer.
  - `apps/run-staged` decides when the live view and colors are on.
- Dependencies: `ratatui` 0.30 (already in the lockfile) with its default crossterm backend. Dev-only: `portable-pty` for terminal end-to-end tests.
- Behavior change outside the TUI: with colors allowed, linters see `FORCE_COLOR=1` / `CLICOLOR_FORCE=1`. `NO_COLOR=1` turns this off.
- Interplay with `add-signal-handling`: this change restores the terminal on Ctrl+C by itself. Cancelling children and restoring the index on interrupt stay in `add-signal-handling`.
