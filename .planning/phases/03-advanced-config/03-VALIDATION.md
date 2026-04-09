---
phase: 03
slug: advanced-config
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-09
---

# Phase 03 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | `crates/fast-staged/Cargo.toml` |
| **Quick run command** | `cargo test -p fast-staged config` |
| **Full suite command** | `cargo test -p fast-staged` |
| **Estimated runtime** | ~2 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p fast-staged config`
- **After every plan wave:** Run `cargo test -p fast-staged`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | CFG-06 | unit | `cargo test -p fast-staged config::parse_toml` | ✅ | ⬜ pending |
| 03-01-01 | 01 | 1 | CFG-07 | unit | `cargo test -p fast-staged config::defaults` | ✅ | ⬜ pending |
| 03-01-01 | 01 | 1 | CFG-08 | unit | `cargo test -p fast-staged config::parse_toml` | ✅ | ⬜ pending |
| 03-01-01 | 01 | 1 | CFG-09 | unit | `cargo test -p fast-staged config::parse_toml` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. Phase 2 already has 40 passing tests in the config module with the cargo test framework fully configured.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Runtime execution order | CFG-06 | Execution engine not built until Phase 5 | Verify parsed Config.groups order matches declaration order |
| Runtime timeout enforcement | CFG-07 | Timeout execution not built until Phase 5 | Verify parsed Group.timeout has correct Duration value |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
