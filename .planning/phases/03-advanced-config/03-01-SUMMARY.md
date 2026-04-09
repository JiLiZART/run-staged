---
phase: 03-advanced-config
plan: 01
subsystem: config
tags: [rust, serde, toml, serde_json, preserve_order, indexmap, config-parsing]

requires:
  - phase: 02-config-core
    provides: "RawConfig/RawJsonConfig types, parse_toml, parse_json, resolve_group with cascading defaults"

provides:
  - "Declaration-order group preservation in TOML parser via toml preserve_order feature"
  - "Declaration-order group preservation in JSON parser via serde_json preserve_order feature"
  - "Comprehensive per-group override tests: execution_order, timeout, continue_on_error, per_file (CFG-06 through CFG-09)"
  - "Fix for JSON named-groups-only configs (no config field names) falling through to error"

affects: [04-git-core, 05-glob-matching, 06-command-execution]

tech-stack:
  added:
    - "serde_json preserve_order feature — serde_json::Map backed by IndexMap"
    - "toml preserve_order feature — toml::map::Map backed by IndexMap"
  patterns:
    - "Use preserve_order crate features instead of manual sort() for deterministic iteration"
    - "Fall through to structured parse when flat detection fails in JSON parser"

key-files:
  created: []
  modified:
    - "crates/fast-staged/Cargo.toml — preserve_order features on serde_json and toml"
    - "crates/fast-staged/src/config/types.rs — RawConfig.groups: toml::map::Map, RawJsonConfig.groups: serde_json::Map"
    - "crates/fast-staged/src/config/parse_toml.rs — remove sort(), iterate map directly, 10 new tests"
    - "crates/fast-staged/src/config/parse_json.rs — remove sort(), iterate map directly, fix fallback path, 6 new tests"

key-decisions:
  - "Enable preserve_order on both toml and serde_json crates so serde(flatten) captures groups in declaration order without manual sort()"
  - "JSON named-groups-only config (no config field names) now falls through to structured parse instead of error"

patterns-established:
  - "Ordering pattern: use crate-level preserve_order features, not sort() calls, for deterministic config group order"
  - "JSON dual-path detection: flat -> structured fallback, not flat -> error"

requirements-completed: [CFG-06, CFG-07, CFG-08, CFG-09]

duration: 25min
completed: 2026-04-09
---

# Phase 03 Plan 01: Advanced Config Summary

**Declaration-order group preservation fixed in TOML and JSON parsers using preserve_order crate features, with 16 new tests covering ordering and all per-group override scenarios (CFG-06 through CFG-09)**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-04-09T22:10:00Z
- **Completed:** 2026-04-09T22:35:00Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Fixed group ordering bug: TOML and JSON parsers now iterate groups in declaration order (zebra, alpha, middle — not alphabetical)
- Enabled `preserve_order` feature on both `serde_json` and `toml` crates, replacing the manual `.sort()` calls
- Fixed JSON named-groups-only configs that previously fell through to a parse error instead of succeeding
- Added 5 ordering tests (3 TOML, 2 JSON) and 11 per-group override tests (7 TOML, 4 JSON)
- Total test count increased from 18 to 56, all passing with no clippy warnings

## Task Commits

1. **Task 1: Fix group ordering — replace HashMap with IndexMap-backed types and remove sort()** - `c0c906b` (feat)
2. **Task 2: Add comprehensive per-group override tests for CFG-06 through CFG-09** - `8d34cd7` (test)

## Files Created/Modified

- `crates/fast-staged/Cargo.toml` — Added preserve_order feature to serde_json and toml
- `crates/fast-staged/src/config/types.rs` — RawConfig.groups changed to `toml::map::Map`, RawJsonConfig.groups to `serde_json::Map`
- `crates/fast-staged/src/config/parse_toml.rs` — Removed sort(), iterate map directly, 10 new tests added
- `crates/fast-staged/src/config/parse_json.rs` — Removed sort(), iterate map directly, fixed fallback path, 6 new tests added

## Decisions Made

- Used `preserve_order` crate features (toml + serde_json) so `serde(flatten)` captures groups in insertion/declaration order without needing manual sort
- JSON parser fallback path: when flat `HashMap<String, Vec<String>>` detection fails, now attempts structured `RawJsonConfig` parse before returning error (fixes named-groups-only JSON configs)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed JSON named-groups-only configs falling through to parse error**
- **Found during:** Task 1 (GREEN phase — test_json_named_groups_preserve_declaration_order was panicking with unwrap_err() on Ok)
- **Issue:** JSON with only named groups (no config field names like "timeout", "patterns") would fail flat detection (objects are not Vec<String>), then hit `unwrap_err()` on structured parse — but structured parse actually succeeded. The code was structured to always produce an error in this path.
- **Fix:** Changed the final else branch from `unwrap_err()` to `match serde_json::from_value::<RawJsonConfig>(raw)` — succeed if structured parse works, error only if both paths fail
- **Files modified:** crates/fast-staged/src/config/parse_json.rs
- **Verification:** test_json_named_groups_preserve_declaration_order passes
- **Committed in:** c0c906b (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Essential fix — named-groups-only JSON configs would have been completely broken. No scope creep.

## Issues Encountered

- TOML tests still failed after type change alone: needed `toml` crate `preserve_order` feature in addition to `serde_json`. The TOML crate's `Map` type defaults to BTreeMap (sorted) without the feature.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Config module complete: discovery, TOML/JSON/package.json parsing, ordered groups, per-group overrides — all tested
- Phase 04 (git-core) can proceed: `get_changed_files()` via gix crate
- No blockers from this plan

---
*Phase: 03-advanced-config*
*Completed: 2026-04-09*
