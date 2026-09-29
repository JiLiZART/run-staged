## Purpose

Define the command-line interface of the `fast-staged` binary.

## ADDED Requirements

### Requirement: Supported flags
The binary SHALL accept `--config <path>`, `--quiet`, `--unstaged`, and `--allow-empty`, and forward them to runtime behavior as defined in the `config-loading`, `output-reporting`, `git-staging`, and `file-matching` capabilities.

#### Scenario: Flags accepted
- **WHEN** the binary runs with `--quiet --allow-empty` in a clean repository
- **THEN** it exits 0 without a usage error

### Requirement: Version flag
`--version` SHALL print `fast-staged <semver>` to stdout and exit 0.

#### Scenario: Print version
- **WHEN** the binary runs with `--version`
- **THEN** stdout contains `fast-staged` followed by the crate version and the exit code is 0

### Requirement: Help flag
`--help` SHALL print usage including every supported flag and exit 0.

#### Scenario: Print help
- **WHEN** the binary runs with `--help`
- **THEN** stdout lists `--config`, `--quiet`, `--unstaged`, and `--allow-empty` and the exit code is 0

### Requirement: Unknown flags rejected
An unknown flag SHALL be rejected with a usage error (suggesting a close match when one exists) and exit code 2, distinct from runtime errors which exit 1.

#### Scenario: Typo in flag
- **WHEN** the binary runs with `--quet`
- **THEN** it exits with code 2 and suggests `--quiet`
