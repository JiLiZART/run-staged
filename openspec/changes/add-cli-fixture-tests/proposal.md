## Why

run-staged's behavior is mostly about side effects: what gets staged, what gets restored, which processes run with which arguments, and how it behaves while git holds the index lock. Unit tests cannot prove any of that. The MVP's process tests build repositories by hand in every test, depend on `sh` and `sleep` being present (so they break on Windows), and pick up the developer's global git config. We need one shared way to run the real binary against small, declarative git repositories that behaves the same on every OS.

## What Changes

- New test-only crate `crates/testkit` (package `run-staged-testkit`, not published):
  - Builds temporary git repositories from fixture directories, in an environment isolated from the user's git config.
  - Runs the compiled `run-staged` binary inside them and captures the result.
  - Provides assertions for exit code, output, index contents, working-tree contents, and recorded command invocations.
- A portable `fixture-tool` executable that stands in for linters and formatters. It records its arguments, can change or delete the files it receives, can sleep, and can exit with a chosen code.
- Helpers for lock scenarios: hold `<index>.lock` for a set time, and run with `GIT_INDEX_FILE` set, the way `git commit -a` does.
- Output is normalized before snapshot comparison: durations, temporary paths, and path separators.
- Fixture directories live in `apps/run-staged/tests/fixtures/<name>/`.
- CI runs the fixture suite on Linux, macOS, and Windows.

## Capabilities

### New Capabilities
- `fixture-testing`: fixture layout, repository isolation, the fake command tool, lock helpers, output normalization, and the cross-platform guarantee for end-to-end CLI tests.

### Modified Capabilities
None.

## Impact

- Code: new `crates/testkit/` (a library plus the `fixture-tool` binary), and new `apps/run-staged/tests/` with fixtures.
- Dependencies (dev only): `tempfile`, `insta`.
- Depends on task group 1 of `build-run-staged-cli`, which creates `apps/run-staged`. The later tasks of that change use this harness.
- The harness uses the `git` CLI to set up and inspect repositories. This keeps it independent of the gix code under test. The binary under test never needs git, and one helper removes git from its PATH on purpose.
