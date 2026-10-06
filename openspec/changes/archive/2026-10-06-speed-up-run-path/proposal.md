## Why

`bench/RESULTS.md` shows run-staged is 2nd at 10-100 staged files, but falls to 5th at 1 000 files (160 ms vs hk 68 ms) and last at 10 000 files (1 447 ms vs lefthook 490 ms). Phase timing of the 10 000-file run shows **~97% of the time is three serial passes that read and hash every staged file**: partial-staging detection (457 ms), the "before" snapshot (453 ms) and the "after" comparison (428 ms). Running the commands takes 12 ms. At 10 files we trail hk by ~10 ms: process startup (`--version` takes 9.4 ms vs 3.6 ms for hk), ~10 ms of exec overhead to spawn one `true`, and 3 ms of glob matching.

The tool's whole value is "faster than lint-staged". Being slower than four competitors on large commits contradicts that.

## What Changes

- **One snapshot pass instead of three hash passes.** Use git's stat shortcut: a file whose metadata matches its fresh index entry has the index's blob id, so it doesn't need hashing. That gives both the partially-staged set and the "before" state from a single pass that hashes only the files with mismatching stats.
- **Stat-based "after" check.** After the commands run, re-stat the candidates and hash only files whose stat changed (or might have changed, using the racy-git rule). Unchanged files are never read again.
- **Parallel hashing.** The files that still need hashing are hashed across all cores.
- **Load the index once** per run instead of in every `Repo` method. Look up partially-staged files in a set instead of the current O(n²) scan.
- **Cheaper small runs:** investigate and cut the fixed costs measured at 10 files (process startup, async runtime setup, per-spawn exec overhead, matcher setup).
- **No behavior change.** The same files are staged, the same warnings are printed and the same exit codes are returned. Existing tests must keep passing.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
<!-- none: performance only; every requirement in git-staging, command-execution and file-matching is unchanged. skip_specs is set. -->

## Impact

- `crates/git` (`Repo`: snapshot/compare API, index caching, parallel hasher), `crates/core` (`run` uses the snapshot), possibly `crates/exec`, `crates/matcher` and `apps/run-staged/src/main.rs` (runtime/startup).
- No config, CLI or output changes. No new dependencies are expected (`std::thread::scope` for parallelism).
- Success is measured with `bench/run.sh`. Targets are listed in design.md.
