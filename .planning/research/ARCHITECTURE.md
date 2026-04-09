# Architecture Patterns

**Domain:** git pre-commit staged file runner (Rust CLI with TUI + async execution)
**Researched:** 2026-04-09
**Confidence:** HIGH — based on detailed project docs (TODO.md, ROADMAP.md, codebase mapping) and known patterns for ratatui/tokio CLI tools

## Recommended Architecture

A **layered pipeline** with a shared state model driving a reactive TUI render loop. Execution and rendering run concurrently: a tokio task pool drives command execution and emits `Message` events, while the ratatui render loop reads `StateModel` on each frame tick without blocking the executor.

```
CLI Entry (main.rs)
       │
       ▼
  run() [lib.rs]                ← orchestrates full pipeline
  ┌────────────────────────────────────────────────────────┐
  │  1. Config Layer    config.rs                          │
  │     find_config_file() → load() → Config               │
  │                                                        │
  │  2. Git Layer       git.rs                             │
  │     GitContext::get_changed_files() → Vec<PathBuf>     │
  │     create_staged_diff() → .git/fast-staged.patch      │
  │                                                        │
  │  3. Matching Layer  commands.rs                        │
  │     Commands::match_from_files() → Vec<FileCommand>    │
  │                                                        │
  │  4. Execution Layer commands.rs + state.rs             │
  │     Commands::execute() ──→ tokio JoinSet              │
  │         └─ per-task: TaskState::run_single_command()   │
  │              └─ emits Message → StateModel (Arc<Mutex>)│
  │                                                        │
  │  5. TUI Layer       renderer.rs                        │
  │     Renderer::view(model, frame)                       │
  │     OR quiet text summary                              │
  └────────────────────────────────────────────────────────┘
       │
       ▼
  Cleanup: delete .git/fast-staged.patch
           revert from stash on error (if --revert)
```

### Component Boundaries

| Component | Module | Responsibility | Communicates With |
|-----------|--------|----------------|-------------------|
| CLI Entry | `main.rs` | Parse args via clap, call `run()`, top-level error formatting | `lib.rs` (run) |
| Orchestrator | `lib.rs` | Wire pipeline together, hold cancellation flag, cleanup | All layers |
| Config | `config.rs` | Discover and parse config files (TOML/JSON/package.json) | Orchestrator (receives RunConfig), returns Config |
| Git | `git.rs` | List staged/unstaged files via gix, create/delete patch backup | Orchestrator |
| Matching | `commands.rs` | Glob-match staged files to Group patterns → Vec<FileCommand> | Config (reads Groups), Git (reads file list) |
| Execution | `commands.rs` | Spawn tokio tasks, manage JoinSet, enforce timeout, handle cancellation | State (writes), Matching (reads FileCommand) |
| State | `state.rs` | StateModel + Message enum + update() fn (Elm-style) | Execution (writes via Arc<Mutex>), TUI (reads) |
| TUI | `renderer.rs` | ratatui frame render, event loop, terminal init/restore | State (reads StateModel) |
| Error | `error.rs` | AppError enum with thiserror | All layers (return Result<T, AppError>) |

### Data Flow

**Happy path — left to right through pipeline:**

```
clap args (RunConfig)
    → Config::load(RunConfig) → Config { groups: Vec<Group>, timeout, ... }
    → GitContext::get_changed_files() → Vec<PathBuf>
       side-effect: writes .git/fast-staged.patch
    → Commands::match_from_files(files, config) → Vec<FileCommand>
    → Commands::execute(file_commands, state_model, cancellation_flag)
       spawns tokio tasks (JoinSet), one per FileCommand
       each task: TaskState::run_single_command()
         → tokio::process::Command → stdout/stderr captured
         → updates Arc<Mutex<CommandStatus>> in StateModel
         → emits Message to channel
    → StateModel::update(msg) on each Message (Elm pattern)
    → Renderer::view(&StateModel, &mut Frame) on each ratatui tick
    → terminal restored; cleanup .git/fast-staged.patch
    → exit 0 (all success) or exit 1 (any failure)
```

**Cancellation flow:**

```
Ctrl+C → tokio::signal::ctrl_c() task
    → sets Arc<AtomicBool> cancellation_flag = true
    → execute() polls flag: JoinSet::abort_all()
    → all TaskStates set to CommandStatus::Cancelled
    → TUI render loop sees flag, breaks, restores terminal
    → cleanup runs (delete patch, optionally revert stash)
```

**Quiet mode (--quiet):**

```
Same pipeline except:
    → no ratatui init
    → execute() runs to completion
    → text summary printed to stdout (N success, M failed)
```

**Error flow:**

```
Any layer returns Err(AppError::X)
    → propagated via ? to run()
    → run() triggers cleanup (patch delete, stash revert if --revert)
    → main() formats error with color, prints to stderr
    → exit 1
```

## Patterns to Follow

### Pattern 1: Elm-style State Update (Message / StateModel)

**What:** Immutable-ish state transitions via `update(model, msg) -> Model`. Messages are the only mutation path.
**When:** Async tasks and UI thread both need to observe state without races.
**Why:** Decouples execution from rendering; TUI reads stable snapshots.

```rust
enum Message {
    TaskStarted { group: String, pattern: String, file: PathBuf },
    TaskDone { id: TaskId, elapsed: Duration },
    TaskFailed { id: TaskId, stderr: String, elapsed: Duration },
    TaskTimeout { id: TaskId },
    TaskCancelled { id: TaskId },
    TotalFilesKnown(usize),
}

fn update(model: &mut StateModel, msg: Message) {
    match msg {
        Message::TaskDone { id, elapsed } => { /* mutate model */ }
        // ...
    }
}
```

State is behind `Arc<Mutex<StateModel>>` — executor holds write lock briefly per message; TUI holds read lock per frame.

### Pattern 2: JoinSet with Per-Group Execution Strategy

**What:** Groups run in parallel by default. Within a group, commands run parallel or sequential per `ExecutionOrder`.
**When:** Config defines `execution_order: ExecutionOrder` per group.

```rust
// Groups run concurrently via outer JoinSet
let mut group_set: JoinSet<()> = JoinSet::new();
for group in matched_groups {
    group_set.spawn(run_group(group, cancellation_flag.clone(), state.clone()));
}

// Within a group: parallel or sequential
async fn run_group(group: MatchedGroup, cancel: Arc<AtomicBool>, state: Arc<Mutex<StateModel>>) {
    match group.execution_order {
        ExecutionOrder::Parallel => {
            let mut task_set = JoinSet::new();
            for cmd in group.commands { task_set.spawn(run_cmd(cmd)); }
            task_set.join_all().await;
        }
        ExecutionOrder::Sequential => {
            for cmd in group.commands { run_cmd(cmd).await; }
        }
    }
}
```

### Pattern 3: ratatui Event Loop with Tick + Channel

**What:** ratatui render loop driven by a tick timer and an mpsc channel for messages from executors.
**When:** TUI needs live updates without polling.

```rust
// Typical ratatui async pattern
loop {
    terminal.draw(|f| renderer.view(&model, f))?;

    // Non-blocking: drain all pending messages before next tick
    while let Ok(msg) = rx.try_recv() {
        update(&mut model, msg);
    }

    if all_done(&model) || cancel_flag.load(Ordering::Relaxed) { break; }
    tokio::time::sleep(TICK_RATE).await;
}
restore_terminal();
```

### Pattern 4: Shared Arc<Mutex<CommandStatus>> for Per-Task Live Status

**What:** Each `TaskState` holds `Arc<Mutex<CommandStatus>>`. TUI reads it on each frame; executor writes on transitions.
**When:** Per-command spinner/status needs live updates.
**Note:** Keep the Mutex lock duration minimal (clone out the status, don't hold during render).

### Pattern 5: Config Discovery via Ranked Path Probe

**What:** Walk through ranked candidates; return first that exists.
**When:** Config loading startup.

```rust
fn find_config_file(override_path: Option<&Path>) -> Result<ConfigSource, AppError> {
    if let Some(p) = override_path { return probe(p); }
    let candidates = [
        (".fast-staged.toml", ConfigSource::TomlFile),
        ("fast-staged.toml",  ConfigSource::TomlFile),
        (".fast-staged.json", ConfigSource::JsonFile),
        ("fast-staged.json",  ConfigSource::JsonFile),
        ("package.json",      ConfigSource::PackageJson),
    ];
    for (name, kind) in candidates {
        if Path::new(name).exists() { return Ok(kind(name.into())); }
    }
    Err(AppError::ConfigNotFound { checked_paths: ... })
}
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: Holding Mutex Across Await Points

**What:** Locking `Arc<Mutex<StateModel>>` and then calling `.await` inside the guard.
**Why bad:** Poisons the lock, blocks TUI render, potential deadlock in tokio.
**Instead:** Clone the data out of the lock, drop the guard, then await.

```rust
// BAD
let guard = state.lock().await;
guard.tasks.get_mut(&id).unwrap().status = run_command().await;

// GOOD
let result = run_command().await;
let mut guard = state.lock().await;
guard.tasks.get_mut(&id).unwrap().status = result;
```

### Anti-Pattern 2: Spawning One tokio Task Per File

**What:** For a group with 200 matched files, spawning 200 independent tokio tasks.
**Why bad:** Excessive overhead; terminal output overwhelms TUI; no back-pressure.
**Instead:** Use a bounded semaphore (`tokio::sync::Semaphore`) to cap concurrency, or process in chunks per `ExecutionBehavior::Batch`.

### Anti-Pattern 3: Shelling Out to git CLI

**What:** `std::process::Command::new("git").args(["diff", "--cached"])`.
**Why bad:** Requires git in PATH, slower process spawning, no type-safe API.
**Instead:** Use `gix` crate for all git operations (staged file listing, diff, stash).

### Anti-Pattern 4: Blocking the tokio Runtime in TUI Render

**What:** Calling blocking I/O or `std::thread::sleep` inside the ratatui tick loop on the main tokio thread.
**Why bad:** Starves other spawned tasks; drops TUI responsiveness.
**Instead:** Use `tokio::time::sleep` for ticks; move any blocking I/O to `tokio::task::spawn_blocking`.

### Anti-Pattern 5: Panic-on-Error in Non-main Functions

**What:** Using `unwrap()` or `expect()` in domain functions (config, git, matching).
**Why bad:** Produces unreadable panics; defeats `AppError` user-friendly messages.
**Instead:** Return `Result<T, AppError>` and propagate with `?` everywhere; only `unwrap` in tests.

### Anti-Pattern 6: Terminal Not Restored on Panic

**What:** ratatui switches terminal to raw mode; if process panics, terminal stays broken.
**Why bad:** User's shell becomes unusable.
**Instead:** Register a panic hook that calls `restore_terminal()` before the process exits.

```rust
let original_hook = std::panic::take_hook();
std::panic::set_hook(Box::new(move |info| {
    restore_terminal().ok();
    original_hook(info);
}));
```

## Component Build Order (Dependencies)

The graph of what must exist before what:

```
1. error.rs        (AppError)
       ↓
2. config.rs       (Config, Group, RunConfig, ConfigSource)
       ↓
3. git.rs          (GitContext — needs Config for unstaged flag)
       ↓
4. state.rs        (StateModel, TaskState, Message, CommandStatus)
       ↓
5. commands.rs     (Matching + Execution — needs Config, state, git output)
       ↓
6. renderer.rs     (TUI — needs StateModel)
       ↓
7. lib.rs          (run() — wires all above)
       ↓
8. main.rs         (CLI args, calls run(), error formatting)
```

**Rationale for this order:**
- `error.rs` first: all other modules return `AppError`; no circular deps.
- `config.rs` before `git.rs`: git layer accepts config flags (unstaged, allow_empty).
- `state.rs` before `commands.rs`: executor writes into StateModel; state must be defined first.
- `commands.rs` combines matching + execution; matching is pure and fast, must be done before spawning tasks.
- `renderer.rs` reads StateModel; decoupled from execution — can be developed and tested with mock state.
- `lib.rs` last (except main.rs): orchestrates all others.

**Parallelizable work:**
- `config.rs` and `error.rs` can be built simultaneously (no cross-dep).
- `renderer.rs` and `commands.rs` can be scaffolded in parallel; both depend on `state.rs`.

## Scalability Considerations

| Concern | At 10 staged files | At 500 staged files | At 5000 staged files |
|---------|-------------------|---------------------|----------------------|
| Task spawning | 1 task per file fine | Consider bounded semaphore | Must use Batch mode or chunking |
| TUI rendering | Scrollable list | Paginate or summarize by group | Show group-level aggregation only |
| Memory | Trivial | Store stdout/stderr lazily | Stream output, avoid buffering all |
| gix perf | Instant | Fast (native, no process spawn) | Fast (gix reads index directly) |

The `ExecutionBehavior::Batch` and chunking features (ROADMAP item: "files chunking support") directly address the 500+ file case.

## Distribution Architecture

The dual distribution (cargo + npm) does not affect runtime architecture but has build implications:

```
Single compiled binary
    ├── cargo install fast-staged      (Rust ecosystem)
    └── npm install fast-staged        (JS ecosystem, SWC pattern)
         └── @fast-staged/<platform>   (platform-specific optional dep)
              └── fast-staged[.exe]   (pre-built binary)
```

The binary is identical in both paths — no NAPI-RS native module, no Node.js runtime dependency. CI must build for all target triples and publish to npm registries.

## Sources

- Project `TODO.md` and `ROADMAP.md` (HIGH confidence — authoritative project intent)
- Project `.planning/codebase/ARCHITECTURE.md` (HIGH confidence — derived from existing code + docs)
- Tokio + ratatui standard async TUI event loop pattern (HIGH confidence — widely established in Rust ecosystem)
- Elm architecture (Message/Model/update) adapted to Rust TUI (MEDIUM confidence — standard ratatui community pattern, verified by multiple ratatui examples in ratatui-org/ratatui GitHub)
