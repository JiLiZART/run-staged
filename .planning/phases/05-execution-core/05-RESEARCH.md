# Phase 5: Execution Core - Research

**Researched:** 2026-04-26
**Domain:** Tokio async process execution, glob matching, parallel/sequential task management, timeout/kill, restore-on-failure wiring
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Use `globset` crate for glob matching. Reject `glob` (no batch compilation) and `globwalk` (walks fs).
- **D-02:** matchBase semantics — auto-prefix `**/` to slashless patterns. Patterns containing `/` are kept literal.
- **D-03:** Negation via `!` prefix — separate include vs exclude GlobSet; `include.is_match(p) && !exclude.is_match(p)`. Exclude always wins.
- **D-04:** Always case-sensitive. No per-group `case_insensitive` knob in Phase 5.
- **D-05:** Parse command strings with `shlex` crate, then `tokio::process::Command::new(argv[0]).args(&argv[1..])`. No shell wrapping.
- **D-06:** File-arg passing: `{files}` placeholder replaces that token; otherwise positional append.
- **D-07:** Add `path_format: PathFormat { Relative, Absolute }` to `Group` and raw structs. Default = `Relative`. `Absolute` = canonicalized. Wire end-to-end through config layer.
- **D-08:** System PATH resolution (default `Command::new` behavior). ENOENT → `AppError::CommandNotFound { command }`.
- **D-09:** Capture stdout+stderr in memory via `Command::output().await`. Failures dump after run. Successful commands emit nothing.
- **D-10:** Success summary: `✓ {group}/{pattern}: {cmd} ({duration}, {N} file(s))`. Suppressed by `--quiet`.
- **D-11:** Failure output: bordered/boxed section per failed command. `print_error_box()` helper isolated for Phase 8 swap.
- **D-12:** `--quiet` = suppress success lines only. Errors always print.
- **D-13:** Group-level failure: `continue_on_error=false` → signal siblings via per-group `Arc<AtomicBool>` + `tokio::select!`. `continue_on_error=true` → collect all failures.
- **D-14:** Cross-group: `Config.continue_on_error=false` → skip subsequent groups on any failure. `true` → run all groups.
- **D-15:** Exit code: ANY failure = non-zero (1). `continue_on_error` never hides failures.
- **D-16:** Unlimited parallelism — `tokio::task::JoinSet`. No `max_concurrent` cap.
- **D-17:** Timeout precedence: group > global > none.
- **D-18:** Per-spawn granularity. Each `Command::output()` wrapped in `tokio::time::timeout(duration, ...)`.
- **D-19:** On timeout (Unix): send SIGTERM via `nix` crate, wait 2 s, then SIGKILL via `Child::kill().await`. Map to `AppError::CommandTimeout { command, duration }`.
- **D-20:** On timeout (Windows): `Child::kill().await` directly. No grace window.
- **D-21:** `--allow-empty` suppresses both `AppError::NoStagedFiles` and `AppError::NoFilesMatched`. Exit 0 silently.
- **D-22:** `--unstaged` + `--allow-empty` combo: same suppression.
- **D-23:** When `--allow-empty` triggers no-op exit, skip backup/cleanup entirely.
- **D-24:** `per_file=true`: one spawn per file; respects group `execution_order`.
- **D-25:** Batch: append all files positionally.
- **D-26:** Auto-chunk batch invocations to platform `ARG_MAX`. Unix: `sysconf(_SC_ARG_MAX)`; Windows: 32767. 25% headroom for env.
- **D-27:** Empty file list after matching: skip command silently — do NOT spawn.
- **D-28:** On any failure: `restore()` then `cleanup()`. On success: `cleanup()`. Skip both in `--unstaged` mode.
- **D-29:** Restore scope = worktree only. Index preserved.
- **D-30:** If `restore()` fails: `AppError::RestoreFailed { patch_path, source }`. Do NOT delete patch.
- **D-31:** Order: restore() first, cleanup() second. Restore failure aborts cleanup.

### Claude's Discretion
- Internal module layout: flat `match.rs`, `commands.rs`, `output.rs`.
- `TaskState` / `CommandStatus` minimal shapes; Phase 8 extends without breaking.
- Test layout: per-module `#[cfg(test)]` + integration tests using `make_real_git_repo`.
- Exact wording of `AppError::CommandNotFound`, `CommandFailed`, `CommandTimeout`, `RestoreFailed` messages.
- Unix signal mechanism: `nix` crate (researcher pick — confirmed below).

### Deferred Ideas (OUT OF SCOPE)
- EXEC-04 Ctrl+C / signal handling — Phase 7.
- Live TUI rendering — Phase 8.
- `--config <path>`, `--version`, `--help` — Phase 6.
- Configurable `max_concurrent` parallelism.
- `case_insensitive` per-group glob option.
- Stdin-based file passing.
- Auto-detect shell metachars fallback.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| EXEC-01 | Tool runs matched commands in parallel by default | `tokio::task::JoinSet` unlimited parallel spawn; cross-group and within-group both parallel by default |
| EXEC-02 | Tool supports sequential execution mode via config | `ExecutionOrder::Sequential` drives sequential `.await` loop; already in `Group` struct |
| EXEC-03 | Tool exits with non-zero code when any command fails | Collect `Vec<CommandOutcome>`, check for any failures, `process::exit(1)` from `main()`; D-15 |
| EXEC-05 | Tool supports per-command timeout with configurable duration | `tokio::time::timeout()` wrapping `child.wait_with_output()`; SIGTERM + SIGKILL on Unix; `Child::kill()` on Windows |
| EXEC-06 | Tool matches staged files to commands via glob patterns (matchBase semantics) | `globset::GlobSetBuilder`; `**/` auto-prefix for slashless patterns; separate include/exclude GlobSets |
| EXEC-07 | Tool supports `--allow-empty` flag to skip execution when no files match | Suppress `NoStagedFiles` and `NoFilesMatched`; skip backup/cleanup; exit 0 silently |
</phase_requirements>

---

## Summary

Phase 5 wires the execution core: glob-based file matching, parallel/sequential command dispatch, timeout enforcement, and restore-on-failure. All crates are already in `Cargo.toml` except `shlex` (add version `1.3.0`) and `nix` (already a transitive dep at `0.29.0` — add directly). The `AppError` enum already has `CommandNotFound(String)`, `CommandFailed(i32)`, and `Timeout(String)` variants; rename/extend as needed. The `Group` struct needs only `path_format: PathFormat` added — all other execution fields already exist.

The central integration point is `lib.rs::run()` lines 28–56: the eprintln debug block and Phase 5 TODO are replaced by `match_files()` + `execute_all()` + restore/cleanup wiring. Output is `Vec<CommandOutcome>` collected at end; no channels or state model needed yet (that is Phase 8).

**Primary recommendation:** Add `shlex = "1.3.0"` and `nix = { version = "0.31.2", features = ["signal"] }` to Cargo.toml, add `path_format` field end-to-end through config layer, then implement three new modules: `src/matcher.rs`, `src/executor.rs`, `src/output.rs`.

---

## Standard Stack

### Core — already in Cargo.toml (verified via `cargo metadata`)

| Library | Version in project | Purpose | Notes |
|---------|-------------------|---------|-------|
| `globset` | 0.4.18 (latest) | Compile patterns into a GlobSet, match files | `GlobSetBuilder`, case-sensitive default |
| `tokio` | 1.51.1 | Async process spawn, timeout, task join | `features = ["full"]` — `process`, `time`, `task` all available |
| `thiserror` | 2 | `AppError` derive | No changes to base crate |

### Add to Cargo.toml

| Library | Version | Purpose | Why |
|---------|---------|---------|-----|
| `shlex` | 1.3.0 | POSIX shell word-splitting | Parses `"prettier --write"` into `["prettier", "--write"]` |
| `nix` | 0.31.2 (latest) | Send SIGTERM on Unix before SIGKILL | `features = ["signal"]` — `nix::sys::signal::kill()` |
| `dunce` | 1.0.5 | Windows-friendly path canonicalize | Avoids UNC `\\?\` paths when `path_format = Absolute` |

**Note:** `nix` `0.29.0` is already a transitive dependency (via `gix`). Declaring it directly at `0.31.2` with `features = ["signal"]` is safe — Cargo will unify to the highest compatible version.

**Installation:**
```bash
cargo add shlex@1.3.0
cargo add nix@0.31.2 --features signal
cargo add dunce@1.0.5
```

**Version verification (run before writing):**
```bash
cargo search shlex    # → 1.3.0
cargo search nix      # → 0.31.2
cargo search dunce    # → 1.0.5
```

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `globset` | `glob` | `glob` has no batch compile; GlobSet matches all patterns in one pass — better for N patterns × M files |
| `shlex` | `shell_words` | `shell_words` is POSIX-only; `shlex` is nearly identical but also handles Windows invocations (argv0 resolution is left to the OS anyway) |
| `nix` | `libc` raw syscall | `libc::kill()` works but is unsafe; `nix::sys::signal::kill()` provides a safe typed wrapper — prefer nix |
| `dunce::canonicalize` | `std::fs::canonicalize` | `std::fs::canonicalize` returns `\\?\C:\...` UNC paths on Windows that most tools do not accept — dunce suppresses the prefix |

---

## Architecture Patterns

### Recommended Module Layout

```
crates/fast-staged/src/
├── lib.rs            # run() orchestrator — wires matcher + executor + output + restore
├── main.rs           # CLI arg parsing (minimal, Phase 6 replaces with clap)
├── error.rs          # AppError — add CommandTimeout, RestoreFailed variants
├── git.rs            # Existing — no changes
├── config/
│   ├── mod.rs
│   ├── types.rs      # Add PathFormat enum + path_format field to Group + RawGroupConfig + RawJsonGroupConfig
│   ├── defaults.rs   # Extend resolve_group() for path_format cascading
│   ├── parse_toml.rs # Pass path_format through RawGroupSettings
│   └── parse_json.rs # Same
├── matcher.rs        # NEW — match_files(config, files) → Vec<MatchedGroup>
├── executor.rs       # NEW — execute_all(matched, git_root) → Vec<CommandOutcome>
└── output.rs         # NEW — print_summary(), print_error_box()
```

### Pattern 1: GlobSet with matchBase (D-02, D-03, D-04)

```rust
// Source: globset 0.4.18 docs + crate source
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

fn build_globset(patterns: &[&str], case_sensitive: bool) -> Result<(GlobSet, GlobSet), globset::Error> {
    let mut include = GlobSetBuilder::new();
    let mut exclude = GlobSetBuilder::new();
    for pat in patterns {
        let (set, raw) = if let Some(negated) = pat.strip_prefix('!') {
            (&mut exclude, negated)
        } else {
            (&mut include, *pat)
        };
        // matchBase: if pattern has no '/', auto-prefix '**/'
        let normalized = if !raw.contains('/') {
            format!("**/{raw}")
        } else {
            raw.to_string()
        };
        let glob = GlobBuilder::new(&normalized)
            .case_insensitive(!case_sensitive)  // false = case-sensitive (default)
            .build()?;
        set.add(glob);
    }
    Ok((include.build()?, exclude.build()?))
}

fn is_match(include: &GlobSet, exclude: &GlobSet, path: &Path) -> bool {
    include.is_match(path) && !exclude.is_match(path)
}
```

**Confidence:** HIGH — verified against globset 0.4.18 API (`GlobSetBuilder`, `GlobBuilder::case_insensitive`, `GlobSet::is_match`).

**GlobSet empty-include edge case:** When the include set has zero patterns (all patterns are negations), `GlobSet::is_match` returns `false` on an empty `GlobSet`. This is correct — nothing matches by default. The planner must not accidentally create a group with zero include patterns.

### Pattern 2: tokio::process spawn + output capture (D-05, D-08, D-09)

```rust
// Source: tokio docs — tokio::process::Command
use tokio::process::Command;
use std::process::Stdio;

async fn spawn_command(
    argv: &[String],
    working_dir: &Path,
) -> Result<std::process::Output, AppError> {
    Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::CommandNotFound(argv[0].clone())
            } else {
                AppError::Io(e)
            }
        })
}
```

**Path resolution:** `tokio::process::Command::new(name)` delegates to the OS for PATH lookup — `execvp` on Unix, `CreateProcess` on Windows. No explicit `which` call needed.

**ENOENT detection:** `io::ErrorKind::NotFound` from spawn maps cleanly to `CommandNotFound`. Other `Io` errors (permissions, etc.) stay as `AppError::Io`.

### Pattern 3: tokio::time::timeout + SIGTERM grace + SIGKILL (D-18, D-19, D-20)

```rust
// Source: tokio docs + nix 0.31 docs
use tokio::time::{timeout, Duration};
use tokio::process::Command;

async fn spawn_with_timeout(
    argv: &[String],
    dir: &Path,
    duration: Duration,
) -> Result<std::process::Output, AppError> {
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| if e.kind() == io::ErrorKind::NotFound {
            AppError::CommandNotFound(argv[0].clone())
        } else { AppError::Io(e) })?;

    match timeout(duration, child.wait_with_output()).await {
        Ok(result) => result.map_err(AppError::Io),
        Err(_elapsed) => {
            // Timeout — Unix: SIGTERM then wait 2 s then SIGKILL
            #[cfg(unix)]
            {
                if let Some(pid) = child.id() {
                    use nix::sys::signal::{kill, Signal};
                    use nix::unistd::Pid;
                    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                    // Give process 2 s to exit cleanly
                    let _ = timeout(Duration::from_secs(2), child.wait()).await;
                }
            }
            // Force-kill (SIGKILL on Unix, TerminateProcess on Windows)
            let _ = child.kill().await;
            Err(AppError::CommandTimeout {
                command: argv[0].clone(),
                duration,
            })
        }
    }
}
```

**CRITICAL — `child.id()` race:** After calling `child.wait_with_output()` (even inside `timeout`), the pid may already be reaped on some platforms. Use `child.spawn()` + `child.id()` captured immediately after spawn, before the wait future. Store pid in a variable before the timeout select.

**Windows:** `Child::kill()` calls `TerminateProcess` — no SIGTERM equivalent. The `#[cfg(unix)]` block is skipped entirely.

**`tokio::process::Child::start_kill()` vs `kill()`:** `start_kill()` initiates the kill and returns without waiting; `kill().await` sends kill AND waits for the kill to complete. For the teardown path, prefer `kill().await` to avoid zombie processes on Unix.

### Pattern 4: tokio::task::JoinSet for parallel groups (D-13, D-14, D-16)

```rust
use tokio::task::JoinSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

async fn execute_group_parallel(
    tasks: Vec<CommandTask>,
    cancel: Arc<AtomicBool>,
    continue_on_error: bool,
) -> Vec<CommandOutcome> {
    let mut set = JoinSet::new();
    for task in tasks {
        let cancel = Arc::clone(&cancel);
        set.spawn(async move { run_task(task, cancel).await });
    }

    let mut outcomes = Vec::new();
    while let Some(result) = set.join_next().await {
        let outcome = result.unwrap_or_else(|e| CommandOutcome::JoinError(e.to_string()));
        if outcome.is_failure() && !continue_on_error {
            // Signal siblings to stop at next check point
            cancel.store(true, Ordering::Relaxed);
        }
        outcomes.push(outcome);
    }
    outcomes
}
```

**D-13 cancellation:** Per-group `Arc<AtomicBool>`. Tasks poll it before spawn and optionally between chunks. `JoinSet::abort_all()` is an alternative but does not give tasks a chance to clean up — prefer the flag approach for Phase 5 (consistent with Phase 7 signal handling).

**D-14 cross-group:** Global `continue_on_error` gate: after each group's outcomes are collected, check for failures before spawning next group.

### Pattern 5: ARG_MAX chunking (D-26)

```rust
#[cfg(unix)]
fn arg_max() -> usize {
    // sysconf(_SC_ARG_MAX) returns the POSIX limit.
    // 75% headroom for environment variables.
    let raw = unsafe { libc::sysconf(libc::_SC_ARG_MAX) };
    let limit = if raw > 0 { raw as usize } else { 131072 }; // 128 KiB fallback
    (limit as f64 * 0.75) as usize
}

#[cfg(windows)]
fn arg_max() -> usize {
    // CreateProcess limit: 32767 characters total.
    (32767.0 * 0.75) as usize
}

fn chunk_files<'a>(files: &'a [PathBuf], cmd_len: usize) -> Vec<&'a [PathBuf]> {
    let budget = arg_max().saturating_sub(cmd_len);
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut current_len = 0;
    for (i, f) in files.iter().enumerate() {
        let len = f.as_os_str().len() + 1; // +1 for space/null
        if current_len + len > budget && start < i {
            chunks.push(&files[start..i]);
            start = i;
            current_len = 0;
        }
        current_len += len;
    }
    if start < files.len() {
        chunks.push(&files[start..]);
    }
    chunks
}
```

**`libc` is already available** as a transitive dependency (via `nix`). No new crate needed. No crate in the Rust ecosystem provides a ready-made `arg_max()` helper — roll our own.

### Pattern 6: path_format (D-07)

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub enum PathFormat {
    #[default]
    Relative,   // relative to git root — matches lint-staged default
    Absolute,   // dunce::canonicalize — avoids UNC paths on Windows
}
```

Paths from `staged_files()` / `unstaged_files()` are already relative to the git root (gix returns index-relative paths). For `PathFormat::Relative`, pass as-is. For `PathFormat::Absolute`, prepend the git workdir and call `dunce::canonicalize()`.

**Config field names:** `"relative"` / `"absolute"` (lowercase strings in TOML/JSON).

### Anti-Patterns to Avoid

- **Spawning through `sh -c`:** Introduces shell injection via filenames containing metacharacters. `shlex` + direct exec is the safe default (D-05).
- **`std::process::Command` instead of `tokio::process::Command`:** Blocks the executor thread. Always use `tokio::process::Command` in async code.
- **Storing `Output` in `Arc<Mutex<_>>` during Phase 5:** Phase 8 will introduce channels. Phase 5 just collects `Vec<CommandOutcome>` at the end — no shared state needed.
- **Calling `restore()` inside a `Drop` impl:** Async restore cannot run in a `Drop`. Use explicit branching in `run()` per D-31.
- **Forgetting `deny_unknown_fields` gap:** `RawGroupConfig` has `#[serde(deny_unknown_fields)]` — adding `path_format` to `Group` and `RawGroupSettings` MUST also add it to `RawGroupConfig`/`RawJsonGroupConfig` or TOML configs that use it will fail to parse.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Glob pattern compilation + matching | Custom regex matcher | `globset::GlobSetBuilder` | Edge cases in `**` expansion, character classes, `?` semantics — globset handles all of them correctly |
| Shell word-splitting | `split_whitespace()` | `shlex::split()` | `split_whitespace` breaks on `--flag="value with space"` and similar; shlex handles all POSIX quoting correctly |
| Process kill + wait | Custom process tracking | `tokio::process::Child` + `nix::sys::signal::kill` | Race conditions in PID reuse, zombie process cleanup — always use tokio's Child handle |
| SIGTERM-then-SIGKILL grace window | Manual sleep + poll | `tokio::time::timeout` + `child.wait()` | Use async wait; blocking sleep in tokio is an anti-pattern |
| Path deduplication | Custom HashSet loop | Standard `HashSet<PathBuf>` insert before appending | Already available in std |

---

## AppError — Current State vs Phase 5 Needs

Read from `src/error.rs` (HIGH confidence — file read directly):

| Variant | Exists | Phase 5 action |
|---------|--------|----------------|
| `CommandNotFound(String)` | YES | Add hint message: "Is `{0}` installed and on PATH?" |
| `CommandFailed(i32)` | YES — takes a single i32 exit code | Extend to `CommandFailed { command: String, exit_code: i32 }` for planner to decide |
| `Timeout(String)` | YES — takes duration string | Rename to `CommandTimeout { command: String, duration: String }` OR keep as-is and format at call site — planner decides exact shape |
| `Cancelled` | YES | Reuse in Phase 7; Phase 5 can trigger it via AtomicBool |
| `RestoreFailed` | NO | ADD: `RestoreFailed { patch_path: PathBuf, source: String }` |
| `NoFilesMatched` | YES | Used by matcher; `--allow-empty` suppresses it |

**Note:** Renaming `Timeout` → `CommandTimeout` is backwards-compatible since no existing code returns `Timeout` except the Phase 5 implementation itself (not yet written). The planner can choose the exact variant shape.

---

## `lib.rs::run()` Integration Point

Current shape (lines 1–58, read directly):

```rust
pub async fn run(unstaged: bool) -> Result<(), AppError> {
    let config = config::load_config(None)?;
    let git_ctx = git::GitContext::open()?;
    let files = if unstaged { git_ctx.unstaged_files()? } else { git_ctx.staged_files()? };
    if !unstaged { git_ctx.backup()?; }
    // ... debug eprintln block ...
    if !unstaged { git_ctx.cleanup()?; }
    Ok(())
}
```

Phase 5 replaces the debug block with:

```rust
pub async fn run(unstaged: bool, allow_empty: bool, quiet: bool) -> Result<(), AppError> {
    let config = config::load_config(None)?;
    let git_ctx = git::GitContext::open()?;

    let files = match if unstaged { git_ctx.unstaged_files() } else { git_ctx.staged_files() } {
        Ok(f) => f,
        Err(AppError::NoStagedFiles) if allow_empty => return Ok(()),
        Err(e) => return Err(e),
    };

    if !unstaged { git_ctx.backup()?; }

    let matched = matcher::match_files(&config, &files)?;
    // match_files returns Err(NoFilesMatched) if nothing matched
    // That error must be caught here for allow_empty suppression

    let outcomes = executor::execute_all(&matched, git_ctx.workdir()).await;
    let has_failures = outcomes.iter().any(CommandOutcome::is_failure);

    output::print_results(&outcomes, quiet);

    if has_failures {
        if !unstaged {
            if let Err(restore_err) = git_ctx.restore() {
                return Err(restore_err); // cleanup skipped per D-30
            }
            git_ctx.cleanup()?;
        }
        return Err(AppError::ExecutionFailed); // or let main() check outcomes
    } else {
        if !unstaged { git_ctx.cleanup()?; }
    }
    Ok(())
}
```

`main.rs` must also be updated to pass `allow_empty` and `quiet` flags (read from `std::env::args()`).

---

## Common Pitfalls

### Pitfall 1: `shlex` on Windows
**What goes wrong:** `shlex::split()` implements POSIX shell quoting. On Windows, `\"` is the native quoting style, and backslash is a path separator. `shlex::split("C:\\Program Files\\tool.exe")` will collapse the double backslashes.
**Why it happens:** shlex is a POSIX spec implementation.
**How to avoid:** Document that command strings should use forward slashes or single-quoted tokens on Windows, or that users should avoid spaces in binary paths (use short-path workaround). The majority of pre-commit hook commands are Unix tools — acceptable limitation for v1.
**Warning signs:** Tests involving `C:\path with space` in command strings.

### Pitfall 2: `child.id()` after partial wait
**What goes wrong:** After `timeout()` fires, the child may have already been reaped by the kernel. Calling `kill()` on a reaped PID could signal a different process if PID was recycled.
**Why it happens:** Linux recycles PIDs rapidly in CI environments with many short-lived processes.
**How to avoid:** Store `child.id()` immediately after `child.spawn()`, before any awaiting. Keep the `Child` handle alive until after the kill sequence. Use `child.kill().await` (tokio tracks the handle safely) rather than raw `libc::kill(pid)` for the final kill.
**Warning signs:** Flaky tests that occasionally kill the wrong process.

### Pitfall 3: `deny_unknown_fields` breakage on `path_format`
**What goes wrong:** Adding `path_format` to `Group` but forgetting `RawGroupConfig` causes `serde` to reject any config file that sets `path_format`.
**Why it happens:** `RawGroupConfig` has `#[serde(deny_unknown_fields)]`; serde errors on unrecognized fields.
**How to avoid:** Add `path_format: Option<String>` to `RawGroupConfig` AND `RawJsonGroupConfig` in `types.rs`, and thread it through `RawGroupSettings` → `defaults::resolve_group()`.
**Warning signs:** Config parser unit tests passing but integration test that loads a `.toml` with `path_format = "absolute"` fails with `ConfigInvalid`.

### Pitfall 4: Empty include GlobSet matches nothing
**What goes wrong:** If all patterns in a group start with `!`, the include `GlobSet` is empty. `globset::GlobSet::is_match()` on an empty set returns `false` — so nothing matches, which is correct. However, if the include set is not built at all (zero non-negation patterns), calling `GlobSetBuilder::build()` on an empty builder still produces a valid `GlobSet` that matches nothing.
**Why it happens:** globset documentation does not prominently warn about empty-include semantics.
**How to avoid:** After building, if include set is empty and the group has patterns, emit `AppError::ConfigInvalid("group '{}' has only negation patterns — nothing can match")`.

### Pitfall 5: Batch spawn with empty file list
**What goes wrong:** Tools like `eslint` when invoked with no arguments lint the entire repository. Calling `eslint` with zero file args is catastrophic.
**Why it happens:** The file matching step may return zero matches for a pattern. Without the D-27 guard, the empty args would be passed.
**How to avoid:** In `executor.rs`, before spawning any batch command, check `if files.is_empty() { continue; }`. This is D-27 and must be verified in tests.
**Warning signs:** Integration test that stages no `.ts` files but has an `eslint` command for `*.ts` — if eslint runs, the guard is missing.

### Pitfall 6: `tokio::process::Child` not reaped on drop
**What goes wrong:** If a `tokio::process::Child` is dropped without calling `.wait()`, tokio may not reap the child process, leading to zombie processes.
**Why it happens:** tokio `Command::output()` automatically waits; but `Command::spawn()` + dropped `Child` does not.
**How to avoid:** Always use `Command::output().await` (which spawns + waits) for the normal path. In the timeout path, always call `child.kill().await` after the timeout — this both kills AND waits.

### Pitfall 7: Cross-group cancellation races
**What goes wrong:** When `continue_on_error=false` (global), after a group fails, the next group may have already been spawned (if groups were kicked off in parallel).
**Why it happens:** Groups are executed sequentially by the outer loop (D-14), but WITHIN a group the parallelism is high.
**How to avoid:** Outer group loop is strictly sequential: `for group in matched_groups { let outcomes = execute_group(group).await; if has_failure && !global_coe { break; } }`. Groups are never parallel with each other in Phase 5.

---

## Code Examples

### shlex word splitting

```rust
// Source: shlex 1.3.0 docs — shlex::split returns Option<Vec<String>>
let input = r#"prettier --write --parser typescript"#;
let argv = shlex::split(input).ok_or_else(|| AppError::ConfigInvalid(
    format!("invalid command syntax: {input:?}")
))?;
// argv = ["prettier", "--write", "--parser", "typescript"]
```

`shlex::split` returns `None` on unbalanced quotes. Map to `AppError::ConfigInvalid`.

### GlobSet matchBase

```rust
// Source: globset 0.4.18 — GlobBuilder + case_insensitive
use globset::{GlobBuilder, GlobSetBuilder};

let mut b = GlobSetBuilder::new();
// "*.ts" → "**/*.ts" (matchBase, D-02)
b.add(GlobBuilder::new("**/*.ts").case_insensitive(false).build()?);
let set = b.build()?;
assert!(set.is_match("src/foo/bar.ts"));
assert!(!set.is_match("src/foo/bar.js"));
```

### tokio process output capture

```rust
// Source: tokio::process::Command — .output() captures both stdout and stderr
use tokio::process::Command;
use std::process::Stdio;

let output = Command::new("cargo")
    .arg("fmt")
    .current_dir(&repo_root)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .await?;

let exit_code = output.status.code().unwrap_or(-1);
let stderr = String::from_utf8_lossy(&output.stderr);
```

### JoinSet pattern

```rust
// Source: tokio::task::JoinSet docs
use tokio::task::JoinSet;

let mut set: JoinSet<CommandOutcome> = JoinSet::new();
for task in tasks {
    set.spawn(async move { run_task(task).await });
}
let mut results = Vec::new();
while let Some(res) = set.join_next().await {
    results.push(res.expect("task panicked"));
}
```

### print_error_box (pre-TUI)

```rust
// Source: Phase 5 design (D-11) — isolated helper for Phase 8 swap
pub fn print_error_box(command: &str, exit_code: i32, stderr: &str, stdout: &str) {
    let width = 60;
    let border = "─".repeat(width);
    eprintln!("┌{border}┐");
    eprintln!("│ FAILED: {command:<width$}│", width = width - 9);
    eprintln!("│ Exit code: {exit_code:<width$}│", width = width - 13);
    if !stderr.is_empty() {
        eprintln!("├{border}┤");
        for line in stderr.lines().take(20) {
            eprintln!("│ {line:<width$}│", width = width - 2);
        }
    }
    eprintln!("└{border}┘");
}
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| `glob` crate (single pattern at a time) | `globset` (batch compile, single pass match) | O(N) match per file vs O(N×M); use globset |
| Shell-out `sh -c "cmd file1 file2"` | Direct spawn via argv array | No metachar injection; faster; cross-platform |
| `std::process::Command` in async | `tokio::process::Command` | Non-blocking; required in tokio async context |
| `anyhow::Error` | `AppError` typed enum with `thiserror` | Already established — enforce |
| `tokio::spawn` returning `JoinHandle` | `JoinSet::spawn` + `join_next()` | JoinSet provides structured concurrency; handles cancellation cleanly |

---

## Environment Availability

Step 2.6: All phase dependencies are Rust crates resolved at compile time. No external service or CLI is required for the new execution logic (the existing `backup()`/`restore()` already shell out to `git`).

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` | Build | ✓ | (workspace) | — |
| `shlex` | D-05 | ✗ (not in Cargo.toml yet) | 1.3.0 | None — add to Cargo.toml |
| `nix` (direct) | D-19 Unix SIGTERM | ✗ (transitive only) | 0.31.2 | `libc` raw unsafe — prefer nix |
| `dunce` | D-07 Absolute paths | ✗ (not in Cargo.toml) | 1.0.5 | `std::fs::canonicalize` (produces UNC on Windows — acceptable in v1 if dunce is omitted) |
| `true`, `false`, `echo`, `sleep` (test helpers) | Integration tests | ✓ Unix; Windows: use `cmd /c exit 0` | n/a | Windows alternative: `cmd.exe /c exit N` |

**Missing dependencies with no fallback:**
- `shlex` — must add to Cargo.toml (Wave 0 task)

**Missing dependencies with fallback:**
- `nix` — already transitive; explicit declaration preferred for `signal` feature
- `dunce` — `std::fs::canonicalize` is acceptable fallback for v1

**Windows test binaries:** `true`/`false`/`sleep` do not exist on Windows. Integration tests that use these must `#[cfg(unix)]` gate them. Windows-compatible alternatives: `cmd.exe /c exit 0` (success), `cmd.exe /c exit 1` (failure). For timeout tests: spawn a Rust helper binary (or use a very short `tokio::time::sleep` in a spawned task — but that tests tokio not the executor).

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `#[tokio::test]` |
| Config file | none — Cargo detects test modules automatically |
| Quick run command | `cargo test -p fast-staged -- --test-thread=1 2>&1` |
| Full suite command | `cargo test -p fast-staged 2>&1` |

Note: `--test-thread=1` is used for tests that mutate CWD (`CWD_LOCK` Mutex pattern from Phase 4). Matcher and executor tests that don't mutate CWD can run fully parallel.

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EXEC-01 | Groups run in parallel; wall-clock < sum | integration | `cargo test exec_parallel` | ❌ Wave 0 |
| EXEC-02 | Sequential mode runs in order | integration | `cargo test exec_sequential` | ❌ Wave 0 |
| EXEC-03 | Any failure → non-zero exit | unit | `cargo test exit_code_propagation` | ❌ Wave 0 |
| EXEC-05 | Timeout kills process, maps to CommandTimeout | unit+integration | `cargo test timeout_kills` | ❌ Wave 0 |
| EXEC-06 | matchBase, negation, case-sensitive | unit | `cargo test glob_matchbase` | ❌ Wave 0 |
| EXEC-07 | --allow-empty: NoFilesMatched exits 0 | unit | `cargo test allow_empty` | ❌ Wave 0 |

### Test Strategy Per Area

**Glob matching (matcher.rs unit tests — no real git repo needed):**
- `*.ts` matches `src/foo/bar.ts` and does not match `src/foo/bar.js`
- `!*.snap` excludes `.snap` files when include set matches
- `src/*.ts` matches `src/bar.ts` but not `nested/src/bar.ts`
- `*.TS` does NOT match `foo.ts` (case-sensitive)
- Empty file list after match returns `Err(NoFilesMatched)` if no global `--allow-empty`

**Process execution (executor.rs unit tests — use real `echo`/`true`/`false`):**
- Successful command: exit 0, stdout captured, no failure in outcomes
- Failed command: exit 1, stderr captured, `CommandFailed` outcome
- CommandNotFound: spawn returns `ENOENT`, maps to `AppError::CommandNotFound`
- `{files}` placeholder: files inserted at correct position in argv
- Positional append: files appended after argv when no `{files}` token
- Empty file list: command NOT spawned (D-27)
- ARG_MAX chunking: large file list split into multiple invocations

**Timeout (Unix only, `#[cfg(unix)]`):**
- `sleep 60` with 100ms timeout → `AppError::CommandTimeout`
- Verify process is dead after timeout (check PID no longer in /proc on Linux)

**Parallelism timing:**
- Two `sleep 0.1` commands in parallel: wall-clock < 0.2 s (allow 2× margin for CI)
- Two `sleep 0.1` commands sequential: wall-clock ≥ 0.2 s

**Output (output.rs unit tests):**
- `print_error_box` does not panic on empty stderr
- `--quiet` suppresses success line, prints failure box

**Integration tests (use `make_real_git_repo` from Phase 4):**
- Full pipeline: stage files → run `echo` command → exit 0
- Full pipeline: stage files → run `false` command → exit 1, restore called

### Sampling Rate
- Per task commit: `cargo test -p fast-staged 2>&1 | tail -5`
- Per wave merge: `cargo test -p fast-staged && cargo clippy -p fast-staged`
- Phase gate: Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/fast-staged/src/matcher.rs` — covers EXEC-06 unit tests
- [ ] `crates/fast-staged/src/executor.rs` — covers EXEC-01, EXEC-02, EXEC-03, EXEC-05
- [ ] `crates/fast-staged/src/output.rs` — covers print helpers
- [ ] `crates/fast-staged/tests/execution_integration.rs` — integration test using `make_real_git_repo`
- [ ] No test framework install needed — `#[tokio::test]` already available via `tokio = { features = ["full"] }`

---

## Open Questions

1. **`shlex` on Windows and backslash paths**
   - What we know: shlex is POSIX-only; backslash is an escape character
   - What's unclear: whether Windows users in pre-commit contexts always use WSL/Git Bash (where POSIX applies)
   - Recommendation: Document the limitation, proceed with shlex for v1. No blocker.

2. **`restore()` vs git error on clean worktree**
   - What we know: `restore()` is a no-op when no patch file exists
   - What's unclear: if backup ran but produced an empty patch (no staged changes), `git apply --cached` on an empty patch may error
   - Recommendation: In `restore()`, check patch file size > 0 before calling `git apply`. Or tolerate the non-zero exit (git apply on an empty patch actually succeeds — verify in a test).

3. **`CommandFailed` variant shape**
   - Current: `CommandFailed(i32)` — just an exit code
   - Phase 5 needs: command name + exit code for the failure box
   - Recommendation: Extend to `CommandFailed { command: String, exit_code: i32 }`. No breaking change since the variant is not yet used in any match arm in existing code.

---

## Sources

### Primary (HIGH confidence)
- `crates/fast-staged/Cargo.toml` — confirmed `globset = "0.4"`, `tokio = "1"`, `thiserror = "2"` already present
- `crates/fast-staged/src/error.rs` — read directly; `CommandNotFound(String)`, `CommandFailed(i32)`, `Timeout(String)`, `Cancelled` all exist; `RestoreFailed` does not exist
- `crates/fast-staged/src/lib.rs` — read directly; integration point confirmed at lines 28–56
- `crates/fast-staged/src/config/types.rs` — read directly; `Group` has all fields except `path_format`
- `crates/fast-staged/src/config/defaults.rs` — read directly; `resolve_group()` pattern confirmed
- `crates/fast-staged/src/git.rs` — read directly; `backup()`/`restore()`/`cleanup()` contract confirmed
- `cargo metadata` + `cargo search` — confirmed `globset 0.4.18`, `tokio 1.51.1`, `shlex 1.3.0`, `nix 0.31.2`, `dunce 1.0.5`

### Secondary (MEDIUM confidence)
- tokio::process::Command docs — async spawn, `output()`, `kill()`, `id()` API shapes (training knowledge, consistent with tokio 1.x stable API)
- globset 0.4 docs — `GlobSetBuilder`, `GlobBuilder::case_insensitive`, `GlobSet::is_match` (training knowledge, API stable since 0.4)
- nix 0.31 docs — `nix::sys::signal::kill`, `Pid::from_raw` (training knowledge verified against crate version 0.31)

### Tertiary (LOW confidence)
- ARG_MAX via `libc::sysconf(_SC_ARG_MAX)` — standard POSIX; behavior verified in training data; no specific crate doc URL available

---

## Project Constraints (from CLAUDE.md)

- Language: Rust edition 2024, nightly toolchain (only for release -Z flags; test/dev use stable)
- Async runtime: `tokio` with full features — already present
- No `anyhow`, no `Box<dyn Error>` — always `Result<T, AppError>`
- `thiserror` for typed error variants
- No shelling out for git operations (git.rs exceptions already documented)
- snake_case module names, PascalCase structs/enums
- `#[serde(deny_unknown_fields)]` on `RawGroupConfig` — must update when adding `path_format`
- All new modules follow `src/foo/mod.rs` re-export pattern OR flat files for simple modules

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crates verified via `cargo metadata` and `cargo search`
- Architecture patterns: HIGH — all code read directly from source; integration patterns verified
- Pitfalls: HIGH (deny_unknown_fields, empty-include, empty-files-spawn) / MEDIUM (Windows shlex, PID reuse race)

**Research date:** 2026-04-26
**Valid until:** 2026-06-01 (crate versions stable; tokio and globset APIs are stable)
