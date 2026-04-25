# Phase 5: Execution Core - Context

**Gathered:** 2026-04-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Run matched commands against staged files. Parallel-by-default with sequential opt-in, glob-based file matching with matchBase semantics, per-command timeout enforcement, non-zero exit propagation, `--allow-empty` support, and restore-on-failure wiring.

**In scope:** EXEC-01, EXEC-02, EXEC-03, EXEC-05, EXEC-06, EXEC-07.

**Out of scope (deferred):**
- EXEC-04 Ctrl+C / signal handling — Phase 7
- TUI rendering with ratatui — Phase 8
- `--config <path>`, `--version`, `--help` flags — Phase 6 (CLI Interface)

</domain>

<decisions>
## Implementation Decisions

### Glob Matching (EXEC-06)
- **D-01:** Use `globset` crate. Compiles all patterns into a single `GlobSet` matcher; matches the lint-staged-class workload (N patterns × M files) efficiently. Rejects `glob` (no batch compilation) and `globwalk` (walks fs — we already have the file list).
- **D-02:** matchBase semantics — auto-prefix `**/` to slashless patterns. Internally rewrite `*.ts` → `**/*.ts`. Patterns containing `/` (e.g., `src/*.ts`) are kept literal and do NOT recurse. Predictable and ergonomic; matches lint-staged behavior.
- **D-03:** Negation patterns honored. A pattern starting with `!` is added to an exclude `GlobSet`; final match = `include.is_match(p) && !exclude.is_match(p)`. Order within a group doesn't matter — exclude always wins. Matches lint-staged.
- **D-04:** Always case-sensitive. `*.TS` does NOT match `foo.ts`. Predictable across platforms. Matches git's default and lint-staged. No per-group `case_insensitive` knob in this phase.

### Command Spawn & Argument Passing (EXEC-01, EXEC-02)
- **D-05:** Parse command strings with `shlex` (shell-like word splitting), then spawn via `tokio::process::Command::new(argv[0]).args(&argv[1..])`. NO shell wrapping. Faster, safer (no metachar injection from filenames), cross-platform. Children that need pipes/redirects are out of scope — users wrap such cases in a script themselves.
- **D-06:** File-arg passing supports BOTH a `{files}` placeholder AND positional append:
  - If the parsed argv contains a token equal to `{files}`, replace that token with the file list (expanded inline at that position).
  - Otherwise, append all files positionally after the parsed argv.
  - Single behavior covers both lint-staged-style (`prettier --write`) and tools that need files mid-command (`cmd --files {files} --strict`).
- **D-07:** Add `path_format: PathFormat { Relative, Absolute }` to `Group` (and `RawGroupConfig`/`RawJsonGroupConfig`). Default = `Relative` (paths relative to git root, matching lint-staged). `Absolute` = canonicalized full paths. Cascading defaults via existing `defaults::resolve_group()` pattern. Phase 5 wires the config field AND uses it during arg construction.
- **D-08:** Use system PATH for command resolution (default `Command::new` behavior). `ENOENT`/`NotFound` from spawn maps to `AppError::CommandNotFound { command }` with an actionable hint ("Is it installed and in PATH?").

### Output Capture & Reporting (pre-TUI)
- **D-09:** Capture per-command stdout+stderr in memory via `Command::output().await`. Successful commands emit no captured output. Failed commands dump their captured stderr (and stdout) AFTER the run completes. Phase 8 TUI replaces this presentation layer; the capture mechanism stays.
- **D-10:** Successful run summary: one line per command — `✓ {group}/{pattern}: {cmd} ({duration}, {N} file(s))`. Honors `--quiet` (suppressed when set). Errors always print regardless of `--quiet`.
- **D-11:** Failure output: bordered/boxed section per failed command containing: command string, exit code, captured stderr (and stdout if non-empty). Multiple failures stack visually. Border styling defers to a small `print_error_box()` helper — Phase 8 will replace with ratatui rendering.
- **D-12:** `--quiet` semantics in this phase = suppress success summary lines only. Errors always print. Reserves the flag for its long-term meaning (no TUI redraw) so behavior doesn't change when Phase 8 lands.

### Failure & Exit-Code Semantics (EXEC-03)
- **D-13:** Group-level failure: respect `Group.continue_on_error`. When `false` (default) and a command fails, signal cancellation to in-flight siblings IN THAT GROUP via a per-group `Arc<AtomicBool>` and let `tokio::select!` drop them. When `true`, let in-flight siblings finish; collect all failures.
- **D-14:** Cross-group failure: respect global `Config.continue_on_error` (Phase 3 D-09). When `false` (default) and a group has any failure, skip subsequent groups. When `true`, run every group regardless.
- **D-15:** Exit code: ANY command failure produces non-zero exit (1). `continue_on_error` controls whether OTHER work runs — NEVER whether failures are reported. Satisfies EXEC-03 cleanly. Matches lint-staged.
- **D-16:** Parallelism is unlimited — `tokio::task::JoinSet` spawns all matched commands without a concurrency cap. Each command is its own OS process; the OS scheduler handles CPU contention. No `max_concurrent` config in Phase 5.

### Timeout Enforcement (EXEC-05)
- **D-17:** Timeout precedence cascades: group `timeout` overrides global `timeout`; if neither is set, NO timeout is enforced. Already aligned with Phase 3 `defaults::resolve_group()` resolution.
- **D-18:** Granularity = per spawn. Each individual `Command::output()` future is wrapped in `tokio::time::timeout(duration, ...)`. With `per_file=true` and 100 files, each invocation gets the full timeout.
- **D-19:** On timeout (Unix): send SIGTERM via `nix::sys::signal::kill(Pid, Signal::SIGTERM)` (or `tokio::process::Child::start_kill` if it provides SIGTERM access — verify in research), wait up to 2 seconds for the process to exit, then SIGKILL via `Child::kill().await`. Map result to `AppError::CommandTimeout { command, duration }`.
- **D-20:** On timeout (Windows): no SIGTERM equivalent — invoke `Child::kill().await` directly (TerminateProcess under the hood). No 2-second grace window on Windows. Document the platform difference in the timeout docstring.

### --allow-empty Scope (EXEC-07)
- **D-21:** `--allow-empty` suppresses BOTH `AppError::NoStagedFiles` (Phase 4 git layer) AND `AppError::NoFilesMatched` (Phase 5 matching layer). When the flag is set and either fires, exit 0 silently — no stderr, no stdout. User intent: "this hook is allowed to be a no-op."
- **D-22:** `--unstaged` + `--allow-empty` combo: same suppression. `unstaged_files()` already returns `Ok(vec![])` for a clean worktree; with `--allow-empty` set, post-glob empty also exits 0. Symmetric with the staged path.
- **D-23:** When `--allow-empty` triggers a no-op exit, skip backup/cleanup entirely. No git work needed if no commands run.

### Per-File vs Batch Invocation
- **D-24:** When `Group.per_file = true`, each matched file produces ONE spawn of the command. Per-file spawns within a group respect the group's `execution_order` — `Parallel` spawns N processes concurrently; `Sequential` runs them one at a time. Same knob covers per_file and batch modes.
- **D-25:** Batch (per_file=false) without `{files}` placeholder: append all files as positional arguments — `cmd file1 file2 file3 ...`. Most tools accept this (eslint, prettier, biome).
- **D-26:** Auto-chunk batch invocations to respect platform `ARG_MAX`. On Unix, query via `sysconf(_SC_ARG_MAX)`; on Windows, use 32767 (the documented `CreateProcess` limit). Subtract a 25% headroom buffer for environment variables. Files split into chunks; each chunk runs as a separate sequential invocation within the same group entry. Required for monorepos with 1000+ staged files.
- **D-27:** Empty file list (group has patterns but zero matches): skip the command entirely — do NOT spawn. Many tools (eslint with no args) lint the entire repo when invoked empty — that's catastrophic. Skip silently with no error.

### Restore-on-Failure Wiring
- **D-28:** Closes the TODO at `lib.rs:54`. On ANY command failure during execution: call `git_ctx.restore()`, then `git_ctx.cleanup()`. On success: just `git_ctx.cleanup()`. Uses a drop-guard or explicit branching in `run()`. Skip restore+cleanup entirely in `--unstaged` mode (Phase 4 D-12 — no backup taken in that mode).
- **D-29:** Restore scope = WORKTREE ONLY. The patch is the HEAD-vs-index diff (Phase 4 backup). Re-applying it reverts tool-modified worktree files back to the state that was originally staged. User's staged changes in the index are preserved — never hard-reset.
- **D-30:** If `restore()` itself fails (corrupt patch, conflict): surface as `AppError::RestoreFailed { patch_path, source }` with the absolute patch path in the message. **Do NOT delete the patch file** when restore fails — it's the user's only manual recovery path (`git apply .git/fast-staged.patch`). cleanup() is skipped in this branch.
- **D-31:** Order on failure: restore() FIRST, cleanup() SECOND. Restore failure aborts cleanup (D-30). Restore success → cleanup proceeds. Single guard pattern (e.g., `match execute_all().await { Ok(_) => cleanup(), Err(e) => { try_restore(); cleanup_unless_restore_failed(); Err(e) } }`).

### Claude's Discretion
- Internal module layout: whether to split into `match.rs` + `executor.rs` + `output.rs` or keep flat. Recommend flat-ish — `match.rs`, `commands.rs` (executor + spawn helpers), `output.rs` (failure box rendering).
- `TaskState` / `CommandStatus` / `Message` enum design. The ROADMAP architecture pre-specifies these, but Phase 5 doesn't need the live-update infrastructure yet (Phase 8 does). Build minimal shapes now that Phase 8 can extend.
- Test layout — unit tests per module + integration tests using the existing `make_real_git_repo` helper from Phase 4.
- Exact wording of `AppError::CommandNotFound`, `CommandFailed`, `CommandTimeout`, `RestoreFailed` messages.
- Signal sending mechanism on Unix: `nix` crate vs `libc` raw syscall vs whatever tokio exposes. Researcher should pick.

### Folded Todos
None — no pending todos matched this phase.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Specs
- `.planning/ROADMAP.md` — Phase 5 success criteria (6 items) and requirement IDs (EXEC-01, 02, 03, 05, 06, 07)
- `.planning/REQUIREMENTS.md` — EXEC-01 through EXEC-07 requirement definitions (note EXEC-04 is Phase 7)
- `.planning/PROJECT.md` — Constraints (tokio, no shell-out for git but commands DO spawn processes)

### Codebase Maps
- `.planning/codebase/ARCHITECTURE.md` — Pipeline diagram, planned `Commands::execute()` shape, StateModel/Message/TaskState references
- `.planning/codebase/STACK.md` — Async runtime is tokio with full features
- `.planning/codebase/CONVENTIONS.md` — `AppError` variant style, `Result<T, AppError>` signatures, snake_case modules
- `.planning/codebase/STRUCTURE.md` — Workspace layout, planned `crates/fast-staged/src/commands.rs`
- `.planning/codebase/TESTING.md` — Test patterns: `#[cfg(test)] mod tests`, integration tests under `crates/fast-staged/tests/`

### Prior Phase Context (load-bearing decisions inherited)
- `.planning/phases/02-config-core/02-CONTEXT.md` — Config struct decisions (D-01 to D-13)
- `.planning/phases/03-advanced-config/03-CONTEXT.md` — Group ordering (D-01/02), continue_on_error semantics (D-08/09), per-group cascading (D-06/07)
- `.planning/phases/04-git-integration/04-CONTEXT.md` and `04-VERIFICATION.md` — backup/restore/cleanup contract, `--unstaged` semantics, lib.rs:54 TODO origin

### Existing Code (Phase 5 builds on these)
- `crates/fast-staged/src/lib.rs` — `run(unstaged: bool)`; integration point is between `backup()` and `cleanup()` calls (currently has placeholder eprintln blocks and lib.rs:54 TODO)
- `crates/fast-staged/src/git.rs` — `GitContext::backup()`, `restore()`, `cleanup()`, `staged_files()`, `unstaged_files()`
- `crates/fast-staged/src/config/types.rs` — `Group` struct (needs `path_format` field added), `RawGroupConfig`, `Config`, `ExecutionOrder`
- `crates/fast-staged/src/config/defaults.rs` — `resolve_group()` pattern; extend for `path_format` cascading
- `crates/fast-staged/src/config/parse_toml.rs` and `parse_json.rs` — Add `path_format` to allowed fields (RawGroupConfig has `deny_unknown_fields`)
- `crates/fast-staged/src/error.rs` — `AppError` enum; new variants needed: `CommandFailed`, `CommandTimeout`, `CommandNotFound`, `RestoreFailed` (verify which already exist)
- `crates/fast-staged/src/main.rs` — currently parses `--unstaged`; will need `--allow-empty` and `--quiet` parsing surfaced into `run()`

### External Crate Docs (researcher should validate against current versions)
- `globset` — pattern compilation, builders, set matching
- `shlex` — POSIX-shell-compatible word splitting
- `tokio::process::Command` and `tokio::time::timeout`
- `tokio::task::JoinSet` — managing parallel command futures

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Group` already has `timeout: Option<Duration>`, `execution_order: ExecutionOrder`, `continue_on_error: bool`, `per_file: bool`. Phase 5 only needs to ADD `path_format` and consume the rest.
- `defaults::resolve_group()` cascading-defaults pattern is the template for `path_format` resolution.
- `git_ctx.backup()`/`restore()`/`cleanup()` are ready and tested — Phase 5 just needs to wire failure → restore.
- Phase 4 added `make_real_git_repo` integration-test helper — reuse for Phase 5 integration tests that exercise full pipeline.
- `AppError` enum is the standard error surface; extend with new variants (don't introduce `anyhow` or `Box<dyn Error>`).

### Established Patterns
- Module layout: `src/foo/mod.rs` re-exports + submodules (see `config/`).
- Raw* deserialization structs → resolved typed structs. Apply this pattern to any new config fields.
- Tests colocated in `#[cfg(test)] mod tests` blocks per module + integration tests under `crates/fast-staged/tests/`.
- `parse_*_str()` helpers for testing parsers without filesystem.
- `#[serde(deny_unknown_fields)]` on RawGroupConfig means `path_format` MUST be added to that struct or parse will reject configs that use it.
- Existing code uses `tokio` everywhere; stay tokio-only (no `async-std`, no `smol`).

### Integration Points
- `lib.rs::run()` lines ~24–55: replace the debug eprintln block + Phase 5 TODO with `match::match_files(&config, &files)?` + `commands::execute(matched, &config).await?` + restore/cleanup wiring per D-28/D-31.
- `main.rs`: extend CLI parsing to surface `--allow-empty` and (optionally) `--quiet` into `run()` signature. Phase 6 will replace ad-hoc parsing with clap; Phase 5 can keep the manual `std::env::args` style or do a small clap intro — researcher decides.
- Config layer needs `path_format` field added end-to-end: types.rs → RawGroupConfig (TOML) → RawJsonGroupConfig → defaults::resolve_group() → tests in parse_toml.rs/parse_json.rs.

</code_context>

<specifics>
## Specific Ideas

- Match lint-staged behavior wherever there's no good reason to diverge — users migrating from lint-staged should not be surprised. Specifically: matchBase via `**/` prefix, negation with `!`, case-sensitive default, positional file append, batch chunking.
- Failure box rendering should be simple ASCII for now — Phase 8 will replace with ratatui. Keep the `print_error_box()` function isolated so the swap is mechanical.
- Don't pre-build StateModel/Message/Channel infrastructure for the TUI yet — Phase 8 needs it; Phase 5's pre-TUI mode just collects results into a `Vec<CommandOutcome>` and prints at the end.

</specifics>

<deferred>
## Deferred Ideas

- **EXEC-04 Ctrl+C / signal handling** — Phase 7 (Signal Handling & Resilience). Phase 5 builds the cancellation primitive (`Arc<AtomicBool>` per group); Phase 7 wires the signal handler into it.
- **Live TUI rendering** — Phase 8. Phase 5 must NOT couple to ratatui; capture-and-print only.
- **`--config <path>`, `--version`, `--help`** — Phase 6 (CLI Interface). Phase 5 extends `run()` to accept `allow_empty: bool` and `quiet: bool` parameters; main.rs wiring stays minimal until Phase 6 introduces clap.
- **Configurable max parallelism (`max_concurrent`)** — out of scope. Reconsider after real-world feedback if anyone reports system overload.
- **`case_insensitive` per-group glob option** — out of scope. Reconsider if requested.
- **Stdin-based file passing (`--stdin-filepath` style)** — out of scope. Tools that need this can be wrapped by the user.
- **Auto-detect shell metachars and fall back to sh -c** — out of scope. Direct exec only; users wrap pipe/redirect commands in a script.

### Reviewed Todos (not folded)
None — no todos surfaced for this phase.

</deferred>

---

*Phase: 05-execution-core*
*Context gathered: 2026-04-25*
