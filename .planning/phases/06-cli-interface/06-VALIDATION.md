---
phase: 6
slug: cli-interface
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-29
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (`cargo test`) |
| **Config file** | None — `cargo test` discovers tests automatically |
| **Quick run command** | `cargo test -p fast-staged --lib` |
| **Full suite command** | `cargo test -p fast-staged` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p fast-staged --lib`
- **After every plan wave:** Run `cargo test -p fast-staged`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 6-01-* | 01 | 1 | CLI-01, CLI-02, CLI-03 | — | clap rejects unknown flags (exit 2); explicit `--config` hard-fails on missing path | unit + integration | `cargo test -p fast-staged` | ❌ W0 (new tests) | ⬜ pending |
| 6-02-* | 02 | 1 | CLI-01 | — | `lib::run(config_path, ...)` honors explicit-path override | unit | `cargo test -p fast-staged --lib` | ✅ (existing) | ⬜ pending |
| 6-03-* | 03 | 2 | CLI-01, CLI-02, CLI-03 | — | --version, --help, --config success/failure, --quiet, unknown-flag exit 2 | integration (binary) | `cargo test -p fast-staged --test execution_integration` | ❌ W0 (new test fns) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/fast-staged/tests/execution_integration.rs` — append 6 new test functions: `version_flag`, `help_flag`, `unknown_flag_rejected`, `config_explicit_loads`, `config_missing_exits_nonzero`, `quiet_flag_suppresses_output`
- [ ] No framework install needed — `cargo test` is built-in.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `--help` output reads naturally for a Unix CLI user | CLI-02 | Subjective tone/clarity check — automated test only verifies presence of flag names | Run `cargo run -p fast-staged -- --help` and skim output for missing context (about, version, flag descriptions) |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
