# Phase 3: Advanced Config - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-09
**Phase:** 03-advanced-config
**Areas discussed:** Group ordering, Group-level validation, Config-time vs runtime boundaries, Multi-group interaction semantics
**Mode:** auto (all decisions auto-selected)

---

## Group Ordering

| Option | Description | Selected |
|--------|-------------|----------|
| Declaration order | Preserve config file order, track insertion position | ✓ |
| Alphabetical | Sort groups by name | |
| Explicit priority field | Add `priority` field to group config | |

**User's choice:** [auto] Declaration order (recommended default)
**Notes:** TOML HashMap doesn't preserve order — parser needs explicit tracking. JSON preserves key order via serde_json.

---

## Group-Level Validation

| Option | Description | Selected |
|--------|-------------|----------|
| Strict parse-time | Validate non-empty patterns, valid timeout, valid execution_order at parse time | ✓ |
| Lazy runtime | Validate only when group is about to execute | |
| Hybrid | Validate syntax at parse time, semantics at runtime | |

**User's choice:** [auto] Strict parse-time (recommended default)
**Notes:** Fail fast with clear error messages. Already partially in place from Phase 2's deny_unknown_fields.

---

## Config-Time vs Runtime Boundaries

| Option | Description | Selected |
|--------|-------------|----------|
| Config parsing only | Phase 3 handles parsing/validation, Phase 5 handles execution | ✓ |
| Include basic execution | Add simple sequential execution to prove config works | |

**User's choice:** [auto] Config parsing only (recommended default)
**Notes:** Clean separation of concerns. Phase 3 tests verify parsed Config struct values.

---

## Multi-Group Interaction Semantics

| Option | Description | Selected |
|--------|-------------|----------|
| Independent groups | Per-group continue_on_error controls within-group; global controls between-group | ✓ |
| Cascading failure | Any group failure stops all subsequent groups | |
| Isolated | Each group always runs regardless of others | |

**User's choice:** [auto] Independent groups (recommended default)
**Notes:** Per-group continue_on_error is scoped to commands within that group. Global continue_on_error controls whether execution continues to next group after a group failure.

---

## Claude's Discretion

- Test count and organization
- Whether to refactor Phase 2 code or add incremental validation
- Error message wording

## Deferred Ideas

None
