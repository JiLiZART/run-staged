## Context

The MVP tests in `crates/fast-staged/tests/` repeat about 50 lines of `git init` / `git add` helpers in each file, write into fixed `temp_dir()` names (so parallel runs collide), and call `sh -c`, `sleep`, and `true`, which do not exist on Windows. `build-run-staged-cli` needs a harness before it ports that behavior (see its task 1.4). It also needs lock and `GIT_INDEX_FILE` scenarios, which the MVP never tested.

## Goals / Non-Goals

**Goals:**
- A fixture is either a directory you can read at a glance, or a short builder call.
- The same test gives the same result on all three CI operating systems.
- Snapshot the user-visible output, but assert side effects (index, working tree, invocations, lock files) directly.

**Non-Goals:**
- Mocking git inside the binary. Real repositories only; the binary under test is exactly what users run.
- A general-purpose test framework. The testkit API grows only as tests need it.
- Testing the npm launcher. That belongs to `add-npm-distribution`.

## Decisions

**`crates/testkit` (package `run-staged-testkit`, `publish = false`) with a library and a `[[bin]] fixture-tool`.** `apps/run-staged` takes it as a dev-dependency. Tests live in `apps/run-staged/tests/`, so `env!("CARGO_BIN_EXE_run-staged")` gives the binary under test for free.
- Alternative considered: a second binary in `apps/run-staged`. Rejected because `cargo install run-staged` would then install it too.

**Locating `fixture-tool`.** Cargo does not build a dev-dependency's binaries. On first use (guarded by a `OnceLock`), the testkit runs `$CARGO build -p run-staged-testkit --bin fixture-tool` with the same profile and target directory, then returns the path to the built binary.
- Alternative considered: re-entering the test executable as the tool with `harness = false`. Rejected because it loses libtest filtering and parallelism.
- Alternative considered: Cargo artifact dependencies. Rejected because they need nightly.

**Repository setup and inspection use the `git` CLI, not gix.** The harness must not share code with the thing it checks: if gix wrote a bad index, a gix-based assertion could read it back as fine. Environment for setup and for the binary:
- `GIT_CONFIG_NOSYSTEM=1`
- `GIT_CONFIG_GLOBAL=<empty file in tempdir>`
- `HOME` / `USERPROFILE` / `XDG_CONFIG_HOME` pointing at the tempdir
- fixed `GIT_AUTHOR_*`, `GIT_COMMITTER_*`, and `*_DATE`
- `init.defaultBranch=main` passed with `-c`

**The no-git PATH option** filters PATH entries that contain `git` / `git.exe`, then prepends the `fixture-tool` directory. The binary itself never execs git, so this proves the "No git executable" requirement end to end.

**Lock helpers.**
- `hold_index_lock(release_after: Option<Duration>)` creates `<index>.lock` with dummy bytes before spawning the binary. A helper thread deletes it after the delay. The helper records the lock's mtime and length, so tests can assert the binary never touched it.
- The `commit_a_env()` helper copies `.git/index` to `.git/index.lock` after the staged layer is applied, sets `GIT_INDEX_FILE` to it, and afterwards reads it back with `git --git-dir … ls-files -s` using `GIT_INDEX_FILE`.
- A real `git commit -a` test also exists (in `build-run-staged-cli`), so the simulation is checked against git's actual behavior.

**Fixture layers are copied, not referenced.** Each layer directory is copied into a `tempfile::TempDir`, followed by `git add -A` and `git commit`, then `git add -A` for `staged/`. Tests can never change the checked-in fixtures. `.gitkeep` files are skipped.

**Invocation log in `<git-dir>/fixture-tool.log`, one JSON object per line.** Each record is appended with a single `O_APPEND` write smaller than the pipe-buffer size, so parallel invocations never interleave. The log lives in the git directory, so it is never staged.

**Snapshots with `insta`.** Normalization runs before `assert_snapshot!`. Snapshots are stored next to the tests. The rules live in one function.

**Timeouts.** The binary runs in a child process with a `try_wait` loop plus a short sleep. No extra crate is needed.

## Risks / Trade-offs

- [The on-demand `cargo build` of `fixture-tool` slows the first test] → It runs once per test binary. In CI, `cargo build --workspace --all-targets` runs first, so it is a no-op.
- [`git` CLI differences across versions or platforms] → Only basic porcelain is used (`init`, `add`, `commit`, `config`, `show :path`, `diff --cached --name-only`, `ls-files -s`). The CI matrix catches drift.
- [Timing-based lock tests are flaky on slow CI runners] → Delays go in one direction only: release after 1s against a 5s limit, and never release against a 5s limit. There are no upper-bound timing assertions.
- [Windows file locking when a tempdir is deleted while a killed process still holds handles] → Kill and wait for the child before dropping the `TempDir`. Deletion errors are ignored.
