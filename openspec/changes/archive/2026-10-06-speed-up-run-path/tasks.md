## 1. Baseline

- [x] 1.1 Record the baseline: `bench/run.sh --scenarios overhead,scaling,partial-staging --tools run-staged,hk,lefthook` and save the run-staged means in this task's notes; verify `bench/results/scaling-10000.json` exists
  - Baseline (run-staged mean ± σ): overhead 52 ± 6 ms · scaling-10 40 ± 1 · scaling-100 44 ± 2 · scaling-1000 178 ± 80 · scaling-10000 1602 ± 187 · partial-staging 45 ± 2
- [x] 1.2 Add regression tests before refactoring, in `apps/run-staged/tests/restage.rs` (process-level; the detection logic spans git + core so unit tests there couldn't pin it): (a) same-size rewrite of a staged file in the same second is detected as changed; (b) a file with a stale index stat but unchanged content is not reported as changed or partial; (c) a partially staged file that a command modifies is reported as partial-modified, not staged; plus untouched-partial-no-warning and a 200-file case above the parallel threshold; verify they pass on current code with `cargo test -p run-staged --test restage`

## 2. Snapshot and compare (crates/git)

- [x] 2.1 Cache the read-only index in `Repo` (`OnceCell`); `staged_files` uses it, `stage()` still reloads under the lock and clears the cache; verify `cargo test -p run-staged-git` passes
- [x] 2.2 Add `Repo::snapshot(paths) -> Snapshot` (stat + id + partial per path, stat shortcut, hash only on mismatch); verify unit tests for clean, partial, deleted, symlink and stale-stat entries
- [x] 2.3 Add `Repo::changes(&Snapshot) -> Changes { changed, deleted, partial_modified }` with stat compare + racy guard (mtime before snapshot start, truncated to seconds); verify the 1.2 tests pass against it (the racy-mtime branch itself isn't unit-testable on APFS: ns mtime + ctime catch every same-size rewrite first; covered by `same_size_rewrite_is_staged`)
- [x] 2.4 Parallel hashing via `std::thread::scope` + per-thread `Hasher` from `ThreadSafeRepository::to_thread_local()` above 64 paths; verify a test with 200 files gives identical results in serial and parallel modes

## 3. Wire into core

- [x] 3.1 Replace `partially_staged`/`worktree_ids`/`stage_changes` in `run_staged_core::run` with `snapshot` → tasks → `changes` → `stage`; verify `cargo test --workspace` passes, including `apps/run-staged/tests/git_staging.rs`
- [x] 3.2 Delete `partially_staged` and `worktree_ids` if they have no remaining callers; verify `cargo clippy --workspace -- -D warnings` is clean
- [x] 3.3 Check the large-commit result: `bench/run.sh --scenarios scaling --tools run-staged` with `scaling-10000` ≤ 300 ms and `scaling-1000` ≤ 60 ms; if missed, re-time the phases in a scratch copy and add a follow-up task naming the slowest phase
  - 10k: 1602 → 586 ms after the snapshot change; phase re-timing showed (a) the fixture's index stats are stale (index carries the pristine copy's ino/ctime, so every file is hashed — a real commit right after `git add` has fresh stats) and (b) hashing through gix's filter pipeline stops scaling past ~4 threads (1 → 565 ms, 4 → 150 ms, 14 → ~1 s; work-stealing didn't help). Capped hash threads at 4: 10k now ~315 ms, 1000 ~62 ms with every file hashed. Follow-up: find the contended lock in the gix filter pipeline

## 4. Small-run fixed costs

- [x] 4.1 Switch `apps/run-staged/src/main.rs` to a `current_thread` tokio runtime (IO + time); verify the concurrency scenario still shows parallelism ≥ 3x and `cargo test --workspace` passes
  - 10 files 34 → 32 ms; concurrency 0.58 s (3.4x)
- [x] 4.2 Time the exec path for one `true` spawn in a scratch copy, remove the avoidable costs found, and record the findings in this task; verify `bench/run.sh --scenarios scaling --files 10 --tools run-staged` improves
  - run-staged's own exec path is ~0.13 ms (60 µs pre-spawn, 70 µs pipe drain). The ~8 ms per spawn is the child's lifetime when launched from a locally built Rust binary on this machine — std, tokio and raw `libc::posix_spawn` all show it, inside and outside the sandbox, with any stdio/env; Python and brew's hyperfine spawn the same `/usr/bin/true` in ~1 ms. Root cause unknown and outside our code; PATH lookup (~1.3 ms with 56 entries) is the only avoidable part and only matters for repeated spawns. No code change
- [x] 4.3 Time `match_files` on 10 files and cut glob-compilation overhead if it is ≥ 1 ms; verify matcher tests pass
  - The 3.2 ms was globset's first regex compile in a fresh process (2.2 ms; later builds 44 µs). Added a match-all fast path for `*`, `**`, `**/*` (lint-staged's documented `"*": prettier` case) with an equivalence test; 10 files → 29.7 ms
- [x] 4.4 Measure `opt-level = 3` vs `"z"` (`--version` and `scaling-10` means, binary size) and record the numbers; keep `"z"` unless the gain is ≥ 2 ms, and ask before switching
  - `opt-level = 3`: no gain (10-file phases 21–26 ms vs 21–23 ms with `"z"`; matching not faster). Keeping `"z"`, nothing to ask

## 5. Verify

- [x] 5.1 Run the full `bench/run.sh` and regenerate `bench/RESULTS.md`; verify the design.md targets are met (`scaling-10000` ≤ 300 ms, `scaling-1000` ≤ 60 ms, `scaling-10` ≤ 30 ms, 100-file overhead and partial-staging ≤ 35 ms) and that the unstaged table still shows run-staged "preserved: yes"
  - After (mean ± σ, rank of 8): overhead 32 ± 2 (2nd, hk 29) · scaling-10 29 ± 3 (2nd, hk 28) · scaling-100 31 ± 2 (1st) · scaling-1000 47 ± 2 (1st, hk 64) · scaling-10000 274 ± 11 (1st, lefthook 506) · partial-staging 32 ± 2 (2nd, hk 27) · concurrency 0.58 s (2nd); run-staged still preserves unstaged edits
- [x] 5.2 `cargo fmt -- --check` (clean for every file this change touches; 8 untouched files were already unformatted), `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`; verify all pass
