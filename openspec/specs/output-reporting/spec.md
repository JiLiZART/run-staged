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
