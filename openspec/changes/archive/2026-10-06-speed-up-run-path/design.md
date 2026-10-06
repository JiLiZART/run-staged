## Context

Measured by adding `eprintln!` phase timers to a scratch copy of `run_staged_core::run`, bench fixture `overhead-*`, M3 Max, warm cache:

| Phase | 10 files | 100 files | 10 000 files |
|:---|---:|---:|---:|
| config + repo open + staged list | 5 ms | 5 ms | 13 ms |
| match + plan | 3 ms | 3 ms | 8 ms |
| `partially_staged` (hash all) | 1.5 ms | 4.5 ms | 457 ms |
| `worktree_ids` before (hash all) | 0.6 ms | 3.4 ms | 453 ms |
| run commands (`true`) | 11 ms | 11 ms | 12 ms |
| `stage_changes` (hash all again) | 0.6 ms | 3.7 ms | 428 ms |
| outside `run()` (exec, runtime, main) | ~16 ms | ~14 ms | ~70 ms |

`run-staged --version` takes 9.4 ms vs hk's 3.6 ms. Hashing costs ~45 µs per tiny file because every file goes through `filter_pipeline().convert_to_git`, and it happens serially, three times. The code never uses the stat shortcut that `unstaged_files` already applies. After `rsync`, the fixtures' index stats are fresh (the same state a real commit sees right after `git add`), so a stat shortcut works in the benchmark just as it does in real use.

## Goals / Non-Goals

**Goals** (bench defaults, same machine; measured with `bench/run.sh`):
- `scaling-10000` ≤ 300 ms (now 1 447; fastest competitor is lefthook at 490)
- `scaling-1000` ≤ 60 ms (now 160; hk 68)
- `scaling-10` ≤ 30 ms (now 37; hk 27); `overhead`/`partial-staging` at 100 files ≤ 35 ms (now 45 / 51)
- Identical observable behavior: all existing unit and fixture tests pass unchanged

**Non-Goals:**
- Hiding unstaged edits from commands (a feature, not speed)
- Changing config, CLI, output or chunking semantics
- A permanent tracing or profiling flag. Timers stay in a scratch copy, not in the repo

## Decisions

**D1. One snapshot pass replaces `partially_staged` + the "before" `worktree_ids`.**
A new `Repo::snapshot(paths) -> Snapshot` stores, per path: `fs stat`, `id: Option<ObjectId>` and `partial: bool`. For each path, `symlink_metadata` runs first. If the unconflicted index entry's stat matches and is not racy (`Stat::matches` + `is_racy`, exactly as `unstaged_files` does today), then `id = entry.id` and `partial = false`, with no file read. Otherwise the file is hashed: `partial = (id != entry.id)`. Alternative: keep both passes and add the shortcut to each, which still does 2× the stats and keeps duplicated code. Rejected.

**D2. The "after" check compares stats and only hashes when needed.**
`Repo::changes(&Snapshot) -> Changes { changed, deleted, partial_modified }` re-stats every path. A path whose stat (size, mtime, ctime, ino, mode) equals the snapshot's, **and** whose mtime is older than the snapshot's start time truncated to whole seconds (racy-git guard for coarse-mtime filesystems), is unchanged and is not read. Any other path is hashed and compared with `snapshot.id`. A formatter that rewrites a file through rename changes its ino, and an in-place write changes its mtime; both get hashed. A rewrite that produces identical content is hashed, found equal, and not staged, which matches today's behavior. Alternative: compare only mtime and size, as some tools do. Rejected: it misses same-size rewrites within the timestamp granularity.

**D3. Parallel hashing above a threshold.**
The paths that need hashing in D1 and D2 are split into chunks across `std::thread::available_parallelism()` threads using `std::thread::scope`. Each thread gets its own `Hasher`, because `gix::filter::Pipeline` and `gix::Repository` are not `Sync`. Threads get a thread-local repo via `ThreadSafeRepository::to_thread_local()`; `Repo` keeps the `ThreadSafeRepository`, or reopens from the git dir. Below 64 files the loop stays serial, since spawning threads would cost more than it saves. No rayon: `std` covers this in ~25 lines.

**D4. Read the index once per run.**
`Repo` caches the read-only index (`OnceCell<gix::index::File>`) for `staged_files`, `snapshot` and `changes`. `stage()` keeps reloading **under the lock** (it must see concurrent writes) and drops the cache afterwards. `GIT_INDEX_FILE` handling is unchanged.

**D5. `core::run` wiring.**
`partially_staged` / `worktree_ids` / `stage_changes` become `snapshot` → run → `changes` → `stage(changed, deleted)`; `partial_modified` produces the warnings. The O(n²) `partial.contains` disappears because `partial` is a per-entry flag. `partially_staged` and `worktree_ids` are deleted if they have no other callers (both are `pub`; check for tests first).

**D6. Small-run fixed costs: measure first, then fix only what is proven.**
In order:
(a) Switch `tokio::runtime::Runtime::new()` (one worker per core, 14 threads here) to a `current_thread` runtime with IO and time enabled. Child processes, pipes and timeouts are all IO-driven, so this keeps concurrency.
(b) Measure the ~10 ms that spawning one `true` takes (tokio's SIGCHLD driver setup, PATH lookup and `canonicalize`, pipe draining) and remove what is avoidable.
(c) Measure the 3 ms spent in `match_files` (glob compilation per run).
(d) Try `opt-level = 3` vs `"z"`. **Keep `"z"`** unless the switch saves ≥ 2 ms at 10 files. If it does, report the binary size change and ask before switching, since a small binary is an explicit project goal.

## Risks / Trade-offs

- [The stat shortcut misses a content change that keeps the stat identical] → the racy guard in D2 and a full stat comparison including ino and ctime; the same trust model git uses. A test covers a same-size rewrite within the same second.
- [The thread-local repo per thread costs more than it saves on mid-size commits] → the 64-file threshold, checked against `scaling-100` and `scaling-1000`.
- [A cached index goes stale if a command runs `git add`] → only reads before the commands use the cache. `changes()` uses the snapshot rather than the index, and `stage()` reloads under the lock.
- [Benchmark noise hides a ~5 ms gain] → compare hyperfine means with σ on the same machine, and repeat any run whose σ exceeds 10% of the mean.

## Open Questions

- Whether to adopt `opt-level = 3`. This is decided by the D6(d) measurement and your call on binary size; it doesn't affect the other tasks.
