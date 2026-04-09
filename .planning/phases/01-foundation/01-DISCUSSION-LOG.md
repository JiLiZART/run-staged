# Phase 1: Foundation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-04-09
**Phase:** 01-foundation
**Areas discussed:** AppError design, Crate naming, CI workflow shape, Exit code wiring, Test strategy

---

## AppError Design

### Q1: How granular should the AppError enum be?

| Option | Description | Selected |
|--------|-------------|----------|
| Full variants upfront | Define all ~10 variants now (ConfigNotFound, ConfigInvalid, NotGitRepository, etc.). Future phases just use them. | :heavy_check_mark: |
| Minimal scaffolding | Start with 4-5 broad categories (Config, Git, Execution, Io). Add specifics later. | |
| You decide | Claude picks | |

**User's choice:** Full variants upfront
**Notes:** User reviewed the full enum preview and approved the variant list.

### Q2: Where should the error module live?

| Option | Description | Selected |
|--------|-------------|----------|
| lib.rs + main.rs split | Error enum in src/error.rs re-exported from lib.rs. main.rs is thin. Enables integration testing. | :heavy_check_mark: |
| All in main.rs | Single file, simple start. | |
| You decide | Claude picks | |

**User's choice:** lib.rs + main.rs split

---

## Crate Naming

### Q1: How should the crate be named?

| Option | Description | Selected |
|--------|-------------|----------|
| Rename to fast-staged | Single crate named 'fast-staged'. Binary matches project. cargo install works. | :heavy_check_mark: |
| Keep 'cli', set binary name | Crate stays 'cli', [[bin]] name = 'fast-staged'. | |
| You decide | Claude picks | |

**User's choice:** Rename to fast-staged

---

## CI Workflow Shape

### Q1: What should the CI workflow look like?

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal Rust CI | Single Linux runner: fmt + clippy + build + test. | |
| Multi-OS matrix now | Linux + macOS + Windows from day one. | |
| You decide | Claude picks based on Phase 1 success criteria. | :heavy_check_mark: |

**User's choice:** You decide (Claude's discretion)

### Q2: Should CI use nightly or stable toolchain?

| Option | Description | Selected |
|--------|-------------|----------|
| Nightly | Matches .cargo/config.toml -Z flags. | |
| Stable for CI, nightly local | CI uses stable for broader compat. Nightly flags local only. | :heavy_check_mark: |
| You decide | Claude picks | |

**User's choice:** Stable for CI, nightly local

---

## Exit Code Wiring

### Q1: How should main() be structured?

| Option | Description | Selected |
|--------|-------------|----------|
| #[tokio::main] + process::exit | main() calls run(), matches Result, prints error, exits 1. | :heavy_check_mark: |
| Return ExitCode from main | Use Rust Termination trait. More idiomatic but complex with async. | |
| You decide | Claude picks for ratatui compatibility | |

**User's choice:** #[tokio::main] + process::exit

---

## Test Strategy (additional area)

### Q1: What testing should Phase 1 include?

| Option | Description | Selected |
|--------|-------------|----------|
| Integration test only | One test: run binary, assert exit 0. Unit tests deferred to Phase 2. | :heavy_check_mark: |
| Unit + integration | Unit tests for AppError Display + integration test. | |
| You decide | Claude picks | |

**User's choice:** Integration test only

---

## Claude's Discretion

- CI workflow exact shape (single-OS vs multi-OS, caching)
- Dependency version pinning strategy
- .cargo/config.toml cleanup approach for stable CI compatibility

## Deferred Ideas

None -- discussion stayed within phase scope
