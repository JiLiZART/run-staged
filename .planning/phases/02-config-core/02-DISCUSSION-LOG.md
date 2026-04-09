# Phase 2: Config Core - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-09
**Phase:** 02-config-core
**Areas discussed:** Config struct shape, Directory traversal, Config file priority

---

## Config Struct Shape

### Q1: How should the Config struct model glob-to-command mappings?

| Option | Description | Selected |
|--------|-------------|----------|
| Flat map | HashMap<String, Vec<String>> — simple, Phase 3 wraps later | |
| Single default group | Config always has groups, Phase 2 creates implicit "default" | |
| Pre-build Phase 3 | Full Group struct with all Phase 3 fields as Option<T> | ✓ |

**User's choice:** Pre-build Phase 3
**Notes:** Config should parse all flat params as root Group struct with Vec<Groups> inside. Groups can redefine default behavior. Works like default_param -> root_param -> group_param.

### Q2: How should patterns be structured in TOML?

| Option | Description | Selected |
|--------|-------------|----------|
| Top-level keys | Glob patterns as top-level TOML keys | |
| Under [patterns] table | Patterns nested under [patterns] section | ✓ |

**User's choice:** Under [patterns] table
**Notes:** User defined three valid pattern definition styles: (1) root `[patterns]` for implicit group, (2) `[group1.patterns]` sub-table for named groups, (3) `[group1]` with inline `patterns = { ... }`.

### Q3: Should commands accept both string and array?

| Option | Description | Selected |
|--------|-------------|----------|
| Both (string + array) | Serde untagged enum, matches lint-staged | |
| Array only | Always require array syntax | ✓ |

**User's choice:** Array only

---

## Directory Traversal

### Q1: Should config discovery walk up parent directories?

| Option | Description | Selected |
|--------|-------------|----------|
| CWD only | Only look in current working directory | |
| Walk up to git root | Search CWD then parents up to .git | ✓ |
| Walk up to filesystem root | Search all the way up | |

**User's choice:** Walk up to git root

### Q2: Walk order across directories?

| Option | Description | Selected |
|--------|-------------|----------|
| All formats per dir | Check all 5 formats in each dir before moving up | ✓ |
| Format priority across dirs | Check one format across all dirs first | |

**User's choice:** All formats per dir

---

## Config File Priority

### Q1: First match wins or merge multiple configs?

| Option | Description | Selected |
|--------|-------------|----------|
| First match wins | Stop at first config found | ✓ |
| Merge configs | Deep merge closer overrides farther | |

**User's choice:** First match wins

### Q2: How should --config override interact with discovery?

| Option | Description | Selected |
|--------|-------------|----------|
| Skip discovery entirely | Use specified file directly, error if not found | ✓ |
| Override priority, keep fallback | Check specified first, fall back to discovery | |

**User's choice:** Skip discovery entirely

### Q3: Should JSON format mirror TOML?

| Option | Description | Selected |
|--------|-------------|----------|
| Mirror TOML | Same structure across formats | ✓ |
| Flat lint-staged style | Lint-staged's flat glob->commands format | |

**User's choice:** Mirror TOML
**Notes:** Also detect flat lint-staged style JSON (glob keys at root) and parse as single implicit group.

---

## Claude's Discretion

- Validation strictness (strict vs lenient serde, unknown field handling)
- Error message detail (line numbers, suggestions)

## Deferred Ideas

None — discussion stayed within phase scope
