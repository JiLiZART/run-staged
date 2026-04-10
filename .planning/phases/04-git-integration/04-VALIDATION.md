---
phase: 4
slug: git-integration
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-10
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | `crates/fast-staged/Cargo.toml` |
| **Quick run command** | `cargo test -p fast-staged --lib` |
| **Full suite command** | `cargo test -p fast-staged` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p fast-staged --lib`
- **After every plan wave:** Run `cargo test -p fast-staged`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | GIT-01 | integration | `cargo test -p fast-staged staged_files` | ❌ W0 | ⬜ pending |
| 04-01-02 | 01 | 1 | GIT-06 | integration | `cargo test -p fast-staged not_git_repo` | ❌ W0 | ⬜ pending |
| 04-01-03 | 01 | 1 | GIT-07 | integration | `cargo test -p fast-staged no_staged_files` | ❌ W0 | ⬜ pending |
| 04-01-04 | 01 | 1 | GIT-05 | integration | `cargo test -p fast-staged unstaged_files` | ❌ W0 | ⬜ pending |
| 04-01-05 | 01 | 2 | GIT-02 | integration | `cargo test -p fast-staged backup` | ❌ W0 | ⬜ pending |
| 04-01-06 | 01 | 2 | GIT-03 | integration | `cargo test -p fast-staged restore` | ❌ W0 | ⬜ pending |
| 04-01-07 | 01 | 2 | GIT-04 | integration | `cargo test -p fast-staged cleanup` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/fast-staged/tests/git_integration.rs` — integration test stubs for GIT-01 through GIT-07
- [ ] Test helper: `make_real_git_repo()` fixture using `git init` + `git add` for real index files

*Existing cargo test infrastructure covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| None | - | - | - |

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
