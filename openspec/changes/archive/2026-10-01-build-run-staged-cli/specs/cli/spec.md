## MODIFIED Requirements

### Requirement: Supported flags
The binary, named `run-staged`, SHALL accept `--config <path>`, `--quiet`, `--unstaged`, `--allow-empty`, and `--install`. It SHALL pass them to the runtime behavior defined in the `config-loading`, `output-reporting`, `git-staging`, `file-matching`, and `hook-install` capabilities. `--install` SHALL NOT be combined with `--unstaged` or `--allow-empty`; doing so SHALL be a usage error (exit code 2).

#### Scenario: Flags accepted
- **WHEN** the binary runs with `--quiet --allow-empty` in a clean repository
- **THEN** it exits 0 without a usage error

#### Scenario: Conflicting install flags
- **WHEN** the binary runs with `--install --unstaged`
- **THEN** it exits with code 2 and prints a usage error

### Requirement: Version flag
`--version` SHALL print `run-staged <semver>` to stdout and exit 0.

#### Scenario: Print version
- **WHEN** the binary runs with `--version`
- **THEN** stdout contains `run-staged` followed by the crate version, and the exit code is 0

### Requirement: Help flag
`--help` SHALL print usage that lists every supported flag and exit 0.

#### Scenario: Print help
- **WHEN** the binary runs with `--help`
- **THEN** stdout lists `--config`, `--quiet`, `--unstaged`, `--allow-empty`, and `--install`, and the exit code is 0
