## Context

The MVP in `crates/fast-staged` (about 5.4k lines) implements the current main specs in one crate: `config/`, `git.rs`, `matcher.rs`, `commands.rs`, `output.rs`, and `lib.rs::run`. `git.rs` reads through gix, but it shells out to `git diff --cached` for the backup and to `git apply --cached` for restore. Nothing stages task edits. The npm name `fast-staged` is taken, and crates.io and the npm scope `@run-staged` are free, so the product is renamed to run-staged. For motivation, see proposal.md.

## Goals / Non-Goals

**Goals:**
- Each crate is small enough to read in one sitting and answers one question ("how is config found?", "how is the index written?").
- Keep working while a git process holds the index lock. Never depend on the `git` binary.
- Port MVP code and tests instead of rewriting them.

**Non-Goals:**
- Hiding unstaged hunks through stash (the lint-staged approach). Partly staged files are skipped with a warning.
- Restoring the working tree on failure. As before, only the index is restored.
- Publishing to crates.io. This change makes the crates publishable; releasing is separate.
- Keeping the MVP's config file names. The project has not shipped, so there is nothing to migrate.

## Decisions

**Crate map.** Folders use short names. Packages are named `run-staged-<name>`, because crates.io names are global.

| Folder | Package | Holds | Depends on |
|---|---|---|---|
| `crates/config` | `run-staged-config` | discovery, TOML/JSON parsing, `ResolvedGroup` settings | — |
| `crates/matcher` | `run-staged-matcher` | globset matching of paths to groups and patterns | config |
| `crates/exec` | `run-staged-exec` | shlex parsing, `{files}`, chunking, spawn, timeout, outcomes | config |
| `crates/git` | `run-staged-git` | repo open (env-aware), staged and unstaged listing, blob hashing, index backup, restore, staging, lock, hook install | — |
| `crates/output` | `run-staged-output` | success lines, failure boxes, step lines, summary (pure `String` builders) | exec (for `CommandOutcome`) |
| `crates/core` | `run-staged-core` | `run(RunOptions) -> RunReport`, step events, `AppError` that wraps each crate's error | all of the above |
| `apps/run-staged` | `run-staged` | clap, tokio runtime, printing, exit codes | core, output |

Each crate has its own `thiserror` error enum. `core::AppError` wraps them with `#[from]`, so `?` still works at the top level and every message stays close to the code that produces it.
- Alternative considered: one core crate with modules (the earlier draft). Rejected at the user's request: several small crates make it easier to see where behavior lives.
- `output` depends on `exec` for `CommandOutcome`. The outcome is produced by `exec`, so the type belongs there. `StepEvent` and `RunReport` live in `core`, and the binary turns them into plain values before calling `output`.
- Alternative considered: a separate `types` crate. Rejected because it would be one more crate just to break a single edge that does no harm.
- `MatchedGroup` / `MatchedPattern` (the matcher's output, which is also exec's input) are plain data and live in `config::types`. `matcher` re-exports them, and `exec` reads them without depending on `matcher`.

**The MVP stays untouched**, apart from `publish = false`. The new binary is named `run-staged`, so there is no clash over executable names and nothing in the MVP needs renaming.

**gix only; the lock model.** git's own rules: the lock for index path `P` is `P.lock`. During `git commit -a` / `git commit <paths>`, git keeps `.git/index.lock` locked and gives hooks `GIT_INDEX_FILE=.git/index.lock`. Hooks are expected to update that file (which is how `git add` inside lint-staged works: it locks `.git/index.lock.lock`). So:
- Open the repository with `gix::ThreadSafeRepository::discover_with_environment_overrides`, so `GIT_DIR` and `GIT_WORK_TREE` apply. Resolve the index path from `GIT_INDEX_FILE`, falling back to `repo.index_path()`.
- **Reads**: load the index with `gix::index::File::at(path, ...)` directly. This takes no lock and does no stat refresh. The HEAD tree comes from the odb, and worktree blob ids from `gix::objs::compute_hash` after the worktree-to-git conversion (`gix::filter::Pipeline`), so `autocrlf`/`eol` do not create fake changes. The staged list is the diff of the HEAD tree against the index, computed in memory. An existing `index.lock` never matters, because it is never opened.
- **Backup**: `std::fs::copy(index_path, git_dir/"run-staged.index")`. git replaces the index by atomic rename, so a copy is always a consistent file. This is simpler than generating and applying a patch, and needs no `git apply`.
- **Writes**: `gix_lock::File::acquire_to_update_resource(index_path, Fail::AfterDurationWithBackoff(5s), None)`, then write the new index into the lock and `commit()` (atomic rename). The lock guard releases on drop, so every early return (`?`) releases the lock.
  - Restore writes the backup bytes.
  - Staging loads the current index, writes blobs with `repo.write_blob`, and upserts entries with a fresh stat from the file. It removes entries for deleted files, then `index.write_to(lock)`.
- The lock is taken only for the few milliseconds of the write. It is never held while commands run, so commands that call git themselves are not blocked.
- Alternative considered: keep `git add` / `git apply` shell-outs. Rejected because they need the `git` binary, and they fail when the index is locked.
- Alternative considered: a patch backup written by gix-diff. Rejected because applying a patch needs a patch-apply engine, while a full-index snapshot restores exactly.

**Change detection.** Before the run, compute the worktree blob id for each matched path and compare it with the index entry. Unequal means partly staged: record it and exclude it from staging. After success, recompute: a changed or missing id is a task edit. mtime is not used, because it gives false positives when content is rewritten unchanged, and coarse filesystem timestamps can miss real edits.

**Step events and summary.** `core::run` takes a `&mut dyn FnMut(StepEvent)` callback and returns `RunReport { outcomes, file_count, elapsed, partial_warnings }`. The binary prints step lines and the summary through `output`. `add-terminal-ui` later plugs into the same callback.

**Hook script** (written by `git::install_hook`; `core.hooksPath` is read with gix config; mode 0755 on Unix):
```sh
#!/bin/sh
# run-staged
if [ -x ./node_modules/.bin/run-staged ]; then exec ./node_modules/.bin/run-staged "$@"; fi
exec run-staged "$@"
```
Git for Windows runs hooks through its bundled `sh`, so the same script works there.

**Zero timeout** resolves to `None` in `config`. It is not a parse error.

**MSRV.** Every crate gets `rust-version` = the lowest toolchain that actually builds the locked graph: 1.88. Edition 2024 alone needs 1.85, and declared dependency MSRVs top out at 1.87, but `human_format` (pulled in by gix's default `comfort` feature) uses let-chains without declaring an MSRV. That is why this was checked with `cargo +1.88 check`, not read from metadata. CI runs `cargo +1.88 check -p run-staged`.

## Risks / Trade-offs

- [gix's index-writing API changes between versions] → All writes stay in one module of `crates/git`, and gix is pinned in the workspace. Fixture tests cover restore, staging, deletion, and `GIT_INDEX_FILE`.
- [A full-index snapshot restore throws away index changes another process made during the run] → The restore happens only when the run fails, and the lint-staged patch approach has the same race. The window is the run itself, and the pre-commit context makes concurrent staging unlikely.
- [Entries written by the tool have stat data that does not match git's refresh] → The stat comes from the file right after hashing, so git's next `status` sees it clean or refreshes it cheaply.
- [Seven crates add some Cargo overhead] → They are all in one workspace with a single lockfile and shared `[workspace.dependencies]`. Publishing order is scripted in the release task.
- [The MVP drifts or confuses readers] → It is frozen, marked `publish = false`, and deleted in a follow-up once every MVP test has a fixture equivalent.

## Migration Plan

1. Create the workspace skeleton (`crates/*` stubs, `apps/run-staged` with `--version`).
2. Apply `add-cli-fixture-tests`.
3. Port crate by crate, leaves first (config → matcher, exec, git, output → core → app), with each crate's unit tests.
4. Add the new behaviors and rename the docs.
5. Retarget the pending changes to the crate paths.

Rollback: the MVP still builds as `fast-staged`.
