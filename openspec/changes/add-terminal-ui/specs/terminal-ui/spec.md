## Purpose

Show live progress of running commands in the terminal and guarantee the terminal is restored afterwards.

## ADDED Requirements

### Requirement: Live progress view
When the TUI is active, the tool SHALL display every scheduled invocation grouped under its config group name, each with a label `<pattern>: <command>`, a spinner while running, and a final status (success, failed, timed out, cancelled) once finished. The view SHALL update while commands run.

#### Scenario: Running command shows spinner
- **WHEN** a command is still running
- **THEN** its row shows an animated spinner under its group heading

#### Scenario: Final status
- **WHEN** one command succeeds and another fails
- **THEN** their rows show distinct success and failure indicators

### Requirement: Elapsed time
Each row SHALL show elapsed time, increasing while the command runs and fixed at the final duration once it finishes.

#### Scenario: Timer increments
- **WHEN** a command runs for 3 seconds
- **THEN** its elapsed time visibly increases during those 3 seconds

### Requirement: TUI activation
The TUI SHALL be active only when stdout is a terminal and `--quiet` is not given. Otherwise the tool SHALL use the plain-text output defined in `output-reporting` and SHALL NOT emit terminal control sequences.

#### Scenario: Piped output
- **WHEN** stdout is piped to a file
- **THEN** the file contains plain-text result lines and no escape sequences

### Requirement: Terminal restoration
The tool SHALL restore the terminal to its normal mode before exiting on success, on error, on interrupt, and on panic. Failure details SHALL be printed after the terminal is restored.

#### Scenario: Panic during rendering
- **WHEN** a panic occurs while the TUI is active
- **THEN** the terminal is back in normal mode when the process exits

#### Scenario: Ctrl+C in TUI
- **WHEN** the user presses Ctrl+C while the TUI is active
- **THEN** interrupt handling runs as defined in `command-execution` and the terminal is restored
