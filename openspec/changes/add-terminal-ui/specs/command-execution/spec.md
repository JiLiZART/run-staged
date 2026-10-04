## ADDED Requirements

### Requirement: Color environment for commands
When colored output is allowed (stderr is a terminal, `NO_COLOR` is unset or empty, and `TERM` is not `dumb`), the tool SHALL start every command with `FORCE_COLOR=1` and `CLICOLOR_FORCE=1` in its environment, so tools that detect a non-terminal still produce colored output for the failure box and the live view. A variable the user already set (`FORCE_COLOR`, `CLICOLOR_FORCE`, or `NO_COLOR`) SHALL be passed through unchanged and SHALL NOT be overridden. When colored output is not allowed, the tool SHALL NOT add either variable.

#### Scenario: Colors forced in a terminal
- **WHEN** stderr is a terminal and neither variable is set
- **THEN** each command sees `FORCE_COLOR=1` and `CLICOLOR_FORCE=1`

#### Scenario: User setting wins
- **WHEN** stderr is a terminal and the user runs with `FORCE_COLOR=0`
- **THEN** commands see `FORCE_COLOR=0`

#### Scenario: Piped run adds nothing
- **WHEN** stderr is piped
- **THEN** commands see neither `FORCE_COLOR` nor `CLICOLOR_FORCE`, unless the user set them

#### Scenario: NO_COLOR disables forcing
- **WHEN** stderr is a terminal and `NO_COLOR=1` is set
- **THEN** commands see `NO_COLOR=1` and neither forced-color variable is added
