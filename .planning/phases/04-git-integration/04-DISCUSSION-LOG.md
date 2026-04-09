# Phase 4: Git Integration - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-10
**Phase:** 04-git-integration
**Areas discussed:** gix API approach, Patch backup strategy, Restore behavior, Module structure
**Mode:** Auto (all decisions auto-selected from recommended defaults)

---

## gix API Approach

| Option | Description | Selected |
|--------|-------------|----------|
| gix::discover from CWD | Mirrors config discovery upward walk, finds repo automatically | ✓ |
| gix::open with explicit path | Requires knowing repo root upfront | |
| Pass repo path from config discovery | Reuses .git/ path already found | |

**User's choice:** [auto] gix::discover from CWD
**Notes:** Recommended default — consistent with how config discovery works

| Option | Description | Selected |
|--------|-------------|----------|
| Read index directly, filter staged entries | Access repo.index(), filter by stage flags | ✓ |
| Compare index vs HEAD tree | Diff-based approach to find what changed | |

**User's choice:** [auto] Read index directly, filter staged entries
**Notes:** Direct index access is simpler and more aligned with what lint-staged does

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal gix features | Only index + diff features enabled | ✓ |
| Full gix features | Enable everything for convenience | |

**User's choice:** [auto] Minimal gix features
**Notes:** Project optimizes for binary size (LTO, opt-level "z")

| Option | Description | Selected |
|--------|-------------|----------|
| Keep .git/ scan in config discovery | Separation of concerns, Phase 2 decision | ✓ |
| Replace with gix everywhere | Unified approach | |

**User's choice:** [auto] Keep .git/ scan in config discovery
**Notes:** Phase 2 explicitly chose this — no reason to change

---

## Patch Backup Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| gix diff API for patch generation | No shell-out, consistent with project constraint | ✓ |
| Shell out to git diff --cached | Simpler but violates no-shell-out constraint | |

**User's choice:** [auto] gix diff API for patch generation
**Notes:** Project constraint: "gix crate (no shelling out to git CLI)"

| Option | Description | Selected |
|--------|-------------|----------|
| Overwrite existing patch | New run takes priority, stale patch is useless | ✓ |
| Error if patch exists | Safer but blocks legitimate re-runs | |
| Prompt user | Not possible in pre-commit hook context | |

**User's choice:** [auto] Overwrite existing patch
**Notes:** Simpler and more robust for pre-commit hook use case

---

## Restore Behavior

| Option | Description | Selected |
|--------|-------------|----------|
| lib.rs orchestrates backup/restore | Git module provides primitives, caller decides | ✓ |
| Git module auto-restores on drop | RAII pattern, harder to control | |

**User's choice:** [auto] lib.rs orchestrates
**Notes:** Keeps git module focused, orchestration logic belongs in run()

| Option | Description | Selected |
|--------|-------------|----------|
| Restore index only, keep working tree | Matches lint-staged behavior | ✓ |
| Restore both index and working tree | Full rollback, discards formatter output | |

**User's choice:** [auto] Restore index only, keep working tree
**Notes:** Formatters write to working tree — those changes should persist

---

## Module Structure

| Option | Description | Selected |
|--------|-------------|----------|
| src/git.rs single file | Simple, follows error.rs pattern | ✓ |
| src/git/ directory from start | Anticipates complexity | |

**User's choice:** [auto] src/git.rs single file
**Notes:** Start simple, split later if needed

| Option | Description | Selected |
|--------|-------------|----------|
| GitContext struct with methods | Encapsulates repo handle, clean API | ✓ |
| Free functions | Simpler but no shared state | |

**User's choice:** [auto] GitContext struct with methods
**Notes:** Need to hold the gix repo handle across multiple operations

---

## Claude's Discretion

- Exact gix feature flags in Cargo.toml
- gix API details for index reading (version-dependent)
- Error message wording
- Test strategy (real git repos vs mocked data)

## Deferred Ideas

None
