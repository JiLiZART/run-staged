## 1. Testkit crate

- [x] 1.1 Create `crates/testkit` (package `run-staged-testkit`, `publish = false`) with a library and a `[[bin]] fixture-tool`, and add it as a dev-dependency of `apps/run-staged`. Verify: `cargo build -p run-staged-testkit --bin fixture-tool` succeeds.
- [x] 1.2 Implement `fixture-tool` options (log record, `--sleep`, `--append`, `--replace`, `--delete`, `--stdout`, `--stderr`, `--exit`), writing JSON log lines to `<git-dir>/fixture-tool.log`. Verify: unit tests for each option, and a test with 20 parallel invocations that produces 20 intact log lines.
- [x] 1.3 Implement `fixture_tool_path()`: build once on demand with `$CARGO`, then return the path. Verify: a test calls it twice and gets the same existing path.

## 2. Repository builder

- [x] 2.1 Implement the isolated git environment and `Repo::from_fixture(name)`, which copies the `committed/`, `staged/`, and `unstaged/` layers and skips `.gitkeep`. Verify: tests for "Staged over committed", "Partially staged file", and "Repository without commits".
- [x] 2.2 Implement the in-code builder (layers, extra files, git config entries). Verify: the "Inline fixture with git config" test.
- [x] 2.3 Keep tempdirs when `RUN_STAGED_KEEP_FIXTURES=1` is set and print their path. Verify: a manual run shows the kept directory, and a default run leaves nothing behind.
- [x] 2.4 Isolation test: set a hostile `GIT_CONFIG_GLOBAL` / `HOME` on the parent process, then check that fixture repositories do not see `core.autocrlf` or `core.hooksPath`. Verify: the test passes.

## 3. Running, lock helpers, assertions

- [x] 3.1 Implement `repo.run(args)` / `run_in(subdir, args)` with extra env: the binary from `CARGO_BIN_EXE_run-staged`, `fixture-tool` first on PATH, the isolated env, and a timeout (default 30s) with kill and wait. Verify: tests for "Run in subdirectory" and "Hung binary".
- [x] 3.2 Add the no-git PATH option. Verify: the "Run without git" test, using a config whose command is `git status`.
- [x] 3.3 Add `hold_index_lock(release_after)`, reporting whether the lock was touched, and `commit_a_env()`. Verify: the "Lock released during run" and "Commit -a style index" tests.
- [x] 3.4 Add the accessors `index_content`, `worktree_content`, `staged_paths`, `git_dir_has`, and `invocations()` (reading through the git CLI and honoring a custom `GIT_INDEX_FILE`). Verify: the "Assert staged content" test.
- [x] 3.5 Add output normalization and `insta` snapshot helpers. Verify: unit tests for each rule (duration, `<repo>`, separators, CRLF), including the "Stable success line" example.

## 4. Adoption and CI

- [x] 4.1 Add seed fixtures (`basic-js`, `rust-crate`, `nested-config`, `partial-stage`, `no-commits`) and one smoke test per fixture against `apps/run-staged`. Verify: `cargo test -p run-staged` passes locally.
- [ ] 4.2 CI: `cargo build --workspace --all-targets` before `cargo test --workspace` on Linux, macOS, and Windows, with `INSTA_UPDATE=no`. Verify: the CI matrix is green, with no test skipped by accident on Windows.
- [x] 4.3 Document the fixture layout, the lock helpers, and `RUN_STAGED_KEEP_FIXTURES` in `crates/testkit/README.md`. Verify: the file exists and explains how to add a fixture.
