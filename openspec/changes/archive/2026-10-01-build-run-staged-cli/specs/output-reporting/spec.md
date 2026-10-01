## ADDED Requirements

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
