# Technology Stack

**Project:** fast-staged
**Researched:** 2026-04-09
**Overall Confidence:** MEDIUM — versions from training data (cutoff Aug 2025); verify exact patch versions on crates.io before pinning in Cargo.toml

---

## Recommended Stack

### Core Framework

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Rust | edition 2024, nightly | All application logic | Edition 2024 unlocks async closures, precise captures; nightly required for `-Zlocation-detail=none` / `-Zfmt-debug=none` size flags already in `.cargo/config.toml` |
| tokio | 1.x (`features = ["full"]`) | Async runtime, signal handling, timers | Defacto Rust async runtime. `tokio::signal::ctrl_c()` for Ctrl+C, `tokio::time::timeout()` for command timeouts, `JoinSet` for parallel task management. No credible alternative in 2025. |
| ratatui | 0.29.x | Live terminal UI | Active successor to `tui-rs` (unmaintained). Async-compatible, widget library included, strong community. Crossterm backend chosen over termion for cross-platform support (Windows + Unix). |
| crossterm | 0.28.x | Terminal backend for ratatui | The only ratatui backend that works on Windows. Required because fast-staged targets Windows (existing MSVC CRT config). |

### Git Integration

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| gix | 0.66.x | Git staged file listing, diff, stash | Pure Rust git implementation — no shelling out, no dependency on system git, faster startup, type-safe API. Alternative `git2` (libgit2 binding) has C FFI overhead and licensing complexity. Project already mandates gix. |

### Config Parsing

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| serde | 1.x (`features = ["derive"]`) | Serialization framework | Universal foundation. All config deserialization uses serde derive macros. |
| toml | 0.8.x | TOML config parsing | Official TOML crate, serde integration, supports `.fast-staged.toml`. Use `toml::from_str()`. |
| serde_json | 1.x | JSON config parsing | Used for `.fast-staged.json` and `package.json["fast-staged"]` extraction. Also needed for lint-staged JSON compat format. |

### CLI Argument Parsing

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| clap | 4.x (`features = ["derive"]`) | CLI argument parsing | Most mature Rust CLI parser. Derive API keeps arg structs co-located with doc strings. Generates `--help` automatically. Features needed: `--config`, `--quiet`, `--unstaged`, `--allow-empty`, `--install`. |

### Error Handling

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| thiserror | 1.x or 2.x | Typed error enum | `#[derive(thiserror::Error)]` on `AppError` enum. The right choice for library-style typed errors (versus `anyhow` which is for application-level opaque errors). This codebase needs typed errors (caller branches on `AppError::NoStagedFiles` vs `AppError::NotGitRepository`). |
| color-eyre | 0.6.x | Pretty top-level error display | Renders errors with colors, suggestions, and spantrace in `main()`. Only applies at the top level — all internal functions use `AppError`. Lighter alternative: `miette` (better for structured diagnostics). Either works; `color-eyre` is simpler setup. |

### Glob Pattern Matching

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| globset | 0.4.x | Glob pattern matching against staged file paths | Part of the `ignore` crate ecosystem (used by ripgrep/fd). Supports `**`, `?`, `{a,b}` brace expansion. Compile patterns once at config load, match repeatedly. Alternative `glob` crate is older and lacks brace expansion. |

### Duration Parsing

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| parse_duration | 2.x | Human-readable timeout strings ("1sec", "5s", "500ms") | Already referenced in TODO.md. Small, no deps. Alternative: `humantime` crate (also fine, slightly more popular). |

### Process Execution

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| tokio::process | (built into tokio) | Async child process spawning | `tokio::process::Command` is the async analog of `std::process::Command`. Required for non-blocking parallel execution of linting tools. `kill_on_drop(true)` ensures cleanup on cancellation. |

### npm Binary Distribution

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| @nicolo-ribaudo/node-publish-binary | N/A — use SWC pattern manually | Platform binary npm packages | SWC pattern: one npm package per platform (e.g., `fast-staged-linux-x64-gnu`, `fast-staged-darwin-arm64`) as `optionalDependencies`, with a thin JS wrapper that selects the right binary. No NAPI-RS needed — just zip/copy pre-built binaries. |
| GitHub Actions cross-compilation | — | Build targets for all platforms | Use `cross` or official GitHub runners for each target. Targets needed: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`. |

---

## Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio-util | 0.7.x | Additional async utilities (CancellationToken) | Use `CancellationToken` instead of raw `Arc<AtomicBool>` for task cancellation — cleaner API, integrates with tokio's task machinery |
| unicode-width | 0.1.x | Correct Unicode character widths for TUI | Required if displaying file paths or command names that may contain CJK or emoji characters in ratatui widgets |
| dirs | 5.x | Platform-aware config directory lookup | If adding `--install` subcommand that writes git hooks to platform-correct path |

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Async runtime | tokio | async-std | async-std has smaller ecosystem, fewer integrations, less maintained in 2025. tokio is the standard. |
| Git | gix | git2 (libgit2) | git2 requires C library, has FFI overhead, more complex cross-compilation for static binaries. gix is pure Rust, easier to statically link. |
| Git | gix | shell out to `git` | Runtime dependency on system git, subprocess overhead, parsing text output is fragile. Forbidden by project constraints. |
| TUI | ratatui | cursive | cursive is simpler but less flexible; doesn't support fine-grained async updates as cleanly as ratatui + crossterm. |
| TUI | ratatui | egui (terminal) | egui is for GUI apps; no serious terminal use case. |
| CLI parsing | clap | argh / lexopt | argh is lighter but lacks auto-generated help richness. lexopt is for hand-rolled parsers. clap derive is the standard for user-facing CLIs. |
| Error handling (top-level) | color-eyre | miette | miette is excellent for structured diagnostic output but has higher API complexity. color-eyre is simpler for a CLI that just needs pretty error rendering. |
| Error handling (domain) | thiserror | anyhow | anyhow erases error type — callers can't branch on variant. thiserror preserves types, which this codebase needs (`NoStagedFiles` vs `ConfigNotFound` have different exit codes). |
| Glob | globset | glob | glob crate lacks `{a,b}` brace expansion used in the config examples (`"*.(js|ts|tsx|jsx)"`). globset supports it. Note: globset uses `{a,b}` syntax, not `(a|b)` — config parsing needs to normalize or use a regex adapter. |
| Config format | toml | yaml | TOML is more readable for structured configs with inline tables; no indentation sensitivity bugs. YAML has footguns with type coercion. |
| npm distribution | Plain binary zips | NAPI-RS | NAPI-RS creates native `.node` modules — requires Node.js. fast-staged is a standalone CLI; plain binary distribution avoids the Node.js runtime dependency entirely. CI workflow currently uses NAPI-RS and MUST be replaced. |
| Cancellation | tokio-util CancellationToken | Arc\<AtomicBool\> | CancellationToken is a first-class tokio primitive designed exactly for this; supports hierarchical cancellation (cancel parent, all children cancel). Arc\<AtomicBool\> works but is lower-level. |

---

## Version Confidence Notes

Versions listed are from training data (cutoff August 2025). Before writing Cargo.toml:

- **ratatui**: verify current on crates.io — was at 0.28.x as of mid-2025, likely 0.29.x by Apr 2026. MEDIUM confidence.
- **gix**: verify — was at ~0.64.x mid-2025, moving fast. MEDIUM confidence.
- **tokio**: 1.x stable, major version stable since 2021. HIGH confidence.
- **clap**: 4.x stable. HIGH confidence.
- **serde / serde_json / toml**: all 1.x stable. HIGH confidence.
- **thiserror**: 2.x released late 2024 (API-compatible upgrade). HIGH confidence.
- **globset**: 0.4.x, part of BurntSushi's ignore crate ecosystem. HIGH confidence.
- **crossterm**: 0.28.x as of 2025. MEDIUM confidence.

---

## Critical Non-Obvious Decisions

### 1. gix staged-file API shape

The gix API for listing index entries (staged files) is not the same as `git diff --cached`. You need:

```rust
let repo = gix::open(".")?;
let index = repo.index()?;
for entry in index.entries() {
    // entry.path(&index) gives the staged path
}
```

To get only *changed* staged files (not every file tracked), diff the index against HEAD:

```rust
let head_tree = repo.head_commit()?.tree()?;
let index = repo.index()?;
let diff = repo.diff_index_to_tree(/* ... */);
```

The exact gix API varies by version. The gix book and examples directory are the authoritative source. Plan a research spike on gix API before writing the git layer.

### 2. ratatui + tokio integration

ratatui draws synchronously in a render loop; tokio runs async tasks. The standard pattern is:

- Run tokio tasks on a separate thread/task pool
- Communicate task state to TUI via `tokio::sync::watch` channel or `Arc<Mutex<StateModel>>`
- TUI render loop polls every ~16ms (60fps) or on state change via `mpsc` notification

The `StateModel` + `Message` architecture in ARCHITECTURE.md is correct for this pattern. Use `tokio::sync::mpsc` for messages from executor to UI, and `tokio::sync::watch` for the cancellation flag.

### 3. Glob syntax mismatch

The ROADMAP config examples use `(a|b)` syntax: `"*.(js|ts|tsx|jsx)"`. This is NOT standard glob syntax — it is regex-style. Standard glob brace expansion is `*.{js,ts,tsx,jsx}`. The `globset` crate supports `{a,b}` but NOT `(a|b)`. Decision needed at config layer: either normalize `(a|b)` → `{a,b}` at parse time, or use a regex-based file matcher for pattern groups.

### 4. npm distribution CI is broken and must be rebuilt from scratch

The existing `.github/workflows/CI.yml` is NAPI-RS — it will not work. Replacement needs:
- One job per target platform (matrix strategy)
- `cargo build --release --target <triple>`
- Artifact upload per platform
- npm package creation with platform-specific binary as `bin` + `optionalDependencies` model
- Reference: SWC npm distribution, oxc-project/oxc npm distribution

### 5. `--quiet` flag bypasses ratatui entirely

When `--quiet` is set, never call `ratatui::init()` — calling it in a non-TTY context (e.g., CI log output) corrupts terminal state. Guard all TUI initialization behind TTY detection:

```rust
if opts.quiet || !std::io::stdout().is_terminal() {
    run_quiet(commands).await?;
} else {
    run_tui(commands).await?;
}
```

---

## Installation (initial Cargo.toml)

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
ratatui = "0.29"
crossterm = "0.28"
gix = { version = "0.66", default-features = false, features = ["index", "status", "diff"] }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
serde_json = "1"
thiserror = "2"
color-eyre = "0.6"
globset = "0.4"
parse_duration = "2"
tokio-util = { version = "0.7", features = ["rt"] }

[dev-dependencies]
tempfile = "3"
```

**Note:** Verify all versions on crates.io before committing. Pin to `~X.Y` (minor) not `X` (major) for production to avoid unexpected breakage from minor semver bumps that gix in particular ships frequently.

---

## Sources

- Project files: `.planning/PROJECT.md`, `.planning/codebase/STACK.md`, `.planning/codebase/ARCHITECTURE.md`, `.planning/codebase/CONCERNS.md`, `TODO.md`, `ROADMAP.md`
- Training knowledge (Aug 2025 cutoff) for crate version ranges and ecosystem state — MEDIUM confidence
- Crate versions: verify at https://crates.io before pinning
- gix docs: https://github.com/Byron/gitoxide
- ratatui docs: https://ratatui.rs
- SWC npm distribution pattern: https://github.com/swc-project/swc (reference for binary npm packages)
