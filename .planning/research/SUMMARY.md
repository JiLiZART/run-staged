# Research Summary: fast-staged

## Executive Summary

fast-staged is a native Rust CLI that replaces lint-staged in pre-commit hooks. The tool reads a config file, lists staged files via the git index, matches them against glob patterns, and runs configured commands in parallel — blocking the commit if any command fails. The competitive angle is zero Node.js dependency (distributed as a self-contained binary via npm optional deps or cargo), faster startup, and a live ratatui TUI showing per-command progress.

## Recommended Stack

| Crate | Purpose | Confidence |
|-------|---------|------------|
| tokio (full) | Async runtime, process spawning, signal handling | HIGH |
| ratatui + crossterm | Terminal UI rendering | HIGH |
| gix | Git index/staged file operations | HIGH |
| clap (derive) | CLI argument parsing | HIGH |
| serde + toml + serde_json | Config parsing (TOML, JSON, package.json) | HIGH |
| thiserror | Typed error enum | HIGH |
| globset | Glob pattern matching with matchBase behavior | HIGH |
| tokio-util | CancellationToken for graceful shutdown | HIGH |

## Table Stakes Features

1. Staged file detection via git index
2. Glob pattern matching (with matchBase semantics like lint-staged)
3. Parallel command execution
4. Non-zero exit code on any command failure
5. Human-readable error messages
6. Config discovery across multiple file formats
7. Signal handling (Ctrl+C) with child process cleanup
8. --quiet mode for CI environments
9. --allow-empty for commits with no matching files
10. Git stash/patch backup before execution

## Key Differentiators

- Zero Node.js runtime dependency
- Live ratatui TUI with per-command progress spinners
- Named groups with per-group settings (timeout, execution order, continue_on_error)
- Dual distribution: cargo install + npm binary packages (SWC pattern)

## Architecture

Layered pipeline: `error` -> `config` -> `git` -> `state` -> `commands` -> `renderer` -> `lib` -> `main`

State management uses Elm pattern: Message enum + `update(model, msg)` with `Arc<Mutex<StateModel>>` shared between async executor and TUI render loop.

## Suggested Build Order (8 phases)

1. **Foundation** — AppError enum, CI fix, Cargo.toml deps, exit code correctness
2. **Config Layer** — multi-format discovery and parsing, glob syntax normalization
3. **Git Layer** — staged file listing via gix, patch backup/restore (highest correctness risk)
4. **Matching + Execution** — glob matching, tokio JoinSet execution, child process cleanup
5. **TUI Layer** — ratatui render loop, per-command spinners, terminal restore on panic
6. **Advanced Config** — named groups, per-group timeout/execution_order/continue_on_error
7. **Compatibility + Polish** — lint-staged JSON config compat, --unstaged, stash-based restore
8. **npm Distribution** — cross-platform CI matrix, SWC-style npm packages, publish workflow

## Top 5 Pitfalls

1. **Partial staging corruption** — formatters overwrite curated index; must snapshot and actively restore
2. **Exit code not propagated** — main() returning 0 on failure makes the entire tool useless
3. **Orphaned child processes** — dropping tokio Child does NOT kill the process; must call kill()
4. **ratatui terminal corruption** — panic hook must call restore() or user's shell is unusable
5. **Glob semantics divergence** — lint-staged matchBase:true means `*.ts` matches any subdirectory

## Key Gaps Needing Spikes

- gix staged-file API shape (varies by minor version)
- Stash restore conflict handling in gix
- Glob `(a|b)` syntax normalization (ROADMAP uses non-standard syntax)
- Exact ratatui async event loop pattern for current version

## Confidence

Overall: **HIGH** for planning purposes; **MEDIUM** for exact crate versions (verify on crates.io).

---
*Synthesized: 2026-04-09 from STACK.md, FEATURES.md, ARCHITECTURE.md, PITFALLS.md*
