## Purpose

Ships the precompiled `run-staged` binary through npm, so Node.js projects can replace lint-staged with one dev dependency and no Rust toolchain.

## ADDED Requirements

### Requirement: Package layout
The project SHALL publish a main npm package `@run-staged/cli` that exposes a `run-staged` bin entry and contains no native code. It SHALL also publish one platform package per supported target, named `@run-staged/<platform>`, containing that target's release binary. Supported platforms SHALL be:
- `darwin-arm64`, `darwin-x64`
- `linux-x64-gnu`, `linux-arm64-gnu`
- `linux-x64-musl`, `linux-arm64-musl`
- `win32-x64-msvc`, `win32-arm64-msvc`

Each platform package SHALL declare matching `os` and `cpu` fields, and Linux packages SHALL also declare `libc`. The main package SHALL list every platform package in `optionalDependencies` at exactly its own version.

#### Scenario: Only the host binary is installed
- **WHEN** `npm install -D @run-staged/cli` runs on linux x64 glibc
- **THEN** `node_modules/@run-staged/linux-x64-gnu` is installed and no other platform package is installed

### Requirement: No install scripts or downloads
No published package SHALL declare `preinstall`, `install`, or `postinstall` scripts. Installing SHALL NOT fetch anything beyond the npm packages themselves.

#### Scenario: Install with scripts disabled
- **WHEN** `npm install -D @run-staged/cli --ignore-scripts` runs
- **THEN** `npx run-staged --version` prints `run-staged <version>`

### Requirement: Launcher behavior
The `run-staged` bin entry SHALL be a thin launcher that finds the binary for the current platform and runs it with:
- the same arguments;
- the same working directory and environment;
- the same stdin, stdout, and stderr handles.

The launcher SHALL exit with the binary's exit code. If the binary is killed by a signal, the launcher SHALL end the same way. SIGINT and SIGTERM received by the launcher SHALL reach the binary. The launcher SHALL NOT read or change the binary's output.

#### Scenario: Exit code passed through
- **WHEN** the binary exits with code 1 because a command failed
- **THEN** `npx run-staged` exits with code 1

#### Scenario: Arguments passed through
- **WHEN** a user runs `npx run-staged --quiet --config cfg.toml`
- **THEN** the binary receives `--quiet --config cfg.toml`

#### Scenario: Interactive output preserved
- **WHEN** the launcher runs in a terminal
- **THEN** the binary's stdout is a TTY

#### Scenario: Terminated by signal
- **WHEN** the launcher receives SIGTERM while the binary runs (on Unix)
- **THEN** the binary receives SIGTERM, and the launcher's exit status reports termination by SIGTERM

### Requirement: Platform resolution
On Linux, the launcher SHALL detect the C library (glibc or musl) and prefer the matching package. If only the other Linux package for the same CPU is installed, it SHALL use that one. When `RUN_STAGED_BINARY` is set, the launcher SHALL run that path and skip resolution.

#### Scenario: Alpine Linux
- **WHEN** the launcher runs on Alpine (musl) x64
- **THEN** it runs the binary from `@run-staged/linux-x64-musl`

#### Scenario: Explicit binary override
- **WHEN** `RUN_STAGED_BINARY` points to a locally built binary
- **THEN** the launcher runs that binary

### Requirement: Unsupported or missing platform
If no platform package matches the host, or the matching package is not installed (for example, because optional dependencies were skipped), the launcher SHALL:
- print an error naming the detected platform, the supported platforms, and two fixes: reinstall without omitting optional dependencies, or `cargo install run-staged`;
- exit with code 1.

#### Scenario: Optional dependencies omitted
- **WHEN** the project was installed with `--omit=optional` and a user runs `npx run-staged`
- **THEN** stderr names the missing `@run-staged/<platform>` package and the exit code is 1

### Requirement: Package manager compatibility
Installing and running through the `run-staged` bin SHALL work with npm (version 9 or later), pnpm (version 8 or later), and Yarn (1.x, and 4.x in both `node-modules` and Plug'n'Play linker modes).

#### Scenario: pnpm project
- **WHEN** a pnpm project adds `@run-staged/cli` as a dev dependency and runs `pnpm exec run-staged --version`
- **THEN** it prints `run-staged <version>`

#### Scenario: Yarn Plug'n'Play project
- **WHEN** a Yarn 4 PnP project adds `@run-staged/cli` and runs `yarn run-staged --version`
- **THEN** it prints `run-staged <version>`

### Requirement: Drop-in pre-commit usage
A project that installs `@run-staged/cli` SHALL be able to run it from a git pre-commit hook in either of two ways: a husky hook containing `npx run-staged` (or the package manager's equivalent), or, when the project has a `node_modules` layout, a hook written by `npx run-staged --install`. A failing command SHALL abort the commit in both cases. Yarn Plug'n'Play projects SHALL use the husky-style hook, and the migration docs SHALL say so.

#### Scenario: Husky hook
- **WHEN** `.husky/pre-commit` runs `npx run-staged` and a staged file fails a configured command
- **THEN** `git commit` is aborted

#### Scenario: Built-in hook with npm install
- **WHEN** `npx run-staged --install` was run and `run-staged` is not on the global PATH
- **THEN** the hook runs the project's `node_modules/.bin/run-staged` on commit

### Requirement: Version lockstep and release
Every npm package SHALL have the same version as the `run-staged` crate. A release SHALL publish all platform packages before it publishes `@run-staged/cli`. If any platform build or publish fails, `@run-staged/cli` SHALL NOT be published.

#### Scenario: Version reported
- **WHEN** `@run-staged/cli@X.Y.Z` is installed from npm
- **THEN** `npx run-staged --version` prints `run-staged X.Y.Z`

#### Scenario: Partial release blocked
- **WHEN** the `win32-arm64-msvc` build fails during a release
- **THEN** `@run-staged/cli` at that version is not published
