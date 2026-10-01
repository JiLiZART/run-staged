## Why

`crates/fast-staged` is an MVP that proved the core ideas (config, matching, execution, backup). But it is one large crate, it shells out to the `git` binary for backup and restore, and it is missing roadmap features. The biggest gap is re-staging the edits formatters make. Without it, the tool cannot replace lint-staged.

The name `fast-staged` is also already taken on npm, so the product needs a unique name before it is published anywhere. The project is renamed to **run-staged**.

## What Changes

- **Rename**: the binary, crate, config files, package.json key, hook marker, and backup file all change from `fast-staged` to `run-staged`.
- **Small crates**: each area gets a small library crate that can be read on its own. Folders are `crates/<short-name>` and packages are `run-staged-<name>`:
  - `crates/config`
  - `crates/git`
  - `crates/matcher`
  - `crates/exec`
  - `crates/output`
  - `crates/core`: orchestration
  - `apps/run-staged`: a thin binary crate, published as `run-staged`.
- **MVP kept as reference**: it stays in `crates/fast-staged`, unpublished and frozen.
- **Ported behavior**: the new binary implements all current main specs. Behavior is ported from the MVP, not redesigned, except for the changes listed below.
- **Cargo install**: `cargo install run-staged` works on stable Rust.
- **No git binary**: all repository access goes through gix. The tool reads the index without taking any lock, so it works while git holds `index.lock`, for example inside `git commit -a`. It also honors `GIT_INDEX_FILE`, `GIT_DIR`, and `GIT_WORK_TREE`. Writes take `<index>.lock` with a bounded wait.
- **BREAKING (vs MVP)**: the backup is now a snapshot of the index file at `<git-dir>/run-staged.index`. It replaces the `git apply`-based patch at `.git/fast-staged.patch`.
- **Re-staging task edits**: after a successful run, the tool stages changes that commands made to matched files. Partly staged files are skipped with a warning.
- **New group settings**:
  - `read_only`: files in the group are never re-staged.
  - `chunk_size`: caps how many files go into one invocation.
- **Zero timeout** means no timeout.
- **Output**: a run summary (affected files and total time), plus lint-staged-style step lines.
- **`--install`**: writes a git `pre-commit` hook, without husky.

## Capabilities

### New Capabilities
- `cargo-distribution`: Cargo install, crate layout contract, and runtime requirements.
- `hook-install`: the `--install` flag and how it writes the hook.

### Modified Capabilities
- `cli`: the binary is named `run-staged` and gains the `--install` flag.
- `config-loading`: the config files are renamed, the `read_only` and `chunk_size` keys are added, and a zero timeout means none.
- `command-execution`: batch mode follows `chunk_size`.
- `git-staging`: gix-only access, lock-safe reads, locked writes, index snapshot backup, re-staging, and partly staged files.
- `output-reporting`: adds the run summary and step lines.

## Impact

- **Code**:
  - New: `crates/{config,git,matcher,exec,output,core}/` and `apps/run-staged/`.
  - Root `Cargo.toml`: workspace members are now `crates/*` and `apps/*`.
  - The MVP's `Cargo.toml` gets `publish = false`.
- **Docs**: `CLAUDE.md`, `ROADMAP.md`, `TODO.md`, the repo's own config file (`fast-staged.toml` becomes `run-staged.toml`), and CI all move to the new name.
- **Pending changes**: `add-signal-handling`, `add-terminal-ui`, and `add-lint-staged-compat` already use the new name. After this change they need retargeting to the new crate paths.
- **Dependencies**: the same set as the MVP. The gix features for index writing and blob hashing must be enabled.
- **Tests**: process-level tests use `add-cli-fixture-tests`. Apply that change right after task group 1.
- **Out of scope**: npm distribution (`add-npm-distribution`), the TUI, signals, lint-staged compatibility, and hiding unstaged hunks.
