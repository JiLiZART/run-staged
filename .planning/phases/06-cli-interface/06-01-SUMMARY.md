---
phase: 06-cli-interface
plan: "01"
subsystem: cli
tags: [clap, cli, run-signature, config-path]
dependency_graph:
  requires: [05-05]
  provides: [CLI-01, CLI-02, CLI-03]
  affects: [main.rs, lib.rs, Cargo.toml]
tech_stack:
  added: []
  patterns: [clap-derive-parser, cwd-anchored-path-resolution]
key_files:
  created: []
  modified:
    - crates/fast-staged/src/main.rs
    - crates/fast-staged/src/lib.rs
    - crates/fast-staged/Cargo.toml
decisions:
  - "D-03 honored: ConfigNotFound variant not extended — reused as unit variant as-is"
  - "D-04 implemented: relative --config paths resolved against CWD in main.rs before lib::run"
  - "D-08 implemented: run() first param is config_path: Option<PathBuf> forwarded to load_config"
metrics:
  duration_minutes: 10
  completed_date: "2026-05-12"
  tasks_completed: 2
  files_modified: 3
---

# Phase 06 Plan 01: CLI Interface (clap Parser + run() signature) Summary

Replace hand-rolled argv inspection with clap 4 derive Parser, extend lib::run() with config_path parameter, and add Cargo.toml description for the --help about line.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add Cargo.toml description for clap about line | 63c244f | crates/fast-staged/Cargo.toml |
| 2 | Replace main.rs argv block with clap derive Parser + extend lib::run signature | 2ddd2f8 | crates/fast-staged/src/main.rs, crates/fast-staged/src/lib.rs |

## Files Modified

### crates/fast-staged/Cargo.toml

- Added `description` field (line 2, alphabetically ordered per taplo `reorder_keys = true`)
- Applied `align_entries = true` to `[package]` block
- Line delta: +2 net (1 new key, alignment updated)

### crates/fast-staged/src/main.rs

- Complete rewrite: 14 lines → 44 lines
- Removed `std::env::args().any()` hand-rolled block
- Added `use clap::Parser`
- Added `#[derive(Parser)]` `Cli` struct with `--config`, `--quiet`, `--unstaged`, `--allow-empty`
- Added `#[command(version, about, long_about = None)]`
- Added CWD-anchored `--config` path resolution (D-04)
- Updated `fast_staged::run()` call to pass `config_path` as first arg

### crates/fast-staged/src/lib.rs

- `run()` signature extended: `config_path: Option<std::path::PathBuf>` added as first param
- `config::load_config(None)` → `config::load_config(config_path.as_deref())`
- Doc comment updated with `config_path` bullet (D-08)
- Line delta: +8 net (new param, doc bullet, formatting)

## Decision Mapping (D-01 through D-08)

| Decision | Implementation |
|----------|----------------|
| D-01 | `#[derive(Parser)]` on `Cli` struct — no new deps (clap already in Cargo.toml) |
| D-02 | `--config /no/such/file.toml` → `load_config(Some(path))` → `AppError::ConfigNotFound` → exit 1 |
| D-03 | `ConfigNotFound` variant reused as unit variant, not extended with a path field |
| D-04 | `p.is_relative()` check in `main()`: joins with `current_dir()` before passing to `lib::run` |
| D-05 | Strict unknown-flag rejection is clap default; `--quet` exits 2 with suggestion |
| D-06 | `#[command(version, about)]` reads `CARGO_PKG_DESCRIPTION` from Cargo.toml description field |
| D-07 | Long-only flags for all domain args; clap auto-provides `-h` / `-V` short aliases |
| D-08 | `pub async fn run(config_path: Option<std::path::PathBuf>, unstaged, allow_empty, quiet)` |

## Smoke-Test Outputs

```
$ ./target/debug/fast-staged --version
fast-staged 0.1.0
EXIT: 0

$ ./target/debug/fast-staged --help
A fast Rust replacement for lint-staged. Run configured commands on staged files.

Usage: fast-staged [OPTIONS]

Options:
      --config <CONFIG>  Path to the config file (overrides auto-discovery)
      --quiet            Suppress success summary output
      --unstaged         Run on modified (not staged) files instead of staged files
      --allow-empty      Exit 0 when no staged files or no patterns match
  -h, --help             Print help
  -V, --version          Print version
EXIT: 0

$ ./target/debug/fast-staged --quet
error: unexpected argument '--quet' found
  tip: a similar argument exists: '--quiet'
EXIT: 2

$ ./target/debug/fast-staged --config /no/such/file.toml
error: configuration file not found
EXIT: 1
```

## Test Results

- 140 tests passed across 5 suites (no regressions; 1 new test counted from suite re-run)
- `cargo clippy -p fast-staged --no-deps -- -D warnings`: no issues
- `cargo fmt -p fast-staged -- --check`: no issues

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None — all wiring is live. `config_path` flows through to `load_config` which has full implementation from Phase 2.

## Self-Check: PASSED

- crates/fast-staged/src/main.rs: exists, contains `derive(Parser)`, `Cli::parse()`
- crates/fast-staged/src/lib.rs: exists, contains `config_path: Option<std::path::PathBuf>`
- crates/fast-staged/Cargo.toml: exists, contains `description =`
- Commit 63c244f: exists (Task 1)
- Commit 2ddd2f8: exists (Task 2)
