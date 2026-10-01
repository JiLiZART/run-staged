# output-reporting Specification

## Purpose
Report command results and errors to the user in plain text.

## Requirements

### Requirement: Success lines
For each successful invocation the tool SHALL print to stdout `✓ <group>/<pattern>: <command> (<duration>, <N> file|files)`, with duration shown as milliseconds under one second and seconds with two decimals otherwise.

#### Scenario: Success output
- **WHEN** `cargo fmt` succeeds on 2 files in group `default` pattern `*.rs` in 1.5 seconds
- **THEN** stdout contains `✓ default/*.rs: cargo fmt (1.50s, 2 files)`

### Requirement: Failure report
For each failed or timed-out invocation the tool SHALL print to stderr a bordered box containing `FAILED: <command>`, `Exit code: <n>`, up to the first 40 lines of the command's stderr, and up to the first 20 lines of its stdout (sections omitted when empty).

#### Scenario: Failure box
- **WHEN** a command exits 1 writing `lint error` to stderr
- **THEN** stderr shows a box with `FAILED:`, `Exit code: 1`, and `lint error`

### Requirement: Cancelled report
For each invocation cancelled because of a sibling failure the tool SHALL print to stderr `✗ <group>/<pattern>: <command> (cancelled by sibling failure)`.

#### Scenario: Sibling cancelled
- **WHEN** a sequential group's first command fails and a second command never starts
- **THEN** stderr contains a cancelled line for the second command

### Requirement: Quiet mode
With `--quiet` the tool SHALL suppress success lines. Failure boxes, cancelled lines, and error messages SHALL still be printed.

#### Scenario: Quiet success
- **WHEN** all commands succeed with `--quiet`
- **THEN** stdout is empty and the exit code is 0

#### Scenario: Quiet failure
- **WHEN** a command fails with `--quiet`
- **THEN** the failure box is still printed to stderr

### Requirement: Error exit
Any runtime error SHALL be printed to stderr as `error: <message>` and the process SHALL exit with code 1.

#### Scenario: Runtime error format
- **WHEN** the tool runs outside a git repository
- **THEN** stderr starts with `error:` and the exit code is 1

### Requirement: Run summary
When commands have run, the tool SHALL print a final line to stdout: `<N> file|files, <duration>`. `<N>` is the number of distinct files matched by at least one group. `<duration>` is the wall-clock time from start to the end of cleanup, formatted like the per-command durations. The summary SHALL be suppressed by `--quiet`. It SHALL be printed for both successful and failed runs, and not for runs that exit early (no config, no staged files, or nothing matched).

#### Scenario: Summary after success
- **WHEN** two commands succeed on three distinct files in total
- **THEN** the last stdout line is `3 files, <duration>`

#### Scenario: Summary after failure
- **WHEN** a command fails and the tool is not in quiet mode
- **THEN** stdout still ends with the summary line and the exit code is 1

### Requirement: Step lines
In non-quiet staged mode, the tool SHALL print one line to stdout when each run step starts and one line when it ends. The steps are:
- backing up the original state;
- running tasks;
- staging task changes;
- cleaning up.

A step that starts SHALL use the prefix `⋯`. A step that finishes SHALL use `✔`. A step that fails SHALL use `✗`. Steps that do not run, such as staging after a failure, SHALL NOT be printed. `--unstaged` mode SHALL print only the running-tasks step.

#### Scenario: Successful staged run
- **WHEN** all commands succeed in staged mode without `--quiet`
- **THEN** stdout shows start and finish lines for backup, tasks, staging, and cleanup, in that order

#### Scenario: Failed run skips staging step
- **WHEN** a command fails
- **THEN** no staging step line is printed and the tasks step ends with `✗`
