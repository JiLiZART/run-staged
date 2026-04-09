# fast-staged

## What This Is

A fast Rust replacement for lint-staged and nano-staged. Runs configured commands on git-staged files with parallel execution, glob pattern matching, and a live terminal UI. Distributed as both a Cargo binary crate and platform-specific npm packages (like SWC).

## Core Value

Run linting/formatting commands on staged files faster than lint-staged, with zero Node.js runtime dependency.

## Requirements

### Validated

- Cargo workspace structure with `crates/fast-staged` — existing
- Release profile optimized for binary size (LTO, strip, panic=abort) — existing
- Cross-platform target support (Windows MSVC static CRT) — existing

### Active

- [ ] Config discovery and parsing (TOML, JSON, package.json, lint-staged compat)
- [ ] Git staged file detection via gix
- [ ] Glob pattern matching of staged files to command groups
- [ ] Parallel and sequential command execution with timeout
- [ ] Live ratatui TUI with per-command progress, spinners, elapsed time
- [ ] CLI argument parsing (--config, --quiet, --unstaged, --allow-empty, --stash, --revert)
- [ ] Typed error handling with user-friendly messages
- [ ] Signal handling (Ctrl+C) with graceful cleanup
- [ ] Git stash backup before execution, restore on failure
- [ ] Dual distribution: cargo install + npm binary packages for all platforms

### Out of Scope

- JS function configs from lint-staged — too complex, marginal value for v1
- Real-time file watching / daemon mode — not needed for pre-commit hook use case
- NAPI-RS native Node.js bindings — distributing as standalone binaries via npm instead
- Plugin system — keep it simple, one tool one job

## Context

- Existing codebase is a skeleton: main.rs prints "Hello, world!", no dependencies declared
- CI workflow is currently NAPI-RS based and needs replacement with standard Rust binary CI
- Pre-commit hook (husky + lint-staged) is broken due to missing package.json
- TODO.md and ROADMAP.md contain detailed implementation specs for all layers
- Config format priority: .fast-staged.toml > .fast-staged.json > fast-staged.toml > fast-staged.json > package.json[fast-staged] > lint-staged configs
- lint-staged config compat: parse JSON/YAML configs only, skip JS function configs
- npm distribution model follows SWC pattern: platform-specific optional dependencies with a thin wrapper package

## Constraints

- **Language**: Rust (edition 2024), nightly toolchain for size optimization flags
- **Async runtime**: tokio with full features
- **Git**: gix crate (no shelling out to git CLI)
- **TUI**: ratatui for terminal rendering
- **Config parsing**: serde + toml + serde_json
- **CLI**: clap for argument parsing
- **Errors**: thiserror for typed error enum

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust over Node.js | Performance + zero runtime dependency | -- Pending |
| gix over git CLI | No external dependency, faster, type-safe | -- Pending |
| ratatui for TUI | Full control over rendering, async-friendly | -- Pending |
| Dual cargo+npm distribution | Reach both Rust and JS ecosystems | -- Pending |
| Subset lint-staged compat | JSON/YAML only, skip JS functions — pragmatic balance | -- Pending |
| TOML-first config format | Ergonomic for Rust ecosystem, supports typed config well | -- Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? -> Move to Out of Scope with reason
2. Requirements validated? -> Move to Validated with phase reference
3. New requirements emerged? -> Add to Active
4. Decisions to log? -> Add to Key Decisions
5. "What This Is" still accurate? -> Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check -- still the right priority?
3. Audit Out of Scope -- reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-09 after initialization*
