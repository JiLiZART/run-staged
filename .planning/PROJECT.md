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

- [x] Config discovery and parsing (TOML, JSON, package.json) — Validated in Phase 2: Config Core
- [x] Git staged file detection via gix — Validated in Phase 4: Git Integration
- [x] Glob pattern matching of staged files to command groups — Validated in Phase 5: Execution Core
- [x] Parallel and sequential command execution with timeout — Validated in Phase 5: Execution Core
- [ ] Live ratatui TUI with per-command progress, spinners, elapsed time
- [x] CLI argument parsing (--config, --quiet, --unstaged, --allow-empty, --stash, --revert) — Validated in Phase 6: CLI Interface (clap derive Parser + explicit --config hard-fail)
- [x] Typed error handling with user-friendly messages — Validated in Phase 1: Foundation
- [x] Git backup/restore via patch file for on-failure rollback — Validated in Phase 4: Git Integration
- [ ] Signal handling (Ctrl+C) with graceful cleanup
- [ ] Git stash backup before execution, restore on failure
- [ ] Dual distribution: cargo install + npm binary packages for all platforms

### Out of Scope

- JS function configs from lint-staged — too complex, marginal value for v1
- Real-time file watching / daemon mode — not needed for pre-commit hook use case
- NAPI-RS native Node.js bindings — distributing as standalone binaries via npm instead
- Plugin system — keep it simple, one tool one job

## Context

- Phase 1 complete — crate renamed to fast-staged, 10 deps declared, AppError enum (11 variants), async main with exit codes, pure Rust CI
- Phase 2 complete — config module with types, TOML/JSON parsers, priority-based discovery (CWD to git root), package.json support, 40 tests
- Phase 3 complete — advanced config: per-group overrides, execution_behavior (per-file/batch), path_format (relative/absolute), declaration-order iteration
- Phase 4 complete — git integration: GitContext with staged_files/unstaged_files via gix, patch-based backup/restore/cleanup in .git/, --unstaged flag wired end-to-end, 74 tests
- Phase 5 complete — execution core: glob matcher with matchBase + negation, commands.rs primitives (build_argv, chunk_files, spawn_command), execute_all orchestrator with per-group parallel/sequential dispatch + Arc<AtomicBool> cancellation + cross-group failure gate, per-spawn timeout with SIGTERM/SIGKILL escalation, run() integration with restore-on-failure, 139 tests across 5 suites. Two backlog items filed (999.1 dogfood matcher bug, 999.2 ROADMAP drift)
- Phase 6 complete — CLI interface: clap 4 derive Parser replaces hand-rolled argv block; lib::run signature accepts `config_path: Option<PathBuf>` as first parameter (bypasses auto-discovery); relative --config paths CWD-anchored in main.rs; missing --config hard-fails with AppError::ConfigNotFound (exit 1); --quet typo rejected with exit 2 + clap suggestion; --version/--help wired via Cargo.toml description; 6 process-level integration tests added (146 tests total across 5 suites)
- CI workflow replaced: NAPI-RS removed, now stable Rust with fmt + clippy + test
- Pre-commit hook (husky + lint-staged) is broken due to missing package.json
- TODO.md and ROADMAP.md contain detailed implementation specs for all layers
- Config format priority: .fast-staged.toml > .fast-staged.json > fast-staged.toml > fast-staged.json > package.json[fast-staged] > lint-staged configs
- lint-staged config compat: parse JSON/YAML configs only, skip JS function configs
- npm distribution model follows SWC pattern: platform-specific optional dependencies with a thin wrapper package

## Constraints

- **Language**: Rust (edition 2024), stable toolchain (nightly flags removed in Phase 1)
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
*Last updated: 2026-05-12 after Phase 6: CLI Interface completion*
