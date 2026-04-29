# Requirements: fast-staged

**Defined:** 2026-04-09
**Core Value:** Run linting/formatting commands on staged files faster than lint-staged, with zero Node.js runtime dependency.

## v1 Requirements

### Config Discovery & Parsing

- [x] **CFG-01**: Tool discovers config files in priority order: .fast-staged.toml > .fast-staged.json > fast-staged.toml > fast-staged.json > package.json[fast-staged]
- [x] **CFG-02**: Tool parses TOML config with glob patterns mapped to commands
- [x] **CFG-03**: Tool parses JSON config with glob patterns mapped to commands
- [x] **CFG-04**: Tool reads fast-staged config section from package.json
- [x] **CFG-05**: Tool validates config and shows clear error messages for invalid configs
- [x] **CFG-06**: Tool supports named groups with per-group execution_order setting
- [x] **CFG-07**: Tool supports per-group timeout override
- [x] **CFG-08**: Tool supports per-group continue_on_error setting
- [x] **CFG-09**: Tool supports per-group per-file vs batch execution mode
- [ ] **CFG-10**: Tool discovers and parses lint-staged JSON/YAML configs (no JS functions)
- [ ] **CFG-11**: Tool shows clear error when encountering JS function configs

### Git Integration

- [x] **GIT-01**: Tool detects staged files from git index via gix
- [x] **GIT-02**: Tool saves staged diff to .git/fast-staged.patch before command execution
- [x] **GIT-03**: Tool restores staged diff from patch on command failure
- [x] **GIT-04**: Tool cleans up patch file after successful execution
- [x] **GIT-05**: Tool supports --unstaged flag to run on modified (not staged) files
- [x] **GIT-06**: Tool exits with clear error when not in a git repository
- [x] **GIT-07**: Tool exits with clear error when no staged files found (unless --allow-empty)

### Command Execution

- [x] **EXEC-01**: Tool runs matched commands in parallel by default
- [x] **EXEC-02**: Tool supports sequential execution mode via config
- [x] **EXEC-03**: Tool exits with non-zero code when any command fails
- [ ] **EXEC-04**: Tool handles Ctrl+C signal and kills all child processes
- [x] **EXEC-05**: Tool supports per-command timeout with configurable duration
- [x] **EXEC-06**: Tool matches staged files to commands via glob patterns (with matchBase semantics)
- [x] **EXEC-07**: Tool supports --allow-empty flag to skip execution when no files match

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

- [x] **ERR-01**: Tool uses typed AppError enum with thiserror
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
| CFG-01 | Phase 2 | Complete |
| CFG-02 | Phase 2 | Complete |
| CFG-03 | Phase 2 | Complete |
| CFG-04 | Phase 2 | Complete |
| CFG-05 | Phase 2 | Complete |
| CFG-06 | Phase 3 | Complete |
| CFG-07 | Phase 3 | Complete |
| CFG-08 | Phase 3 | Complete |
| CFG-09 | Phase 3 | Complete |
| CFG-10 | Phase 9 | Pending |
| CFG-11 | Phase 9 | Pending |
| GIT-01 | Phase 4 | Complete |
| GIT-02 | Phase 4 | Complete |
| GIT-03 | Phase 4 | Complete |
| GIT-04 | Phase 4 | Complete |
| GIT-05 | Phase 4 | Complete |
| GIT-06 | Phase 4 | Complete |
| GIT-07 | Phase 4 | Complete |
| EXEC-01 | Phase 5 | Complete |
| EXEC-02 | Phase 5 | Complete |
| EXEC-03 | Phase 5 | Complete |
| EXEC-04 | Phase 7 | Pending |
| EXEC-05 | Phase 5 | Complete |
| EXEC-06 | Phase 5 | Complete |
| EXEC-07 | Phase 5 | Complete |
| CLI-01 | Phase 6 | Pending |
| CLI-02 | Phase 6 | Pending |
| CLI-03 | Phase 6 | Pending |
| UI-01 | Phase 8 | Pending |
| UI-02 | Phase 8 | Pending |
| UI-03 | Phase 8 | Pending |
| UI-04 | Phase 8 | Pending |
| UI-05 | Phase 8 | Pending |
| ERR-01 | Phase 1 | Complete |
| ERR-02 | Phase 7 | Pending |
| ERR-03 | Phase 7 | Pending |

**Coverage:**
- v1 requirements: 36 total
- Mapped to phases: 36
- Unmapped: 0

---
*Requirements defined: 2026-04-09*
*Last updated: 2026-04-09 after roadmap creation — all 36 requirements mapped to phases 1-9*
