# cargo-distribution Specification

## Purpose
Defines how the `run-staged` binary is installed with Cargo, how the code is split into readable library crates, and what the binary needs at runtime.

## Requirements

### Requirement: Cargo installable binary
The project SHALL publish a crate named `run-staged` that installs one executable named `run-staged` through `cargo install`. The crate SHALL also install with `cargo install --git <repo-url> run-staged` and with `cargo install --path apps/run-staged`. No other crate in the workspace SHALL produce an executable named `run-staged`.

#### Scenario: Install from local path
- **WHEN** a user runs `cargo install --locked --path apps/run-staged --root <dir>`
- **THEN** `<dir>/bin/run-staged` exists, and running it with `--version` prints `run-staged <version>` and exits 0

#### Scenario: Only one run-staged executable
- **WHEN** the whole workspace is built
- **THEN** exactly one package produces an executable named `run-staged`

### Requirement: Stable toolchain build
The installable crate and all its library dependencies SHALL build on the stable Rust toolchain at or above the declared minimum supported Rust version. The build SHALL NOT require nightly-only compiler flags.

#### Scenario: Stable build
- **WHEN** `cargo +stable build --release -p run-staged` runs on a supported platform
- **THEN** the build succeeds

### Requirement: Self-contained runtime
The installed binary SHALL run without Node.js, without a `git` executable, and without any support files next to it.

#### Scenario: Runs without git on PATH
- **WHEN** the binary runs in a repository with a staged file, and PATH contains no `git` executable
- **THEN** it lists, backs up, runs, re-stages, and cleans up exactly as it does when git is installed

### Requirement: Focused library crates
Each area of behavior SHALL live in its own library crate, published as `run-staged-<area>`, with its own error type:
- `config`: discovery, parsing, and resolved settings.
- `git`: repository access, index backup, restore, staging, and hook install.
- `matcher`: glob matching.
- `exec`: command invocation.
- `output`: text formatting.
- `core`: orchestrates the other crates.

A crate SHALL depend only on the crates it needs. `matcher`, `exec`, and `git` SHALL NOT depend on each other. `output` SHALL depend on no workspace crate other than `exec`. The binary crate SHALL only parse arguments, call `core`, print, and map the result to an exit code.

#### Scenario: Binary delegates to core
- **WHEN** the core run returns an error
- **THEN** the binary prints `error: <message>` and exits with code 1

#### Scenario: Leaf crates are independent
- **WHEN** the workspace dependency graph is inspected
- **THEN** none of `run-staged-matcher`, `run-staged-exec`, or `run-staged-git` depends on another of those three, and `run-staged-output` depends only on `run-staged-exec` among workspace crates
