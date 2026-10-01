## Why

Users migrating from lint-staged should be able to run run-staged against their existing config without rewriting it. This is roadmap Phase 9 (CFG-10, CFG-11).

## What Changes

- Discover lint-staged JSON/YAML configs (`.lintstagedrc`, `.lintstagedrc.json`, `.lintstagedrc.yaml`, `.lintstagedrc.yml`, `package.json["lint-staged"]`) at lower priority than run-staged configs.
- Accept lint-staged command values as a string or an array of strings.
- Detect JavaScript lint-staged configs and fail with a clear error suggesting a static config.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `config-loading`: adds lint-staged config discovery and JS-config rejection.

## Impact

- Code: `crates/config/src/discover.rs` (candidate list), `crates/config/src/parse_json.rs` (string command values), a new YAML parse path in `crates/config`.
- Dependencies: one YAML deserializer. `serde_yaml` is unmaintained — pick a maintained serde-compatible crate and record the choice in the implementation commit.
- Out of scope: JS function configs (project-level decision), lint-staged `--diff`/`--relative` CLI flags.
