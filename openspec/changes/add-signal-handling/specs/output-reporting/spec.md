## ADDED Requirements

### Requirement: Actionable error hints
Errors with a known remediation SHALL include a hint line after the message. At minimum: not a git repository (run inside a repository), configuration file not found (list the searched file names and mention `--config`), command not found (check the program is installed and on PATH), and command timeout (raise `timeout` in the config).

#### Scenario: Missing config hint
- **WHEN** no configuration file is found
- **THEN** stderr contains the error, the searched file names, and a mention of `--config`

#### Scenario: Command not found hint
- **WHEN** a configured program is not installed
- **THEN** the failure output suggests checking that the program is installed and on PATH

### Requirement: No panic on unavailable working directory
When the current working directory cannot be determined, the tool SHALL print `error: cannot determine current directory: <detail>` and exit with code 1 instead of panicking.

#### Scenario: Deleted working directory
- **WHEN** the tool starts in a directory that has been removed
- **THEN** stderr starts with `error:` and no panic message is printed
