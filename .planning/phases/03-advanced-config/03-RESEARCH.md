# Phase 03: Advanced Config - Research

**Researched:** 2026-04-09
**Domain:** Rust config parsing — group ordering, per-group overrides, validation, tests
**Confidence:** HIGH

## Summary

Phase 3 is primarily a test coverage and validation hardening phase built on top of the Phase 2 type system. All four target structs (`Config`, `Group`, `ExecutionOrder`, `RawGroupConfig`) are fully in place. The cascading defaults resolver (`resolve_group`) is already wired and tested. The TOML and JSON parsers already handle named groups.

The two concrete gaps are: (1) group ordering — both parsers currently `sort()` named groups alphabetically rather than preserving declaration order, which contradicts decision D-01; and (2) test coverage of multi-group configs with per-group overrides, which is minimal in Phase 2 (only single-group scenarios are exercised for the new features). The work is incremental: fix the ordering bug, add validation for the all-empty-patterns edge case (D-04), and write comprehensive multi-group tests.

**Primary recommendation:** Fix insertion-order preservation in both parsers first (highest risk item), then add multi-group tests that verify ordering, cascading defaults, and validation. No new structs or crates are needed.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Groups execute in declaration order from the config file. TOML's HashMap doesn't guarantee insertion order — the parser must track insertion order explicitly (e.g., use `toml::Table` iteration order or manually track position during deserialization). If a "default" group exists from `[patterns]`, it comes first.
- **D-02:** JSON groups follow key order as parsed by serde_json (which preserves insertion order).
- **D-03:** Validate at parse time: each named group must have non-empty patterns, valid timeout format (via parse_duration), and valid execution_order string ("parallel" or "sequential").
- **D-04:** A config with named groups but ALL groups having empty patterns should return AppError::ConfigInvalid — at least one group must have patterns.
- **D-05:** Unknown fields inside named groups are caught by `#[serde(deny_unknown_fields)]` on RawGroupConfig (already in place from Phase 2).
- **D-06:** Phase 3 scope is config parsing and validation only. Success criteria referencing runtime behavior are verified by testing the parsed Config struct's values, not by actually running commands. Actual execution semantics come in Phase 5.
- **D-07:** Tests verify that Config.groups preserves order, each Group has correct per-group overrides, and cascading defaults apply correctly across multiple groups.
- **D-08:** Each group is independent. `continue_on_error` is per-group scope — it controls whether commands WITHIN a group continue after failure, not whether the NEXT group runs. Whether to stop after a failed group is a global-level `continue_on_error` setting.
- **D-09:** When global `continue_on_error` is false (default), a failed group stops execution. When true, all groups run regardless of individual failures.

### Claude's Discretion

- Test count and organization — how many tests and how to structure them across modules
- Whether to refactor existing Phase 2 parsing code or add incremental validation on top
- Error message wording for group-specific validation failures

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CFG-06 | Tool supports named groups with per-group execution_order setting | `Group.execution_order: ExecutionOrder` exists; `resolve_group()` handles it; tests needed for multi-group ordering |
| CFG-07 | Tool supports per-group timeout override | `Group.timeout: Option<Duration>` exists; `resolve_group()` cascades from root; tests needed for per-group override + root inheritance across multiple groups |
| CFG-08 | Tool supports per-group continue_on_error setting | `Group.continue_on_error: bool` exists; `resolve_group()` handles it; D-08/D-09 semantic boundary must be tested |
| CFG-09 | Tool supports per-group per-file vs batch execution mode | `Group.per_file: bool` exists; `resolve_group()` handles it; tests needed verifying both true/false values survive parsing |
</phase_requirements>

---

## Standard Stack

### Core (already in Cargo.toml — no new dependencies)

| Library | Version | Purpose | Status |
|---------|---------|---------|--------|
| `toml` | 1.x | TOML deserialization via `toml::Value` (Table iteration) | In place |
| `serde_json` | 1.x | JSON deserialization; `IndexMap`-backed objects preserve insertion order | In place |
| `serde` | 1.x | Derive macros on Raw* structs | In place |
| `parse_duration` | 2.x | Human-readable timeout parsing (`"30s"`, `"1min"`) | In place |
| `thiserror` | 2.x | `AppError` derive | In place |

### No New Dependencies Required

Phase 3 introduces no new crates. All required types and crates were provisioned in Phase 2 per decision D-01.

**Installation:**
```bash
# No new packages — existing Cargo.toml is sufficient
```

---

## Architecture Patterns

### Group Ordering: The Core Technical Problem

**What goes wrong today:** Both parsers call `named_group_names.sort()`, which gives alphabetical order, not declaration order. This violates D-01.

**TOML fix — use `toml::Table` iteration order:**

`toml::Table` is internally an `IndexMap<String, toml::Value>` which preserves insertion order. When `RawConfig` is deserialized, `raw.groups: HashMap<String, toml::Value>` loses that order because `std::collections::HashMap` is unordered.

**Fix:** Change `RawConfig.groups` type from `HashMap<String, toml::Value>` to `toml::Table` (which is `IndexMap<String, toml::Value>`). Then iterate `raw.groups` directly without sorting.

```rust
// In types.rs — change RawConfig:
#[derive(Debug, Deserialize)]
pub(crate) struct RawConfig {
    pub timeout: Option<String>,
    pub execution_order: Option<String>,
    pub continue_on_error: Option<bool>,
    pub per_file: Option<bool>,
    pub patterns: Option<HashMap<String, Vec<String>>>,
    #[serde(flatten)]
    pub groups: toml::Table,  // was HashMap<String, toml::Value>
}
```

`toml::Table` is `type Table = IndexMap<String, Value>` and is re-exported from the `toml` crate. Serde's `flatten` with `IndexMap`-based maps preserves insertion order.

**JSON fix — serde_json preserves order by default:**

`serde_json` uses `IndexMap` internally for `Map<String, Value>` objects, so `Value::Object` iterates in insertion order. The existing `RawJsonConfig.groups: HashMap<String, serde_json::Value>` also loses order. Fix: change to `serde_json::Map<String, serde_json::Value>` which preserves order.

```rust
// In types.rs — change RawJsonConfig:
#[derive(Debug, Deserialize)]
pub(crate) struct RawJsonConfig {
    pub timeout: Option<String>,
    pub execution_order: Option<String>,
    pub continue_on_error: Option<bool>,
    pub per_file: Option<bool>,
    pub patterns: Option<HashMap<String, Vec<String>>>,
    #[serde(flatten)]
    pub groups: serde_json::Map<String, serde_json::Value>,  // was HashMap
}
```

`serde_json::Map` uses `IndexMap` when `serde_json` is compiled with the `preserve_order` feature, and the `indexmap` crate otherwise. **Important:** check whether serde_json in the project is compiled with `preserve_order`. If not, the fix requires either enabling that feature or using `indexmap::IndexMap<String, serde_json::Value>` directly.

**Verification needed:**

```bash
# Check if preserve_order is already enabled:
grep -r "preserve_order" crates/fast-staged/Cargo.toml
# Also check serde_json docs: serde_json::Map is IndexMap when preserve_order is enabled
```

If `preserve_order` is NOT enabled, the cleanest fix is:
```toml
# Cargo.toml
serde_json = { version = "1", features = ["preserve_order"] }
```

**Alternative for both parsers (no type change):** After flattened deserialization, re-parse the raw TOML/JSON string to extract group key order using a separate pass. This is more complex and not recommended. The type-level fix is cleaner.

### Parser Structure (No Change Needed)

Both `parse_toml.rs` and `parse_json.rs` follow the same pattern:
1. Deserialize into Raw* struct
2. Build "default" group from top-level `patterns` (if present) — always first
3. Iterate named groups, skip non-tables/non-objects, skip empty patterns, call `resolve_group()`
4. Validate at least one group exists
5. Build `Config` from resolved values

The only change in step 3 is: remove the `.sort()` call after the ordering fix.

### Validation Gap: D-04

D-04 requires `AppError::ConfigInvalid` when ALL named groups have empty patterns. The current code silently skips empty-patterns groups (the `continue` in both parsers). If every group is skipped, the `groups.is_empty()` check at line 100 of `parse_toml.rs` catches it and returns `"no patterns defined"`. This already satisfies D-04 — no code change needed, only a test confirming this behavior for multi-group inputs.

### Test Organization (Claude's Discretion)

Recommended structure: add tests in-file (`#[cfg(test)] mod tests`) within each parser file. No separate test file needed at this scale. Target test additions:

**In `parse_toml.rs`:**
- Multi-group declaration order preserved (3 groups in non-alphabetical order)
- Per-group `execution_order` override (group overrides root)
- Per-group `timeout` override and inheritance
- Per-group `continue_on_error` override
- Per-group `per_file` override
- Mixed groups: some with per-group overrides, some inheriting root
- All named groups empty-patterns → `ConfigInvalid`
- Default group first, then named groups

**In `parse_json.rs`:**
- Multi-group declaration order preserved (requires `preserve_order` fix)
- Per-group settings overrides (mirror TOML tests)
- Mixed default + named groups

**In `defaults.rs`:**
- Already has 6 tests covering `resolve_group` — Phase 3 adds no new scenarios there (per-group field resolution is already tested)

**In `discover.rs`:**
- No new tests needed — Phase 3 doesn't change discovery logic

### Anti-Patterns to Avoid

- **Sorting named groups:** The existing `named_group_names.sort()` call MUST be removed in both parsers.
- **Testing runtime behavior:** Per D-06, tests assert values in parsed `Config`/`Group` structs only. No command execution.
- **Confusing per-group vs global `continue_on_error`:** Per D-08, the `Group.continue_on_error` field only controls within-group behavior. Tests should NOT try to verify cross-group behavior (that's Phase 5).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Insertion-order HashMap | Custom ordered map | `toml::Table` (IndexMap) / `serde_json::Map` with `preserve_order` | Already in the dependency tree |
| Timeout parsing | Custom duration parser | `parse_duration::parse()` (already wired) | Handles "30s", "1min", "500ms" etc. |
| Unknown field rejection | Manual field name checking | `#[serde(deny_unknown_fields)]` on `RawGroupConfig` (already in place) | Serde handles this at zero cost |

**Key insight:** The entire problem domain for Phase 3 is already solved at the type level. The work is fixing one ordering bug and writing thorough tests.

---

## Common Pitfalls

### Pitfall 1: `HashMap` vs `IndexMap` for Group Ordering
**What goes wrong:** Using `HashMap<String, toml::Value>` or `HashMap<String, serde_json::Value>` for flattened groups loses insertion order. Both parsers have this bug today.
**Why it happens:** Rust's `HashMap` uses random seed by default (SipHash). Even the same input produces non-deterministic ordering.
**How to avoid:** Change the type to `toml::Table` (TOML) and `serde_json::Map<String, serde_json::Value>` with `preserve_order` feature (JSON). Both are backed by `IndexMap` which preserves insertion order.
**Warning signs:** Test that writes `[c]`, `[a]`, `[b]` groups and expects `["c", "a", "b"]` order will flake — sometimes passes, sometimes fails.

### Pitfall 2: `serde_json` `preserve_order` Feature Not Enabled
**What goes wrong:** `serde_json::Map` is an alias for `BTreeMap` (alphabetical order) when compiled without `preserve_order`.
**Why it happens:** The feature is off by default to minimize compile time.
**How to avoid:** Add `features = ["preserve_order"]` to the `serde_json` dependency in `Cargo.toml` before changing the type to `serde_json::Map`.
**Warning signs:** JSON group order tests will show alphabetical ordering even after the type change.

### Pitfall 3: `#[serde(flatten)]` Behavior with Config Field Names
**What goes wrong:** When `RawConfig` uses `#[serde(flatten)]` to capture groups, config field names (`timeout`, `execution_order`, etc.) at the root level are consumed by their declared fields and do NOT appear in the flattened `groups` map. This is correct behavior, but a group named `"timeout"` would collide.
**Why it happens:** Serde flatten with known field names excludes them from the catch-all map.
**How to avoid:** This is already handled correctly. No group should be named after a config field — this is an acceptable limitation (not worth validating in Phase 3, deferred naturally).

### Pitfall 4: `toml::Table` Re-Export Path
**What goes wrong:** `toml::Table` may not be directly importable as a type in the `Deserialize` derive unless the full path is used.
**Why it happens:** `toml::Table` is `type Table = IndexMap<String, Value>` — it's a type alias, not a struct. It deserializes correctly via the `IndexMap` impl.
**How to avoid:** Use `toml::map::Map<String, toml::Value>` as the explicit type, or verify `toml::Table` is in scope. Both work identically.

### Pitfall 5: Default Group Position When Both Patterns and Named Groups Exist
**What goes wrong:** If a config has both top-level `[patterns]` and named groups, the "default" group must come first (D-01). The existing code already does this (default group is pushed first, then named groups). Adding order-preservation must not break this invariant.
**Why it happens:** Ordering fixes apply only to the named groups loop; the default group is always first.
**How to avoid:** Keep the existing push-default-first logic. Only the `named_group_names.sort()` removal touches ordering.

---

## Code Examples

### TOML Multi-Group Config (from CONTEXT.md)
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
Expected: `config.groups[0].name == "lint"`, `config.groups[1].name == "format"` (declaration order).

### JSON Multi-Group Config
```json
{
  "timeout": "30s",
  "lint": {
    "execution_order": "sequential",
    "patterns": { "*.rs": ["cargo clippy"] }
  },
  "format": {
    "continue_on_error": true,
    "patterns": { "*.rs": ["cargo fmt"], "*.ts": ["prettier --write"] }
  }
}
```
Expected: `config.groups[0].name == "lint"`, `config.groups[1].name == "format"` (requires `preserve_order`).

### Ordering Fix for TOML Parser (types.rs change)
```rust
// Source: toml crate docs — toml::Table is IndexMap<String, Value>
// Change in types.rs RawConfig:
#[serde(flatten)]
pub groups: toml::Table,  // replaces HashMap<String, toml::Value>

// Change in parse_toml.rs — remove sort, iterate directly:
for (group_name, value) in &raw.groups {
    if !value.is_table() { continue; }
    // ... rest of loop unchanged
}
```

### Ordering Fix for JSON Parser (types.rs + Cargo.toml changes)
```toml
# Cargo.toml
serde_json = { version = "1", features = ["preserve_order"] }
```

```rust
// types.rs RawJsonConfig:
#[serde(flatten)]
pub groups: serde_json::Map<String, serde_json::Value>,  // replaces HashMap

// parse_json.rs build_config_from_json — remove sort, iterate directly:
for (group_name, value) in &raw.groups {
    if !value.is_object() { continue; }
    // ... rest of loop unchanged
}
```

### Test Scaffold for Ordering
```rust
#[test]
fn test_named_groups_preserve_declaration_order() {
    let toml = r#"
[zebra]
[zebra.patterns]
"*.ts" = ["eslint"]

[alpha]
[alpha.patterns]
"*.rs" = ["cargo fmt"]

[middle]
[middle.patterns]
"*.go" = ["gofmt"]
"#;
    let config = parse(toml).unwrap();
    // Groups must appear in declaration order, not alphabetical
    assert_eq!(config.groups[0].name, "zebra");
    assert_eq!(config.groups[1].name, "alpha");
    assert_eq!(config.groups[2].name, "middle");
}
```

---

## State of the Art

| Old Approach | Current Approach | Notes |
|--------------|------------------|-------|
| `HashMap` for group order | `IndexMap`-backed types (`toml::Table`, `serde_json::Map` with preserve_order) | Standard for order-preserving TOML/JSON deserialization in Rust |
| Sort groups alphabetically | Preserve declaration order | Required by D-01 |

---

## Open Questions

1. **`serde_json` `preserve_order` feature status**
   - What we know: The feature exists and is well-documented. `serde_json = "1"` in Cargo.toml does NOT enable it.
   - What's unclear: Whether enabling it breaks any existing test expectations (unlikely, since no JSON ordering tests exist yet).
   - Recommendation: Add `features = ["preserve_order"]` as the first task in Wave 0. Run `cargo test` to confirm no regressions before proceeding.

2. **`toml::Table` type alias availability**
   - What we know: `toml::Table` is documented as `pub type Table = Map<String, Value>`. `toml::map::Map` is the concrete type.
   - What's unclear: Whether `toml::Table` is directly usable in a `#[derive(Deserialize)]` struct without import issues.
   - Recommendation: Use `toml::map::Map<String, toml::Value>` as the explicit type if `toml::Table` causes import resolution issues.

---

## Environment Availability

Step 2.6: SKIPPED — Phase 3 is config parsing and test additions only. No external tools, services, or databases required. All dependencies are in-process Rust crates already in Cargo.toml.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (cargo test) |
| Config file | none — standard `#[cfg(test)]` inline modules |
| Quick run command | `cargo test --manifest-path crates/fast-staged/Cargo.toml` |
| Full suite command | `cargo test --manifest-path crates/fast-staged/Cargo.toml` |

Current baseline: 40 tests pass across 4 suites.

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CFG-06 | Named groups preserve declaration order in TOML | unit | `cargo test parse_toml -- --nocapture` | ❌ Wave 0 |
| CFG-06 | Named groups preserve declaration order in JSON | unit | `cargo test parse_json -- --nocapture` | ❌ Wave 0 |
| CFG-06 | Per-group `execution_order` overrides root | unit | `cargo test parse_toml -- --nocapture` | ❌ Wave 0 |
| CFG-07 | Per-group `timeout` overrides root timeout | unit | `cargo test parse_toml -- --nocapture` | ❌ Wave 0 |
| CFG-07 | Per-group `timeout` inherits root when unset | unit | `cargo test parse_toml -- --nocapture` | ❌ Wave 0 |
| CFG-08 | Per-group `continue_on_error` overrides root | unit | `cargo test parse_toml -- --nocapture` | ❌ Wave 0 |
| CFG-08 | Global `continue_on_error` does not affect Group struct | unit | `cargo test parse_toml -- --nocapture` | ❌ Wave 0 |
| CFG-09 | Per-group `per_file: true` round-trips through parser | unit | `cargo test parse_toml -- --nocapture` | ❌ Wave 0 |
| CFG-09 | Per-group `per_file: false` (default) correct | unit | `cargo test parse_toml -- --nocapture` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test --manifest-path crates/fast-staged/Cargo.toml`
- **Per wave merge:** `cargo test --manifest-path crates/fast-staged/Cargo.toml`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] Enable `serde_json` `preserve_order` feature — `Cargo.toml` change
- [ ] Fix `RawConfig.groups` type → `toml::Table` (or `toml::map::Map<String, toml::Value>`) in `types.rs`
- [ ] Fix `RawJsonConfig.groups` type → `serde_json::Map<String, serde_json::Value>` in `types.rs`
- [ ] Remove `named_group_names.sort()` in `parse_toml.rs`
- [ ] Remove `named_group_names.sort()` in `parse_json.rs`
- [ ] Add ordering tests to `parse_toml.rs`
- [ ] Add per-group override tests to `parse_toml.rs`
- [ ] Add ordering tests to `parse_json.rs`
- [ ] Add per-group override tests to `parse_json.rs`

---

## Project Constraints (from CLAUDE.md)

| Directive | Impact on Phase 3 |
|-----------|-------------------|
| Language: Rust edition 2024 | All new code in `crates/fast-staged/src/` |
| Async runtime: tokio | Not used in config parsing (all sync I/O in config module) |
| Config parsing: serde + toml + serde_json | Use these only; no alternative TOML/JSON parsers |
| Error handling: thiserror `AppError` enum | All config errors via `AppError::ConfigInvalid(String)` |
| Naming: snake_case files, PascalCase structs | Follow existing conventions |
| `rustfmt` enforced in CI | Format all new code |
| `clippy` enforced in CI | No clippy warnings in new code |
| GSD workflow enforcement | Use `/gsd:execute-phase` — do not make direct edits outside GSD |

---

## Sources

### Primary (HIGH confidence)

- Direct code inspection of `crates/fast-staged/src/config/` — all 6 files read and analyzed
- `toml` crate source: `toml::Table` is `IndexMap<String, Value>` — insertion order preserved
- `serde_json` crate docs: `Map<String, Value>` uses `IndexMap` when `preserve_order` feature is enabled
- Phase 2 decisions in `03-CONTEXT.md` — locked decisions D-01 through D-09

### Secondary (MEDIUM confidence)

- Rust `std::collections::HashMap` documentation: unordered (hash-based), insertion order not preserved — standard knowledge
- `serde` flatten behavior with `IndexMap`-backed maps: preserves key insertion order per serde docs

### Tertiary (LOW confidence)

None — all claims are grounded in code inspection or standard Rust documentation.

---

## Metadata

**Confidence breakdown:**
- Ordering bug diagnosis: HIGH — confirmed by direct code inspection (`named_group_names.sort()` present in both parsers)
- Fix approach (toml::Table, serde_json Map + preserve_order): HIGH — standard Rust pattern
- Test coverage gaps: HIGH — confirmed by running `cargo test` (40 tests, none test multi-group ordering)
- serde_json preserve_order default-off: HIGH — documented behavior
- No new dependencies needed: HIGH — all Phase 3 types pre-built in Phase 2

**Research date:** 2026-04-09
**Valid until:** 2026-05-09 (stable libraries, low churn)
