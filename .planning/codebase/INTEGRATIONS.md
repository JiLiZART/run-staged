# External Integrations

**Analysis Date:** 2026-04-09

## APIs & External Services

None. This is a local CLI tool with no network calls or external API dependencies.

## Data Storage

**Databases:**
- None. No database is used.

**File Storage:**
- Local filesystem only
- Reads config files from current working directory (`.fast-staged.toml`, `fast-staged.toml`, `.fast-staged.json`, `fast-staged.json`, `package.json`)
- Planned: writes backup patch to `.git/fast-staged.patch` before running tasks, deletes it after (documented in `TODO.md`)

**Caching:**
- None

## Authentication & Identity

**Auth Provider:**
- Not applicable — no authentication required

## Git Integration

**Provider:** Local git repository via `gix` crate (planned)
- Lists staged files (index vs HEAD)
- Reads git directory path (`.git/`)
- Planned: stash operations for pre-task backup/rollback
- Planned: diff generation (`git diff --cached` equivalent) for patch backup

**Hook Integration:**
- Installed as a git pre-commit hook (replaces husky + lint-staged pipeline)
- Current dev setup uses Husky (`/.husky/pre-commit`) invoking `yarn lint-staged` — this is the existing workflow the tool aims to replace
- Planned `--install` CLI command to write the git hook itself without requiring husky

## Monitoring & Observability

**Error Tracking:**
- None

**Logs:**
- Terminal output only via `ratatui` TUI (planned)
- Quiet mode (`--quiet` flag, planned): plain text summary output without TUI

## CI/CD & Deployment

**Hosting:**
- Distributed as a compiled binary (no hosting infrastructure)

**CI Pipeline:**
- None detected

## Environment Configuration

**Required env vars:**
- None required at runtime

**Secrets location:**
- Not applicable

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- None

## Child Process Execution

**Mechanism:** Spawns user-defined shell commands as child processes (planned via `tokio::process::Command`)
- Commands are read from config patterns (e.g., `"*.rs" = ["cargo fmt", "cargo clippy"]`)
- Planned execution modes: per-file or batch (pass all matching files to one invocation)
- Planned timeout enforcement per command via `tokio::time::timeout()`
- Planned graceful cancellation on `SIGINT`/`SIGTERM` via `tokio::signal`

---

*Integration audit: 2026-04-09*
