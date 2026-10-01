# hook-install Specification

## Purpose
Lets users set up run-staged as their git pre-commit hook with one command, so they do not need husky or another hook manager.

## Requirements

### Requirement: Install pre-commit hook
`run-staged --install` SHALL write an executable `pre-commit` hook into the repository's hooks directory. The hooks directory is `core.hooksPath` when set, resolved relative to the working-tree root; otherwise it is `<git-dir>/hooks`. The hook SHALL run `run-staged`, preferring the project's `node_modules/.bin/run-staged` when that file exists, and pass on its exit code. It SHALL contain the marker line `# run-staged`. After writing, the tool SHALL print the hook path and exit 0 without running any commands. No `git` executable SHALL be needed.

#### Scenario: Fresh install
- **WHEN** the repository has no `pre-commit` hook and the user runs `run-staged --install`
- **THEN** `.git/hooks/pre-commit` exists, is executable, contains `# run-staged`, and the exit code is 0

#### Scenario: Custom hooks path
- **WHEN** `core.hooksPath` is set to `.githooks`
- **THEN** the hook is written to `.githooks/pre-commit`

#### Scenario: Hook runs run-staged on commit
- **WHEN** the hook is installed and `git commit` runs with a staged file that a failing command matches
- **THEN** the commit is aborted

### Requirement: Existing hook protection
If a `pre-commit` hook already exists and contains the `# run-staged` marker, `--install` SHALL rewrite it and exit 0. If a hook exists without the marker, the tool SHALL leave it unchanged, print `error: pre-commit hook already exists at <path>` with a hint to add `run-staged` to it by hand, and exit with code 1.

#### Scenario: Reinstall is idempotent
- **WHEN** `--install` runs twice
- **THEN** both runs exit 0 and the hook contents are the same

#### Scenario: Foreign hook kept
- **WHEN** a `pre-commit` hook exists that was not written by run-staged
- **THEN** the file is unchanged and the exit code is 1

### Requirement: Install outside a repository
`--install` outside a git repository SHALL print `error: not a git repository` and exit with code 1.

#### Scenario: No repository
- **WHEN** `run-staged --install` runs outside any repository
- **THEN** it exits with code 1 and prints the not-a-git-repository error
