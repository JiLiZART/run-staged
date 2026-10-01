<!-- GSD:project-start source:PROJECT.md -->
## Project

**run-staged**

A fast Rust replacement for lint-staged and nano-staged. Runs configured commands on git-staged files with parallel execution, glob pattern matching, and a live terminal UI. Distributed as both a Cargo binary crate and platform-specific npm packages (like SWC).

**Core Value:** Run linting/formatting commands on staged files faster than lint-staged, with zero Node.js runtime dependency.

### Constraints

- **Language**: Rust (edition 2024), nightly toolchain for size optimization flags
- **Async runtime**: tokio with full features
- **Git**: gix crate (no shelling out to git CLI)
- **TUI**: ratatui for terminal rendering
- **Config parsing**: serde + toml + serde_json
- **CLI**: clap for argument parsing
- **Errors**: thiserror for typed error enum
<!-- GSD:project-end -->

<!-- GSD:stack-start source:codebase/STACK.md -->
## Technology Stack

## Languages
- Rust (edition 2024) - Library crates in `crates/{config,git,matcher,exec,output,core}/src/`, binary in `apps/run-staged/src/`; `crates/fast-staged/` is the frozen MVP reference
- None currently present in source; `.prettierignore` references JS artifacts (wasi), suggesting future WASM/JS output may be planned
## Runtime
- Native binary (compiled Rust)
- Target: cross-platform; explicit Windows MSVC static CRT config in `.cargo/config.toml`
- Cargo (Rust toolchain) — workspace at `Cargo.toml`
- Lockfile: `Cargo.lock` — NOT present yet (project is in skeleton state)
- Yarn 4.13.0 also present (`.yarn/releases/yarn-4.13.0.cjs`, `.yarnrc.yml`) — used for dev tooling (husky, lint-staged) not for the Rust binary itself
## Frameworks
- Shared versions in root `[workspace.dependencies]`; each crate declares only what it uses
- `tokio` (async runtime, `features = ["full"]`) — async command execution, signal handling
- `ratatui` — terminal UI rendering
- `gix` — git operations (staged file listing, stash, diff)
- `clap` — CLI argument parsing (`--config`, `--quiet`, `--unstaged`, `--allow-empty`)
- `thiserror` — typed error enum (`AppError`)
- `toml` — TOML config parsing
- `serde_json` — JSON config parsing (`run-staged.json`, `package.json` section)
- `parse_duration` — human-readable timeout parsing (e.g., `"1sec"`, `"5s"`)
- Taplo — TOML formatter, config at `.taplo.toml`
- Prettier — JS/general formatter, ignore rules at `.prettierignore`
- Husky — git hooks, config at `.husky/pre-commit`
- lint-staged — staged file linting, invoked via husky pre-commit hook
## Key Dependencies
- `gix` — core git interaction (staged files, stash, patch diff)
- `ratatui` — full TUI rendering with live task progress
- `tokio` — async runtime for parallel command execution
- `thiserror` — structured error types
- `toml` + `serde_json` — multi-format config loading
## Configuration
- No environment variables required at runtime
- Tool reads project-local config files in this priority order (planned):
- `Cargo.toml` (workspace) — release profile: `lto = true`, `opt-level = "z"`, `strip = true`, `panic = "abort"`, `codegen-units = 1`
- `.cargo/config.toml` — Windows MSVC static CRT; unstable `rustflags` for size/debug-info stripping (`-Zlocation-detail=none`, `-Zfmt-debug=none`)
- `.taplo.toml` — TOML formatter settings (aligned entries, indented tables, reordered keys)
- `.editorconfig` — 2-space indent, LF line endings, UTF-8
## Platform Requirements
- Rust toolchain (nightly implied by `-Z` flags in `.cargo/config.toml`)
- Yarn 4 (for dev tooling: husky, lint-staged)
- Taplo CLI (TOML formatting)
- Compiled native binary
- Designed to run as a git pre-commit hook (replaces/complements lint-staged/nano-staged)
- No server or network access required
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

## Project Context
## Naming Patterns
- snake_case for all file names: `main.rs`, and planned modules like `execute_commands`, `match_files_to_commands`, `run_ui`
- Functions use snake_case: `execute_commands()`, `get_changed_files()`, `load_config()`, `find_config_file()`, `restore_terminal()`, `run_single_command()`
- Structs use PascalCase: `TaskState`, `Config`, `Group`, `AppError`, `CommandStatus`, `BackupState`, `ConfigSource`
- Enums use PascalCase with PascalCase variants: `CommandStatus::Cancelled`, `ExecutionOrder::Parallel`, `ExecutionBehavior::PerFile`, `PathFormat::Relative`
- Traits use PascalCase: `StatusDisplay`
- Constants/statics: not yet defined, follow Rust convention of SCREAMING_SNAKE_CASE
- Keys use snake_case: `timeout`, `execution_order`, `continue_on_error`, `path_format`
- Section headers use groupN naming: `[group1]`, `[group1.patterns]`
## Code Style
- Tool: `rustfmt` (enforced via CI: `cargo fmt -- --check`)
- No custom `rustfmt.toml` present; uses Rust edition 2024 defaults
- Edition: Rust 2024 (set in `crates/cli/Cargo.toml`)
- Tool: taplo (config in `.taplo.toml`)
- `align_entries = true` — values in a block align vertically
- `indent_tables = true` — nested tables are indented
- `reorder_keys = true` — keys are sorted alphabetically
- Excludes: `node_modules/**/*.toml`
- Tool: EditorConfig (`.editorconfig`)
- Indent: 2 spaces for all files
- Line endings: LF
- Charset: UTF-8
- Trailing whitespace trimmed (except `.md` files)
- Final newline required
- Linter: oxlint (run via `yarn lint`, referenced in CI)
- Prettier referenced in `.prettierignore` (excludes: `target/`, `.yarn/`, generated NAPI files)
- No explicit `.prettierrc` found — likely using defaults or napi-rs template defaults
## Linting
- Tool: Clippy (`cargo clippy`, enforced in CI)
- No custom `clippy.toml` — uses default Clippy lints
- Build flags from `.cargo/config.toml`: `-Zlocation-detail=none -Zfmt-debug=none` (nightly flags for size optimization)
- Tool: oxlint (`yarn lint`)
- No eslint config found; oxlint is the sole JS linter
## Error Handling
- Use a typed `AppError` enum with `thiserror::Error` derive
- All public functions return `Result<T, AppError>` (not `Box<dyn Error>`)
- Error variants carry context fields, e.g.:
- Use `.map_err()` to wrap lower-level errors at call sites
- IO errors: `#[from] std::io::Error` for automatic conversion
- Git errors: `#[from] gix::Error` for automatic conversion
- Planned dependency: `thiserror` for error derive macros
## Logging
- Planned: terminal UI via `ratatui` for status display (not `println!`-style logging)
- Debug/development: standard `println!` macro only (current stub uses this)
## Comments
- Complex algorithms (e.g., git staging logic)
- Non-obvious configuration decisions
- Planned: inline code examples in doc comments for public API surface
## Function Design
- `backup_files_to_stash()`, `revert_files_from_stash()`, `find_config_file()`, `load_config_from_package_json()`, `command_exists()`
- Use `Arc<AtomicBool>` for shared cancellation flags (designed pattern)
- Use `Option<String>` for optional config values like `timeout`
- Pass `&Path` for file path parameters
- `Result<T, AppError>` for fallible operations
- `async fn` with `Result<()>` for top-level orchestration functions
- Runtime: `tokio` (planned, not yet in `Cargo.toml` but referenced in TODO.md)
- Async functions use `.await` directly, not blocking wrappers
- Concurrent task management via `tokio::task::JoinSet`
## Module Design
- Workspace at root `Cargo.toml`, crates under `crates/`
- Current crate: `crates/cli/` — the main binary
- Expected future modules within `crates/cli/src/`:
- `resolver = "2"` (modern feature resolver)
- Separate dev and release profiles with explicit optimization settings
## Build Targets
- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl` (via zigbuild)
- `wasm32-wasip1-threads`
- `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

## Pattern Overview
- Rust Cargo workspace: small library crates in `crates/` (packages `run-staged-<name>`) plus the `apps/run-staged` binary; `crates/fast-staged` is the frozen MVP
- The binary (`apps/run-staged`) parses args and calls `run_staged_core::run`; git access is gix-only and lock-safe (no `git` process)
- Intended design is documented in `TODO.md` and `ROADMAP.md`: a pipeline of config loading → git file discovery → command matching → parallel/sequential execution → TUI rendering
- CI pipeline targets NAPI-RS native Node.js bindings (`.node` / `.wasm` artifacts), suggesting the final distribution will be a Node.js-consumable native module in addition to (or instead of) a standalone binary
## Intended Layers (from TODO.md / ROADMAP.md)
- Purpose: Discover and parse configuration from multiple file formats
- Planned functions: `Config::find_file()`, `Config::load()`, `Config::load_from_package_json()`
- Config sources searched in priority order: `.run-staged.toml`, `run-staged.toml`, `.run-staged.json`, `run-staged.json`, `package.json["run-staged"]`
- Planned enum `RunConfig { config: Option<PathBuf>, quiet: bool, unstaged: bool, stash: bool, allow_empty: bool, fail_on_changes: bool, revert: bool }`
- Planned enum: `ConfigSource { TomlFile(PathBuf), JsonFile(PathBuf), PackageJson(PathBuf) }`
- Planned struct: `Config` with `groups: Vec<Group>`, global `timeout: Option<Duration>`, `execution_order: Option<ExecutionOrder>`, `continue_on_error: bool` 
- Planned struct: `Group { name, patterns: HashMap<FilePattern, CommandList>, execution_order: ExecutionOrder, timeout: Option<Duration>, continue_on_error: bool, execution_behavior: ExecutionBehavior, path_format: PathFormat, run_on: ExecutionBehavior }`
- Purpose: Obtain the list of staged (or optionally unstaged) files from the repository
- Planned function: `GitContext::get_changed_files(options)`
- Uses `gix` crate for git operations
- Returns `Err(AppError::NotGitRepository)` if not in a git repo, `Err(AppError::NoStagedFiles)` if nothing staged
- Backup: copies the index to `<git-dir>/run-staged.index` before execution; restored under `<index>.lock` on failure, deleted after
- Purpose: Map staged files to commands defined in config groups
- Planned function: `Commands::match_from_files()` → returns list of `FileCommand` structs
- Iterates groups, matches files against glob patterns, collects unique file count
- Returns `Err(AppError::NoFilesMatched)` if no files match any group
- Purpose: Run matched commands, respecting group execution order, run on per-file or batch, and timeout
- Planned function: `Commands::execute(file_commands, options)`
- Supports `ExecutionOrder::Parallel` (default, via `tokio::task`) and `ExecutionOrder::Sequential`
- Supports `ExecutionBehavior::PerFile` (one process per file) and `ExecutionBehavior::Batch` (all files as args to one invocation)
- Timeout via `tokio::time::timeout()`
- Graceful cancellation via `Arc<AtomicBool>` cancellation flag
- Purpose: Track live status of each running command use `StateModel { groups: HashMap<GroupName, HashMap<PatternName, TaskState>>, total_count, execution_time, errors: String[] }` struct
- Handle updates to `StateModel` via `Message` enum, use `fn update(model: &StateModel, msg: Message) -> Model`
- Planned struct: `TaskState { filenames, command, status: Arc<Mutex<CommandStatus>>, stash_id: Option<String> }`
- Planned method: `Commands::run_command(task_state)`
- Planned enum: `CommandStatus { Running, Done, Failed, Timeout, Cancelled }` with a `StatusDisplay` trait for colored TUI rendering
- Purpose: Render a TUI using `ratatui` showing live command progress
- Planned function: `Renderer::view(model: &mut StateModel, f: &mut Frame)`
- Displays: per-command status with elapsed time, total affected file count, total execution time, grouped output by config group
- `--quiet` flag bypasses TUI entirely: runs tasks then prints a text summary
- If any errors, prints them in bordered colored box and exits
- Purpose: Centralized typed errors
- Planned enum `AppError` (using `thiserror`): `ConfigNotFound`, `ConfigInvalid`, `NotGitRepository`, `NoStagedFiles`, `NoFilesMatched`, `CommandNotFound`, `IoError`, `GitError`, `Cancelled`
## Data Flow
- `StateModel` holds all necessary state for UI
- `Message` enum is used to communicate between async executor and UI thread
- UI reads `StateModel` on each frame without blocking execution
## Key Abstractions
- Purpose: Represents parsed user configuration including groups and patterns
- File: `crates/config/src/`
- Pattern: Deserialized from TOML/JSON via `serde`
- Purpose: A named set of file patterns with associated commands and execution settings
- File: `crates/config/src/types.rs`
- Pattern: Contains `HashMap<FilePattern, CommandList>` plus execution knobs
- Purpose: Live tracking of a single command execution (one per file+command pair)
- File: `crates/exec/src/lib.rs`
- Pattern: Shared state via `Arc<Mutex<T>>` between async executor and UI thread
- Purpose: Typed, user-readable error enum covering all failure modes
- File: each crate has its own `error.rs`; `crates/core/src/lib.rs` wraps them in `AppError`
- Pattern: `thiserror::Error` derive with contextual messages
## Entry Points
- Location: `apps/run-staged/src/main.rs`
- Planned responsibilities: Parse CLI args, call `run()`, handle top-level errors with colored output
- Location: `crates/core/src/lib.rs`
- Triggers: Called from `main()`
- Responsibilities: Orchestrates config → git → match → execute → UI pipeline via `pub async fn run()`
## Error Handling
- All domain functions return `Result<T, AppError>` (planned)
- `?` operator propagates errors up to `main()`
- User-facing messages include actionable hints (e.g., "Run 'git add' to stage files")
## Cross-Cutting Concerns
<!-- GSD:architecture-end -->

## Workflow: OpenSpec

Planning lives in `openspec/` (spec-driven schema). `openspec/specs/` is the source of truth for current behavior; `openspec/changes/` holds pending work.

- `/opsx:explore` — think through an idea before committing to a change
- `/opsx:propose` — create a change (proposal, delta specs, design, tasks)
- `/opsx:apply` — implement a change's tasks
- `/opsx:archive` — archive a finished change and merge its deltas into `openspec/specs/`

Run `openspec list` / `openspec list --specs` to see state, `openspec validate --all --strict` before archiving.
