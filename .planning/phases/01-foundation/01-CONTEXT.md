# Phase 1: Foundation - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning

<domain>
## Phase Boundary

The project compiles with all required dependencies, exits with the correct code, and every error in the codebase is typed. This phase delivers: dependency declaration, typed AppError enum, correct exit code wiring, crate renaming, and a working CI workflow. No runtime behavior beyond exiting cleanly.

</domain>

<decisions>
## Implementation Decisions

### AppError Design
- **D-01:** Define the full ~10 variant AppError enum upfront in Phase 1 (ConfigNotFound, ConfigInvalid, NotGitRepository, NoStagedFiles, NoFilesMatched, CommandNotFound, CommandFailed, Timeout, Cancelled, Io, Git). Future phases use existing variants without modifying the enum.
- **D-02:** AppError lives in `src/error.rs`, re-exported from `lib.rs`. The crate has a lib.rs + main.rs split — main.rs is a thin entry point calling `lib::run()`.

### Crate Naming
- **D-03:** Rename crate from `cli` to `fast-staged`. Both the package name and binary name are `fast-staged`. Directory becomes `crates/fast-staged/`.

### CI Workflow
- **D-04:** Replace NAPI-RS CI with a pure Rust workflow. No Node.js, yarn, or oxlint.
- **D-05:** CI uses **stable** Rust toolchain. Nightly is local-only for size optimization flags (`-Z` flags in `.cargo/config.toml`).
- **D-06:** `.cargo/config.toml` nightly-only flags must be handled so stable CI doesn't break (e.g., remove or gate behind profile/environment).

### Claude's Discretion
- CI workflow exact shape (minimal single-OS vs multi-OS matrix, caching strategy) — Claude picks what satisfies Phase 1 success criteria
- Dependency version pinning strategy (exact vs semver ranges)
- `.cargo/config.toml` cleanup approach for stable CI compatibility

### Exit Code Wiring
- **D-07:** `#[tokio::main]` async main that calls `fast_staged::run().await`. On error, prints to stderr and calls `process::exit(1)`. Clean separation: main.rs handles process lifecycle, lib.rs handles logic.
- **D-08:** `pub async fn run() -> Result<(), AppError>` is the library entry point in lib.rs. In Phase 1 it returns `Ok(())` (no-op pipeline).

### Test Strategy
- **D-09:** Phase 1 includes one integration test only: run the binary and assert exit code 0. Unit tests for AppError deferred to Phase 2 when the enum gets exercised.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Specs
- `.planning/ROADMAP.md` -- Phase 1 success criteria and dependency declarations
- `.planning/REQUIREMENTS.md` -- ERR-01 requirement (typed AppError enum with thiserror)
- `.planning/PROJECT.md` -- Constraints: Rust edition 2024, tokio, gix, ratatui, clap, thiserror

### Existing Config
- `Cargo.toml` -- Workspace config with release profile (LTO, strip, panic=abort)
- `crates/fast-staged/Cargo.toml` -- Crate package config (currently named `cli`, needs rename)
- `.cargo/config.toml` -- Nightly-only rustflags that need stable CI compatibility fix
- `.github/workflows/CI.yml` -- Current NAPI-RS CI workflow to be replaced

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- None -- codebase is a skeleton (main.rs prints Hello World)

### Established Patterns
- Workspace layout: `crates/*/` with root Cargo.toml
- Release profile: LTO, opt-level "z", strip, panic=abort, codegen-units=1
- `.cargo/config.toml`: Windows MSVC static CRT, nightly `-Z` flags for size optimization

### Integration Points
- `crates/fast-staged/src/main.rs` -- entry point, currently stub
- `.github/workflows/CI.yml` -- must be fully replaced (NAPI-RS based)

</code_context>

<specifics>
## Specific Ideas

- AppError preview shape was reviewed and approved (see D-01 for full variant list)
- main() pattern: `if let Err(e) = fast_staged::run().await { eprintln!("error: {e}"); process::exit(1); }`
- Integration test: `assert!(Command::new(env!("CARGO_BIN_EXE_fast-staged")).status().unwrap().success())`

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope

</deferred>

---

*Phase: 01-foundation*
*Context gathered: 2026-04-09*
