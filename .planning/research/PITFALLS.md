# Domain Pitfalls: Git Pre-Commit Staged File Runner (fast-staged)

**Domain:** Git pre-commit hook tool — runs configured commands against staged files
**Researched:** 2026-04-09
**Confidence:** HIGH (based on lint-staged source history, git internals, Rust async toolchain patterns)

---

## Critical Pitfalls

Mistakes that cause data loss, silent corruption, or require rewrites.

---

### Pitfall 1: Partial Staging Corruption (the #1 cause of data loss in this domain)

**What goes wrong:**
A user stages only *some* changes in a file (`git add -p`). The working tree version differs from the index version. A formatter or linter runs, rewrites the file on disk, and `git add` re-stages the entire modified file — silently overwriting the partial staging intent. The commit now contains unstaged changes the user never intended to include.

**Why it happens:**
Commands like `prettier --write` or `cargo fmt` modify files in the working tree. If the tool re-stages files after running (which lint-staged does with `git add`), the working tree content replaces the carefully curated index content.

**Consequences:**
- Silent data loss of unstaged hunks — user commits changes they didn't intend to
- Corrupted git history that is hard to recover
- User trust destroyed on first occurrence

**Prevention:**
1. Before running any commands, snapshot the index with `git stash --keep-index` or write a patch: `git diff HEAD -- <staged files>` saved as `.git/fast-staged.patch`
2. After commands complete, do NOT blindly re-add files; instead restore the index to its pre-run state using `git apply --cached .git/fast-staged.patch`
3. The planned `.git/fast-staged.patch` backup (from ROADMAP) is correct — but it must also be *applied back* to restore the index, not just kept as a recovery artifact
4. The stash approach (`--stash` flag) must stash unstaged changes *before* running and restore them *after* — this is the safe sequence lint-staged uses

**Warning signs:**
- Tests pass locally but commit contains unexpected changes
- `git diff HEAD~1 HEAD` shows more changes than expected
- Users reporting "I only staged X but Y got committed"

**Phase:** Implement in the git layer (Phase with `GitContext::create_staged_diff()` and stash/restore). This must be correct before any other feature.

---

### Pitfall 2: Stash Restore Fails Silently on Merge Conflicts

**What goes wrong:**
After commands run (formatters modify files), restoring the stash of unstaged changes produces a merge conflict because the formatter changed the same lines. The tool either crashes, hangs, or — worse — silently drops the restore and leaves the working tree in a dirty, unknown state.

**Why it happens:**
`git stash pop` can fail with conflicts just like a merge. If the restore step is not validated for success and the tool exits 0, the user has lost their unstaged working tree changes.

**Consequences:**
- Unstaged work lost permanently if not recoverable
- Working tree left in a conflict state with conflict markers committed
- User has no indication restore failed

**Prevention:**
1. Always check the exit code of the stash restore operation
2. If stash restore fails, abort the commit (return non-zero exit), print a clear message listing what is in the stash ref
3. Keep the stash ref until verified successfully applied; do not drop it preemptively
4. When using `gix` for stash: verify the apply operation succeeded before proceeding

**Warning signs:**
- Stash restore returning non-zero in tests
- Merge conflict markers appearing in files after tool runs
- `.git/refs/stash` still present after tool exits

**Phase:** Stash/restore logic in the git layer. Must be tested with partial staging scenarios before shipping.

---

### Pitfall 3: Orphaned Child Processes on Interrupt

**What goes wrong:**
User presses Ctrl+C during execution. The Rust process exits but child processes (eslint, cargo fmt, etc.) continue running in the background. On Unix these inherit the same terminal and can produce interleaved output or hold file locks. On Windows, child processes in a job object may or may not be cleaned up depending on how they were spawned.

**Why it happens:**
`tokio::process::Command` spawns children; dropping the `Child` handle does NOT kill the process — it detaches it. Simply cancelling the tokio task does not send SIGKILL to the child.

**Consequences:**
- Tool appears to have exited but formatters still write to disk
- Next `git commit` attempt fails due to file locks (Windows)
- CPU/disk thrash from abandoned long-running lint passes

**Prevention:**
1. Store child `Child` handles; on cancellation call `child.kill().await` explicitly
2. On Unix: use `kill(pid, SIGTERM)` followed by a grace period then `SIGKILL` via the `nix` crate or libc
3. Register a drop guard or use `tokio_util::sync::CancellationToken` that triggers kills
4. Test: send SIGINT during execution, verify no zombie processes with `ps aux`

**Warning signs:**
- `ps` shows `cargo fmt` or `eslint` still running after tool exits
- Subsequent commits fail with "file already in use" on Windows
- CI machines accumulating zombie processes

**Phase:** Signal handling implementation. The `Arc<AtomicBool>` cancellation flag in the plan is the right concept — but killing actual child processes requires explicit `child.kill()`, not just checking the flag.

---

### Pitfall 4: ratatui Terminal State Not Restored on Panic or Error

**What goes wrong:**
The TUI calls `ratatui::init()` which switches the terminal to raw mode and hides the cursor. If the program panics, returns an error through `?`, or is killed with SIGKILL, the terminal is left in raw mode: no echo, no line wrapping, cursor hidden. The user's shell becomes unusable until they run `reset`.

**Why it happens:**
`ratatui::restore()` must be called explicitly on every exit path. Panics bypass `Drop` implementations unless the panic hook calls restore. Error propagation with `?` skips restore if it's only in the happy path.

**Consequences:**
- Developer's terminal is broken after every crash during development
- Experienced frequently during the build phase — every bug leaves terminal corrupted
- End users lose trust immediately on first bad experience

**Prevention:**
1. Install a custom panic hook that calls `ratatui::restore()` before printing the panic message
2. Use RAII: wrap terminal state in a struct whose `Drop` impl calls `ratatui::restore()`
3. In `main()`, use a pattern like: `let result = run(); ratatui::restore(); result?`
4. Test: introduce a deliberate panic in a command handler; verify terminal is usable after

**Warning signs:**
- Terminal appears broken (no echo) after killing the process in development
- `tput reset` or `reset` needed after every test run

**Phase:** TUI layer setup. Must be correct from the first commit that touches ratatui — fixing it later means auditing every exit path retroactively.

---

### Pitfall 5: Glob Matching Semantics Diverge from lint-staged

**What goes wrong:**
Users migrate from lint-staged and copy their config patterns. The glob implementation chosen for fast-staged uses different semantics: `*.js` in lint-staged matches files in any subdirectory (it uses micromatch with `matchBase: true`), but most Rust glob crates require `**/*.js` for the same behavior. Users' existing configs silently match zero files.

**Why it happens:**
lint-staged's `micromatch` library enables `matchBase` by default, which makes `*.js` equivalent to `**/*.js`. Standard POSIX glob and most Rust glob crates (`glob`, `globset`, `wax`) do not do this.

**Consequences:**
- Commands silently never run (no files matched)
- Users think the tool is broken when it's a glob semantic difference
- Hard to debug: no error, just silence

**Prevention:**
1. Implement `matchBase` behavior: if a pattern contains no `/`, treat it as `**/<pattern>` during matching
2. Document this behavior explicitly in the README and config reference
3. In the `NoFilesMatched` error, include which patterns were tried and against which files, so users can debug
4. Add a test suite with lint-staged-parity glob cases

**Warning signs:**
- Users reporting "my patterns worked in lint-staged but not in fast-staged"
- Patterns like `*.ts`, `*.rs`, `*.json` matching zero files in subdirectory repos

**Phase:** Matching layer. Must be validated against lint-staged's pattern test suite before shipping compatibility claims.

---

### Pitfall 6: Exit Code Not Propagated to Git

**What goes wrong:**
The pre-commit hook invokes fast-staged. A linter fails. fast-staged exits 0 (success). Git proceeds with the commit. The point of the tool — blocking bad commits — is completely defeated.

**Why it happens:**
- `main()` catches an error, prints it, but returns `Ok(())` accidentally
- `std::process::exit()` is called with 0 in an error handler
- TUI cleanup code swallows the error return
- On Windows, exit codes above 255 wrap around

**Consequences:**
- Every commit goes through regardless of linter failures
- Tool appears to work but provides zero protection
- Silent regression that may go unnoticed for weeks

**Prevention:**
1. `main()` must return `ExitCode` or call `std::process::exit(1)` on any `AppError`
2. Add an integration test: run fast-staged with a command that exits 1; assert fast-staged exits non-zero
3. Ensure TUI restore and cleanup code runs *before* returning the exit code, not instead of it
4. Never use `.unwrap_or(())` or `.ok()` on the top-level run result

**Warning signs:**
- `echo $?` shows 0 after a known-failing linter run
- CI passes despite introducing syntax errors

**Phase:** Error handling layer (foundational). Must be the first tested behavior in the project — before any feature work.

---

## Moderate Pitfalls

---

### Pitfall 7: Config Discovered from Wrong Directory

**What goes wrong:**
fast-staged is invoked from a subdirectory (monorepo, nested project) or from a git hook that sets a different working directory. Config discovery using `std::env::current_dir()` finds nothing, or finds a parent config that was not intended.

**Why it happens:**
Git hooks run with `GIT_DIR` set but `cwd` may be the repo root or the directory of the file being committed. `current_dir()` is unreliable in hook context.

**Prevention:**
1. Walk up from `current_dir()` toward the git root (using `gix` to find the repo root), checking each directory for config files — the node_modules resolution algorithm described in ROADMAP is the right approach
2. Prefer config adjacent to the git root, not `current_dir()`
3. Provide `--config` override for non-standard layouts

**Phase:** Config discovery layer.

---

### Pitfall 8: Filenames with Spaces and Special Characters Break Command Execution

**What goes wrong:**
A staged file is named `my component.tsx` or `file (copy).rs`. The file path is interpolated into the command string and the shell splits it into multiple arguments. The linter receives `my` and `component.tsx` as separate files, fails, or corrupts them.

**Why it happens:**
Commands are often defined as strings like `"eslint"` with files appended. If file paths are joined by spaces and passed to a shell, the shell performs word splitting. Rust's `Command::arg()` is safe, but if the command string is passed through a shell (`sh -c "eslint {files}"`) the quoting must be explicit.

**Prevention:**
1. Never use `sh -c` with interpolated file paths; instead parse the command string into binary + args, then use `Command::arg()` per file
2. If shell features (pipes, environment variable expansion) in user commands must be supported, wrap each file path in single quotes and escape embedded single quotes
3. Test with staged files containing spaces in filenames

**Phase:** Execution layer (command spawning).

---

### Pitfall 9: Async Executor Starvation Under High File Count

**What goes wrong:**
A repo with 500 staged files spawns 500 concurrent `cargo clippy` processes. The system runs out of file descriptors, process table slots, or memory. All tasks fail with obscure OS errors.

**Why it happens:**
`tokio::task::spawn` does not limit concurrency. Each spawned OS process consumes resources. 500 concurrent compiler invocations will OOM most developer machines.

**Prevention:**
1. Use a semaphore (`tokio::sync::Semaphore`) to cap concurrent child processes (default: `num_cpus::get()` or 8)
2. For `Batch` execution mode, this is less of an issue — one invocation per pattern, not per file
3. Expose a `parallelism` config option for power users

**Phase:** Execution layer.

---

### Pitfall 10: `--allow-empty` Not Respected by All Early-Exit Paths

**What goes wrong:**
`--allow-empty` is intended to make fast-staged exit 0 when no files match, instead of returning `NoFilesMatched` or `NoStagedFiles`. If any early-exit path (git error, config error) is confused with "no files" the flag silently suppresses real errors.

**Why it happens:**
The `allow_empty` flag is checked in one place but the `NoStagedFiles` and `NoFilesMatched` errors bubble up from multiple layers. Easy to miss an error variant.

**Prevention:**
1. Make `allow_empty` only suppress `AppError::NoStagedFiles` and `AppError::NoFilesMatched` — never `ConfigNotFound`, `NotGitRepository`, or `IoError`
2. Add tests for each error variant to confirm `--allow-empty` does/doesn't suppress it

**Phase:** CLI argument handling + error handling layer.

---

### Pitfall 11: Nightly Rust Toolchain Breaks CI / npm Distribution

**What goes wrong:**
The project uses nightly-only `-Z` flags for binary size optimization. The nightly toolchain changes frequently. A random nightly update breaks compilation. The npm distribution CI fails because nightly is not pinned to a specific date.

**Why it happens:**
Nightly Rust is unstable by design. Without a pinned `rust-toolchain.toml`, CI uses "latest nightly" which may regress any day.

**Prevention:**
1. Pin nightly in `rust-toolchain.toml` with a specific date: `channel = "nightly-2026-03-01"`
2. Update the pin deliberately (weekly/monthly) with verification
3. Separate the release binary profile (nightly with `-Z` flags) from the development/CI profile (stable, no `-Z` flags)
4. The `.cargo/config.toml` `-Z` flags bug (noted in CONCERNS.md) must be fixed: move them to a nightly-only release script, not global config

**Phase:** CI/build configuration — must be fixed before any CI work.

---

### Pitfall 12: gix API Surface is Unstable

**What goes wrong:**
`gix` (gitoxide) has not reached 1.0. Its API changes between minor versions. The staged file listing API (`gix::Repository::index()`) may change signature, return type, or behavior. Locking to a version prevents security updates; updating may break builds silently.

**Why it happens:**
Pre-1.0 crates in Rust do not follow semver stability guarantees by convention. `gix` releases frequently.

**Prevention:**
1. Pin `gix` to an exact minor version in `Cargo.toml` (e.g., `gix = "=0.68.0"`)
2. Write an integration test that validates the exact list of staged files returned by `gix` against `git diff --name-only --cached` output — this detects API behavior changes even if it compiles
3. Track `gix` changelog when updating

**Phase:** Git layer implementation.

---

## Minor Pitfalls

---

### Pitfall 13: Windows Path Separator in Glob Patterns

**What goes wrong:**
Glob patterns use `/` (forward slash). On Windows, `gix` returns paths with `\` (backslash). The pattern `**/*.rs` does not match `src\main.rs`.

**Prevention:**
Normalize all paths to forward slashes before glob matching. Use `Path::to_slash()` from the `path-slash` crate or manually replace separators.

**Phase:** Matching layer.

---

### Pitfall 14: npm Wrapper Package Silently Falls Back to Wrong Binary

**What goes wrong:**
The npm wrapper package (following the SWC distribution model) tries to load the platform-specific optional dependency. If the optional package is missing (e.g., on an unsupported platform), it silently falls back to nothing or crashes with an unhelpful error.

**Prevention:**
1. Detect the platform at install time and warn if no binary is available
2. The wrapper `package.json` `bin` entry must fail loudly if the binary is absent, not silently succeed with exit 0
3. Test on Linux x64, macOS arm64, macOS x64, Windows x64 — all four before any npm release

**Phase:** Distribution/packaging phase.

---

### Pitfall 15: `.git/fast-staged.patch` Not Cleaned Up on SIGKILL

**What goes wrong:**
A `SIGKILL` (untrappable) leaves `.git/fast-staged.patch` on disk. Next run finds a stale patch file from a previous interrupted session and either overwrites it (losing recovery data) or errors out.

**Prevention:**
1. On startup, check for a stale `.git/fast-staged.patch` and warn the user
2. Treat the stale patch as recovery information: print its path and suggest manual review before overwriting
3. Include a timestamp or commit SHA in the patch filename to distinguish sessions

**Phase:** Git layer (backup/restore).

---

### Pitfall 16: TUI Flicker on Fast Commands

**What goes wrong:**
Commands complete in <50ms. The TUI renders a "Running" frame, then immediately "Done". On slow terminals this appears as a flicker or the spinner is never seen. Users think the tool did nothing.

**Prevention:**
Hold "Running" state for a minimum display duration (e.g., 100ms) even if the command finished faster. This is a UX convention, not a correctness issue.

**Phase:** TUI rendering layer.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Git layer — staged file detection | `gix` API instability; incorrect staged file list | Write integration tests against `git diff --cached` as ground truth |
| Git layer — stash/backup | Partial staging corruption (Pitfall 1 and 2) | Implement full save/restore cycle with conflict detection before any other feature |
| Execution layer — command spawning | Orphaned child processes (Pitfall 3); spaces in filenames (Pitfall 8) | Use `Command::arg()` not shell interpolation; explicit child kill on cancel |
| TUI layer | Terminal not restored on panic/error (Pitfall 4) | Install panic hook calling `ratatui::restore()` on first TUI commit |
| Matching layer | Glob semantics diverge from lint-staged (Pitfall 5) | Implement `matchBase` behavior; validate against lint-staged test corpus |
| Error handling | Exit code not propagated to git (Pitfall 6) | Integration test: assert non-zero exit on failure before any feature work |
| CLI args | `--allow-empty` over-suppresses errors (Pitfall 10) | Per-variant suppression logic with test for each AppError variant |
| CI/build | Nightly toolchain breakage (Pitfall 11); bogus `[target.release]` | Fix `.cargo/config.toml` and pin nightly in `rust-toolchain.toml` first |
| npm distribution | Silent fallback on unsupported platform (Pitfall 14) | Loud failure with actionable error; test all 4 target platforms before release |
| Config discovery | Wrong directory in git hook context (Pitfall 7) | Walk to git root, not just `current_dir()` |

## Sources

- Domain knowledge synthesized from lint-staged source code (v15 architecture), lint-staged GitHub issue history (partial staging bugs are the most-starred issues), and git internals
- Rust async process management: `tokio::process` documentation on `Child` handle drop behavior
- ratatui terminal restore patterns: official ratatui examples
- gix pre-1.0 stability: gitoxide project changelog and semantic versioning policy
- Confidence: HIGH for pitfalls 1-6 (verified against multiple real-world tool failures); MEDIUM for pitfalls 7-16 (engineering reasoning, standard patterns)
