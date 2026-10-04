## Context

`run_staged_core::run` already reports steps through a synchronous `&mut dyn FnMut(StepEvent)` callback: `Started` / `Finished` / `Failed` for each step, plus `Outcomes` once all tasks are done.

`run_staged_exec::run_one` spawns each command with piped stdout and stderr, reads both to the end, and returns a `CommandOutcome`. Nothing is known about a command while it runs. A timeout is a `Failure` with `exit_code = -1` and a "timed out" message, so it can't be told apart from other failures by type.

`crates/output::format_error_box` writes plain box-drawing characters and passes the captured bytes through as they are. `ratatui` 0.30 (with `ratatui-widgets` 0.3 and crossterm 0.29) is already in `Cargo.lock`, but none of the new crates use it yet.

For motivation, see proposal.md.

## Goals / Non-Goals

**Goals:**
- A colorful, animated panel that shows what is happening right now: step, command, latest output line, overall progress.
- Failure boxes that keep the tool's own colors, in any mode where stderr is a terminal.
- With stderr and stdout piped, output is byte-for-byte what it is today, so the existing fixture snapshots do not change.
- The renderer can be tested deterministically: no real terminal, no real clock.

**Non-Goals:**
- Scrolling through full command output inside the panel. Only the last line is shown live, and the full output goes in the failure box.
- Keyboard interaction, or a full-screen dashboard.
- Configurable themes or spinner sets.

## Decisions

### Crates and data flow

**New crate `crates/tui` (package `run-staged-tui`), depending only on `exec` (for outcome types), `output` (for `format_duration` and the ANSI sanitizer), and `ratatui`.** It has three parts:
- `model.rs`: a pure view model. It folds events into a tree and computes the rows for an instant `now`.
- `render.rs`: draws the panel. That is a `Block` with titles and a state-colored border, a `List` of `ListItem`s, and a `LineGauge` at the bottom, laid out with `Layout::vertical([Fill(1), Length(1)])` inside the block.
- `term.rs`: the terminal guard and the draw loop.

`core` stays free of ratatui, and `output` stays free of terminal I/O.

**Events.**
- `core::StepEvent` gains `Planned(&Plan)`, emitted after matching and before the backup. `Plan` lists the groups (name, patterns, file count) and the scheduled invocations (id, group, command).
- `exec::execute_all` takes an optional `&dyn Fn(ExecEvent)` with three events: `Started { id }`, `Line { id, text }`, and `Finished { id, outcome }`. Passing `None` keeps today's behavior.
- A single `plan_tasks` function, shared by `exec` and `core`, expands chunks and per-file runs into ids, so the ids in `Planned` match the ids in the events.

**Distinguishing timeouts.** `CommandOutcome::Failure` gains `timed_out: bool`, set by the timeout path. The failure box (yellow) and the tree (`⏱`) use it, instead of looking for "timed out" in stderr.

**Streaming output.** `run_one` replaces read-to-end with two `tokio::io::BufReader` line loops, one each for stdout and stderr. Each loop:
- appends every chunk to the capture buffer it has today, so the failure-box content is unchanged;
- sends `ExecEvent::Line` for every non-empty line, after sanitizing it.

Lines are split on `\n` and on `\r`, so progress bars that redraw with carriage returns show up as separate updates. Lines are only sent when a listener exists. Alternative considered: polling partial buffers. Rejected, because line callbacks are simpler and lose nothing.

### Rendering and animation

**Threading.** The binary creates a `std::sync::mpsc` channel. The `StepEvent` and `ExecEvent` callbacks send owned events into it and never block on the terminal. A dedicated render thread owns the `Terminal` and the model. It calls `recv_timeout(50ms)`, applies the events, and draws. Drawing is rate-limited to at most 20 frames per second (50 ms apart), and happens at least every 80 ms while something runs.

**Inline viewport.** `Viewport::Inline(h)`, with `h = min(2 borders + rows(plan) + tails + 1 gauge, terminal_height - 1)`, computed once from `Planned` and counting one tail row for each command that can run at the same time. No alternate screen. When content exceeds `h`, the model reduces it as the spec orders: drop tails, then collapse finished groups, then truncate with `… N more`. This is a pure function of the model, `h`, and `now`.

**Animation**, all derived from `now` so it is deterministic in tests:
- **Spinner:** braille frames `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`, frame `(now - started) / 80ms`.
- **Pulse:** a 6-step cyan ramp (dim → bright → dim) indexed by `((now - started) % 1200ms) / 200ms`, applied to the spinner and the label of running rows. It is skipped when `NO_COLOR` is set.
- **Type-in:** a step label shows `ceil(len * min(1, (now - started) / 150ms))` characters. Commands do not type in, because they start in bulk and it would look noisy.
- **Completion highlight:** while `now - finished_at < 300ms`, the row is bold with a bright green `✔`, bright red `✗`, or bright yellow `⏱`. After that, the normal colors are used and the duration is dim.
- **Gauge:** `LineGauge::ratio(finished / total)` with the label `finished/total`. Its filled style follows the border state: cyan, then green, or red on failure. It uses thick line symbols.
- **Border:** `Block::bordered().border_type(Rounded)`, titled ` run-staged ` on the left and ` 3 files · 1.24s ` on the right.

**Colors are off** (everything drawn in default foreground, keeping bold and dim) when `NO_COLOR` is non-empty.

### Failure boxes and color

**One "color allowed" decision**, computed once in the binary: `stderr.is_terminal() && NO_COLOR is unset/empty && TERM != "dumb"`. It controls:
1. the colored failure box;
2. the forced-color environment for commands.

The live view decides its own colors from `NO_COLOR` only, because it draws on stdout.

**ANSI sanitizer**, in `output::ansi` and shared with `tui`: a small state machine over bytes, about 60 lines, with no new dependency. It keeps or drops SGR sequences (`ESC [ … m`) and always drops every other CSI, OSC, and C0 control except `\t`. It has two modes:
- `keep_sgr`: for the colored failure box;
- `strip_all`: for the plain box and for output tails.

Alternative considered: the `strip-ansi-escapes` / `vte` crates. Rejected, because our needs (keep only SGR) are narrow and easy to test exhaustively.

**Colored failure box.** `format_error_box(outcome, colored: bool)`:
- When colored, the border characters and the `FAILED:` line are wrapped in red (or yellow when `timed_out`). Content lines are passed through `keep_sgr` and followed by `ESC[0m`.
- When not colored, content lines are passed through `strip_all`. The border is identical to today's, so piped snapshots are unchanged unless a command itself emitted escape codes.

**Forced color for commands.** When color is allowed, `exec` adds `FORCE_COLOR=1` and `CLICOLOR_FORCE=1` to the command's environment, but only if the parent environment has neither of them and no `NO_COLOR`. This is a `ColorEnv` flag on the exec options.

### Terminal lifecycle

**No raw mode.** The renderer only draws, so Ctrl+C stays an ordinary SIGINT. The terminal guard:
1. hides the cursor on start;
2. on `Drop`, shows the cursor, resets colors, and moves below the viewport;
3. registers a panic hook that does the same and then chains to the previous hook. This is needed because the release profile has `panic = "abort"`, which skips `Drop`.

On Ctrl+C, a `tokio::signal::ctrl_c` task in the binary sets a flag. The render thread then draws a final frame, restores the terminal, and the process exits with code 130. When `add-signal-handling` lands, it takes over cancellation through the same flag.

### Testing

- **Model:** fake `Instant`s. Cover the spinner at 0, 80, and 160 ms; the pulse index; type-in at 0, 75, and 150 ms; the 299/300 ms highlight boundary; the tail updates and its removal; and the reduction order (tails, collapse, truncate).
- **Render:** draw into `TestBackend`, then `insta` buffer snapshots, including cell styles where they matter. Cover these frames:
  - backup running (pulse frame);
  - a mixed tasks tree with tails and the gauge at 3/7;
  - a failed final frame with a red border;
  - a small terminal;
  - NO_COLOR (assert no cell has a foreground color).
- **Sanitizer:** a table test covering SGR kept and stripped; cursor-move, clear-screen, OSC title, and bare `\r` removed; and UTF-8 kept intact.
- **Failure box:** colored (red, yellow, and the reset at line end) and plain (identical to today's snapshot when the input has no escapes).
- **Exec:** a test asserting `Line` events in order, using `fixture-tool --stdout`. Color-env tests use a new `fixture-tool --print-env NAME` option that writes the variable's value to stdout.
- **PTY end-to-end** (`portable-pty`, dev-only; it works on Windows through ConPTY too):
  - a successful run shows a rounded border, the four `✔` steps, the `n/n` gauge, the hide- and show-cursor sequences, and exit code 0;
  - a failing run shows a red border, a colored failure box below the panel, and exit code 1;
  - `TERM=dumb` and `--quiet` fall back to plain output.

## Risks / Trade-offs

- [`FORCE_COLOR` changes the output of tools that check it, and could break a command that parses its own piped output] → It is only added when stderr is a terminal (an interactive commit). `NO_COLOR=1` or setting the variable yourself turns it off. Documented in the README.
- [Tools that ignore `FORCE_COLOR` and `CLICOLOR_FORCE` still print without color] → Accepted. A PTY per command would capture colors from every tool, but it is costly and changes how tools behave. Revisit if users ask.
- [Terminals without full Unicode render braille, `◌`, or the rounded corners as boxes] → Only common glyphs are used. If this shows up, an ASCII glyph set behind `TERM` detection can be added without changing the specs.
- [Planned ids could drift from execution] → `plan_tasks` is the single source, plus a test that every `ExecEvent` id is in the plan.
- [Streaming lines from very chatty commands floods the channel] → `Line` events are coalesced: the render thread keeps only the newest line per id before drawing, so the channel drains every tick.
- [A very fast run (< 1 frame) never animates] → The final frame is always drawn, so the scrollback shows the full panel.
- [The hidden cursor stays hidden after SIGKILL] → This cannot be prevented, and it is rare in hook runs.

## Migration Plan

This replaces the earlier drafts of `add-terminal-ui` (0 tasks were done). CI and piped runs keep today's output, and the existing snapshots stay valid. Interactive users get the panel, colored failure boxes, and forced-color commands. To get the plain output back: `TERM=dumb`, `NO_COLOR=1` (colors only), or `--quiet`.
