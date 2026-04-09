# Architecture

**Analysis Date:** 2026-04-09

## Pattern Overview

**Overall:** CLI tool with layered pipeline architecture (planned)

**Key Characteristics:**
- Rust Cargo workspace with a single `cli` crate currently (`crates/cli`)
- The binary is a stub (`main.rs` prints "Hello, world!") — implementation is not yet written
- Intended design is documented in `TODO.md` and `ROADMAP.md`: a pipeline of config loading → git file discovery → command matching → parallel/sequential execution → TUI rendering
- CI pipeline targets NAPI-RS native Node.js bindings (`.node` / `.wasm` artifacts), suggesting the final distribution will be a Node.js-consumable native module in addition to (or instead of) a standalone binary

## Intended Layers (from TODO.md / ROADMAP.md)

**Config Layer:**
- Purpose: Discover and parse configuration from multiple file formats
- Planned functions: `find_config_file()`, `load_config()`, `load_config_from_package_json()`
- Config sources searched in priority order: `.fast-staged.toml`, `fast-staged.toml`, `.fast-staged.json`, `fast-staged.json`, `package.json["fast-staged"]`
- Planned enum: `ConfigSource { TomlFile(PathBuf), JsonFile(PathBuf), PackageJson(PathBuf) }`
- Planned struct: `Config` with `groups: Vec<Group>`, global `timeout: Option<Duration>`
- Planned struct: `Group { name, patterns: HashMap<FilePattern, CommandList>, execution_order: ExecutionOrder, timeout: Option<Duration>, continue_on_error: bool, execution_behavior: ExecutionBehavior, path_format: PathFormat }`

**Git Layer:**
- Purpose: Obtain the list of staged (or optionally unstaged) files from the repository
- Planned function: `get_changed_files(options)`
- Uses `gix` crate for git operations
- Returns `Err(AppError::NotGitRepository)` if not in a git repo, `Err(AppError::NoStagedFiles)` if nothing staged
- Planned backup support: writes `.git/fast-staged.patch` before execution, deletes after

**Matching Layer:**
- Purpose: Map staged files to commands defined in config groups
- Planned function: `match_files_to_commands()` → returns list of `FileCommand` structs
- Iterates groups, matches files against glob patterns, collects unique file count
- Returns `Err(AppError::NoFilesMatched)` if no files match any group

**Execution Layer:**
- Purpose: Run matched commands, respecting group execution order and timeout
- Planned function: `execute_commands(file_commands, cancellation_flag) -> Result<(Vec<TaskState>, JoinSet<()>)>`
- Supports `ExecutionOrder::Parallel` (default, via `tokio::task`) and `ExecutionOrder::Sequential`
- Supports `ExecutionBehavior::PerFile` (one process per file) and `ExecutionBehavior::Batch` (all files as args to one invocation)
- Timeout via `tokio::time::timeout()`
- Graceful cancellation via `Arc<AtomicBool>` cancellation flag

**State / Task Layer:**
- Purpose: Track live status of each running command
- Planned struct: `TaskState { filename, command, status: Arc<Mutex<CommandStatus>>, stash_id: Option<String> }`
- Planned method: `TaskState::run_single_command(timeout, cancellation_flag)`
- Planned enum: `CommandStatus { Running, Done, Failed, Timeout, Cancelled }` with a `StatusDisplay` trait for colored TUI rendering

**UI Layer:**
- Purpose: Render a TUI using `ratatui` showing live command progress
- Planned function: `run_ui(states, total_files, cancellation_flag)`
- Displays: per-command status with elapsed time, total affected file count, total execution time, grouped output by config group
- `--quiet` flag bypasses TUI entirely: runs tasks then prints a text summary

**Error Layer:**
- Purpose: Centralized typed errors
- Planned enum `AppError` (using `thiserror`): `ConfigNotFound`, `ConfigInvalid`, `NotGitRepository`, `NoStagedFiles`, `NoFilesMatched`, `CommandNotFound`, `IoError`, `GitError`, `Cancelled`

## Data Flow

**Happy Path (planned):**

1. `main()` parses CLI args (via `clap`): `--config`, `--unstaged`, `--allow-empty`, `--quiet`
2. `load_config()` → finds config file, parses into `Config` with `Vec<Group>`
3. `get_changed_files()` → calls `gix` to get staged file list; writes `.git/fast-staged.patch` backup
4. `match_files_to_commands()` → maps files to commands per group, returns `Vec<FileCommand>` + total file count
5. `execute_commands()` → spawns async tasks per command/file, returns `Vec<TaskState>` + `JoinSet`
6. `run_ui()` (or quiet text output) → renders live TUI via `ratatui`, reads `TaskState` statuses until all complete
7. Cleanup: deletes `.git/fast-staged.patch`; if errors exist and revert configured, restores from stash

**Cancellation Flow:**

1. `tokio::signal::ctrl_c()` watcher sets `Arc<AtomicBool>` cancellation flag
2. `execute_commands()` polls flag, calls `JoinSet::abort_all()`, marks tasks `CommandStatus::Cancelled`
3. `run_ui()` polls flag, breaks render loop, restores terminal

**State Management:**
- Each `TaskState` holds an `Arc<Mutex<CommandStatus>>` shared between the executor task and the UI render loop
- UI reads status on each frame without blocking execution

## Key Abstractions

**Config:**
- Purpose: Represents parsed user configuration including groups and patterns
- Planned file: `crates/cli/src/config.rs` (not yet created)
- Pattern: Deserialized from TOML/JSON via `serde`

**Group:**
- Purpose: A named set of file patterns with associated commands and execution settings
- Planned file: `crates/cli/src/config.rs`
- Pattern: Contains `HashMap<FilePattern, CommandList>` plus execution knobs

**TaskState:**
- Purpose: Live tracking of a single command execution (one per file+command pair)
- Planned file: `crates/cli/src/executor.rs` (not yet created)
- Pattern: Shared state via `Arc<Mutex<T>>` between async executor and UI thread

**AppError:**
- Purpose: Typed, user-readable error enum covering all failure modes
- Planned file: `crates/cli/src/error.rs` (not yet created)
- Pattern: `thiserror::Error` derive with contextual messages

## Entry Points

**Binary Entry Point:**
- Location: `crates/cli/src/main.rs`
- Current state: Stub — only prints "Hello, world!"
- Planned responsibilities: Parse CLI args, call `run()`, handle top-level errors with colored output

**Library Entry Point (planned):**
- Location: `crates/cli/src/lib.rs` (not yet created)
- Triggers: Called from `main()` or from NAPI-RS bindings
- Responsibilities: Orchestrates config → git → match → execute → UI pipeline via `pub async fn run()`

## Error Handling

**Strategy:** Typed error enum (`AppError`) returned as `Result<T, AppError>` throughout; top-level `main()` formats errors with color (planned: `color_eyre` or `anyhow`)

**Patterns:**
- All domain functions return `Result<T, AppError>` (planned)
- `?` operator propagates errors up to `main()`
- User-facing messages include actionable hints (e.g., "Run 'git add' to stage files")

## Cross-Cutting Concerns

**Logging:** Not yet determined; `DEBUG: napi:*` env var in CI suggests NAPI-RS debug output
**Validation:** Config validation at load time via serde + explicit checks in `load_config()`
**Authentication:** Not applicable — local git tool
**Async Runtime:** `tokio` with full feature set (`tokio::time`, `tokio::task`, `tokio::signal`)

---

*Architecture analysis: 2026-04-09*
