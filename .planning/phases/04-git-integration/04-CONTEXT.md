# Phase 4: Git Integration - Context

**Gathered:** 2026-04-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Staged file detection via gix plus patch backup and restore. The tool opens the git repository, reads staged (or unstaged) files from the index, creates a `.git/fast-staged.patch` backup before execution, restores on failure, and cleans up on success. No command execution logic (Phase 5), no CLI flag parsing (Phase 6) — this phase delivers the git module that downstream phases consume.

</domain>

<decisions>
## Implementation Decisions

### gix API Approach
- **D-01:** Use `gix::discover` from CWD to open the repository — mirrors config discovery's upward walk behavior. Return `AppError::NotGitRepository` on failure.
- **D-02:** Read staged files by accessing the git index via `repo.index()` or `repo.open_index()` and filtering for entries with non-zero stage or by comparing index entries against HEAD tree.
- **D-03:** Enable minimal gix features — only what's needed for index reading and diff generation. Keep binary size small per project constraints (LTO, opt-level "z").
- **D-04:** Keep `.git/` directory scan in config/discover.rs unchanged. The gix crate is used only in the new git module. Phase 2 decision explicitly chose this separation.

### Patch Backup Strategy
- **D-05:** Use gix diff API to generate the staged diff patch (index vs HEAD). Project constraint mandates no git CLI shell-out (`gix crate, no shelling out to git CLI`).
- **D-06:** Patch file stored at `.git/fast-staged.patch` (locked by success criteria SC-2).
- **D-07:** If a patch file already exists from a prior interrupted run, overwrite it — the new run's state takes priority. A leftover patch from a crash is stale.

### Restore Behavior
- **D-08:** `lib.rs` orchestrates backup/restore — the git module provides `backup()` and `restore()` functions, and the caller decides when to invoke them. Keeps the git module focused on git operations only.
- **D-09:** Restore means restoring the staged diff to the index. Working tree changes from commands (e.g., formatter writes) are expected and kept. This matches lint-staged behavior: the staged diff is protected, the working tree reflects tool output.

### Module Structure
- **D-10:** Git module lives at `src/git.rs` (single file initially, split to `src/git/` later if needed). Follows the `error.rs` pattern.
- **D-11:** Public API: `GitContext` struct with methods — `open()`, `staged_files()`, `unstaged_files()`, `backup()`, `restore()`, `cleanup()`. Encapsulates the gix repo handle.
- **D-12:** Integration in `lib.rs::run()`: after config loading, before command execution — open repo, get staged files, create backup, then hand off to execution (Phase 5).

### Claude's Discretion
- Exact gix feature flags to enable in Cargo.toml
- gix API details for index reading (the API varies by version — researcher should verify current crate docs)
- Error message wording for git-specific failures
- Test strategy: real git repos in temp dirs vs mocked index data
- Whether `GitContext` holds `gix::Repository` or a lighter handle

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Specs
- `.planning/ROADMAP.md` — Phase 4 success criteria (GIT-01 through GIT-07)
- `.planning/REQUIREMENTS.md` — GIT-01 to GIT-07 requirement definitions
- `.planning/PROJECT.md` — Constraints: gix crate (no git CLI), Rust edition 2024, tokio async

### Prior Phase Context
- `.planning/phases/01-foundation/01-CONTEXT.md` — AppError design (NotGitRepository, NoStagedFiles, Git(String) variants), lib.rs + main.rs split
- `.planning/phases/02-config-core/02-CONTEXT.md` — Config discovery uses .git/ scan (D-10), not gix — keep that separation

### Existing Code
- `crates/fast-staged/src/error.rs` — AppError with NotGitRepository, NoStagedFiles, Git(String) variants ready to use
- `crates/fast-staged/src/lib.rs` — `pub async fn run()` entry point, currently loads config only
- `crates/fast-staged/src/config/discover.rs` — Git root detection via `.git/` exists check (lines 44-45)
- `crates/fast-staged/Cargo.toml` — Dependencies list (gix not yet added)

### Codebase Maps
- `.planning/codebase/ARCHITECTURE.md` — Overall module architecture
- `.planning/codebase/CONVENTIONS.md` — Naming patterns, error handling conventions

### Research Notes
- STATE.md blocker: "gix staged-file API shape varies by minor version — verify on crates.io before Phase 4"

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AppError::NotGitRepository` — ready for gix::discover failure
- `AppError::NoStagedFiles` — ready for empty staged files case
- `AppError::Git(String)` — generic git error wrapper for gix errors
- `AppError::Io` — covers file I/O for patch read/write

### Established Patterns
- Module in `src/`, re-exported from `lib.rs` (error.rs, config/ patterns)
- `pub async fn run() -> Result<(), AppError>` as orchestration entry point
- Tests use temp directories with `.git/` created via `fs::create_dir_all`

### Integration Points
- `lib.rs::run()` — git integration inserts after `config::load_config()` call
- `error.rs` — existing variants cover all Phase 4 error cases
- `Cargo.toml` — gix dependency needs to be added with appropriate features

</code_context>

<specifics>
## Specific Ideas

- GitContext usage in lib.rs::run():
  ```rust
  let config = config::load_config(None)?;
  let git = git::GitContext::open()?;
  let files = git.staged_files()?;  // or git.unstaged_files() with --unstaged
  git.backup()?;
  // ... Phase 5: match files to commands, execute ...
  git.cleanup()?;  // on success; git.restore()? on failure
  ```
- The gix crate version and feature flags should be verified by the researcher — the API surface has changed across versions
- Patch format must be standard unified diff so it can be applied back correctly

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 04-git-integration*
*Context gathered: 2026-04-10*
