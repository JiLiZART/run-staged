## 1. Workspace skeleton

- [x] 1.1 Set `publish = false` in `crates/fast-staged/Cargo.toml`. Set root `members = ["crates/*", "apps/*"]` and move shared dependency versions into `[workspace.dependencies]`. Verify: `cargo test -p fast-staged` still passes.
- [x] 1.2 Create stub library crates `crates/{config,matcher,exec,git,output,core}` (packages `run-staged-<name>`, with `rust-version`, `description`, `license`, `repository`) and `apps/run-staged` (package and binary `run-staged`), following the design's dependency table. Verify: `cargo build --workspace` passes, and `cargo tree -p run-staged-output --edges normal` shows no workspace crate other than `run-staged-exec`.
- [x] 1.3 Wire clap in `apps/run-staged`: the current flags plus `--install` (conflicts with `--unstaged` and `--allow-empty`). Verify: `--version` prints `run-staged <ver>`, `--help` lists all flags, and `--quet` and `--install --unstaged` both exit 2.
- [x] 1.4 Now apply `add-cli-fixture-tests` so the remaining tasks can add fixture tests.

## 2. Port leaf crates

- [x] 2.1 `config`: port discovery, parsing, and cascade with the new file names (`.run-staged.toml`, `.run-staged.json`, `run-staged.toml`, `run-staged.json`, `package.json["run-staged"]`), plus its error enum. Verify: the ported unit tests pass, plus a test that `fast-staged.toml` is not discovered.
- [x] 2.2 `matcher`: port matching with its unit tests. Verify: `cargo test -p run-staged-matcher`.
- [x] 2.3 `exec`: port invocation, `{files}`, arg-budget chunking, timeout, and not-found handling, with `CommandOutcome`. Verify: `cargo test -p run-staged-exec`, with no `sh` in Windows-run tests.
- [x] 2.4 `output`: port success lines and failure boxes as `String` builders. Verify: `cargo test -p run-staged-output`.

## 3. Git crate (gix only, lock-safe)

- [x] 3.1 Open the repository with environment overrides (`GIT_DIR`, `GIT_WORK_TREE`) and resolve the index path (`GIT_INDEX_FILE` or the default). Verify: unit tests for each variable.
- [x] 3.2 Staged and unstaged listing by reading the index file directly and diffing HEAD-tree vs index in memory (no lock, no stat refresh). Verify: tests for "Committed files are not re-linted", "Deleted file excluded", and "Fresh repository", plus a test with a pre-existing `.git/index.lock` that stays unmodified.
- [x] 3.3 Worktree blob hashing through the gix filter pipeline, and partly staged detection. Verify: unit tests, including `core.autocrlf=true` with CRLF content that must not be reported as changed.
- [x] 3.4 Backup: copy the index to `<git-dir>/run-staged.index`. Restore: write the backup under `<index>.lock` (5s backoff), then delete the backup. On lock failure, keep the backup and return an error naming the lock and backup paths. Verify: unit tests for restore, a held lock that times out, and a lock released within 1s.
- [x] 3.5 Staging: write blobs, upsert entries with fresh stat, remove deleted entries, and write under the lock. Verify: after staging, a test's `git diff --cached` (test-only git use) shows the new content and `git status` shows the file clean.
- [x] 3.6 A guard test that no `git` process is spawned: run the whole git crate test suite with `PATH` stripped of git. Verify: the suite passes.

## 4. Core orchestration and binary

- [x] 4.1 `core::run(RunOptions, on_step) -> Result<RunReport, AppError>`, wiring config → git → matcher → backup → exec → restore or stage → cleanup, with an `AppError` that wraps the crate errors. Verify: a fixture test for every scenario in the existing main specs passes against `apps/run-staged`, and every MVP process-level test has a fixture equivalent.
- [x] 4.2 Binary: print outcomes through `output` and map errors to exit codes 1 and 2. Verify: fixture tests for "Runtime error format" and "Binary delegates to core".

## 5. New config settings and chunking

- [x] 5.1 Add the `read_only` and `chunk_size` keys (TOML and JSON), with cascade and defaults, and reject `chunk_size = 0`. Verify: unit tests for parsing, cascade, and the invalid-value error.
- [x] 5.2 Treat a zero timeout as none. Verify: a unit test, plus a fixture with `timeout = "0sec"` and `fixture-tool --sleep 1000` that succeeds.
- [x] 5.3 Chunk by `min(chunk_size, arg budget)`, honoring `execution_order`. Verify: the "Configured chunk size" fixture test (2 invocations in the log).

## 6. Re-staging and lock behavior (fixtures)

- [x] 6.1 Stage task edits from fully staged files matched by at least one group that is not read-only, and warn about partly staged ones. Verify: fixture tests for "Formatter fix is committed", "Read-only group does not stage", "Failed run does not stage", and "Unstaged hunk protected".
- [x] 6.2 Lock scenarios. Verify these fixture tests:
  - "Index locked by another git process"
  - "Commands may use git during the run"
  - "Busy lock released in time"
  - "Lock never released"
  - "Hook-provided index", using `GIT_INDEX_FILE=.git/index.lock`
- [x] 6.3 A real `git commit -a` end to end: install the hook, change a tracked file without staging it, and run `git commit -a` with a formatter config. Verify: the commit contains the formatted content (Unix-only test).
- [x] 6.4 "Runs without git on PATH": a fixture run with git removed from the child's PATH. Verify: the run succeeds and the edit is staged.

## 7. Output additions

- [x] 7.1 Emit step events from core and print step lines in non-quiet mode. Verify: fixture snapshots for a successful, a failed, and an `--unstaged` run.
- [x] 7.2 Print the summary line from `RunReport`. Verify: fixture tests for the summary after success, the summary after failure, and no summary with `--quiet`.

## 8. Hook install

- [x] 8.1 `git::install_hook`: read `core.hooksPath` through gix, write the marker script with mode 0755, rewrite the hook when the marker is present, and refuse a foreign hook. Verify: fixture tests for "Fresh install", "Custom hooks path", "Reinstall is idempotent", "Foreign hook kept", and "No repository".
- [x] 8.2 Commit through the installed hook with a failing command. Verify: `git commit` is aborted (Unix-only test).

## 9. Rename and CI

- [x] 9.1 Rename the product in `CLAUDE.md`, `ROADMAP.md`, `TODO.md`, and `.github/workflows/CI.yml`. Rename the repo's `fast-staged.toml` to `run-staged.toml` and fix its globs (`*.{js,ts,tsx,jsx}`). Verify: `grep -rn fast-staged` hits only `crates/fast-staged/` and archived specs.
- [ ] 9.2 CI: fmt, clippy `-D warnings`, and tests on Linux, macOS, and Windows, plus `cargo +<msrv> check -p run-staged`, then `cargo install --locked --path apps/run-staged --root <tmp>` and `<tmp>/bin/run-staged --version`. Verify: the CI run is green.
- [ ] 9.3 Run `cargo publish --dry-run` for each library crate in dependency order, then for `run-staged`. Verify: all succeed without metadata warnings.

## 10. Follow-ups

- [x] 10.1 Retarget `add-signal-handling`, `add-terminal-ui`, and `add-lint-staged-compat` to the new crate paths with `/opsx:update`. Verify: `openspec validate --all --strict` passes.
