## MODIFIED Requirements

### Requirement: Success lines
In plain-text mode, the tool SHALL print to stdout `✓ <group>/<pattern>: <command> (<duration>, <N> file|files)` for each successful invocation. The duration SHALL be shown in milliseconds under one second and in seconds with two decimals otherwise. While the live view from `terminal-ui` is active, success lines SHALL NOT be printed, because the tree shows them.

#### Scenario: Success output
- **WHEN** `cargo fmt` succeeds on 2 files in group `default` pattern `*.rs` in 1.5 seconds
- **THEN** stdout contains `✓ default/*.rs: cargo fmt (1.50s, 2 files)`

#### Scenario: No duplicate lines under the live view
- **WHEN** the live view is active and a command succeeds
- **THEN** no `✓ <group>/<pattern>: …` line is printed after the final panel

### Requirement: Failure report
For each failed or timed-out invocation, the tool SHALL print to stderr a bordered box containing `FAILED: <command>`, `Exit code: <n>`, up to the first 40 lines of the command's stderr, and up to the first 20 lines of its stdout. Sections are omitted when empty. The box SHALL span the full terminal width (80 columns when stderr is not a terminal), with a border on both sides of every row; longer lines wrap inside it.

When colored output is allowed (stderr is a terminal, `NO_COLOR` is unset or empty, and `TERM` is not `dumb`):
- the box border and the `FAILED:` line SHALL be red for failed invocations, and yellow for timed-out ones;
- color escape sequences in the captured output SHALL be kept;
- each content line SHALL end with a color reset, so colors never leak into the border or later output.

Cursor-movement and screen-clearing sequences SHALL always be removed from the captured output. When colored output is not allowed, all escape sequences SHALL be removed.

#### Scenario: Failure box
- **WHEN** a command exits 1 writing `lint error` to stderr
- **THEN** stderr shows a box with `FAILED:`, `Exit code: 1`, and `lint error`

#### Scenario: Colored box in a terminal
- **WHEN** stderr is a terminal and a command fails writing `\x1b[31merror\x1b[0m` to stderr
- **THEN** the box border is drawn in red, and the content line keeps the red `error` and ends with a reset before the padding

#### Scenario: Timed-out box
- **WHEN** stderr is a terminal and a command times out
- **THEN** the box border is drawn in yellow

#### Scenario: Plain box when piped
- **WHEN** stderr is piped and a command fails writing `\x1b[31merror\x1b[0m`
- **THEN** the box contains `error` with no escape sequences and no colored border

### Requirement: Quiet mode
With `--quiet` the tool SHALL NOT render the live view and SHALL suppress success lines. Failure boxes, cancelled lines, and error messages SHALL still be printed.

#### Scenario: Quiet success
- **WHEN** all commands succeed with `--quiet`
- **THEN** stdout is empty and the exit code is 0

#### Scenario: Quiet failure
- **WHEN** a command fails with `--quiet`
- **THEN** the failure box is still printed to stderr

#### Scenario: Quiet in a terminal
- **WHEN** the tool runs with `--quiet` and stdout is a terminal
- **THEN** no live view is rendered and stdout stays empty on success

### Requirement: Step lines
In non-quiet staged mode with plain-text output, the tool SHALL print one line to stdout when each run step starts and one line when it ends. The steps are:
- backing up the original state;
- running tasks;
- staging task changes;
- cleaning up.

A step that starts SHALL use the prefix `⋯`. A step that finishes SHALL use `✔`. A step that fails SHALL use `✗`. Steps that do not run, such as staging after a failure, SHALL NOT be printed. `--unstaged` mode SHALL print only the running-tasks step. While the live view is active, step lines SHALL NOT be printed; the step list replaces them.

#### Scenario: Successful staged run
- **WHEN** all commands succeed in staged mode without `--quiet` and stdout is not a terminal
- **THEN** stdout shows start and finish lines for backup, tasks, staging, and cleanup, in that order

#### Scenario: Failed run skips staging step
- **WHEN** a command fails and stdout is not a terminal
- **THEN** no staging step line is printed and the tasks step ends with `✗`
