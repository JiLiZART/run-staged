# Roadmap: fast-staged

## Overview

Build a fast Rust replacement for lint-staged, delivered in 9 phases. Starting from a skeleton codebase with a working Cargo workspace, we lay a typed error foundation, then build config parsing, git integration, command execution, CLI, resilience, a live TUI, and finally lint-staged config compatibility. Each phase delivers a runnable increment; the tool is fully functional by Phase 8, with compatibility polish in Phase 9.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Foundation** - Typed error enum, Cargo deps, CI fix, and correct exit code wiring (completed 2026-04-09)
- [ ] **Phase 2: Config Core** - Multi-format config discovery and parsing (TOML, JSON, package.json)
- [ ] **Phase 3: Advanced Config** - Named groups with per-group timeout, execution order, and error settings
- [ ] **Phase 4: Git Integration** - Staged file detection via gix plus patch backup and restore
- [ ] **Phase 5: Execution Core** - Glob matching, parallel and sequential command execution, timeouts
- [ ] **Phase 6: CLI Interface** - Full clap argument parsing, --version, and --help
- [ ] **Phase 7: Signal Handling & Resilience** - Ctrl+C with child process cleanup, terminal restore, user-friendly errors
- [ ] **Phase 8: TUI Layer** - Live ratatui render loop with per-command spinners, elapsed time, and quiet mode
- [ ] **Phase 9: Lint-staged Compatibility** - lint-staged JSON/YAML config discovery and JS function error handling

## Phase Details

### Phase 1: Foundation
**Goal**: The project compiles with all required dependencies, exits with the correct code, and every error in the codebase is typed
**Depends on**: Nothing (first phase)
**Requirements**: ERR-01
**Success Criteria** (what must be TRUE):
  1. `cargo build --release` succeeds with tokio, ratatui, gix, clap, serde, thiserror, globset declared in Cargo.toml
  2. Running the binary exits with code 0 on success and non-zero on any typed error
  3. AppError enum exists with variants covering config, git, execution, and IO failure categories
  4. CI workflow builds and tests the binary (no NAPI-RS artifacts)
**Plans:** 1/1 plans complete

Plans:
- [x] 01-01-PLAN.md — Crate rename, dependencies, AppError enum, async main, integration test, CI workflow

### Phase 2: Config Core
**Goal**: Users can store their linting config in any supported format and the tool discovers and parses it correctly
**Depends on**: Phase 1
**Requirements**: CFG-01, CFG-02, CFG-03, CFG-04, CFG-05
**Success Criteria** (what must be TRUE):
  1. Running the tool in a directory with `.fast-staged.toml` loads that config; falling back through the priority chain works when higher-priority files are absent
  2. A TOML config mapping glob patterns to commands is parsed and the patterns and commands are accessible at runtime
  3. A JSON config with the same structure is parsed identically to TOML
  4. `package.json` with a `fast-staged` key is read and its config is used
  5. An invalid config (bad syntax or unknown field) prints a clear error message and exits non-zero
**Plans:** 1/2 plans executed

Plans:
- [x] 02-01-PLAN.md — Config types, TOML/JSON parsing, validation, cascading defaults
- [x] 02-02-PLAN.md — Config discovery (priority walk to git root), package.json, lib.rs wiring

### Phase 3: Advanced Config
**Goal**: Users can control execution order, timeouts, and error behavior per command group in their config
**Depends on**: Phase 2
**Requirements**: CFG-06, CFG-07, CFG-08, CFG-09
**Success Criteria** (what must be TRUE):
  1. A config with named groups and `execution_order: sequential` causes groups to run in declared order
  2. A group with `timeout: 30s` stops its command after 30 seconds and reports timeout
  3. A group with `continue_on_error: true` does not abort remaining groups when its command fails
  4. A group with `per_file: true` invokes the command once per matched file; `per_file: false` (default) passes all files as arguments
**Plans:** 1 plan

Plans:
- [ ] 03-01-PLAN.md — Fix group ordering, add per-group override tests (CFG-06 through CFG-09)

### Phase 4: Git Integration
**Goal**: Users can trust that the tool sees exactly the right staged files and that their index is safe if a command fails
**Depends on**: Phase 1
**Requirements**: GIT-01, GIT-02, GIT-03, GIT-04, GIT-05, GIT-06, GIT-07
**Success Criteria** (what must be TRUE):
  1. Running the tool lists staged files read directly from the git index (no shell-out to git)
  2. A `.git/fast-staged.patch` file is created before any command runs
  3. If a command fails, the staged diff is restored from the patch and the working tree matches pre-run state
  4. After successful execution, the patch file is deleted
  5. `--unstaged` flag causes the tool to operate on modified-but-unstaged files instead
  6. Running outside a git repo prints a clear error and exits non-zero
  7. Running with no staged files (and without `--allow-empty`) exits with a clear message and non-zero code
**Plans**: TBD

### Phase 5: Execution Core
**Goal**: Users can run configured commands against matched staged files in parallel with correct exit code propagation
**Depends on**: Phase 3, Phase 4
**Requirements**: EXEC-01, EXEC-02, EXEC-03, EXEC-05, EXEC-06, EXEC-07
**Success Criteria** (what must be TRUE):
  1. Commands for different glob groups run in parallel by default; wall-clock time is less than the sum of individual durations
  2. Sequential execution mode runs commands one after another in declared order
  3. When any command exits non-zero, the tool exits non-zero — even if other commands succeed
  4. A command that exceeds its configured timeout is killed and the tool reports the timeout
  5. Staged files are matched to command groups using glob patterns with matchBase semantics (`*.ts` matches files in any subdirectory)
  6. When `--allow-empty` is set and no files match any pattern, the tool exits 0 silently
**Plans**: TBD

### Phase 6: CLI Interface
**Goal**: Users can invoke the tool with standard flags and get expected Unix CLI behavior
**Depends on**: Phase 1
**Requirements**: CLI-01, CLI-02, CLI-03
**Success Criteria** (what must be TRUE):
  1. `fast-staged --config path/to/config.toml` loads config from the specified path instead of auto-discovery
  2. `fast-staged --version` prints the semver version and exits 0
  3. `fast-staged --help` prints usage, available flags, and exits 0
  4. `--quiet`, `--unstaged`, and `--allow-empty` flags are accepted and forwarded to runtime behavior
**Plans**: TBD

### Phase 7: Signal Handling & Resilience
**Goal**: Users can safely interrupt the tool at any time without orphaned processes or a corrupted terminal
**Depends on**: Phase 5
**Requirements**: EXEC-04, ERR-02, ERR-03
**Success Criteria** (what must be TRUE):
  1. Pressing Ctrl+C kills all spawned child processes before the tool exits
  2. After Ctrl+C, no zombie or orphaned processes remain
  3. Any AppError printed to stderr includes a human-readable message and an actionable hint (e.g., "staged diff not found — run git add first")
  4. A panic inside the ratatui render loop restores the terminal to normal mode before the process exits
**Plans**: TBD
**UI hint**: yes

### Phase 8: TUI Layer
**Goal**: Users get a live terminal UI showing per-command progress, making it easy to see what is running and what failed
**Depends on**: Phase 5, Phase 7
**Requirements**: UI-01, UI-02, UI-03, UI-04, UI-05
**Success Criteria** (what must be TRUE):
  1. While commands run, the terminal shows a spinner and label for each command updating in real time
  2. Elapsed time for each running command is displayed and increments visibly
  3. Commands are grouped by their config group name in the TUI
  4. `--quiet` flag produces plain text output (pass/fail lines) with no TUI rendering
  5. When stdout is not a TTY (e.g., piped to a file or run in CI), the tool automatically falls back to quiet text output
**Plans**: TBD
**UI hint**: yes

### Phase 9: Lint-staged Compatibility
**Goal**: Users migrating from lint-staged can point the tool at their existing config and get a clear result
**Depends on**: Phase 2
**Requirements**: CFG-10, CFG-11
**Success Criteria** (what must be TRUE):
  1. A `.lintstagedrc.json` or `.lintstagedrc.yml` file is discovered and parsed as a valid config
  2. A lint-staged config using JS function values prints a clear error explaining JS functions are not supported and suggests converting to a static config
  3. All other lint-staged JSON/YAML config shapes (string commands, array commands) execute correctly
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation | 1/1 | Complete   | 2026-04-09 |
| 2. Config Core | 1/2 | In Progress|  |
| 3. Advanced Config | 0/1 | Not started | - |
| 4. Git Integration | 0/TBD | Not started | - |
| 5. Execution Core | 0/TBD | Not started | - |
| 6. CLI Interface | 0/TBD | Not started | - |
| 7. Signal Handling & Resilience | 0/TBD | Not started | - |
| 8. TUI Layer | 0/TBD | Not started | - |
| 9. Lint-staged Compatibility | 0/TBD | Not started | - |
