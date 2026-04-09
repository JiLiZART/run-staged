# Coding Conventions

**Analysis Date:** 2026-04-09

## Project Context

This is an early-stage Rust project building a native Node.js binary via NAPI-RS. The only
source file in existence is `crates/cli/src/main.rs` (a 3-line stub). Conventions below
are drawn from tooling configuration, CI pipeline, and design documentation in TODO.md/ROADMAP.md.

## Naming Patterns

**Rust files/modules:**
- snake_case for all file names: `main.rs`, and planned modules like `execute_commands`, `match_files_to_commands`, `run_ui`
- Functions use snake_case: `execute_commands()`, `get_changed_files()`, `load_config()`, `find_config_file()`, `restore_terminal()`, `run_single_command()`
- Structs use PascalCase: `TaskState`, `Config`, `Group`, `AppError`, `CommandStatus`, `BackupState`, `ConfigSource`
- Enums use PascalCase with PascalCase variants: `CommandStatus::Cancelled`, `ExecutionOrder::Parallel`, `ExecutionBehavior::PerFile`, `PathFormat::Relative`
- Traits use PascalCase: `StatusDisplay`
- Constants/statics: not yet defined, follow Rust convention of SCREAMING_SNAKE_CASE

**TOML configuration:**
- Keys use snake_case: `timeout`, `execution_order`, `continue_on_error`, `path_format`
- Section headers use groupN naming: `[group1]`, `[group1.patterns]`

## Code Style

**Formatting (Rust):**
- Tool: `rustfmt` (enforced via CI: `cargo fmt -- --check`)
- No custom `rustfmt.toml` present; uses Rust edition 2024 defaults
- Edition: Rust 2024 (set in `crates/cli/Cargo.toml`)

**Formatting (TOML):**
- Tool: taplo (config in `.taplo.toml`)
- `align_entries = true` — values in a block align vertically
- `indent_tables = true` — nested tables are indented
- `reorder_keys = true` — keys are sorted alphabetically
- Excludes: `node_modules/**/*.toml`

**General formatting (all files):**
- Tool: EditorConfig (`.editorconfig`)
- Indent: 2 spaces for all files
- Line endings: LF
- Charset: UTF-8
- Trailing whitespace trimmed (except `.md` files)
- Final newline required

**JavaScript/TypeScript (build tooling):**
- Linter: oxlint (run via `yarn lint`, referenced in CI)
- Prettier referenced in `.prettierignore` (excludes: `target/`, `.yarn/`, generated NAPI files)
- No explicit `.prettierrc` found — likely using defaults or napi-rs template defaults

## Linting

**Rust:**
- Tool: Clippy (`cargo clippy`, enforced in CI)
- No custom `clippy.toml` — uses default Clippy lints
- Build flags from `.cargo/config.toml`: `-Zlocation-detail=none -Zfmt-debug=none` (nightly flags for size optimization)

**JavaScript/TypeScript:**
- Tool: oxlint (`yarn lint`)
- No eslint config found; oxlint is the sole JS linter

## Error Handling

**Designed pattern** (from TODO.md planned code):
- Use a typed `AppError` enum with `thiserror::Error` derive
- All public functions return `Result<T, AppError>` (not `Box<dyn Error>`)
- Error variants carry context fields, e.g.:
  ```rust
  #[error("Configuration file not found. Checked: {checked_paths:?}")]
  ConfigNotFound { checked_paths: Vec<PathBuf> },
  ```
- Use `.map_err()` to wrap lower-level errors at call sites
- IO errors: `#[from] std::io::Error` for automatic conversion
- Git errors: `#[from] gix::Error` for automatic conversion
- Planned dependency: `thiserror` for error derive macros

## Logging

**Approach:** Not yet implemented. No logging crate referenced in `Cargo.toml`.
- Planned: terminal UI via `ratatui` for status display (not `println!`-style logging)
- Debug/development: standard `println!` macro only (current stub uses this)

## Comments

**Language:** Project documentation in `TODO.md` and `ROADMAP.md` is written in Russian.
Code comments are expected in Rust doc comment style (`///` for public items).

**When to comment:**
- Complex algorithms (e.g., git staging logic)
- Non-obvious configuration decisions
- Planned: inline code examples in doc comments for public API surface

## Function Design

**Size:** Functions are planned as single-responsibility units:
- `backup_files_to_stash()`, `revert_files_from_stash()`, `find_config_file()`, `load_config_from_package_json()`, `command_exists()`

**Parameters:**
- Use `Arc<AtomicBool>` for shared cancellation flags (designed pattern)
- Use `Option<String>` for optional config values like `timeout`
- Pass `&Path` for file path parameters

**Return Values:**
- `Result<T, AppError>` for fallible operations
- `async fn` with `Result<()>` for top-level orchestration functions

**Async:**
- Runtime: `tokio` (planned, not yet in `Cargo.toml` but referenced in TODO.md)
- Async functions use `.await` directly, not blocking wrappers
- Concurrent task management via `tokio::task::JoinSet`

## Module Design

**Structure:**
- Workspace at root `Cargo.toml`, crates under `crates/`
- Current crate: `crates/cli/` — the main binary
- Expected future modules within `crates/cli/src/`:
  - `config.rs` — config loading and validation
  - `git.rs` — staged file discovery
  - `executor.rs` — command execution
  - `ui.rs` — TUI rendering via ratatui
  - `error.rs` — `AppError` enum

**Cargo workspace:**
- `resolver = "2"` (modern feature resolver)
- Separate dev and release profiles with explicit optimization settings

## Build Targets

Configured for cross-platform NAPI-RS binaries:
- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl` (via zigbuild)
- `wasm32-wasip1-threads`

Release profile optimized for binary size:
- `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`

---

*Convention analysis: 2026-04-09*
