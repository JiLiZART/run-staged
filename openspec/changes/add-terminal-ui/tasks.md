## 1. Exec and core events

- [x] 1.1 Extract `plan_tasks` from `exec::tasks_for_group` with stable invocation ids. Verify: existing exec tests pass, plus a unit test that the ids are unique and ordered.
- [x] 1.2 Add `timed_out: bool` to `CommandOutcome::Failure`, set by the timeout path. Verify: the existing timeout test asserts `timed_out == true`, and the other failure tests assert `false`.
- [x] 1.3 Stream output in `run_one`: per-stream line loops (split on `\n` and `\r`) that keep the full capture and emit `ExecEvent::Line` through an optional `&dyn Fn(ExecEvent)`, plus `Started` and `Finished`. Verify: tests with `fixture-tool --stdout` show ordered `Line` events and unchanged captured bytes, every invocation that runs or is cancelled gets exactly one `Finished`, `Started` comes only for spawned ones (groups skipped after an earlier group's failure emit nothing), and `None` still works.
- [x] 1.4 Emit `StepEvent::Planned(&Plan)` from `core::run` before the backup, and pass exec events through `RunOptions::exec` (a `Send` sink; the step callback is a non-`Send` `FnMut`). Config discovery now starts from `RunOptions::cwd`. Verify: a core fixture test sees `Planned` first, then `Started(Backup)`, then command events inside the tasks step.

## 2. Color support

- [x] 2.1 `output::ansi` sanitizer with `keep_sgr` and `strip_all`. Verify: a table test covering SGR kept or stripped; cursor-move, clear, and OSC removed; `\r` removed; UTF-8 intact.
- [x] 2.2 `format_error_box(outcome, colored)`: red or yellow border and `FAILED:` line, `keep_sgr` content with a reset at line end; `strip_all` when not colored. Verify: unit tests for red, yellow, the reset, and that the plain output matches today's box when the input has no escapes. Existing fixture snapshots pass unchanged.
- [x] 2.3 Forced color env: when color is allowed, exec adds `FORCE_COLOR=1` and `CLICOLOR_FORCE=1` unless `FORCE_COLOR`, `CLICOLOR_FORCE`, or `NO_COLOR` is already set. Add `fixture-tool --print-env NAME` to the testkit, and update its README and spec. Verify: exec tests for the four scenarios in the `command-execution` delta.

## 3. View model (`crates/tui`)

- [x] 3.1 Create `crates/tui` (package `run-staged-tui`, depending on `exec`, `output`, and `ratatui`). Verify: `cargo build -p run-staged-tui`, and `cargo tree` shows no dependency on `core` or `git`.
- [x] 3.2 Model: steps (or only the tasks step in unstaged mode), the group and command tree, the six command states, output tails (newest line per id, removed on finish), and finished/total counts. Verify: unit tests for each state, group order, omitting steps that never ran, and tail replacement and removal.
- [x] 3.3 Animation from `now`: spinner (80 ms per frame), cyan pulse (1.2 s ramp, off under NO_COLOR), step-label type-in (150 ms), and the 300 ms completion highlight. Verify: unit tests at the boundary times listed in the design.
- [x] 3.4 Fit: `rows(now, max_height)` reduces in order: drop tails, collapse finished groups, truncate with `… N more`. The gauge row is always kept. Verify: a unit test with 30 commands and a 14-row terminal never exceeds 13 rows and keeps the running commands.

## 4. Rendering and terminal

- [x] 4.1 Panel rendering: a rounded `Block` with left and right titles and a state-colored border, a `List` of styled `ListItem`s, and a `LineGauge` at the bottom (`finished/total` label, state color). Verify: insta `TestBackend` snapshots for backup running, a mixed tree with tails and gauge 3/7, a failed final frame with a red border, a small terminal, and NO_COLOR (no foreground colors).
- [x] 4.2 Terminal guard: an inline viewport sized from the plan, hidden cursor, restore on `Drop`, and a chained panic hook that restores before abort. Verify: a captured-writer test checks the show-cursor and reset sequences on drop and in the hook.
- [x] 4.3 Render thread: a 50 ms `recv_timeout`, at most 20 fps, `Line` coalescing per id, a final frame without tails on `Done`, then join. Verify: a `TestBackend` test plays a scripted event sequence and the last buffer equals the final-frame snapshot.

## 5. Binary integration

- [x] 5.1 Decide `live` (stdout is a terminal, not quiet, `TERM` is not `dumb`) and `color_allowed` (stderr is a terminal, NO_COLOR unset or empty, `TERM` is not `dumb`) in `apps/run-staged`, and pass `color_allowed` to exec and to the failure box. Verify: all existing fixture tests and snapshots pass unchanged (piped).
- [x] 5.2 Live mode: skip success lines and plain step lines. After the join, print failure boxes, cancelled lines, partial warnings, errors, and the summary. Verify: the PTY test from 6.1 shows the failure box below the panel and no `✓ group/pattern` line.
- [x] 5.3 Ctrl+C: a `tokio::signal::ctrl_c` task sets the flag; the render thread draws the final frame, restores the terminal, and the process exits with code 130. Verify: a Unix PTY test sends SIGINT during `fixture-tool --sleep 5000` and asserts exit 130 and a show-cursor sequence.

## 6. End-to-end

- [x] 6.1 Add `portable-pty` as a dev-dependency of `apps/run-staged`. PTY tests:
  - a successful run shows a rounded border, four `✔` steps, an `n/n` gauge, hide- and show-cursor sequences, and exit 0;
  - a failing run shows a red border, a colored failure box (red border bytes) below the panel, and exit 1.

  Verify: both pass on macOS and Linux locally (on Windows through ConPTY in CI).
- [x] 6.2 Activation and color fallbacks: piped output has no escape sequences; `TERM=dumb` under a PTY gives plain output; `--quiet` under a PTY prints nothing on success; `NO_COLOR=1` under a PTY animates without color sequences, and commands see no `FORCE_COLOR`. Verify: the tests pass.
- [x] 6.3 Document the live view, colored failure boxes, `FORCE_COLOR` / `CLICOLOR_FORCE` forcing, and how to opt out (`NO_COLOR`, `TERM=dumb`, `--quiet`) in the README. Verify: the section exists and every command in it was run once.
