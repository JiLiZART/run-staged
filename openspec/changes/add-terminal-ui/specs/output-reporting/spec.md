## MODIFIED Requirements

### Requirement: Quiet mode
With `--quiet` the tool SHALL NOT render the TUI and SHALL suppress success lines. Failure boxes, cancelled lines, and error messages SHALL still be printed.

#### Scenario: Quiet success
- **WHEN** all commands succeed with `--quiet`
- **THEN** stdout is empty and the exit code is 0

#### Scenario: Quiet failure
- **WHEN** a command fails with `--quiet`
- **THEN** the failure box is still printed to stderr

#### Scenario: Quiet in a terminal
- **WHEN** the tool runs with `--quiet` and stdout is a terminal
- **THEN** no TUI is rendered
