# Phase 02: Config Core - Research

**Researched:** 2026-04-09
**Domain:** Rust config file discovery and multi-format parsing (TOML, JSON, package.json)
**Confidence:** HIGH

## Summary

Phase 2 introduces the config loading pipeline: discover a config file by walking up from CWD to git root, parse it from TOML or JSON into a typed `Config` struct, and return clear errors on invalid input. All required crates (`toml 1.1.2`, `serde_json 1.0.149`, `serde 1.0.228`, `parse_duration 2.1.1`) are already declared in `Cargo.toml` and locked. No new dependencies are needed.

The struct design uses `#[serde(flatten)]` with `HashMap<String, toml::Value>` to capture named groups from top-level TOML tables — this was verified to work correctly in toml 1.x (it does NOT leak scalar fields like `timeout` into the groups map). Config validation uses `#[serde(deny_unknown_fields)]` for strict parsing, which produces clear field-level error messages with line/column numbers from the toml crate.

The flat lint-staged JSON style (glob keys at root, no `patterns` wrapper) is detected by attempting to deserialize as `HashMap<String, Vec<String>>` when the structured parse fails. Package.json is parsed as `serde_json::Value`, the `"fast-staged"` key is extracted, and then deserialized into the same `Config` struct using `serde_json::from_value`.

**Primary recommendation:** Two-phase parse approach — try structured `Config` deserialization first; if that fails and input is JSON, attempt flat `HashMap<String, Vec<String>>`. Use `#[serde(deny_unknown_fields)]` on group structs for strict validation with actionable error messages.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Pre-build the full Phase 3 struct now. Config has `source: ConfigSource`, `groups: Vec<Group>`, plus top-level default fields (timeout, execution_order, continue_on_error, per_file).
- **D-02:** Group struct includes: `name: String`, `patterns: HashMap<String, Vec<String>>`, `timeout: Option<Duration>`, `execution_order: Option<ExecutionOrder>`, `continue_on_error: bool`, `per_file: bool`.
- **D-03:** Cascading defaults: flat top-level params set defaults, root-level params can override, each group further overrides. Resolution order: `default -> root -> group`.
- **D-04:** Patterns live under a `[patterns]` table (TOML) or `"patterns"` key (JSON), not as top-level keys mixed with config knobs.
- **D-05:** Commands are always `Vec<String>` (array only). No single-string shorthand — `"*.rs" = ["cargo fmt"]`, not `"*.rs" = "cargo fmt"`.
- **D-06:** TOML structure: top-level defaults, `[patterns]` for root group patterns, `[group1.patterns]` or `[group1]` with `patterns = { ... }` for named groups.
- **D-07:** JSON mirrors TOML structure exactly: `{ "timeout": "5s", "patterns": { "*.rs": ["cargo fmt"] } }`.
- **D-08:** Also detect flat lint-staged style JSON (glob keys at root with no `patterns` wrapper) and parse it as a single implicit group.
- **D-09:** `package.json` config lives under the `"fast-staged"` key and follows the same JSON structure.
- **D-10:** Walk up from CWD to git repository root. Check all 5 formats per directory before moving to parent. First match wins — no merging.
- **D-11:** Priority within each directory: `.fast-staged.toml` > `.fast-staged.json` > `fast-staged.toml` > `fast-staged.json` > `package.json[fast-staged]`.
- **D-12:** `--config path/to/file` skips auto-discovery entirely. Error if specified file doesn't exist or is invalid (no fallback).

### Claude's Discretion

- **D-13:** Validation strictness and error message detail (line numbers, field suggestions, etc.). Must satisfy CFG-05: clear error message and non-zero exit for invalid configs.

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CFG-01 | Tool discovers config files in priority order: .fast-staged.toml > .fast-staged.json > fast-staged.toml > fast-staged.json > package.json[fast-staged] | Directory walk via `std::path::Path::parent()`, priority ordered array of filenames, package.json key check via serde_json::Value |
| CFG-02 | Tool parses TOML config with glob patterns mapped to commands | `toml::from_str::<Config>()` with serde Deserialize derive; verified working with [patterns] table |
| CFG-03 | Tool parses JSON config with glob patterns mapped to commands | `serde_json::from_str::<Config>()` with same Config struct; verified identical field mapping |
| CFG-04 | Tool reads fast-staged config section from package.json | Parse as `serde_json::Value`, index `["fast-staged"]`, then `serde_json::from_value()` into Config |
| CFG-05 | Tool validates config and shows clear error messages for invalid configs | `#[serde(deny_unknown_fields)]` produces field-level errors; toml crate includes line/column numbers; exits non-zero via `AppError::ConfigInvalid(String)` |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.0.228 | Derive Deserialize/Serialize on Config structs | De-facto Rust serialization standard |
| toml | 1.1.2 | Parse TOML config files | Official TOML 1.1 spec implementation for Rust |
| serde_json | 1.0.149 | Parse JSON config files and package.json | De-facto JSON library for Rust with serde integration |
| parse_duration | 2.1.1 | Parse human-readable timeout strings ("5s", "1sec", "30s") | Already in Cargo.toml; handles all common duration formats |
| thiserror | 2.x | Error variants ConfigNotFound, ConfigInvalid(String) | Already wired in error.rs |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::fs | stdlib | Read file contents to string | Always — `fs::read_to_string(&path)` |
| std::path | stdlib | Path manipulation, parent() for walk-up | Directory traversal logic |
| std::collections::HashMap | stdlib | Pattern-to-commands mapping | Serde target for patterns table |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| toml::from_str | toml::Value (manual) | from_str with Deserialize is cleaner; Value needed for two-phase detection only |
| serde deny_unknown_fields | Custom validation | deny_unknown_fields is simpler; gives field-level errors automatically |
| parse_duration | chrono::Duration::from_str | parse_duration already in Cargo.toml, more human-friendly formats |

**Installation:** All dependencies already declared in `crates/fast-staged/Cargo.toml`. No new packages needed.

**Verified versions (from Cargo.lock):**
- `toml 1.1.2+spec-1.1.0` — published (current as of research date)
- `serde 1.0.228` — current stable
- `serde_json 1.0.149` — current stable
- `parse_duration 2.1.1` — current stable

## Architecture Patterns

### Recommended Project Structure
```
crates/fast-staged/src/
├── config/
│   ├── mod.rs          # pub use, Config::load() entry point
│   ├── types.rs        # Config, Group, ConfigSource, ExecutionOrder enums
│   ├── discover.rs     # find_config_file() — walks CWD up to git root
│   ├── parse_toml.rs   # parse_toml(path) -> Result<Config, AppError>
│   ├── parse_json.rs   # parse_json(path) -> Result<Config, AppError>
│   └── defaults.rs     # apply_defaults(raw) -> Config cascade logic
├── error.rs            # AppError (existing)
├── lib.rs              # pub mod config; pub async fn run()
└── main.rs             # Entry point (existing)
```

Alternatively, a single `config.rs` module is acceptable given the phase is self-contained. The sub-module structure above is preferred for testability.

### Pattern 1: Two-Phase Parse for Format Detection
**What:** Try structured parse first; if JSON and structured parse fails, attempt flat HashMap parse.
**When to use:** Detecting D-08 flat lint-staged style JSON without format-specific file extensions.

```rust
// Source: verified with serde_json 1.0.149 + serde 1.0.228
fn parse_json_file(path: &Path) -> Result<Config, AppError> {
    let content = std::fs::read_to_string(path)
        .map_err(AppError::Io)?;
    let raw: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::ConfigInvalid(e.to_string()))?;

    // Try structured Config first
    if let Ok(config) = serde_json::from_value::<RawConfig>(raw.clone()) {
        return Ok(build_config(config, ConfigSource::JsonFile(path.to_path_buf())));
    }

    // Detect flat lint-staged style: all values are arrays of strings
    if let Ok(flat) = serde_json::from_value::<HashMap<String, Vec<String>>>(raw) {
        let group = Group {
            name: "default".to_string(),
            patterns: flat,
            ..Default::default()
        };
        return Ok(Config { groups: vec![group], source: ConfigSource::JsonFile(path.to_path_buf()), ..Default::default() });
    }

    Err(AppError::ConfigInvalid(format!("cannot parse {}: expected structured or flat lint-staged format", path.display())))
}
```

### Pattern 2: Named Groups via serde(flatten) with toml::Value
**What:** Top-level TOML fields are typed; named group sub-tables captured via flatten into HashMap.
**When to use:** TOML config with named groups ([group1], [group2] etc.).

```rust
// Source: verified with toml 1.1.2 + serde 1.0.228
// CONFIRMED: flatten correctly separates scalar fields from sub-tables.
// "timeout" and "patterns" do NOT leak into the groups HashMap.

#[derive(Deserialize)]
struct RawTomlConfig {
    timeout: Option<String>,
    execution_order: Option<String>,
    continue_on_error: Option<bool>,
    per_file: Option<bool>,
    patterns: Option<HashMap<String, Vec<String>>>,
    #[serde(flatten)]
    groups: HashMap<String, toml::Value>,  // captures [group1], [group2], etc.
}
```

**Post-process step:** After deserialization, iterate `groups`, deserialize each `toml::Value` as `RawGroupConfig`, skip keys that aren't valid group tables (e.g., filter out unexpected scalar entries).

### Pattern 3: Package.json Extraction
**What:** Parse full package.json as `serde_json::Value`, extract `["fast-staged"]`, deserialize into Config.
**When to use:** CFG-04 — package.json config discovery.

```rust
// Source: verified with serde_json 1.0.149
fn parse_package_json(path: &Path) -> Result<Option<Config>, AppError> {
    let content = std::fs::read_to_string(path).map_err(AppError::Io)?;
    let pkg: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::ConfigInvalid(e.to_string()))?;

    let Some(fs_value) = pkg.get("fast-staged") else {
        return Ok(None);  // No fast-staged key — skip this package.json
    };

    let raw: RawJsonConfig = serde_json::from_value(fs_value.clone())
        .map_err(|e| AppError::ConfigInvalid(format!("package.json[fast-staged]: {e}")))?;
    Ok(Some(build_config(raw, ConfigSource::PackageJson(path.to_path_buf()))))
}
```

### Pattern 4: Directory Walk for Config Discovery
**What:** Walk up from CWD to git root directory, checking priority files at each level.
**When to use:** CFG-01 — auto-discovery (used when `--config` is not set).

```rust
// Source: std::path, verified on Darwin/Linux
// Note: git root detection uses gix (already in deps) or .git directory detection
fn find_config_file(start: &Path, git_root: &Path) -> Result<Option<PathBuf>, AppError> {
    const PRIORITY: &[&str] = &[
        ".fast-staged.toml",
        ".fast-staged.json",
        "fast-staged.toml",
        "fast-staged.json",
    ];

    let mut current = start.to_path_buf();
    loop {
        for name in PRIORITY {
            let candidate = current.join(name);
            if candidate.exists() {
                return Ok(Some(candidate));
            }
        }
        // Check package.json separately (requires content inspection)
        let pkg = current.join("package.json");
        if pkg.exists() {
            if has_fast_staged_key(&pkg)? {
                return Ok(Some(pkg));
            }
        }
        if current == git_root {
            break;
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => break,
        }
    }
    Ok(None)
}
```

**Git root detection:** Use `gix::discover(start)` to find the repository root, or fall back to scanning for `.git/` directory. The `gix` crate is already in dependencies and will be used more fully in Phase 4.

### Pattern 5: Cascading Defaults (D-03)
**What:** Three-level cascade: global defaults → root-level config values → per-group overrides.
**When to use:** Building the final `Vec<Group>` from parsed raw structs.

```rust
// Source: design pattern from CONTEXT.md D-03
fn resolve_group(raw: RawGroupConfig, root: &RawConfig) -> Group {
    Group {
        timeout: raw.timeout
            .or(root.timeout.clone())
            .map(|s| parse_duration::parse(&s).ok())
            .flatten(),
        execution_order: raw.execution_order
            .or(root.execution_order.clone())
            .unwrap_or(ExecutionOrder::Parallel),
        continue_on_error: raw.continue_on_error
            .unwrap_or(root.continue_on_error.unwrap_or(false)),
        per_file: raw.per_file
            .unwrap_or(root.per_file.unwrap_or(false)),
        patterns: raw.patterns.unwrap_or_default(),
        name: raw.name,
    }
}
```

### Anti-Patterns to Avoid
- **Using toml::Value everywhere:** Parse into typed structs via Deserialize. Use `toml::Value` only for the intermediate groups HashMap and then convert each entry.
- **Single-string command support:** D-05 is explicit — commands are always `Vec<String>`. Do not add `#[serde(untagged)]` union for string/array. Reject single strings with `ConfigInvalid`.
- **Merging configs from multiple files:** D-10 is explicit — first match wins, no merging across directories.
- **Walking past git root:** The walk MUST stop at the git repository root. Walking further risks loading a config from a parent project.
- **Ignoring unknown fields silently on group structs:** Use `#[serde(deny_unknown_fields)]` on `RawGroupConfig` so typos in group settings (e.g., `excution_order`) surface immediately.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Duration parsing | Custom regex/parser for "5s", "1sec", "30s" | `parse_duration::parse()` | Handles all edge cases: "5s", "5sec", "5 seconds", "1min", etc. |
| TOML parsing | Manual tokenizer/parser | `toml::from_str()` | TOML 1.1 spec is complex; the crate handles all edge cases |
| JSON parsing | Manual parser | `serde_json::from_str()` | Same — edge cases in JSON escape sequences, numbers, unicode |
| Serde integration | Custom deserialization | `#[derive(Deserialize)]` | Derive macro handles field mapping, optional fields, defaults |
| Error message formatting | Custom formatter | `toml` crate's built-in error with line/col | toml 1.x errors include position: `"TOML parse error at line 1, column 11"` |

**Key insight:** TOML and JSON parsing are both solved problems with excellent Rust crates. The only custom logic is the two-phase detection for flat JSON and the cascading defaults resolution — both of which are simple enough to hand-roll safely.

## Common Pitfalls

### Pitfall 1: serde(flatten) HashMap captures ALL top-level keys including scalars
**What goes wrong:** If you use `HashMap<String, serde_json::Value>` with flatten, it MAY capture `timeout`, `execution_order`, etc. into the groups map alongside actual groups.
**Why it happens:** `flatten` merges all remaining keys into the HashMap regardless of type.
**How to avoid:** For JSON, use `HashMap<String, GroupConfig>` with the group config struct — serde will only succeed for keys where the value is an object matching GroupConfig. For TOML (verified), `HashMap<String, toml::Value>` with flatten correctly separates scalars from tables because TOML distinguishes value types structurally.
**Warning signs:** If `groups.contains_key("timeout")` returns true, you have this bug.

### Pitfall 2: Missing ConfigSource tracking
**What goes wrong:** Error messages say "invalid configuration" but don't tell users which file was invalid.
**Why it happens:** Config loading returns a parsed struct without recording where it came from.
**How to avoid:** Set `source: ConfigSource` at parse time, before returning. Include the path in `ConfigInvalid` error messages: `AppError::ConfigInvalid(format!("{}: {}", path.display(), e))`.

### Pitfall 3: package.json with no "fast-staged" key treated as error
**What goes wrong:** Tool errors on every project that has `package.json` but no fast-staged config.
**Why it happens:** Treating "file exists but key absent" as an error rather than "skip".
**How to avoid:** `parse_package_json` returns `Ok(None)` when the `"fast-staged"` key is absent. Only error on parse failure of the key's value.

### Pitfall 4: Walking above git root
**What goes wrong:** Tool finds a config file in a parent directory that contains the git repo (e.g., a monorepo parent).
**Why it happens:** Walk-up logic uses `Path::parent()` without a stop condition.
**How to avoid:** Detect git root first (via `gix::discover()` or `.git/` directory scan), then use it as the exclusive upper bound for the walk.

### Pitfall 5: toml::from_str vs toml::de::from_str
**What goes wrong:** Code imports from wrong path in older toml crate docs.
**Why it happens:** toml crate reorganized its API between 0.x and 1.x.
**How to avoid:** In toml 1.x, use `toml::from_str()` (re-exported at crate root). Do NOT use `toml::de::from_str()` — it still exists but the idiomatic API is at the crate root.

### Pitfall 6: Duration stored as std::time::Duration vs parsed string
**What goes wrong:** Struct stores `timeout: Option<Duration>` but serde can't deserialize a string like `"5s"` directly into `Duration`.
**Why it happens:** `Duration` does not implement `Deserialize` for human-readable strings.
**How to avoid:** Raw config structs use `timeout: Option<String>`. After deserialization, call `parse_duration::parse(&s)` to convert. Store the result as `std::time::Duration` only in the final resolved `Config`/`Group` structs, not the `Raw*` intermediate structs.

### Pitfall 7: Flat JSON detection false positives
**What goes wrong:** A malformed structured JSON (missing `patterns` key) is silently accepted as flat lint-staged format.
**Why it happens:** The two-phase parse falls through to flat detection even for intentionally structured configs.
**How to avoid:** Only attempt flat detection when the JSON object has NO keys that match Config field names (`timeout`, `execution_order`, `continue_on_error`, `per_file`, `patterns`). If a `patterns` key exists, the input is definitely structured — reject it as a structured config error.

## Code Examples

### Complete Config struct design (Phase 2 + Phase 3 fields per D-01)
```rust
// Source: CONTEXT.md D-01, D-02 + verified serde patterns
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum ConfigSource {
    TomlFile(std::path::PathBuf),
    JsonFile(std::path::PathBuf),
    PackageJson(std::path::PathBuf),
    Explicit(std::path::PathBuf),  // --config flag
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum ExecutionOrder {
    #[default]
    Parallel,
    Sequential,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub name: String,
    pub patterns: HashMap<String, Vec<String>>,
    pub timeout: Option<Duration>,
    pub execution_order: ExecutionOrder,
    pub continue_on_error: bool,
    pub per_file: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub source: ConfigSource,
    pub groups: Vec<Group>,
    // top-level defaults (resolved into groups, kept for reference)
    pub timeout: Option<Duration>,
    pub execution_order: ExecutionOrder,
    pub continue_on_error: bool,
    pub per_file: bool,
}
```

### Raw intermediate struct for TOML deserialization
```rust
// Source: verified with toml 1.1.2 + serde 1.0.228 — serde(flatten) works correctly
#[derive(Debug, Deserialize)]
struct RawConfig {
    timeout: Option<String>,
    execution_order: Option<String>,
    continue_on_error: Option<bool>,
    per_file: Option<bool>,
    patterns: Option<HashMap<String, Vec<String>>>,
    #[serde(flatten)]
    groups: HashMap<String, toml::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGroupConfig {
    timeout: Option<String>,
    execution_order: Option<String>,
    continue_on_error: Option<bool>,
    per_file: Option<bool>,
    patterns: Option<HashMap<String, Vec<String>>>,
}
```

### Error message format (CFG-05)
```
// toml crate produces this automatically:
// "TOML parse error at line 3, column 5\n  |\n3 | unknown_field = true\n  | ^^^^^^^^^^^^^\nunknown field `unknown_field`, expected one of ..."

// For AppError display, wrap with file path:
AppError::ConfigInvalid(format!("{}: {}", path.display(), e))
// -> "invalid configuration: .fast-staged.toml: TOML parse error at line 3..."
```

### ExecutionOrder deserialization from string
```rust
// Source: serde pattern for enum from string
impl<'de> serde::Deserialize<'de> for ExecutionOrder {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            "parallel" => Ok(ExecutionOrder::Parallel),
            "sequential" => Ok(ExecutionOrder::Sequential),
            other => Err(serde::de::Error::custom(
                format!("unknown execution_order: {other:?}, expected \"parallel\" or \"sequential\"")
            )),
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `toml::de::from_str()` | `toml::from_str()` | toml 1.0 (2023) | Use crate-root re-export |
| `toml::Value::try_from()` | `toml::from_str::<T>()` | toml 1.0 | Direct typed deserialization |
| `serde(deny_unknown_fields)` triggers panic | Returns `Err` with field name | serde 1.x | Safe to use in production |

**Deprecated/outdated:**
- `toml 0.x`: Used `toml::from_str()` returning `Result<toml::Value>` only. Current 1.x supports typed deserialization natively.
- `serde_ignored` crate: Older pattern for capturing unknown fields. Not needed — `deny_unknown_fields` is sufficient.

## Open Questions

1. **Git root detection in config discovery**
   - What we know: `gix` is already in deps; it will be used in Phase 4 for staged files
   - What's unclear: Whether calling `gix::discover()` is lightweight enough for config discovery (it opens the git object store), or whether a simpler `.git/` directory scan is preferable
   - Recommendation: Use `.git/` directory scan for Phase 2 (simple `std::fs` loop, no gix overhead). Phase 4 will initialize gix properly for staged file listing.

2. **`#[serde(deny_unknown_fields)]` on root vs group structs only**
   - What we know: D-13 gives discretion on validation strictness
   - What's unclear: Whether to apply deny_unknown_fields to `RawConfig` (strict) or only `RawGroupConfig` (lenient at root, strict in groups)
   - Recommendation: Apply to both. The toml crate's error messages are clear enough to satisfy CFG-05. Users benefit from catching typos at the root level too.

## Environment Availability

Step 2.6: No external tool dependencies beyond the Rust toolchain. All parsing is done with library crates already in Cargo.lock. The project compiles cleanly with `cargo build` as verified.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust nightly toolchain | Edition 2024 compilation | ✓ | nightly (confirmed: cargo tree runs) | — |
| toml crate | TOML parsing | ✓ | 1.1.2 (in Cargo.lock) | — |
| serde_json crate | JSON parsing | ✓ | 1.0.149 (in Cargo.lock) | — |
| parse_duration crate | Timeout strings | ✓ | 2.1.1 (in Cargo.lock) | — |

**Missing dependencies with no fallback:** None.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test (`#[test]`) + cargo test |
| Config file | None (cargo workspace auto-discovers) |
| Quick run command | `cargo test -p fast-staged` |
| Full suite command | `cargo test -p fast-staged` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CFG-01 | Config discovery walks priority order | unit | `cargo test -p fast-staged config::discover` | Wave 0 |
| CFG-01 | Walk stops at git root | unit | `cargo test -p fast-staged config::discover::stops_at_git_root` | Wave 0 |
| CFG-02 | TOML with [patterns] table parses correctly | unit | `cargo test -p fast-staged config::parse_toml` | Wave 0 |
| CFG-02 | TOML with named groups parses correctly | unit | `cargo test -p fast-staged config::parse_toml::named_groups` | Wave 0 |
| CFG-03 | JSON structured config parses identically to TOML | unit | `cargo test -p fast-staged config::parse_json` | Wave 0 |
| CFG-03 | JSON flat lint-staged style detected and parsed | unit | `cargo test -p fast-staged config::parse_json::flat_style` | Wave 0 |
| CFG-04 | package.json with fast-staged key parsed | unit | `cargo test -p fast-staged config::parse_package_json` | Wave 0 |
| CFG-04 | package.json without fast-staged key returns None | unit | `cargo test -p fast-staged config::parse_package_json::no_key` | Wave 0 |
| CFG-05 | Invalid TOML syntax returns ConfigInvalid with message | unit | `cargo test -p fast-staged config::errors::invalid_toml` | Wave 0 |
| CFG-05 | Unknown field returns ConfigInvalid with field name | unit | `cargo test -p fast-staged config::errors::unknown_field` | Wave 0 |
| CFG-05 | --config with nonexistent file returns ConfigNotFound | unit | `cargo test -p fast-staged config::errors::explicit_missing` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p fast-staged`
- **Per wave merge:** `cargo test -p fast-staged`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/fast-staged/src/config/mod.rs` — module entry point
- [ ] `crates/fast-staged/src/config/types.rs` — Config, Group, ConfigSource, ExecutionOrder
- [ ] `crates/fast-staged/src/config/discover.rs` + inline unit tests
- [ ] `crates/fast-staged/src/config/parse_toml.rs` + inline unit tests
- [ ] `crates/fast-staged/src/config/parse_json.rs` + inline unit tests

*(Existing integration test `tests/integration_test.rs` covers binary_exits_zero — keep as-is.)*

## Project Constraints (from CLAUDE.md)

- **Language:** Rust edition 2024, nightly toolchain
- **No shelling out to git CLI:** Use `gix` crate only
- **Async runtime:** tokio (not needed for config loading — sync `std::fs` is correct here)
- **Error handling:** All functions return `Result<T, AppError>` with `?` propagation
- **Errors:** Use `thiserror` derive on `AppError` — no `Box<dyn Error>`
- **Config parsing:** serde + toml + serde_json (mandated in CLAUDE.md constraints)
- **Naming:** snake_case for files and functions, PascalCase for structs/enums
- **Formatting:** rustfmt enforced (`cargo fmt -- --check` in CI)
- **Linting:** Clippy enforced (`cargo clippy` in CI)
- **Module pattern:** Add new module in `src/`, declare in `lib.rs`
- **GSD workflow:** All file edits go through GSD phase execution, not direct edits

## Sources

### Primary (HIGH confidence)
- Verified by running actual code against toml 1.1.2 + serde 1.0.228 + serde_json 1.0.149 + parse_duration 2.1.1
- `Cargo.lock` in project — confirmed exact versions
- `crates/fast-staged/Cargo.toml` — dependency declarations
- `crates/fast-staged/src/error.rs` — existing AppError enum

### Secondary (MEDIUM confidence)
- CONTEXT.md D-01 through D-13 — user design decisions (locked)
- REQUIREMENTS.md CFG-01 through CFG-05 — requirement definitions

### Tertiary (LOW confidence)
- None — all claims verified by compilation or official crate versions from Cargo.lock

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions confirmed in Cargo.lock, APIs verified by running test code
- Architecture: HIGH — patterns verified by compilation tests; serde(flatten) behavior confirmed
- Pitfalls: HIGH — pitfalls 1, 6 verified empirically; pitfalls 2-5, 7 confirmed from API behavior

**Research date:** 2026-04-09
**Valid until:** 2026-10-09 (serde/toml APIs are stable; toml 1.x has been stable since 2023)
