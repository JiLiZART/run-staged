# Architecture

**Analysis Date:** 2026-04-09

## Pattern Overview

**Overall:** CLI tool with layered pipeline architecture (planned)

**Key Characteristics:**
- Rust Cargo workspace with a single `fast-staged` crate currently (`crates/fast-staged`)
- The binary is a stub (`main.rs` prints "Hello, world!") — implementation is not yet written
- Intended design is documented in `TODO.md` and `ROADMAP.md`: a pipeline of config loading → git file discovery → command matching → parallel/sequential execution → TUI rendering
- CI pipeline targets NAPI-RS native Node.js bindings (`.node` / `.wasm` artifacts), suggesting the final distribution will be a Node.js-consumable native module in addition to (or instead of) a standalone binary

## Intended Layers (from TODO.md / ROADMAP.md)

**Config Layer:**
- Purpose: Discover and parse configuration from multiple file formats
- Planned functions: `Config::find_file()`, `Config::load()`, `Config::load_from_package_json()`
- Config sources searched in priority order: `.fast-staged.toml`, `fast-staged.toml`, `.fast-staged.json`, `fast-staged.json`, `package.json["fast-staged"]`
- Planned enum `RunConfig { config: Option<PathBuf>, quiet: bool, unstaged: bool, stash: bool, allow_empty: bool, fail_on_changes: bool, revert: bool }`
- Planned enum: `ConfigSource { TomlFile(PathBuf), JsonFile(PathBuf), PackageJson(PathBuf) }`
- Planned struct: `Config` with `groups: Vec<Group>`, global `timeout: Option<Duration>`, `execution_order: Option<ExecutionOrder>`, `continue_on_error: bool` 
- Planned struct: `Group { name, patterns: HashMap<FilePattern, CommandList>, execution_order: ExecutionOrder, timeout: Option<Duration>, continue_on_error: bool, execution_behavior: ExecutionBehavior, path_format: PathFormat, run_on: ExecutionBehavior }`

**Git Layer:**
- Purpose: Obtain the list of staged (or optionally unstaged) files from the repository
- Planned function: `GitContext::get_changed_files(options)`
- Uses `gix` crate for git operations
- Returns `Err(AppError::NotGitRepository)` if not in a git repo, `Err(AppError::NoStagedFiles)` if nothing staged
- Planned backup support: writes `.git/fast-staged.patch` before execution, deletes after

**Matching Layer:**
- Purpose: Map staged files to commands defined in config groups
- Planned function: `Commands::match_from_files()` → returns list of `FileCommand` structs
- Iterates groups, matches files against glob patterns, collects unique file count
- Returns `Err(AppError::NoFilesMatched)` if no files match any group

**Execution Layer:**
- Purpose: Run matched commands, respecting group execution order, run on per-file or batch, and timeout
- Planned function: `Commands::execute(file_commands, options)`
- Supports `ExecutionOrder::Parallel` (default, via `tokio::task`) and `ExecutionOrder::Sequential`
- Supports `ExecutionBehavior::PerFile` (one process per file) and `ExecutionBehavior::Batch` (all files as args to one invocation)
- Timeout via `tokio::time::timeout()`
- Graceful cancellation via `Arc<AtomicBool>` cancellation flag

**State / Task Layer:**
- Purpose: Track live status of each running command use `StateModel { groups: HashMap<GroupName, HashMap<PatternName, TaskState>>, total_count, execution_time, errors: String[] }` struct
- Handle updates to `StateModel` via `Message` enum, use `fn update(model: &StateModel, msg: Message) -> Model`
- Planned struct: `TaskState { filenames, command, status: Arc<Mutex<CommandStatus>>, stash_id: Option<String> }`
- Planned method: `Commands::run_command(task_state)`
- Planned enum: `CommandStatus { Running, Done, Failed, Timeout, Cancelled }` with a `StatusDisplay` trait for colored TUI rendering

**UI Layer:**
- Purpose: Render a TUI using `ratatui` showing live command progress
- Planned function: `Renderer::view(model: &mut StateModel, f: &mut Frame)`
- Displays: per-command status with elapsed time, total affected file count, total execution time, grouped output by config group
- `--quiet` flag bypasses TUI entirely: runs tasks then prints a text summary
- If any errors, prints them in bordered colored box and exits

**Error Layer:**
- Purpose: Centralized typed errors
- Planned enum `AppError` (using `thiserror`): `ConfigNotFound`, `ConfigInvalid`, `NotGitRepository`, `NoStagedFiles`, `NoFilesMatched`, `CommandNotFound`, `IoError`, `GitError`, `Cancelled`

## Data Flow

**Happy Path (planned):**

1. `main()` parses CLI args (via `clap`): `--config`, `--unstaged`, `--allow-empty`, `--quiet` into `RunConfig`
2. `Config::load(RunConfig)` → finds config file, parses into `Config` with `Vec<Group>` and other fields
3. `context = GitContext::new(config); files = context.get_changed_files()` → calls `gix` to get staged file list; writes `.git/fast-staged.patch` backup via `context.create_staged_diff()`, restores it on failure
4. `commands = Commands::from_files(files)` → maps files to commands per group, returns `Vec<FileCommand>`
5. `current_msg = commands.execute(model, config)` (called on each render) → creates global worker pool, spawns worker per each group and executes commands in group in parallel or sequentially, each execution produces `Message` enum value that collected via worker pool
6. `Renderer::view(model, frame)` (or quiet text output) → renders live TUI via `ratatui`, reads `StateModel`
7. Cleanup: deletes `.git/fast-staged.patch`; if errors exist and revert configured, restores from stash

**Cancellation Flow:**

1. `tokio::signal::ctrl_c()` watcher sets `Arc<AtomicBool>` cancellation flag
2. `commands.execute(model, config)` updates `model` with relevant data
3. `Renderer::view(model)` read from model, breaks render loop, restores terminal

**State Management:**
- `StateModel` holds all necessary state for UI
- `Message` enum is used to communicate between async executor and UI thread
- UI reads `StateModel` on each frame without blocking execution

## Key Abstractions

**Config:**
- Purpose: Represents parsed user configuration including groups and patterns
- Planned file: `crates/fast-staged/src/config.rs` (not yet created)
- Pattern: Deserialized from TOML/JSON via `serde`

**Group:**
- Purpose: A named set of file patterns with associated commands and execution settings
- Planned file: `crates/fast-staged/src/config.rs`
- Pattern: Contains `HashMap<FilePattern, CommandList>` plus execution knobs

**Commands:**
- Purpose: Live tracking of a single command execution (one per file+command pair)
- Planned file: `crates/fast-staged/src/commands.rs` (not yet created)
- Pattern: Shared state via `Arc<Mutex<T>>` between async executor and UI thread

**AppError:**
- Purpose: Typed, user-readable error enum covering all failure modes
- Planned file: `crates/cli/src/error.rs` (not yet created)
- Pattern: `thiserror::Error` derive with contextual messages

## Entry Points

**Binary Entry Point:**
- Location: `crates/fast-staged/src/main.rs`
- Current state: Stub — only prints "Hello, world!"
- Planned responsibilities: Parse CLI args, call `run()`, handle top-level errors with colored output

**Library Entry Point (planned):**
- Location: `crates/fast-staged/src/lib.rs` (not yet created)
- Triggers: Called from `main()`
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
