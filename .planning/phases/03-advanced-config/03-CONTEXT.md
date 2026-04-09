# Phase 3: Advanced Config - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Named group support with per-group execution order, timeout, continue_on_error, and per_file settings. Phase 2 pre-built the type system and basic parsing — Phase 3 ensures named groups are fully validated, ordered correctly, and documented with comprehensive tests. No runtime execution behavior (that's Phase 5).

</domain>

<decisions>
## Implementation Decisions

### Group Ordering
- **D-01:** Groups execute in declaration order from the config file. TOML's HashMap doesn't guarantee insertion order — the parser must track insertion order explicitly (e.g., use `toml::Table` iteration order or manually track position during deserialization). If a "default" group exists from `[patterns]`, it comes first.
- **D-02:** JSON groups follow key order as parsed by serde_json (which preserves insertion order).

### Group-Level Validation
- **D-03:** Validate at parse time: each named group must have non-empty patterns, valid timeout format (via parse_duration), and valid execution_order string ("parallel" or "sequential").
- **D-04:** A config with named groups but ALL groups having empty patterns should return AppError::ConfigInvalid — at least one group must have patterns.
- **D-05:** Unknown fields inside named groups are caught by `#[serde(deny_unknown_fields)]` on RawGroupConfig (already in place from Phase 2).

### Config-Time vs Runtime Boundaries
- **D-06:** Phase 3 scope is config parsing and validation only. Success criteria referencing runtime behavior (e.g., "causes groups to run in declared order", "stops command after 30 seconds") are verified by testing the parsed Config struct's values, not by actually running commands. Actual execution semantics come in Phase 5.
- **D-07:** Tests verify that Config.groups preserves order, each Group has correct per-group overrides, and cascading defaults apply correctly across multiple groups.

### Multi-Group Interaction Semantics
- **D-08:** Each group is independent. `continue_on_error` is per-group scope — it controls whether commands WITHIN a group continue after failure, not whether the NEXT group runs. Whether to stop after a failed group is a global-level `continue_on_error` setting.
- **D-09:** When global `continue_on_error` is false (default), a failed group stops execution. When true, all groups run regardless of individual failures.

### Claude's Discretion
- Test count and organization — how many tests and how to structure them across modules
- Whether to refactor existing Phase 2 parsing code or add incremental validation on top
- Error message wording for group-specific validation failures

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Specs
- `.planning/ROADMAP.md` — Phase 3 success criteria (CFG-06 through CFG-09)
- `.planning/REQUIREMENTS.md` — CFG-06 to CFG-09 requirement definitions

### Prior Phase Context
- `.planning/phases/02-config-core/02-CONTEXT.md` — Config struct design (D-01 to D-13), format decisions, discovery logic
- `.planning/phases/02-config-core/02-RESEARCH.md` — Technical research on TOML parsing, JSON detection, cascading defaults

### Existing Code
- `crates/fast-staged/src/config/types.rs` — Config, Group, ConfigSource, ExecutionOrder, Raw* structs
- `crates/fast-staged/src/config/defaults.rs` — Cascading defaults resolution (resolve_group)
- `crates/fast-staged/src/config/parse_toml.rs` — TOML parser with named group support (8 tests)
- `crates/fast-staged/src/config/parse_json.rs` — JSON parser with flat detection (10 tests)
- `crates/fast-staged/src/config/discover.rs` — Config file discovery and load_config entry point
- `crates/fast-staged/src/config/mod.rs` — Module root with re-exports
- `crates/fast-staged/src/error.rs` — AppError enum

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Group` struct already has all per-group fields: `timeout`, `execution_order`, `continue_on_error`, `per_file`
- `defaults::resolve_group()` already handles cascading defaults for all group fields
- `RawGroupConfig` already has `#[serde(deny_unknown_fields)]` for strict validation
- `parse_duration` crate already wired for timeout string parsing
- Phase 2 parsers already handle named groups in both TOML and JSON

### Established Patterns
- Raw* structs for serde deserialization → typed Config/Group structs via resolve_group()
- `#[cfg(test)] mod tests` in each parser file for unit tests
- `parse_*_str()` functions for testing without filesystem
- `AppError::ConfigInvalid(String)` for all parse/validation errors

### Integration Points
- Parsers (parse_toml.rs, parse_json.rs) — may need group ordering fixes
- defaults.rs — may need validation additions
- types.rs — types are complete, unlikely to need changes

</code_context>

<specifics>
## Specific Ideas

- Phase 2 decision D-01 explicitly pre-built the Phase 3 struct — the type system should be complete
- The main work is ensuring group ordering is preserved, adding comprehensive multi-group tests, and verifying cascading defaults work correctly when multiple groups have different overrides
- TOML named group example with per-group settings:
  ```toml
  execution_order = "parallel"
  timeout = "30s"

  [lint]
  execution_order = "sequential"
  timeout = "10s"
  [lint.patterns]
  "*.rs" = ["cargo clippy"]

  [format]
  continue_on_error = true
  [format.patterns]
  "*.rs" = ["cargo fmt"]
  "*.ts" = ["prettier --write"]
  ```

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 03-advanced-config*
*Context gathered: 2026-04-09*
