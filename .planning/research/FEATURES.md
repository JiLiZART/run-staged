# Feature Landscape

**Domain:** Git pre-commit staged file runner (lint-staged / nano-staged replacement in Rust)
**Researched:** 2026-04-09
**Confidence:** HIGH — sourced from project ROADMAP.md, TODO.md, and direct knowledge of lint-staged, nano-staged, lefthook, pre-commit

---

## Table Stakes

Features users expect from any staged-file runner. Missing any of these means the tool cannot replace lint-staged.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Staged file detection | Core purpose of the tool — run only on staged files, not entire working tree | Low | Via `gix` index vs HEAD diff; must return empty-list-safe result |
| Glob pattern matching | Users define patterns like `*.ts`, `**/*.rs`, `*.{js,ts}` — foundational config primitive | Medium | Use `globset` crate; must support brace expansion and `**` |
| Pattern-to-command mapping | Each pattern group maps to one or more shell commands | Low | Config-level data structure, deserialized via serde |
| Pass matched files as arguments to each command | Commands receive the matched file list (e.g., `eslint file1.ts file2.ts`) | Medium | Requires string interpolation or appending paths to command argv |
| Config file discovery | Tool finds config automatically without `--config`; reads `.fast-staged.toml`, `fast-staged.toml`, `fast-staged.json`, `.fast-staged.json`, `package.json["fast-staged"]` | Low | Priority order matters; fail with clear "no config found" error listing checked paths |
| Parallel command execution | Multiple pattern groups and files processed concurrently; no user wants a sequential tool by default | Medium | `tokio::task` per command; groups run in parallel unless configured otherwise |
| Non-zero exit code on failure | If any command fails, the overall process must exit non-zero to block the commit | Low | Critical for git hook integration; broken exit codes make the tool useless as a guard |
| Graceful signal handling (Ctrl+C) | Users interrupt runs frequently; orphaned child processes are unacceptable in a pre-commit hook | Medium | `tokio::signal::ctrl_c()` + `Arc<AtomicBool>` cancellation propagated to spawned processes |
| Human-readable errors | "No staged files found", "Config not found", "Command not found in PATH" — with actionable hints | Low | `thiserror` enum + contextual messages; no raw Rust panics or opaque error codes |
| `--allow-empty` flag | Monorepos and CI pipelines need to skip gracefully when nothing is staged without treating it as an error | Low | Suppresses `NoStagedFiles` error; exits 0 with no output |
| `--config <path>` flag | CI and multi-config setups need explicit config override | Low | Bypasses discovery; error if path does not exist |
| `--quiet` flag | CI environments and piped workflows cannot use a TUI; need plain text output + correct exit code | Medium | Bypass ratatui entirely; print summary line; respect exit code |
| Show stderr/stdout of failed commands | Without error output, users cannot debug why a linter failed | Medium | Capture child process output; display in bordered block in TUI (or print in quiet mode) |
| lint-staged config compatibility (JSON/YAML) | Existing lint-staged users migrate without rewriting config | Medium | Parse `{ "*.ts": ["eslint", "prettier --write"] }` shape; skip JS function configs with a clear warning |

---

## Differentiators

Features that set fast-staged apart from lint-staged and nano-staged. Not universally expected, but create real competitive advantage.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Zero Node.js runtime dependency | Runs in any environment without Node/npm installed; works in pure Rust projects, Docker containers without Node, CI runners | Low (implementation) | Distributed as a standalone native binary; the key positioning claim |
| Live TUI with per-command progress and spinners | lint-staged and nano-staged output is scrolling text; fast-staged shows structured live view with elapsed time per command | High | `ratatui` + `tokio` async render loop; `Arc<Mutex<CommandStatus>>` shared state; requires careful terminal cleanup |
| Named groups with per-group execution settings | lint-staged has a flat pattern map; fast-staged supports named groups, each with its own `timeout`, `execution_order`, `continue_on_error`, `execution_behavior`, `path_format` | Medium | Enables complex monorepo setups; group-level timeouts prevent one slow linter from blocking others |
| Sequential execution mode per group | Some workflows require commands to run in order (e.g., format then lint); lint-staged only runs commands sequentially within a pattern by default, but does not expose this as a config knob | Medium | `ExecutionOrder::Sequential` per group via `tokio` serial await chain |
| Per-file vs batch execution mode | Some tools accept one file at a time; others are faster with a file list. Configurable per group | Medium | `ExecutionBehavior::PerFile` vs `ExecutionBehavior::Batch`; batch mode chunks files (e.g., 10 at a time to avoid argv limits) |
| File path format (relative vs absolute) per group | Some tools (e.g., Rust `cargo fmt`) expect relative paths; others need absolute. Eliminates user-side shell path manipulation | Low | `PathFormat::Relative` vs `PathFormat::Absolute` per group; `std::fs::canonicalize()` for absolute |
| Command timeout per group | Prevents a hanging linter from blocking the commit indefinitely; lint-staged has no built-in timeout | Low | `tokio::time::timeout()` wrapping command future; `CommandStatus::Timeout` variant |
| `continue_on_error` per group | Allow subsequent commands to run even if one fails; useful in report-only workflows | Low | Boolean flag in group config; changes sequential abort logic |
| Staged diff backup as patch file | Writes `.git/fast-staged.patch` before running; survives unexpected exits without corrupting the index | Low | `gix` diff generation; file written before any commands launch, deleted on clean exit |
| `--install` command (self-installing git hook) | Users can add fast-staged to a repo without needing husky, lefthook, or any other hook runner | Low | Writes `.git/hooks/pre-commit` (or appends to existing); one command setup |
| Git stash backup + restore on failure | If commands fail and modify working tree files, restores original staged state; lint-staged does this, nano-staged does not | High | `gix::Repository::stash()` before execution; restore stash ref on failure; risky to implement incorrectly (data loss) |
| Grouped TUI display | Shows commands organized by config group with pattern labels; lint-staged has flat output | Medium | Requires group metadata in `StateModel`; `ratatui` widget per group |
| Display total execution time and per-command time | Developers can identify slow linters at a glance | Low | Capture `Instant` at task start/end; display in TUI footer and per-row |
| `--unstaged` flag | Run configured commands on unstaged (modified but not staged) files; useful for pre-push hooks or manual runs | Medium | Switch `gix` diff target from index to working tree |
| npm binary distribution (SWC-style) | JS project developers can install via `npm install -D fast-staged` without Cargo; platform-specific optional deps auto-select the right binary | High | Cross-platform CI builds; platform npm packages; thin JS wrapper; follows SWC/oxc distribution model |

---

## Anti-Features

Things to deliberately NOT build in v1. Each has a clear reason and a stated alternative.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| JavaScript function configs (e.g., `module.exports = { '*.ts': (files) => ... }`) | Requires a JS runtime to evaluate; breaks the zero-Node-dependency value proposition; executes arbitrary code | Parse JSON/YAML lint-staged configs only; emit a clear error message when a JS config is detected ("JS function configs are not supported, use a JSON config") |
| Plugin/extension system | Adds significant complexity; the tool's value is doing one job simply and fast | Users compose commands in config; no hooks, no extension points |
| Daemon mode / file watcher | Not a pre-commit hook use case; adds background process management complexity | Use `cargo watch` or `watchexec` for watch mode workflows |
| NAPI-RS native Node.js bindings | Complicates the build pipeline dramatically; bindings require Node.js to call into anyway | Distribute as standalone binary via npm optional deps (SWC pattern); JS wrapper simply execs the binary |
| Remote config fetching | Security risk; adds network dependency | Users check in their config with their project |
| GUI or web dashboard | Out of scope for a pre-commit hook tool | ratatui TUI is the right scope |
| Global config (outside the repo) | Violates project-local intent; different repos have different lint setups | Config discovery scoped to repo root only |
| Config schema generation / IDE integration | Nice-to-have that delays shipping | Provide example configs in documentation |
| Automatic command installation (e.g., `npm install eslint` if missing) | Unpredictable side effects; wrong for a hook tool | Emit `CommandNotFound` error with "install the tool or check PATH" hint |

---

## Feature Dependencies

```
Staged file detection
  → Glob pattern matching
    → Pattern-to-command mapping
      → Parallel command execution
        → Non-zero exit on failure
        → Graceful signal handling
        → Show stderr/stdout of failed commands
        → Live TUI (depends on: per-command state tracking)
          → Grouped TUI display (depends on: named groups)
          → Per-command execution time display
          → Total execution time display

Config file discovery
  → lint-staged compat parsing
  → Named groups with per-group settings
    → Sequential execution mode per group
    → Per-file vs batch execution mode
    → Command timeout per group
    → continue_on_error per group
    → File path format per group

Staged diff backup
  → Git stash backup + restore on failure (separate, more complex)

--quiet flag (depends on: command execution, non-zero exit)
--allow-empty flag (depends on: staged file detection)
--config flag (depends on: config file discovery)
--unstaged flag (depends on: staged file detection)
--install command (standalone, no upstream deps)

npm binary distribution (depends on: cross-platform binary builds)
```

---

## MVP Recommendation

The minimum viable tool that a developer can actually use as a lint-staged replacement:

**Must have (commit blocker without these):**
1. Staged file detection via gix
2. Config discovery and parsing (TOML-first; JSON second; package.json section third)
3. Glob pattern matching
4. Pattern-to-command mapping + file argument passing
5. Parallel command execution
6. Non-zero exit on any command failure
7. Graceful Ctrl+C handling
8. Human-readable errors (NoConfig, NoStagedFiles, CommandNotFound)
9. `--allow-empty` (required for monorepo/CI use)
10. Show failed command stderr/stdout (otherwise debugging is impossible)

**Include in MVP for differentiation:**
11. `--quiet` flag (required for CI)
12. Staged diff backup as patch file (safety net during development)
13. Live TUI with per-command status and elapsed time (the visible differentiator)

**Defer post-MVP:**
- Named groups with per-group settings (flat config is sufficient for v1)
- Git stash backup + restore (complex, risk of data loss if buggy)
- `--install` command (users can write the hook manually)
- npm binary distribution (implement after binary is stable)
- Sequential execution mode, per-file vs batch, path format, continue_on_error (config complexity; ship flat behavior first)
- lint-staged config compatibility (useful but not required for new users)

---

## Sources

- `/Users/jilizart/Projects/fast-staged/ROADMAP.md` — project-defined feature list (HIGH confidence)
- `/Users/jilizart/Projects/fast-staged/TODO.md` — detailed implementation specs for each feature (HIGH confidence)
- `/Users/jilizart/Projects/fast-staged/.planning/PROJECT.md` — validated requirements and out-of-scope decisions (HIGH confidence)
- `/Users/jilizart/Projects/fast-staged/.planning/codebase/ARCHITECTURE.md` — planned layer boundaries and data structures (HIGH confidence)
- Domain knowledge of lint-staged, nano-staged, lefthook, pre-commit (MEDIUM confidence — training data, August 2025 cutoff)
