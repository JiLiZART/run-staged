## Purpose

Show a live, animated panel of a run-staged run in an interactive terminal (step tree, command output tails, and progress gauge), and guarantee the terminal is restored afterwards.

## ADDED Requirements

### Requirement: Bordered panel
When the live view is active, the tool SHALL draw the run inside a bordered panel with the title `run-staged` on the left and `<N> file|files · <elapsed>` on the right, where `<N>` is the number of distinct matched files. The border SHALL be colored by the overall state:
- cyan while anything is running;
- green when every step finished successfully;
- red once any step or command has failed, timed out, or been cancelled.

#### Scenario: Border turns red on failure
- **WHEN** a command fails while others are still running
- **THEN** the panel border is red from the next frame on

#### Scenario: Successful final frame
- **WHEN** every step finishes successfully
- **THEN** the final frame's border is green and the right title shows the total elapsed time

### Requirement: Step list
Inside the panel, the tool SHALL render the run's steps as a bulleted list, in order:
1. back up the original state;
2. run tasks;
3. stage changes from tasks;
4. clean up.

In `--unstaged` mode, only the run-tasks step SHALL be listed. Each step SHALL show one of these states:
- **pending**: dimmed, with marker `◌`;
- **running**: an animated spinner and a label ending in `…`;
- **done**: `✔` and the step's duration;
- **failed**: `✗`.

A step that never ran (such as staging after a failed task) SHALL be omitted from the final frame.

#### Scenario: Steps before tasks start
- **WHEN** the backup step is running
- **THEN** the backup row shows a spinner, and the tasks, staging, and cleanup rows show `◌`

#### Scenario: Failed run final frame
- **WHEN** a command fails
- **THEN** the final frame shows backup `✔`, tasks `✗`, and cleanup, and no staging row

#### Scenario: Unstaged mode
- **WHEN** the tool runs with `--unstaged` in a terminal
- **THEN** only the run-tasks step is listed

### Requirement: Running animation
While a step or command runs, its row SHALL show a spinner that advances every 80 milliseconds. When colors are enabled, its spinner and label SHALL also pulse through a brightness ramp that repeats every 1.2 seconds. When a step starts, its label SHALL appear one character at a time over 150 milliseconds, and the spinner SHALL be visible from the first frame.

#### Scenario: Label types in
- **WHEN** the backup step started 75 ms ago
- **THEN** about half of its label is visible, and the spinner is shown

#### Scenario: Pulse without color
- **WHEN** `NO_COLOR=1` is set
- **THEN** running rows animate their spinner and type-in, but their color does not change

### Requirement: Completion animation
When a step or command finishes, its spinner SHALL be replaced in the next frame by its final marker. For 300 milliseconds after finishing, the row SHALL be drawn with emphasis (bold, plus a bright color when colors are enabled). After that, it SHALL be drawn in its normal final style.

#### Scenario: Backup completes
- **WHEN** the backup step finishes
- **THEN** the next frame shows `✔` and the backup duration on an emphasized row, and 300 ms later the row is drawn in the normal done style

### Requirement: Nested task tree
Under the run-tasks step, the tool SHALL list each config group that has matched files as a nested bullet `▸ <group>  <patterns>  <N> file|files`. Under each group, it SHALL list every scheduled invocation as a nested bullet with its command and status:
- **queued**: `◌`;
- **running**: an animated spinner;
- **success**: `✔`;
- **failed**: `✗`;
- **timed out**: `⏱`;
- **cancelled**: `⊘`.

Groups SHALL appear in config declaration order, and commands in schedule order.

#### Scenario: Group with two commands
- **WHEN** group `format` matches 2 files for pattern `*.{js,ts}`, and its commands `prettier --write` and `eslint --fix` run
- **THEN** a row `▸ format  *.{js,ts}  2 files` is shown, with one nested row per command

#### Scenario: Distinct final states
- **WHEN** one command succeeds, one fails, one times out, and one is cancelled
- **THEN** their rows show `✔`, `✗`, `⏱`, and `⊘` respectively

### Requirement: Command output tail
While a command runs, the row below it SHALL show the most recent non-empty line the command wrote to stdout or stderr, dimmed and prefixed with `│`. The line SHALL have escape sequences and control characters removed, and SHALL be truncated with `…` to fit the panel width. The tail row SHALL disappear when the command finishes.

#### Scenario: Progress line from a formatter
- **WHEN** a running command prints `checking src/a.ts` and then `checking src/b.ts`
- **THEN** its tail row shows `│ checking src/b.ts`

#### Scenario: Colored output is sanitized
- **WHEN** a running command prints `\x1b[31merror\x1b[0m: bad`
- **THEN** its tail row shows `│ error: bad`

#### Scenario: Tail removed on finish
- **WHEN** the command exits
- **THEN** its tail row is gone in the next frame

### Requirement: Progress gauge
The last row inside the panel SHALL be a gauge filled to the ratio of finished invocations to scheduled invocations, labeled `<finished>/<total>`, and colored like the border. Successful, failed, timed-out, and cancelled invocations all count as finished.

#### Scenario: Gauge advances
- **WHEN** 3 of 7 scheduled invocations have finished
- **THEN** the gauge is about 43% filled and labeled `3/7`

### Requirement: Elapsed time
Every running step and command row SHALL show an elapsed time that increases while it runs. A finished row SHALL show its final duration. Durations SHALL use the same format as the plain success lines (milliseconds under one second, otherwise seconds with two decimals).

#### Scenario: Timer increments
- **WHEN** a command runs for 3 seconds
- **THEN** its elapsed time visibly increases during those 3 seconds and then settles to the final duration

### Requirement: Animation rate
The live view SHALL redraw at least every 100 milliseconds while anything is running, and SHALL NOT redraw more than 20 times per second.

#### Scenario: Spinner advances
- **WHEN** a command runs for one second
- **THEN** its spinner advances through at least 10 frames during that second

### Requirement: Fits the terminal
The panel SHALL be drawn inline below the cursor and SHALL NOT switch to an alternate screen. Its height, including borders and the gauge, SHALL NOT exceed the terminal height minus one. When the content does not fit, the tool SHALL reduce it in this order, stopping as soon as it fits:
1. drop the output tail rows;
2. collapse groups whose commands have all finished to their group row;
3. replace the rows that still do not fit with a trailing `… <N> more` row.

#### Scenario: Small terminal
- **WHEN** 30 commands are scheduled in a 14-row terminal
- **THEN** the panel never exceeds 13 rows, the gauge stays visible, and every running command that fits is shown

### Requirement: Final frame and output after it
When the run ends, the last frame of the panel SHALL stay in the terminal's scrollback, with no output tails. Below it, the tool SHALL print, in order:
1. failure boxes and cancelled lines (as defined in `output-reporting`);
2. warnings about partially staged files;
3. error messages;
4. the summary line.

Success lines and plain step lines SHALL NOT be printed in this mode.

#### Scenario: Failure details after the panel
- **WHEN** a command fails while the live view is active
- **THEN** the final panel is visible, and its failure box is printed below it, followed by the summary

### Requirement: Live view activation
The live view SHALL be active only when all of these hold:
- stdout is a terminal;
- `--quiet` is not given;
- the `TERM` environment variable is not `dumb`.

Otherwise, the tool SHALL use the plain-text output from `output-reporting` and SHALL NOT emit cursor or screen control sequences on stdout. When `NO_COLOR` is set to a non-empty value, the live view SHALL be drawn without colors but still animated.

#### Scenario: Piped output
- **WHEN** stdout is piped to a file
- **THEN** the file contains the plain step lines, success lines, and summary, and no escape sequences

#### Scenario: Dumb terminal
- **WHEN** stdout is a terminal and `TERM=dumb`
- **THEN** plain-text output is used

#### Scenario: NO_COLOR
- **WHEN** the live view is active and `NO_COLOR=1`
- **THEN** no color escape sequences are emitted, and spinners still animate

### Requirement: Terminal restoration
The tool SHALL restore the terminal (visible cursor, default colors, cursor below the final frame) before exiting on success, on error, on Ctrl+C, and on panic. The live view SHALL NOT put the terminal into raw mode, so Ctrl+C keeps its normal signal behavior.

#### Scenario: Panic during rendering
- **WHEN** a panic occurs while the live view is active
- **THEN** the cursor is visible and on a fresh line when the process exits

#### Scenario: Ctrl+C during the live view
- **WHEN** the user presses Ctrl+C while commands run
- **THEN** the terminal is restored before the process exits
