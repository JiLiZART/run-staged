# Phase 6: CLI Interface - Context

**Gathered:** 2026-04-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Standard Unix CLI surface for the `fast-staged` binary. Replace the temporary hand-rolled argv inspection in `crates/fast-staged/src/main.rs` with a clap 4 derive `Parser` struct that:

- Parses `--config <path>`, `--version`, `--help`, plus the already-wired runtime flags `--quiet`, `--unstaged`, `--allow-empty`.
- Forwards all runtime flags to `lib::run()` with the same semantics they have today.
- Adds explicit `config_path: Option<PathBuf>` plumbing through `lib::run()` so `--config <path>` overrides auto-discovery.

Out of scope: TUI rendering (Phase 8), signal handling (Phase 7), lint-staged config compatibility (Phase 9), `--stash` / `--revert` / `--fail-on-changes` (not in CLI-01..03 success criteria — defer).

</domain>

<decisions>
## Implementation Decisions

### CLI parser library
- **D-01:** Use `clap = { version = "4", features = ["derive"] }` already declared in `crates/fast-staged/Cargo.toml`. Hand-rolled `args.iter().any(|a| a == "--flag")` block in `main.rs` (lines 6-9) is removed and replaced with a `#[derive(Parser)]` struct. No new deps.

### `--config <path>` semantics
- **D-02:** Hard-fail when the supplied path does not exist. No fallback to auto-discovery, no warn-and-continue. A user passing `--config` is asserting an explicit choice — silent fallback would mask typos and run the wrong config.
- **D-03:** Distinguish "missing file" from "invalid file" using the existing `AppError` variants:
  - File does not exist → `AppError::ConfigNotFound { path }`
  - File exists but parse fails → `AppError::ConfigInvalid { path, source }`
  Both already exist in `crates/fast-staged/src/error.rs`; planner does not need to add new variants.
- **D-04:** Resolve relative paths against the **current working directory** (CWD), not the git root. Standard Unix CLI behavior — predictable for shell scripts, husky hooks, and CI. Auto-discovery (no `--config`) keeps its existing CWD→git-root walk; only the explicit-path form is CWD-anchored.

### Unknown / unrecognized flags
- **D-05:** Strict reject. Use clap's default behavior — print error with similar-flag suggestion and usage hint, exit code 2 (clap convention). Catches typos like `--quet` or `--allow_empty`. There are no spawned commands to forward args to at this layer (commands receive only their configured `{files}`), so a permissive policy has no upside.

### Help / version surface (Claude's discretion — user did not select for discussion)
- **D-06:** Use clap's default `--help` / `--version` output. About line and version come from `Cargo.toml` `[package]` `description` and `version` fields via `#[command(version, about, long_about = None)]`. No custom usage examples or repo URL in v1 — Cargo.toml description provides the one-line about; users can read the README for examples. Revisit if telemetry shows users confused at the help screen.

### Short flag aliases (Claude's discretion — user did not select for discussion)
- **D-07:** Long-only for project-specific flags (`--quiet`, `--unstaged`, `--allow-empty`, `--config`). Keep clap's two automatic short aliases — `-h` (help) and `-V` (version) — because they are zero-cost and Unix-standard. No `-q`, `-c`, `-u`, `-e`. Rationale: explicit long flags read clearly in pre-commit hooks (`fast-staged --quiet --allow-empty`) and avoid clashing with future flag additions. Can add short aliases later if user feedback demands it.

### Lib API change
- **D-08:** Extend `lib::run()` signature to accept the config path. Current shape:
  ```rust
  pub async fn run(unstaged: bool, allow_empty: bool, quiet: bool) -> Result<(), AppError>
  ```
  becomes:
  ```rust
  pub async fn run(
      config_path: Option<PathBuf>,
      unstaged: bool,
      allow_empty: bool,
      quiet: bool,
  ) -> Result<(), AppError>
  ```
  When `config_path` is `Some`, skip auto-discovery and load directly. When `None`, retain Phase 2 auto-discovery walk.

### Claude's Discretion
- D-06 (help/version style) and D-07 (short flag aliases) above — user skipped these areas in the gray-area select. Planner free to deviate from D-06/D-07 if research uncovers a strong reason; D-01..D-05 and D-08 are locked from explicit user input.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope and requirements
- `.planning/ROADMAP.md` §"Phase 6: CLI Interface" — goal, success criteria, requirement IDs
- `.planning/REQUIREMENTS.md` — CLI-01 (--config), CLI-02 (--version, --help), CLI-03 (--quiet, --unstaged, --allow-empty)
- `.planning/PROJECT.md` §Active — confirms clap parsing as a tracked active requirement

### Existing code that this phase modifies
- `crates/fast-staged/src/main.rs` — current 14-line hand-rolled argv parser, with explicit `// Phase 6 replaces this with full clap parsing` comment on line 5
- `crates/fast-staged/src/lib.rs` — `pub async fn run(...)` signature gets a new `config_path: Option<PathBuf>` parameter
- `crates/fast-staged/src/error.rs` — `AppError::ConfigNotFound` and `AppError::ConfigInvalid` variants already exist; reuse, do not add new
- `crates/fast-staged/Cargo.toml` — clap 4 derive already declared in `[dependencies]`; no edits expected
- `crates/fast-staged/src/config/` — Phase 2 auto-discovery walk (CWD→git-root); the `--config` path bypasses this walk but reuses the parser

### Output / error rendering already in place
- `crates/fast-staged/src/output.rs` (Phase 5) — `success_line` and `error_box` helpers; reuse for any new error rendering. clap's own error output handles unknown-flag and missing-arg errors before our code runs.

### Codebase maps (background reading for planner)
- `.planning/codebase/STACK.md` — confirms clap 4 derive feature
- `.planning/codebase/STRUCTURE.md` — main.rs / lib.rs layout
- `.planning/codebase/CONVENTIONS.md` — Rust 2024 + edition lints, fmt rules

### Backlog items adjacent to this phase
- `.planning/ROADMAP.md` §"Phase 999.1" — dogfood matcher/runner bug from Phase 5; **not** Phase 6 scope, but planner should know the project's pre-commit hook is currently flaky for unrelated reasons.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **clap 4 derive:** Cargo.toml dep already in place (declared Phase 1), zero new dependencies needed. Uses `#[derive(Parser)]` + `#[command(version, about)]`.
- **`AppError` enum:** `ConfigNotFound { path }` and `ConfigInvalid { path, source }` variants exist (`crates/fast-staged/src/error.rs`). New `--config` failure paths plug into existing error rendering with no new variants.
- **Phase 5 `output.rs`:** `success_line` and `error_box` are stable. CLI errors (clap's own + our `--config` failures) flow through `main.rs::main`'s existing `eprintln!("error: {e}")` pattern; no changes to output module expected.
- **Phase 2 config loader:** `Config::load_from_path(&Path)` (or equivalent — verify exact name during research) already exists from Phase 2; `--config` reuses it directly.

### Established Patterns
- **Async tokio main:** `#[tokio::main] async fn main()` returns nothing; errors print to stderr and `process::exit(1)`. Phase 6 keeps this shape — clap parsing happens **before** we enter async context (clap's `Parser::parse()` is sync and panics-via-exit on bad args, which is correct for CLI).
- **Flag → run() forward:** Current `main.rs` extracts three booleans and forwards to `run()`. Phase 6 extends to four arguments (add `config_path: Option<PathBuf>`). No new lifetime/ownership issues — `PathBuf` is owned and `Send`.
- **Snake_case in Rust, kebab-case in CLI flags:** clap's `#[arg(long)]` auto-converts `allow_empty` → `--allow-empty`; matches the existing flag names in `main.rs`.

### Integration Points
- `main.rs` consumes `clap::Parser::parse()` and immediately forwards into `fast_staged::run(...)`. No other code in the crate touches argv directly.
- `lib::run()` is the only consumer of the parsed flag values. Its signature change ripples to `crates/fast-staged/tests/execution_integration.rs` (Phase 5 — 6 e2e tests; verify in-place call sites updated).

</code_context>

<specifics>
## Specific Ideas

- The phrase "expected Unix CLI behavior" in the ROADMAP goal is the user's anchor. Concretely: standard exit codes (0 success, 1 runtime error, 2 argparse error), strict unknown-flag rejection, no surprising "helpful" fallbacks, errors to stderr.
- D-04 (`--config` resolves to CWD) is the user's explicit choice over the more "consistent" git-root resolution. Treat that as load-bearing — do not change in research/planning.
- Hand-rolled parser comment on `main.rs:5` (`// Phase 6 replaces this with full clap parsing`) is the implementation contract from Phase 5. Honor it.

</specifics>

<deferred>
## Deferred Ideas

- **Custom `--help` long-form with usage examples and config-format hint** — pulled out of D-06. Revisit when there's user feedback indicating the default clap help is insufficient. Capture as a small follow-up after Phase 9 (lint-staged compat) if migrators are confused.
- **Short flag aliases for `--quiet` / `--config` / `--unstaged`** — pulled out of D-07. Same trigger: user feedback / scripting ergonomics requests.
- **`--stash`, `--revert`, `--fail-on-changes` flags** — referenced in PROJECT.md as part of the long-term flag set, but not in CLI-01..03. Belong in their own phase after Phase 7 (signal handling) where the on-failure restore semantics are finalized.
- **`fast-staged init` / `check` subcommands** — discussed implicitly by avoiding subcommand structure in this phase. If the tool ever grows beyond a single default action, revisit clap subcommand design.
- **`--version` enriched with git commit hash + build profile** — clap default ships only the Cargo.toml version. Adding `built` or `vergen` for hash+date would be its own micro-phase.

</deferred>

---

*Phase: 06-cli-interface*
*Context gathered: 2026-04-29*
