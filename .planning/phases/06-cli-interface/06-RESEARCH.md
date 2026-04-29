# Phase 6: CLI Interface - Research

**Researched:** 2026-04-29
**Domain:** Rust clap 4 derive, `lib::run()` signature extension, CWD-anchored path resolution
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Use `clap = { version = "4", features = ["derive"] }` already declared in `crates/fast-staged/Cargo.toml`. Remove hand-rolled `args.iter().any(...)` block (main.rs lines 6-9). No new deps.
- **D-02:** Hard-fail when `--config` path does not exist. No fallback to auto-discovery.
- **D-03:** File does not exist → `AppError::ConfigNotFound`; file exists but parse fails → `AppError::ConfigInvalid`. Both variants already exist in `error.rs`.
- **D-04:** Resolve `--config` relative paths against CWD (not git root). Auto-discovery unchanged.
- **D-05:** Strict unknown-flag rejection — clap default behavior, exit code 2.
- **D-08:** `lib::run()` gains `config_path: Option<PathBuf>` as its first parameter:
  ```rust
  pub async fn run(
      config_path: Option<PathBuf>,
      unstaged: bool,
      allow_empty: bool,
      quiet: bool,
  ) -> Result<(), AppError>
  ```

### Claude's Discretion
- **D-06:** Help/version style — user did not lock. Planner free to deviate if research uncovers strong reason. Default: `#[command(version, about, long_about = None)]`.
- **D-07:** Short flag aliases — user did not lock. Default: long-only for project flags; `-h`/`-V` kept as clap auto-aliases.

### Deferred Ideas (OUT OF SCOPE)
- Custom `--help` with usage examples and config-format hints
- Short aliases for `--quiet`/`--config`/`--unstaged`
- `--stash`, `--revert`, `--fail-on-changes` flags
- `fast-staged init` / `check` subcommands
- `--version` enriched with git commit hash + build profile
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-01 | Tool parses CLI args via clap: --config, --quiet, --unstaged, --allow-empty | `#[derive(Parser)]` struct replaces hand-rolled argv block; `config_path: Option<PathBuf>` plumbed to `load_config()` |
| CLI-02 | Tool shows version with --version | `#[command(version)]` reads from `Cargo.toml [package].version`; clap exits 0 automatically |
| CLI-03 | Tool shows help with --help | `#[command(about)]` reads `[package].description`; `-h`/`--help` exits 0 automatically |
</phase_requirements>

---

## Summary

Phase 6 is a well-contained refactor: replace 9 lines of hand-rolled arg detection in `main.rs` with a `#[derive(Parser)]` struct, extend `lib::run()` with one new parameter, and thread it down to the already-implemented `config::load_config(Option<&Path>)`. No new crate dependencies are needed — clap 4.6.0 is already locked in `Cargo.lock`. The config layer is already built to handle explicit vs. auto-discovery paths; `lib::run()` just needs to pass the `Option<PathBuf>` it receives from the parsed args.

The only non-trivial work is: (1) updating the `lib::run()` call signature and its sole call site in `main.rs`, and (2) updating the 6 integration tests in `tests/execution_integration.rs` that call `run_binary()` — those tests invoke the binary and are not source-level callers of `run()`, so they are unaffected by the signature change. The `run_binary()` helper just passes CLI args; adding `--config path` flag support will be exercisable from those tests without modifying test helpers.

**Primary recommendation:** Implement as two tasks — (1) add `#[derive(Parser)]` struct in `main.rs`, wire to `lib::run()`; (2) add a new CLI smoke test asserting `--version`, `--help`, `--config <nonexistent>` exit codes.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.6.0 (locked) | CLI arg parsing | Already declared; derive feature enabled |
| thiserror | 2.x (locked) | Error enum | Already in use; `ConfigNotFound`/`ConfigInvalid` reused |
| tokio | 1.x (locked) | Async runtime | `#[tokio::main]` macro wraps sync clap parse |

No new dependencies. Version verified via `cargo metadata`.

**Installation:** Nothing to install — clap 4 with derive feature is already in `Cargo.toml` and `Cargo.lock`.

---

## Architecture Patterns

### Recommended Project Structure

No new files required. All changes are in:
```
crates/fast-staged/src/
├── main.rs          # Replace hand-rolled block with Parser struct + clap::Parser::parse()
└── lib.rs           # Add config_path: Option<PathBuf> as first param to run()
```

The config layer (`src/config/discover.rs`) already implements `load_config(Option<&Path>)` — no changes there.

### Pattern 1: clap 4 Derive — Parser Struct

**What:** Define a struct that derives `clap::Parser`. Fields become flags. clap auto-generates `--help`, `--version`, short `-h`/`-V` aliases, and unknown-flag rejection.

**When to use:** Single-command binary with no subcommands (this tool's case).

```rust
// Source: clap 4 derive docs — https://docs.rs/clap/4.6.0/clap/_derive/index.html
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to the config file (overrides auto-discovery)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Suppress success output
    #[arg(long)]
    quiet: bool,

    /// Run on modified (not staged) files
    #[arg(long)]
    unstaged: bool,

    /// Exit 0 when no files match any pattern
    #[arg(long)]
    allow_empty: bool,
}
```

**Key behaviors verified (clap 4.6.0):**
- `--config` field named `config` maps to `--config` flag; field type `Option<PathBuf>` makes it optional.
- clap handles `Option<PathBuf>` parsing: it calls `PathBuf::from(str)`, which is infallible (no file existence check). File existence is our responsibility, not clap's.
- `#[command(version)]` reads `version` from `Cargo.toml` at compile time via `CARGO_PKG_VERSION` env var.
- `#[command(about)]` reads `description` from `Cargo.toml [package].description`.
- `snake_case` field names auto-convert to `kebab-case` flags: `allow_empty` → `--allow-empty`. This matches the existing flag in main.rs.
- Unknown flags: clap exits with code 2 and prints a similar-flag suggestion (default behavior, no configuration needed).

### Pattern 2: Sync Parse Before Async Entry

**What:** `clap::Parser::parse()` is synchronous and terminates the process on `--help`/`--version`/error before the tokio runtime starts.

**When to use:** Always for single-command binary. Keep `Parser::parse()` as the first statement in `main()`, before `#[tokio::main]` async body.

```rust
// Source: clap 4 idiomatic pattern
#[tokio::main]
async fn main() {
    let cli = Cli::parse();  // sync; exits process for --help/--version/bad args
    if let Err(e) = fast_staged::run(
        cli.config,
        cli.unstaged,
        cli.allow_empty,
        cli.quiet,
    )
    .await
    {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
```

**Important:** `Cli::parse()` calls `std::process::exit()` internally for `--help`/`--version`/errors — it does NOT return `Result`. This is intentional clap CLI behavior (exit code 0 for `--help`/`--version`, 2 for parse errors).

### Pattern 3: CWD-Anchored Path Resolution for --config

**What:** When `--config` receives a relative path, resolve it against CWD before passing to `load_config`. The `PathBuf` from clap is not canonicalized — it is the raw string as typed by the user.

**When to use:** Always for explicit-path loading.

```rust
// Resolve relative --config path against CWD before passing to lib::run()
let config_path = cli.config.map(|p| {
    if p.is_relative() {
        std::env::current_dir()
            .expect("could not determine CWD")
            .join(p)
    } else {
        p
    }
});
```

Alternatively, resolve inside `lib::run()` — either location is correct per D-04. Resolving in `main.rs` keeps `lib::run()` free of env side-effects and is easier to test.

### Pattern 4: lib::run() Signature Extension

**What:** Add `config_path: Option<PathBuf>` as first parameter. Pass it directly to `config::load_config()`.

```rust
// lib.rs — updated signature (D-08)
pub async fn run(
    config_path: Option<PathBuf>,
    unstaged: bool,
    allow_empty: bool,
    quiet: bool,
) -> Result<(), AppError> {
    let config = config::load_config(config_path.as_deref())?;
    // ... rest unchanged
}
```

`as_deref()` converts `Option<PathBuf>` to `Option<&Path>` — matches the existing `load_config(Option<&Path>)` signature.

### Anti-Patterns to Avoid

- **Calling `Cli::parse()` inside `tokio::main` body is fine, but calling `Cli::try_parse()` + custom error handling is overkill** — the default process-exit behavior is correct for CLI tools.
- **Do not re-validate path existence in clap's `value_parser`** — clap value_parser runs before our AppError handling chain. A failed parse there would print a clap-formatted error, not our styled `error_box`. Let clap own only the type conversion; let our code own the semantic validation (file exists?).
- **Do not use `PathBuf::canonicalize()` for `--config` resolution** — `canonicalize()` fails if the path doesn't exist, which would surface as an `Io` error rather than the expected `ConfigNotFound`. Resolve with `join()` only; existence check is done inside `load_config()`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| `--help` generation | Custom formatter | clap `#[command(about)]` | Handles terminal width, formatting, exit 0 |
| `--version` output | `env!("CARGO_PKG_VERSION")` + manual print | clap `#[command(version)]` | Handles `-V`/`--version`, exit 0, consistent format |
| Unknown-flag rejection | Manual argv scan | clap default | Similar-flag suggestions, exit code 2, correct stderr |
| `Option<PathBuf>` arg parsing | Manual `args.windows(2)` | `#[arg(long)] config: Option<PathBuf>` | Missing-value errors, type conversion, help text |

**Key insight:** clap's derive macro generates the same behavior that took 10+ lines of hand-rolled code, handles edge cases (missing arg value, `--config=path` vs `--config path`), and gives consistent UX.

---

## Common Pitfalls

### Pitfall 1: clap Parse Happens Before AppError Rendering

**What goes wrong:** If you call `Cli::parse()` after the tokio runtime initializes and after any early-return error paths, clap's own errors won't be rendered by your `eprintln!("error: {e}")` block.
**Why it happens:** `Cli::parse()` prints to stderr and calls `std::process::exit()` — bypassing Rust's normal return path entirely.
**How to avoid:** `Cli::parse()` must be the first line of `main()`. It currently is, per the pattern in existing `main.rs`.
**Warning signs:** Tests expecting `AppError` formatting for unknown flags — those never reach Rust code, they terminate inside clap.

### Pitfall 2: PathBuf::from() Never Fails for --config

**What goes wrong:** Developer assumes clap will validate that `--config path` points to an existing file.
**Why it happens:** `PathBuf::from(str)` is infallible — it accepts any string as a path, even nonsense. clap calls this for `Option<PathBuf>` args.
**How to avoid:** Validate existence explicitly in `load_config()`. This is already implemented: `if !path.exists() { return Err(AppError::ConfigNotFound); }`.
**Warning signs:** Expecting exit code 2 (clap error) when a nonexistent path is passed — actual exit will be 1 (our AppError::ConfigNotFound).

### Pitfall 3: snake_case vs kebab-case Field Names

**What goes wrong:** `allow_empty` field becomes `--allow-empty` automatically — but developers sometimes write `#[arg(long = "allow_empty")]` (underscore) which would create `--allow_empty` (non-standard, fails D-05 strict rejection test since the right flag would be unrecognized).
**Why it happens:** clap auto-converts underscores to hyphens for `long` by default. Explicit `long = "..."` overrides this.
**How to avoid:** Use field names as-is: `allow_empty`, `quiet`, `unstaged`. Let clap handle the conversion.
**Warning signs:** Integration test `--allow-empty` being rejected as unknown flag.

### Pitfall 4: --help / --version Short-Circuit Before Tokio Runtime

**What goes wrong:** If any initialization code runs before `Cli::parse()` (e.g., logging init, config probe), `--help` exits without running teardown for that code.
**Why it happens:** `process::exit()` skips destructors.
**How to avoid:** Keep `Cli::parse()` as the first statement. No initialization before it.
**Warning signs:** Resources (temp files, connections) not cleaned up on `--help`.

### Pitfall 5: Integration Test Call Sites for lib::run()

**What goes wrong:** After adding `config_path` parameter, `lib::run()` callers that pass positional args will fail to compile.
**Why it happens:** The 6 tests in `execution_integration.rs` invoke the **binary** via `run_binary()` (process-level), not `lib::run()` directly, so they are unaffected. But any unit test or doc-test calling `fast_staged::run(unstaged, allow_empty, quiet)` directly will break.
**How to avoid:** Search for all callers of `fast_staged::run(` before implementing. Currently: only `main.rs` line 10 calls it directly.
**Warning signs:** Compile error `fast_staged::run(...) — expected 4 arguments, found 3`.

### Pitfall 6: --config Path with .. Components

**What goes wrong:** `../sibling/config.toml` passed to `--config` works on Unix but may behave differently than expected: it resolves relative to CWD at process start (D-04), not relative to any config search root.
**Why it happens:** D-04 is explicit: CWD-anchored. The `join()` approach handles `..` correctly via OS path resolution.
**How to avoid:** No special handling needed. `current_dir().join("../sibling/config.toml")` resolves correctly on all platforms. Do NOT call `canonicalize()` (see Pitfall 2 above).
**Warning signs:** Tests using `../` in config paths — just verify they work with temp dir setups.

---

## Code Examples

### Complete main.rs After Phase 6

```rust
// Source: clap 4.6.0 derive docs + project D-01..D-08
use std::{path::PathBuf, process};

use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to the config file (overrides auto-discovery)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Suppress success summary output
    #[arg(long)]
    quiet: bool,

    /// Run on modified (not staged) files instead of staged files
    #[arg(long)]
    unstaged: bool,

    /// Exit 0 when no staged files or no patterns match
    #[arg(long)]
    allow_empty: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Resolve relative --config path against CWD (D-04).
    let config_path = cli.config.map(|p| {
        if p.is_relative() {
            std::env::current_dir()
                .expect("cannot determine CWD")
                .join(p)
        } else {
            p
        }
    });

    if let Err(e) = fast_staged::run(config_path, cli.unstaged, cli.allow_empty, cli.quiet).await {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
```

### Updated lib::run() Signature

```rust
// Source: D-08 from CONTEXT.md
pub async fn run(
    config_path: Option<PathBuf>,
    unstaged: bool,
    allow_empty: bool,
    quiet: bool,
) -> Result<(), AppError> {
    let config = config::load_config(config_path.as_deref())?;
    // ... rest of function unchanged
}
```

### Existing load_config() — No Changes Required

```rust
// config/discover.rs — already handles both paths
pub fn load_config(explicit_path: Option<&Path>) -> Result<Config, AppError> {
    if let Some(path) = explicit_path {
        if !path.exists() {
            return Err(AppError::ConfigNotFound);  // D-02, D-03
        }
        let source = ConfigSource::Explicit(path.to_path_buf());
        parse_config_at(path, source)
    } else {
        // Auto-discovery walk (unchanged)
        let cwd = std::env::current_dir().map_err(AppError::Io)?;
        let config_path = find_config_file(&cwd)?;
        // ...
    }
}
```

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`cargo test`) |
| Config file | None needed — `cargo test` discovers tests automatically |
| Quick run command | `cargo test -p fast-staged --lib 2>&1` |
| Full suite command | `cargo test -p fast-staged 2>&1` |
| CLI smoke test command | `cargo test -p fast-staged --test execution_integration 2>&1` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-01a | `--config path` loads explicit config | integration (binary) | `cargo test -p fast-staged --test execution_integration config_explicit` | ❌ Wave 0 |
| CLI-01b | `--config missing` exits 1 with ConfigNotFound | integration (binary) | `cargo test -p fast-staged --test execution_integration config_missing` | ❌ Wave 0 |
| CLI-01c | `--quiet` flag forwarded to run() | integration (binary) | `cargo test -p fast-staged --test execution_integration quiet_flag` | ❌ Wave 0 |
| CLI-01d | `--unstaged` accepted (existing test covers behavior) | integration (binary) | existing `end_to_end_unstaged_failure_skips_backup_and_cleanup` | ✅ |
| CLI-01e | `--allow-empty` accepted (existing test covers behavior) | integration (binary) | existing `allow_empty_clean_repo_exits_zero_silently` | ✅ |
| CLI-01f | Unknown flag rejected, exit 2 | integration (binary) | `cargo test -p fast-staged --test execution_integration unknown_flag_rejected` | ❌ Wave 0 |
| CLI-02 | `--version` prints semver, exits 0 | smoke (binary) | `cargo test -p fast-staged --test execution_integration version_flag` | ❌ Wave 0 |
| CLI-03 | `--help` prints usage, exits 0 | smoke (binary) | `cargo test -p fast-staged --test execution_integration help_flag` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p fast-staged --lib`
- **Per wave merge:** `cargo test -p fast-staged`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

The existing `tests/execution_integration.rs` tests the binary process-level. New tests can be added to the same file. No new files required.

New test functions to add in `tests/execution_integration.rs`:
- [ ] `version_flag` — assert exit 0 + stdout contains version string matching `env!("CARGO_PKG_VERSION")`
- [ ] `help_flag` — assert exit 0 + stdout contains `--config`, `--quiet`, `--unstaged`, `--allow-empty`
- [ ] `unknown_flag_rejected` — assert exit 2 (clap convention) for `--quet` (typo)
- [ ] `config_explicit_loads` — write a temp config + `--config <path>`, assert exit 0
- [ ] `config_missing_exits_nonzero` — pass `--config /nonexistent/path.toml`, assert exit 1 + stderr contains "configuration file not found"
- [ ] `quiet_flag_suppresses_output` — assert stdout is empty on success with `--quiet`

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `clap::ArgMatches` (builder API) | `#[derive(Parser)]` (derive API) | clap 3+ | Derive is now standard; builder still valid but verbose |
| `clap 3.x` | `clap 4.x` (locked 4.6.0) | 2022 | No `App::new()` — use `Command::new()` or derive |

**Deprecated in clap 4:**
- `App::new()` → use `Command::new()` (builder) or derive
- `value_t!()` macro → use typed args directly
- `Arg::with_name()` → use `Arg::new()`

(None of these affect us — we are using pure derive.)

---

## Open Questions

1. **`--config` error message verbosity**
   - What we know: `AppError::ConfigNotFound` renders as "configuration file not found" (no path shown, see `error.rs` line 6).
   - What's unclear: Should the error include the supplied path? e.g., "configuration file not found: /path/to/config.toml" (D-03 says use existing variants, but the existing variant has no `{path}` in its display).
   - Recommendation: Research note — the planner should check if `AppError::ConfigNotFound` should be extended with a `path` field for the explicit-path case. D-03 says "reuse variants" but the existing message is generic. Adding a path field is a small change to `error.rs` that would improve usability. This is a LOW confidence concern — either approach is defensible without changing D-03.

2. **`--config` relative path resolution site**
   - What we know: D-04 says CWD-anchored. We can resolve in `main.rs` (before `lib::run()`) or inside `lib::run()`.
   - What's unclear: Both sites are correct. Resolving in `main.rs` keeps `lib::run()` testable without env side-effects.
   - Recommendation: Resolve in `main.rs` using `current_dir().join(p)`. Document as convention.

---

## Environment Availability

Step 2.6: SKIPPED (phase is pure code/config changes within an already-installed Rust workspace; no new external dependencies identified).

---

## Sources

### Primary (HIGH confidence)
- clap 4.6.0 — resolved via `cargo metadata` on project Cargo.lock
- `crates/fast-staged/src/config/discover.rs` — verified `load_config(Option<&Path>)` signature and both branches
- `crates/fast-staged/src/lib.rs` — verified current `run()` signature and `load_config(None)` call site
- `crates/fast-staged/src/main.rs` — verified hand-rolled block (lines 5-13) to be replaced
- `crates/fast-staged/src/error.rs` — verified `ConfigNotFound` and `ConfigInvalid` variants
- `crates/fast-staged/tests/execution_integration.rs` — verified 6 tests use `run_binary()` (process-level, not `lib::run()` directly)
- `crates/fast-staged/Cargo.toml` — verified `clap = { version = "4", features = ["derive"] }` declared

### Secondary (MEDIUM confidence)
- clap 4 derive docs pattern — consistent with project's declared dependency and Rust ecosystem standards

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — clap 4.6.0 locked in Cargo.lock, no new deps
- Architecture: HIGH — all files read directly, API shapes verified from source
- Pitfalls: HIGH — derived from direct code inspection and clap 4 behavior knowledge
- Validation architecture: HIGH — existing test infrastructure understood; new test functions identified

**Research date:** 2026-04-29
**Valid until:** 2026-05-29 (stable; clap 4.x has no breaking changes between 4.x minors)
