---
phase: 2
slug: config-core
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-09
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | `crates/fast-staged/Cargo.toml` (dev-dependencies section) |
| **Quick run command** | `cargo test -p fast-staged` |
| **Full suite command** | `cargo test -p fast-staged` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p fast-staged`
- **After every plan wave:** Run `cargo test -p fast-staged`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | CFG-01 | unit | `cargo test -p fast-staged config_discovery` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | CFG-02 | unit | `cargo test -p fast-staged toml_parsing` | ❌ W0 | ⬜ pending |
| 02-01-03 | 01 | 1 | CFG-03 | unit | `cargo test -p fast-staged json_parsing` | ❌ W0 | ⬜ pending |
| 02-01-04 | 01 | 1 | CFG-04 | unit | `cargo test -p fast-staged package_json` | ❌ W0 | ⬜ pending |
| 02-01-05 | 01 | 1 | CFG-05 | unit | `cargo test -p fast-staged config_error` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/fast-staged/tests/` — test directory structure
- [ ] Test fixture config files (valid TOML, valid JSON, invalid configs, package.json with fast-staged key)

*Existing infrastructure covers framework — cargo test is built-in.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
