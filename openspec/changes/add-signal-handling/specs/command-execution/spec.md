## ADDED Requirements

### Requirement: Interrupt handling
On SIGINT or SIGTERM (Unix) or Ctrl+C / Ctrl+Break (Windows), the tool SHALL stop starting new invocations, terminate running invocations using the same escalation as timeouts, restore the staged diff in staged mode, and exit with code 130. No child process started by the tool SHALL remain running after it exits.

#### Scenario: Ctrl+C while a command runs
- **WHEN** a command is running and the user presses Ctrl+C
- **THEN** the command's process is terminated, the index matches its pre-run state, `.git/fast-staged.patch` is removed, and the tool exits with code 130

#### Scenario: Ctrl+C before later groups start
- **WHEN** the user presses Ctrl+C while group `a` runs and group `b` has not started
- **THEN** no command of group `b` is started

### Requirement: Forced interrupt
A second interrupt received while terminating SHALL kill remaining child processes immediately without waiting for the grace period.

#### Scenario: Double Ctrl+C
- **WHEN** a command ignores SIGTERM and the user presses Ctrl+C twice
- **THEN** the command is killed immediately after the second Ctrl+C
