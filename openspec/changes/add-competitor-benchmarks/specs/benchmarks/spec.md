## Purpose

Provide a reproducible, manually-run benchmark that compares run-staged's speed and concurrency against other staged-file runners (lint-staged, nano-staged, lefthook, pre-commit, prek, hk, stagelint, overcommit) on identical input.

## ADDED Requirements

### Requirement: Single entry point
The repository SHALL provide one benchmark entry point under `bench/` that runs the full suite with a single command. It SHALL require `hyperfine`, `jq`, `git` and `rsync` and exit non-zero with a message naming the missing prerequisite if either is absent. It SHALL accept options to select scenarios, tools, file counts, and the hyperfine run count, and a verify mode that runs each tool once per scenario untimed and reports which tools exit non-zero.

#### Scenario: Run full suite
- **WHEN** a developer runs the benchmark entry point with no arguments and `hyperfine` installed
- **THEN** every scenario runs for every available tool and results are written to `bench/results/`

#### Scenario: Missing hyperfine
- **WHEN** the entry point runs and `hyperfine` is not on PATH
- **THEN** it exits non-zero and prints that `hyperfine` is required

#### Scenario: Verify mode
- **WHEN** the entry point runs with `--verify` and one tool's config makes it exit non-zero
- **THEN** that tool is reported as failing with the tail of its output, and the entry point exits non-zero

#### Scenario: Filter tools and scenarios
- **WHEN** the entry point runs with a tool filter `run-staged,lint-staged` and scenario filter `concurrency`
- **THEN** only those two tools are benchmarked and only the concurrency scenario runs

### Requirement: Tool matrix
The benchmark SHALL support these tools: run-staged (release build from this workspace), lint-staged, nano-staged, lefthook, pre-commit, prek, hk, stagelint, overcommit. Each tool SHALL be configured to perform the equivalent job for each scenario (same globs, same commands, same files). A tool that is not installed or not resolvable SHALL be skipped with a printed notice and SHALL NOT fail the run. Before timing a scenario, each tool SHALL run once untimed; a tool that exits non-zero SHALL be excluded from that scenario and reported as failed, and SHALL NOT fail the run. The version of every benchmarked tool SHALL be recorded with the results.

#### Scenario: Tool not installed
- **WHEN** the suite runs and `overcommit` is not installed
- **THEN** a "skipped: overcommit not found" notice is printed, other tools are benchmarked, and the run exits 0

#### Scenario: Tool fails a scenario
- **WHEN** a tool exits non-zero on its untimed first run of a scenario (for example nano-staged on 10000 files)
- **THEN** it is left out of that scenario's timing, the run continues, and the report lists the tool, the scenario and its last output line

#### Scenario: Versions recorded
- **WHEN** a run completes
- **THEN** the results include each benchmarked tool's version string

### Requirement: Identical isolated fixtures
Each benchmark iteration SHALL run against a throwaway git repository created outside the project working tree, containing the scenario's files staged in the index. The fixture SHALL be reset to the same staged state before every timed iteration so that tools that modify or restage files do not affect later iterations. The benchmark SHALL NOT modify the project repository's index, working tree, or hooks.

#### Scenario: Fixture reset between runs
- **WHEN** a tool's command rewrites a staged file during iteration 1
- **THEN** iteration 2 starts with the original staged content

#### Scenario: Project repo untouched
- **WHEN** the suite completes
- **THEN** `git status` in the project repository is unchanged from before the run

### Requirement: Overhead and scaling scenarios
The `overhead` scenario SHALL configure one glob matching all fixture files with a no-op command, so measured time is the tool's own cost (startup, git reads, matching, process spawning). The `scaling` scenario SHALL repeat `overhead` across staged file counts of 10, 100, 1000 and 10000 by default.

#### Scenario: Scaling across file counts
- **WHEN** the scaling scenario runs with default options
- **THEN** results contain one measurement per tool per file count 10, 100, 1000 and 10000

### Requirement: Concurrency scenario
The `concurrency` scenario SHALL configure 4 independent tasks, each matching a disjoint set of files and running a command that sleeps 0.5 seconds, using each tool's parallel mode where it has one. The report SHALL show, per tool, the measured wall time next to the ideal parallel time (0.5 s) and the fully sequential time (2.0 s), and a parallelism factor computed as sequential time divided by measured time.

#### Scenario: Fully parallel tool
- **WHEN** a tool runs all 4 tasks concurrently with ~50 ms overhead
- **THEN** its reported wall time is about 0.55 s and its parallelism factor is about 3.6

#### Scenario: Sequential-only tool
- **WHEN** a tool has no parallel mode
- **THEN** its wall time is about 2.0 s and the report marks it as sequential

### Requirement: Partial-staging scenario
The `partial-staging` scenario SHALL stage files that also have unstaged modifications and SHALL time every tool in its default mode. Separately and untimed, the benchmark SHALL run each tool once with a probe command and record two facts per tool: whether the tool hid the unstaged modifications from its commands (the probe ran and never saw them), and whether the unstaged modifications were still in the working tree afterwards. A tool whose probe did not run SHALL be reported as such rather than as hiding.

#### Scenario: Unstaged changes preserved
- **WHEN** a tool completes the partial-staging check
- **THEN** the report states whether it hid the unstaged modifications and whether they were preserved, and flags a tool that lost them

#### Scenario: Tool that does not hide unstaged edits
- **WHEN** a tool passes files with unstaged edits to its commands
- **THEN** it is still timed and the report marks it as not hiding unstaged changes

### Requirement: Result reporting
Each run SHALL write raw hyperfine JSON per scenario to `bench/results/` and generate `bench/RESULTS.md` containing: date, OS, CPU model and core count, tool versions, and one markdown table per scenario with mean, standard deviation, min, max, and relative speed versus run-staged, sorted fastest first.

#### Scenario: Markdown report generated
- **WHEN** a run completes
- **THEN** `bench/RESULTS.md` exists with machine info, a tool version list, and one table per executed scenario with run-staged as the 1.00x baseline
