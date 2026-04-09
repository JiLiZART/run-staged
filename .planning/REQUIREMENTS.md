# Requirements: fast-staged

**Defined:** 2026-04-09
**Core Value:** Run linting/formatting commands on staged files faster than lint-staged, with zero Node.js runtime dependency.

## v1 Requirements

### Config Discovery & Parsing

- [ ] **CFG-01**: Tool discovers config files in priority order: .fast-staged.toml > .fast-staged.json > fast-staged.toml > fast-staged.json > package.json[fast-staged]
- [ ] **CFG-02**: Tool parses TOML config with glob patterns mapped to commands
- [ ] **CFG-03**: Tool parses JSON config with glob patterns mapped to commands
- [ ] **CFG-04**: Tool reads fast-staged config section from package.json
- [ ] **CFG-05**: Tool validates config and shows clear error messages for invalid configs
- [ ] **CFG-06**: Tool supports named groups with per-group execution_order setting
- [ ] **CFG-07**: Tool supports per-group timeout override
- [ ] **CFG-08**: Tool supports per-group continue_on_error setting
- [ ] **CFG-09**: Tool supports per-group per-file vs batch execution mode
- [ ] **CFG-10**: Tool discovers and parses lint-staged JSON/YAML configs (no JS functions)
- [ ] **CFG-11**: Tool shows clear error when encountering JS function configs

### Git Integration

- [ ] **GIT-01**: Tool detects staged files from git index via gix
- [ ] **GIT-02**: Tool saves staged diff to .git/fast-staged.patch before command execution
- [ ] **GIT-03**: Tool restores staged diff from patch on command failure
- [ ] **GIT-04**: Tool cleans up patch file after successful execution
- [ ] **GIT-05**: Tool supports --unstaged flag to run on modified (not staged) files
- [ ] **GIT-06**: Tool exits with clear error when not in a git repository
- [ ] **GIT-07**: Tool exits with clear error when no staged files found (unless --allow-empty)

### Command Execution

- [ ] **EXEC-01**: Tool runs matched commands in parallel by default
- [ ] **EXEC-02**: Tool supports sequential execution mode via config
- [ ] **EXEC-03**: Tool exits with non-zero code when any command fails
- [ ] **EXEC-04**: Tool handles Ctrl+C signal and kills all child processes
- [ ] **EXEC-05**: Tool supports per-command timeout with configurable duration
- [ ] **EXEC-06**: Tool matches staged files to commands via glob patterns (with matchBase semantics)
- [ ] **EXEC-07**: Tool supports --allow-empty flag to skip execution when no files match

### CLI Interface

- [ ] **CLI-01**: Tool parses CLI args via clap: --config, --quiet, --unstaged, --allow-empty
- [ ] **CLI-02**: Tool shows version with --version
- [ ] **CLI-03**: Tool shows help with --help

### User Interface

- [ ] **UI-01**: Tool renders live ratatui TUI with per-command progress spinners
- [ ] **UI-02**: Tool shows elapsed time per command in TUI
- [ ] **UI-03**: Tool groups output by config group in TUI
- [ ] **UI-04**: Tool supports --quiet mode with simple text output (no TUI)
- [ ] **UI-05**: Tool auto-detects non-TTY and falls back to quiet mode

### Error Handling

- [ ] **ERR-01**: Tool uses typed AppError enum with thiserror
- [ ] **ERR-02**: Tool shows user-friendly error messages with actionable hints
- [ ] **ERR-03**: Tool restores terminal on panic (ratatui panic hook)

## v2 Requirements

### Distribution

- **DIST-01**: npm binary packages for all major platforms (SWC pattern)
- **DIST-02**: Cross-platform CI matrix build workflow
- **DIST-03**: Automated npm publish on release

### Advanced Features

- **ADV-01**: Git stash-based full save/restore (beyond patch backup)
- **ADV-02**: Per-file vs batch execution with argv chunking for large file sets
- **ADV-03**: Path format configuration (relative, absolute, from-root)

## Out of Scope

| Feature | Reason |
|---------|--------|
| JS function configs from lint-staged | Too complex, marginal value — show clear error instead |
| Real-time file watching / daemon mode | Not needed for pre-commit hook use case |
| NAPI-RS native Node.js bindings | Distributing as standalone binaries via npm instead |
| Plugin system | Keep it simple, one tool one job |
| OAuth/authentication | Local git tool, no network auth needed |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CFG-01 | TBD | Pending |
| CFG-02 | TBD | Pending |
| CFG-03 | TBD | Pending |
| CFG-04 | TBD | Pending |
| CFG-05 | TBD | Pending |
| CFG-06 | TBD | Pending |
| CFG-07 | TBD | Pending |
| CFG-08 | TBD | Pending |
| CFG-09 | TBD | Pending |
| CFG-10 | TBD | Pending |
| CFG-11 | TBD | Pending |
| GIT-01 | TBD | Pending |
| GIT-02 | TBD | Pending |
| GIT-03 | TBD | Pending |
| GIT-04 | TBD | Pending |
| GIT-05 | TBD | Pending |
| GIT-06 | TBD | Pending |
| GIT-07 | TBD | Pending |
| EXEC-01 | TBD | Pending |
| EXEC-02 | TBD | Pending |
| EXEC-03 | TBD | Pending |
| EXEC-04 | TBD | Pending |
| EXEC-05 | TBD | Pending |
| EXEC-06 | TBD | Pending |
| EXEC-07 | TBD | Pending |
| CLI-01 | TBD | Pending |
| CLI-02 | TBD | Pending |
| CLI-03 | TBD | Pending |
| UI-01 | TBD | Pending |
| UI-02 | TBD | Pending |
| UI-03 | TBD | Pending |
| UI-04 | TBD | Pending |
| UI-05 | TBD | Pending |
| ERR-01 | TBD | Pending |
| ERR-02 | TBD | Pending |
| ERR-03 | TBD | Pending |

**Coverage:**
- v1 requirements: 36 total
- Mapped to phases: 0
- Unmapped: 36

---
*Requirements defined: 2026-04-09*
*Last updated: 2026-04-09 after initial definition*
