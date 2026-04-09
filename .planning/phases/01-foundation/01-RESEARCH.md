# Phase 1: Foundation - Research

**Researched:** 2026-04-09
**Domain:** Rust project structure, typed error enums, CI with stable Rust, cargo workspace
**Confidence:** HIGH

## Summary

Phase 1 establishes a buildable Rust project skeleton: renamed crate, declared dependencies, a typed `AppError` enum, a `lib.rs + main.rs` split with proper async exit code wiring, an integration test that asserts exit code 0, and a replacement CI workflow. The project is currently a stub (`main.rs` prints "Hello, world!") with no dependencies declared and the crate named `cli` instead of `fast-staged`.

The single blocking correctness issue is that `.cargo/config.toml` has nightly-only `-Z` flags in the `[build]` section, which will cause stable CI builds to fail with "error: the option Z is only accepted on the nightly compiler." These flags must be removed from the committed config to make stable CI work. A secondary issue is that edition 2024 requires Rust 1.85.0+ — the locally-installed stable was 1.82.0 before update, but `dtolnay/rust-toolchain@stable` in CI will pull the current stable (1.94 as of April 2026), which is fine.

**Primary recommendation:** Rename crate, declare all deps, create `src/error.rs` with the full `AppError` enum, wire `#[tokio::main]` → `run()` in `main.rs`, remove nightly `-Z` flags from `.cargo/config.toml`, add integration test in `tests/`, and replace the NAPI-RS CI with a minimal pure-Rust workflow.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Define the full ~10 variant AppError enum upfront in Phase 1 (ConfigNotFound, ConfigInvalid, NotGitRepository, NoStagedFiles, NoFilesMatched, CommandNotFound, CommandFailed, Timeout, Cancelled, Io, Git). Future phases use existing variants without modifying the enum.
- **D-02:** AppError lives in `src/error.rs`, re-exported from `lib.rs`. The crate has a lib.rs + main.rs split — main.rs is a thin entry point calling `lib::run()`.
- **D-03:** Rename crate from `cli` to `fast-staged`. Both the package name and binary name are `fast-staged`. Directory becomes `crates/fast-staged/`.
- **D-04:** Replace NAPI-RS CI with a pure Rust workflow. No Node.js, yarn, or oxlint.
- **D-05:** CI uses **stable** Rust toolchain. Nightly is local-only for size optimization flags (`-Z` flags in `.cargo/config.toml`).
- **D-06:** `.cargo/config.toml` nightly-only flags must be handled so stable CI doesn't break (e.g., remove or gate behind profile/environment).
- **D-07:** `#[tokio::main]` async main that calls `fast_staged::run().await`. On error, prints to stderr and calls `process::exit(1)`. Clean separation: main.rs handles process lifecycle, lib.rs handles logic.
- **D-08:** `pub async fn run() -> Result<(), AppError>` is the library entry point in lib.rs. In Phase 1 it returns `Ok(())` (no-op pipeline).
- **D-09:** Phase 1 includes one integration test only: run the binary and assert exit code 0. Unit tests for AppError deferred to Phase 2 when the enum gets exercised.

### Claude's Discretion

- CI workflow exact shape (minimal single-OS vs multi-OS matrix, caching strategy) — Claude picks what satisfies Phase 1 success criteria
- Dependency version pinning strategy (exact vs semver ranges)
- `.cargo/config.toml` cleanup approach for stable CI compatibility

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ERR-01 | Tool uses typed AppError enum with thiserror | thiserror 2.0.18 with `derive(Error)` + `#[error]` + `#[from]` attributes; see Code Examples section |
</phase_requirements>

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| thiserror | 2.0.18 | Derive macro for `Error` trait implementation | Zero-cost, eliminates boilerplate, doesn't appear in public API |
| tokio | 1.51.1 | Async runtime for `#[tokio::main]` | Required by CLAUDE.md; `features = ["full"]` as specified |
| clap | 4.6.0 | CLI argument parsing | Required by CLAUDE.md; Phase 1 only needs skeleton |
| serde | 1.0.228 | Serialization framework | Required by CLAUDE.md; with `features = ["derive"]` |
| serde_json | 1.0.149 | JSON parsing | Required by CLAUDE.md |
| toml | 1.1.2 | TOML parsing | Required by CLAUDE.md |
| ratatui | 0.30.0 | Terminal UI | Required by CLAUDE.md |
| gix | 0.81.0 | Git operations | Required by CLAUDE.md |
| globset | 0.4.18 | Glob pattern matching | Required by CLAUDE.md |

**Version note:** All versions verified against crates.io on 2026-04-09. Use semver ranges (e.g., `"2"`, `"1"`) to track patch releases automatically, except where exact pinning is needed for reproducibility.

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| parse_duration | 2.1.1 | Human-readable duration parsing | Referenced in CLAUDE.md for timeout config in later phases; declare now |

**Installation (all deps at once):**
```toml
[dependencies]
thiserror   = "2"
tokio       = { version = "1", features = ["full"] }
clap        = { version = "4", features = ["derive"] }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
toml        = "1"
ratatui     = "0.30"
gix         = "0.81"
globset     = "0.4"
parse_duration = "2"
```

## Architecture Patterns

### Recommended Project Structure after Phase 1

```
crates/fast-staged/
├── Cargo.toml              # package name = "fast-staged", binary name = "fast-staged"
├── src/
│   ├── main.rs             # thin entry point, #[tokio::main], calls lib::run()
│   ├── lib.rs              # pub async fn run() -> Result<(), AppError>; re-exports AppError
│   └── error.rs            # AppError enum with thiserror::Error derive
└── tests/
    └── integration_test.rs # asserts binary exit code 0
```

### Pattern 1: lib.rs + main.rs Split

**What:** Binary crate with separate library root. `main.rs` is a thin process lifecycle wrapper; `lib.rs` owns all logic and exposes the public API.
**When to use:** Any Rust binary where you want testable logic isolated from process concerns. Required by D-02/D-07/D-08.

`main.rs`:
```rust
// Source: CONTEXT.md D-07 approved pattern
use std::process;

#[tokio::main]
async fn main() {
    if let Err(e) = fast_staged::run().await {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
```

`lib.rs`:
```rust
// Source: CONTEXT.md D-08 approved pattern
mod error;
pub use error::AppError;

pub async fn run() -> Result<(), AppError> {
    Ok(()) // Phase 1 no-op; filled in later phases
}
```

### Pattern 2: Typed Error Enum with thiserror 2.x

**What:** Centralized `AppError` enum using `thiserror::Error` derive.
**When to use:** All fallible operations in this codebase. Required by ERR-01, D-01, D-02.

`error.rs`:
```rust
// Source: thiserror 2.0 docs, https://docs.rs/thiserror/latest/thiserror/
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration file not found")]
    ConfigNotFound,

    #[error("invalid configuration: {0}")]
    ConfigInvalid(String),

    #[error("not a git repository")]
    NotGitRepository,

    #[error("no staged files found — run 'git add' to stage files")]
    NoStagedFiles,

    #[error("no staged files matched any configured patterns")]
    NoFilesMatched,

    #[error("command not found: {0}")]
    CommandNotFound(String),

    #[error("command failed with exit code {0}")]
    CommandFailed(i32),

    #[error("command timed out after {0}")]
    Timeout(String),

    #[error("execution cancelled")]
    Cancelled,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("git error: {0}")]
    Git(String),
}
```

**Note on thiserror 2.x vs 1.x:** The `#[error]`, `#[from]`, and `#[source]` attributes work the same way. The main breaking change in 2.0 was raw identifier handling in format strings (e.g., `{r#type}` → `{type}`), which does not affect this enum. The `gix` crate errors are wrapped as `String` rather than using `#[from] gix::Error` to avoid tying AppError's `From` impls to a specific gix version — the exact gix error type hierarchy should be verified in Phase 4 before adding `#[from]`.

### Pattern 3: Integration Test Using CARGO_BIN_EXE

**What:** Integration test in `tests/` that runs the compiled binary and checks exit code.
**When to use:** D-09 mandates one integration test for Phase 1.

`tests/integration_test.rs`:
```rust
// Source: CONTEXT.md D-09 approved pattern
use std::process::Command;

#[test]
fn binary_exits_zero() {
    let status = Command::new(env!("CARGO_BIN_EXE_fast-staged"))
        .status()
        .expect("failed to run binary");
    assert!(status.success());
}
```

Cargo compiles the binary before running integration tests when `env!("CARGO_BIN_EXE_<name>")` is used. No extra setup needed.

### Pattern 4: Crate Rename in Cargo.toml

**What:** Rename package AND binary in crate `Cargo.toml`. Directory rename is also needed.
**When to use:** D-03 mandates rename from `cli` to `fast-staged`.

```toml
[package]
name    = "fast-staged"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "fast-staged"
path = "src/main.rs"
```

The workspace `Cargo.toml` already uses `members = ["crates/*"]` so no workspace-level changes are needed after directory rename.

### Pattern 5: Stable CI Workflow (Pure Rust)

**What:** GitHub Actions workflow using `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, and standard cargo commands. Replaces the existing NAPI-RS based CI.
**When to use:** D-04/D-05 require this.

Key choices (Claude's discretion per CONTEXT.md):
- Single OS (ubuntu-latest) for Phase 1 — full matrix deferred to v2 distribution phase
- `Swatinem/rust-cache@v2` for efficient Cargo dependency caching
- `RUSTFLAGS: -Dwarnings` treats all warnings as errors
- `--locked` flag ensures reproducible builds against Cargo.lock

```yaml
name: CI
on:
  push:
    branches: [main]
    paths-ignore: ['**/*.md', 'LICENSE', '.editorconfig', 'docs/**']
  pull_request:
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true
env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -Dwarnings
jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --locked --workspace --all-targets
      - run: cargo test --locked --workspace
```

### Anti-Patterns to Avoid

- **Using `Box<dyn Error>` instead of typed errors:** AppError must use thiserror derive — do not use boxed errors anywhere.
- **Putting logic in `main.rs`:** Only process lifecycle (arg parsing entry, exit code, error printing) belongs in main. All domain logic goes through `lib::run()`.
- **Using nightly-only `-Z` flags in the committed `.cargo/config.toml`:** These break stable CI (D-06). Remove from `[build]` section. They can be applied locally via `RUSTFLAGS` env var or a gitignored local config override.
- **Using `actions/checkout@v6` or `@v7`/`@v8`:** These versions do not exist (latest is v4). The existing CI has incorrect version references.
- **`#[tokio::main]` returning `Result<>`:** Using `-> Result<(), AppError>` in `main` would print debug format. The approved pattern (D-07) uses `if let Err(e)` + `eprintln!` + `process::exit(1)` for clean user-facing error messages.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Error trait boilerplate | Custom Display + From impls | `thiserror::Error` derive | 10+ variants × 3 traits = 30+ manual impls; thiserror handles all edge cases including source chains |
| Cargo registry caching in CI | Manual `~/.cargo` cache keys | `Swatinem/rust-cache@v2` | Handles workspace fingerprinting, rust version keying, and cache eviction correctly |
| Async runtime setup | Manual reactor bootstrap | `#[tokio::main]` macro | Correct multi-threaded scheduler configuration, signal handling hooks |

**Key insight:** In Rust error handling, the complexity is in the From trait chains and Display formatting. thiserror generates correct, zero-overhead implementations for all standard Error trait requirements.

## Common Pitfalls

### Pitfall 1: Nightly `-Z` flags break stable CI

**What goes wrong:** `.cargo/config.toml` `[build]` section contains `-Zlocation-detail=none` and `-Zfmt-debug=none`. When CI uses a stable toolchain, every `cargo build`/`cargo test`/`cargo clippy` invocation fails with: `error: the option Z is only accepted on the nightly compiler`.

**Why it happens:** `.cargo/config.toml` `[build].rustflags` applies globally to all cargo invocations regardless of toolchain. There is no built-in conditional in config.toml to differentiate stable/nightly.

**How to avoid:** Remove the `-Z` flags from the committed `.cargo/config.toml`. The `[build]` section and `[target.release]` section (which is also not a valid cargo config section — valid profile keys belong in `Cargo.toml` not `.cargo/config.toml`) should both be cleaned up. The Windows CRT static flag in `[target.x86_64-pc-windows-msvc]` is stable and can stay.

**Warning signs:** CI fails on first `cargo check` with "option Z is only accepted on nightly compiler."

### Pitfall 2: `[target.release]` is not a valid `.cargo/config.toml` section

**What goes wrong:** The existing `.cargo/config.toml` has a `[target.release]` section with `-Zunstable-options`. This is not a valid cargo config section (target keys are target triples, not profile names). Profile-level settings belong in `Cargo.toml` under `[profile.release]`.

**Why it happens:** Template/generated config confusion between `.cargo/config.toml` (target triples) and workspace `Cargo.toml` (profile names).

**How to avoid:** Remove the `[target.release]` section entirely from `.cargo/config.toml`. The release profile optimizations (LTO, strip, etc.) are already correctly defined in the workspace `Cargo.toml`.

### Pitfall 3: Rust edition 2024 requires stable 1.85.0+

**What goes wrong:** If CI pins to an older stable (e.g., 1.82 or 1.84), `cargo build` fails: "the crate requires edition 2024, but the latest edition supported by this Rust version is 2021."

**Why it happens:** Rust 2024 edition was stabilized in Rust 1.85.0 (released February 20, 2025). Older stable toolchains don't know it.

**How to avoid:** `dtolnay/rust-toolchain@stable` in CI will pull the latest stable (1.94.1 as of April 2026), which is fine. Do not pin to a specific old version. If minimum version needs to be enforced, use `rust-version = "1.85"` in `Cargo.toml`.

**Warning signs:** CI fails with edition 2024 not supported message.

### Pitfall 4: `CARGO_BIN_EXE_` macro requires hyphen-to-underscore matching

**What goes wrong:** The integration test uses `env!("CARGO_BIN_EXE_fast-staged")`. The binary name in Cargo.toml is `fast-staged` (with hyphen). Cargo's `CARGO_BIN_EXE_` env var uses the binary name as-is (with hyphens), not converted to underscores.

**Why it happens:** Unlike crate names (which become underscored in `extern crate`), binary names in `CARGO_BIN_EXE_*` preserve the original name from `[[bin]].name`.

**How to avoid:** Use exactly `env!("CARGO_BIN_EXE_fast-staged")` — the approved pattern from CONTEXT.md D-09.

### Pitfall 5: thiserror 2.x requires direct dependency

**What goes wrong:** If thiserror is only a transitive dependency (used by another crate), the `derive(Error)` macro on your own types won't compile without a direct dep.

**Why it happens:** thiserror 2.0 made the direct dependency requirement explicit.

**How to avoid:** Add `thiserror = "2"` directly to the crate's `[dependencies]`, not just workspace deps.

## Code Examples

Verified patterns from official sources and CONTEXT.md decisions:

### Cargo.toml package section
```toml
[package]
name    = "fast-staged"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "fast-staged"
path = "src/main.rs"

[dependencies]
thiserror      = "2"
tokio          = { version = "1", features = ["full"] }
clap           = { version = "4", features = ["derive"] }
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"
toml           = "1"
ratatui        = "0.30"
gix            = "0.81"
globset        = "0.4"
parse_duration = "2"
```

### .cargo/config.toml (cleaned for stable CI)
```toml
# nightly -Z flags removed (break stable CI); use RUSTFLAGS env var locally if needed
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

### Full AppError enum
```rust
// Source: thiserror 2.0 docs + CONTEXT.md D-01
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration file not found")]
    ConfigNotFound,

    #[error("invalid configuration: {0}")]
    ConfigInvalid(String),

    #[error("not a git repository")]
    NotGitRepository,

    #[error("no staged files — run 'git add' to stage files")]
    NoStagedFiles,

    #[error("no staged files matched any configured patterns")]
    NoFilesMatched,

    #[error("command not found: {0}")]
    CommandNotFound(String),

    #[error("command failed with exit code {0}")]
    CommandFailed(i32),

    #[error("command timed out after {0}")]
    Timeout(String),

    #[error("execution cancelled")]
    Cancelled,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("git error: {0}")]
    Git(String),
}
```

### Integration test
```rust
// Source: CONTEXT.md D-09 approved pattern
// File: crates/fast-staged/tests/integration_test.rs
use std::process::Command;

#[test]
fn binary_exits_zero() {
    let status = Command::new(env!("CARGO_BIN_EXE_fast-staged"))
        .status()
        .expect("failed to run binary");
    assert!(status.success());
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| thiserror 1.x | thiserror 2.x | Nov 2024 | Breaking: direct dep required; raw identifier format strings changed |
| NAPI-RS CI (node+yarn) | Pure Rust CI (cargo only) | This phase | Eliminates Node.js toolchain from CI |
| `actions/checkout@v3` | `actions/checkout@v4` | Oct 2023 | v4 is current stable |
| Manual cargo caching | `Swatinem/rust-cache@v2` | 2022+ | Smart fingerprint-based caching |
| Rust edition 2021 | Rust edition 2024 | Feb 2025 (Rust 1.85) | Required toolchain >= 1.85 |

**Deprecated/outdated in existing repo:**
- `[build].rustflags` with `-Z` flags in `.cargo/config.toml`: breaks stable builds, must be removed
- `[target.release]` in `.cargo/config.toml`: not a valid section, dead config
- NAPI-RS CI workflow: entire `.github/workflows/CI.yml` must be replaced
- Crate name `cli` in `crates/fast-staged/Cargo.toml`: must be renamed to `fast-staged`

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust nightly | Local dev nightly flags | ✓ | 1.96.0-nightly | — |
| Rust stable | CI (D-05) | ✓ (locally 1.82, CI will pull 1.94+) | 1.94.1 on CI | — |
| Rust 1.85+ | edition 2024 compilation | ✓ (CI latest stable) | 1.94.1 on CI | — |
| cargo | Build tool | ✓ | 1.92.0-nightly local | — |

**Missing dependencies with no fallback:** None — all build requirements are met by the standard Rust toolchain available on GitHub Actions ubuntu-latest runners.

**Note on local stable:** The locally installed stable toolchain is 1.82 (outdated), but `dtolnay/rust-toolchain@stable` in CI will install 1.94.1. Running `rustup update stable` locally will fix local dev. This does not block Phase 1 implementation.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `cargo test` (no external framework needed) |
| Config file | none — cargo test discovers tests automatically |
| Quick run command | `cargo test --locked -p fast-staged` |
| Full suite command | `cargo test --locked --workspace` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ERR-01 | Binary compiles with typed AppError enum | integration (build + run) | `cargo test --locked -p fast-staged binary_exits_zero` | ❌ Wave 0 |
| ERR-01 | Binary exits 0 on success | integration | `cargo test --locked -p fast-staged binary_exits_zero` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --locked -p fast-staged`
- **Per wave merge:** `cargo test --locked --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/fast-staged/tests/integration_test.rs` — covers ERR-01 (binary exits 0)
- [ ] No framework install needed — `cargo test` is built-in

## Project Constraints (from CLAUDE.md)

| Directive | Impact on Phase 1 |
|-----------|-------------------|
| Language: Rust edition 2024 | Crate `Cargo.toml` must specify `edition = "2024"` |
| Async runtime: tokio with full features | `tokio = { version = "1", features = ["full"] }` required |
| Git: gix crate (no shelling out) | Declare `gix` dep now; implementation deferred to Phase 4 |
| TUI: ratatui | Declare dep now; implementation deferred to Phase 8 |
| Config parsing: serde + toml + serde_json | Declare all three deps |
| CLI: clap | Declare dep now; implementation deferred to Phase 6 |
| Errors: thiserror typed enum | ERR-01 direct requirement; must use thiserror derive |
| No direct repo edits outside GSD workflow | All file changes via `/gsd:execute-phase` |
| GSD Workflow Enforcement | Execute through `/gsd:execute-phase` — do not edit files directly |

## Open Questions

1. **gix API for staged files (deferred to Phase 4)**
   - What we know: `gix = "0.81.0"` is the current version; CLAUDE.md references `gix::Repository::stash()`
   - What's unclear: Exact API shape for listing staged files — STATE.md flags this as "varies by minor version"
   - Recommendation: Declare the dep in Phase 1 without using it; research API in Phase 4 research step

2. **ratatui 0.30 async event loop pattern (deferred to Phase 8)**
   - What we know: `ratatui = "0.30.0"` is current; STATE.md flags this for Phase 8 verification
   - What's unclear: Exact event loop integration with tokio for 0.30 vs 0.29
   - Recommendation: Declare dep in Phase 1; research integration pattern in Phase 8

## Sources

### Primary (HIGH confidence)

- crates.io registry — verified versions for all 10 dependencies via `cargo search` on 2026-04-09
- CONTEXT.md decisions D-01 through D-09 — locked decisions from user discussion
- Official Rust edition guide — confirmed edition 2024 requires Rust 1.85.0+

### Secondary (MEDIUM confidence)

- [Announcing Rust 1.85.0 and Rust 2024 | Rust Blog](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/) — edition 2024 stabilization date and minimum version
- [thiserror 2.0.18 docs | docs.rs](https://docs.rs/thiserror/latest/thiserror/) — attribute API confirmed unchanged from 1.x (only raw identifier and direct dep changes)
- [Swatinem/rust-gha-workflows minimal-ci.yml](https://github.com/Swatinem/rust-gha-workflows/blob/master/.github/workflows/minimal-ci.yml) — current standard Rust CI workflow pattern
- [Configuration - The Cargo Book](https://doc.rust-lang.org/cargo/reference/config.html) — confirmed no conditional stable/nightly in config.toml

### Tertiary (LOW confidence)

- None — all critical claims verified with primary or secondary sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions verified against crates.io registry on research date
- Architecture: HIGH — patterns from CONTEXT.md locked decisions + Rust stdlib docs
- Pitfalls: HIGH — nightly flags issue confirmed by direct stable toolchain compilation test locally; edition version confirmed against Rust blog

**Research date:** 2026-04-09
**Valid until:** 2026-07-09 (stable stack; gix and ratatui APIs change faster — re-verify for their phases)
