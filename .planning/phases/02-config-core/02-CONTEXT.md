# Phase 2: Config Core - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Multi-format config file discovery and parsing. The tool finds, reads, validates, and deserializes user config from TOML, JSON, or package.json into a typed Config struct. No command execution, no git integration, no TUI — just config loading with clear error messages.

</domain>

<decisions>
## Implementation Decisions

### Config Struct Design
- **D-01:** Pre-build the full Phase 3 struct now. Config has `source: ConfigSource`, `groups: Vec<Group>`, plus top-level default fields (timeout, execution_order, continue_on_error, per_file).
- **D-02:** Group struct includes: `name: String`, `patterns: HashMap<String, Vec<String>>`, `timeout: Option<Duration>`, `execution_order: Option<ExecutionOrder>`, `continue_on_error: bool`, `per_file: bool`.
- **D-03:** Cascading defaults: flat top-level params set defaults, root-level params can override, each group further overrides. Resolution order: `default -> root -> group`.
- **D-04:** Patterns live under a `[patterns]` table (TOML) or `"patterns"` key (JSON), not as top-level keys mixed with config knobs.
- **D-05:** Commands are always `Vec<String>` (array only). No single-string shorthand — `"*.rs" = ["cargo fmt"]`, not `"*.rs" = "cargo fmt"`.

### Config Formats
- **D-06:** TOML structure: top-level defaults, `[patterns]` for root group patterns, `[group1.patterns]` or `[group1]` with `patterns = { ... }` for named groups.
- **D-07:** JSON mirrors TOML structure exactly: `{ "timeout": "5s", "patterns": { "*.rs": ["cargo fmt"] } }`.
- **D-08:** Also detect flat lint-staged style JSON (glob keys at root with no `patterns` wrapper) and parse it as a single implicit group.
- **D-09:** `package.json` config lives under the `"fast-staged"` key and follows the same JSON structure.

### Config Discovery
- **D-10:** Walk up from CWD to git repository root. Check all 5 formats per directory before moving to parent. First match wins — no merging.
- **D-11:** Priority within each directory: `.fast-staged.toml` > `.fast-staged.json` > `fast-staged.toml` > `fast-staged.json` > `package.json[fast-staged]`.
- **D-12:** `--config path/to/file` skips auto-discovery entirely. Error if specified file doesn't exist or is invalid (no fallback).

### Validation
- **D-13:** Claude's discretion on validation strictness and error message detail (line numbers, field suggestions, etc.). Must satisfy CFG-05: clear error message and non-zero exit for invalid configs.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Specs
- `.planning/ROADMAP.md` — Phase 2 success criteria (CFG-01 through CFG-05)
- `.planning/REQUIREMENTS.md` — CFG-01 to CFG-05 requirement definitions
- `.planning/PROJECT.md` — Constraints: serde + toml + serde_json for config parsing

### Prior Phase Context
- `.planning/phases/01-foundation/01-CONTEXT.md` — AppError design (ConfigNotFound, ConfigInvalid variants), lib.rs + main.rs split pattern

### Existing Code
- `crates/fast-staged/src/error.rs` — AppError enum with ConfigNotFound and ConfigInvalid(String)
- `crates/fast-staged/src/lib.rs` — `pub async fn run()` entry point (currently no-op)
- `crates/fast-staged/Cargo.toml` — serde, serde_json, toml dependencies already declared

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AppError::ConfigNotFound` and `AppError::ConfigInvalid(String)` — ready to use for discovery and parse errors
- `serde`, `serde_json`, `toml` crates — already in Cargo.toml dependencies
- `parse_duration` crate — available for parsing human-readable timeout strings like `"5s"`, `"30s"`

### Established Patterns
- Module in `src/`, re-exported from `lib.rs` (established by `error.rs` pattern)
- `pub async fn run() -> Result<(), AppError>` as the orchestration entry point

### Integration Points
- `lib.rs::run()` — config loading will be the first step in the pipeline
- `error.rs` — may need additional error context for parse failures (e.g., file path in ConfigInvalid)

</code_context>

<specifics>
## Specific Ideas

- Config with named groups example:
  ```toml
  timeout = "5s"

  [patterns]
  "*.rs" = ["cargo fmt"]

  [group1]
  timeout = "10s"
  [group1.patterns]
  "*.ts" = ["eslint --fix"]
  ```
- Flat lint-staged JSON detection: if JSON has no `patterns` key and all values are string arrays, treat keys as glob patterns in a single implicit group
- ConfigSource enum should track which file was loaded (path + format type)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-config-core*
*Context gathered: 2026-04-09*
