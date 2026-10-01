# command-execution Specification

## Purpose
Run the configured commands against matched files with predictable batching, ordering, timeout, and failure semantics.

## Requirements

### Requirement: Command invocation
Each command string SHALL be split into arguments using shell-like quoting rules and executed directly without a shell. Commands SHALL run with the repository root as working directory, and their stdout and stderr SHALL be captured. A command with unbalanced quotes SHALL fail with `invalid command syntax`.

#### Scenario: Quoted arguments
- **WHEN** a command is `prettier --write "my file"`
- **THEN** the process receives `my file` as a single argument

#### Scenario: No shell features
- **WHEN** a command is `cargo fmt && cargo clippy`
- **THEN** `&&` is passed as a literal argument rather than chaining commands

### Requirement: File argument placement
If a command contains the token `{files}`, that token SHALL be replaced by the matched file paths; otherwise the paths SHALL be appended to the end of the command.

#### Scenario: Files appended
- **WHEN** the command is `eslint --fix` and `a.ts` matches
- **THEN** the process runs as `eslint --fix a.ts`

#### Scenario: Files placeholder
- **WHEN** the command is `tool {files} --check` and `a.ts` matches
- **THEN** the process runs as `tool a.ts --check`

### Requirement: Batch and per-file modes
With `per_file = false` (default), the tool SHALL run each command once per pattern with all matched files. It SHALL split the files into several invocations when the list would exceed the group's `chunk_size`, or when the argument list would exceed 75% of the platform argument-length limit, whichever limit is smaller. File order SHALL be preserved. With `per_file = true`, the tool SHALL run each command once per file, and `chunk_size` SHALL be ignored. Chunked invocations of one command SHALL follow the group's `execution_order`.

#### Scenario: Batch mode
- **WHEN** `per_file` is false and `a.rs` and `b.rs` match `*.rs = ["rustfmt"]`
- **THEN** a single `rustfmt a.rs b.rs` process runs

#### Scenario: Per-file mode
- **WHEN** `per_file` is true and `a.rs` and `b.rs` match
- **THEN** `rustfmt a.rs` and `rustfmt b.rs` run as separate processes

#### Scenario: Oversized file list
- **WHEN** matched paths exceed the argument-length budget
- **THEN** the command runs several times, with consecutive chunks that cover every file exactly once

#### Scenario: Configured chunk size
- **WHEN** `chunk_size = 2` and `a.js`, `b.js`, `c.js` match `*.js = ["eslint"]`
- **THEN** `eslint a.js b.js` and `eslint c.js` run as separate invocations

### Requirement: Execution order
Within a group, `execution_order = "parallel"` (default) SHALL start all of the group's invocations concurrently and `"sequential"` SHALL run them one at a time in order. Groups SHALL always run one after another in declaration order.

#### Scenario: Parallel group
- **WHEN** a parallel group has two commands that each sleep 1 second
- **THEN** the group finishes in well under 2 seconds

#### Scenario: Groups do not overlap
- **WHEN** a config has groups `a` then `b`
- **THEN** no command of `b` starts before all commands of `a` finish

### Requirement: Failure propagation
With group `continue_on_error = false` (default), the first failure in a group SHALL mark not-yet-started invocations in that group as cancelled, while already-running invocations finish. With root `continue_on_error = false`, groups after a group with any failure SHALL NOT run. Cancelled invocations SHALL count as failures.

#### Scenario: Later group skipped
- **WHEN** root `continue_on_error` is false and a command in group `a` fails
- **THEN** group `b` does not run

#### Scenario: Continue on error
- **WHEN** root `continue_on_error` is true and a command in group `a` fails
- **THEN** group `b` still runs and the tool still exits non-zero

### Requirement: Command timeout
When a group's resolved `timeout` elapses before a command exits, the tool SHALL terminate it — on Unix by SIGTERM, then SIGKILL if it has not exited after 2 seconds; on Windows by immediate termination — and SHALL report a failure whose message contains `timed out after`.

#### Scenario: Command exceeds timeout
- **WHEN** `timeout = "1s"` and the command sleeps 10 seconds
- **THEN** the command is killed after about 1 second and reported as timed out

### Requirement: Command not found
A command whose executable does not exist SHALL be reported as a failure with exit code 127 and the message `command not found: <program>`.

#### Scenario: Missing executable
- **WHEN** a command is `nonexistent-tool`
- **THEN** it is reported as failed with exit code 127

### Requirement: Path format
With `path_format = "relative"` (default) commands SHALL receive paths relative to the repository root. With `"absolute"` commands SHALL receive absolute paths.

#### Scenario: Absolute paths
- **WHEN** `path_format = "absolute"` and `src/a.rs` matches
- **THEN** the command receives `<repo-root>/src/a.rs`

### Requirement: Exit status
The tool SHALL exit 0 when all invocations succeed. When any invocation fails, times out, or is cancelled, it SHALL exit with code 1 and print `error: command `<command>` failed with exit code <n>` for the first failure.

#### Scenario: One of several fails
- **WHEN** two commands run and one exits 2
- **THEN** the tool exits with code 1 even though the other succeeded
