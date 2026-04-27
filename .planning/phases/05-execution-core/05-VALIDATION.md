---
phase: 5
slug: execution-core
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-26
approved: 2026-04-27
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `#[tokio::test]` (via `tokio = { features = ["full"] }`) |
| **Config file** | none — Cargo discovers test modules and `tests/` automatically |
| **Quick run command** | `cargo test -p fast-staged -- --test-threads=1 2>&1 \| tail -20` |
| **Full suite command** | `cargo test -p fast-staged 2>&1` |
| **Estimated runtime** | ~25 seconds (includes integration tests with real git repos) |

`--test-threads=1` is required for tests that mutate CWD (existing `CWD_LOCK` Mutex pattern from Phase 4 integration tests). Pure matcher and executor unit tests do NOT mutate CWD and may run fully parallel.

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p fast-staged -- --test-threads=1 2>&1 | tail -20`
- **After every plan wave:** Run `cargo test -p fast-staged && cargo clippy -p fast-staged --all-targets -- -D warnings`
- **Before `/gsd:verify-work`:** Full suite must be green; `cargo fmt --check` clean
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 5-01-01 | 01 | 1 | EXEC-06 | unit | `cargo test -p fast-staged --lib matcher::tests` | ❌ W0 | ⬜ pending |
| 5-01-02 | 01 | 1 | EXEC-06 | unit | `cargo test -p fast-staged --lib matcher::tests::matchbase` | ❌ W0 | ⬜ pending |
| 5-01-03 | 01 | 1 | EXEC-06 | unit | `cargo test -p fast-staged --lib matcher::tests::negation` | ❌ W0 | ⬜ pending |
| 5-01-04 | 01 | 1 | EXEC-07 | unit | `cargo test -p fast-staged --lib matcher::tests::empty_match` | ❌ W0 | ⬜ pending |
| 5-02-01 | 02 | 2 | EXEC-01 | unit | `cargo test -p fast-staged --lib commands::tests::spawn` | ❌ W0 | ⬜ pending |
| 5-02-02 | 02 | 2 | EXEC-02 | unit | `cargo test -p fast-staged --lib commands::tests::placeholder` | ❌ W0 | ⬜ pending |
| 5-02-03 | 02 | 2 | EXEC-02 | unit | `cargo test -p fast-staged --lib commands::tests::positional_append` | ❌ W0 | ⬜ pending |
| 5-02-04 | 02 | 2 | EXEC-02 | unit | `cargo test -p fast-staged --lib commands::tests::arg_max_chunking` | ❌ W0 | ⬜ pending |
| 5-03-01 | 03 | 2 | EXEC-01 | integration | `cargo test -p fast-staged --lib commands::tests::parallel_wallclock` | ❌ W0 | ⬜ pending |
| 5-03-02 | 03 | 2 | EXEC-02 | integration | `cargo test -p fast-staged --lib commands::tests::sequential_order` | ❌ W0 | ⬜ pending |
| 5-03-03 | 03 | 2 | EXEC-03 | unit | `cargo test -p fast-staged --lib commands::tests::exit_propagation` | ❌ W0 | ⬜ pending |
| 5-03-04 | 03 | 2 | EXEC-03 | unit | `cargo test -p fast-staged --lib commands::tests::group_cancel` | ❌ W0 | ⬜ pending |
| 5-04-01 | 04 | 2 | EXEC-05 | unit (unix) | `cargo test -p fast-staged --lib commands::tests::timeout_kills` | ❌ W0 | ⬜ pending |
| 5-04-02 | 04 | 2 | EXEC-05 | unit (unix) | `cargo test -p fast-staged --lib commands::tests::sigterm_then_sigkill` | ❌ W0 | ⬜ pending |
| 5-05-01 | 05 | 3 | EXEC-07 | integration | `cargo test -p fast-staged --test execution_integration allow_empty` | ❌ W0 | ⬜ pending |
| 5-05-02 | 05 | 3 | EXEC-03 | integration | `cargo test -p fast-staged --test execution_integration restore_on_failure` | ❌ W0 | ⬜ pending |
| 5-05-03 | 05 | 3 | EXEC-01,03 | integration | `cargo test -p fast-staged --test execution_integration end_to_end_success` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/fast-staged/src/matcher.rs` — module file with `#[cfg(test)] mod tests` block, stubs for EXEC-06 (matchBase, negation, case-sensitive), EXEC-07 (empty match)
- [ ] `crates/fast-staged/src/commands.rs` — module file with `#[cfg(test)] mod tests` block, stubs for EXEC-01/02/03/05 (spawn, placeholder, parallel timing, exit propagation, timeout)
- [ ] `crates/fast-staged/src/output.rs` — module file with `#[cfg(test)] mod tests` block, stubs for `print_error_box` and quiet-mode behavior
- [ ] `crates/fast-staged/tests/execution_integration.rs` — integration test file using `make_real_git_repo` helper from Phase 4 (`tests/git_integration.rs`); imports must reuse existing helper, not duplicate
- [ ] `Cargo.toml` updates — add `shlex = "1.3"` and `nix = { version = "0.31", features = ["signal"] }` dependencies (already-present `globset`, `tokio`, `thiserror` need no change)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Windows `Child::kill()` uses TerminateProcess | EXEC-05 | No Windows CI runner configured; SIGTERM/SIGKILL test only on Unix via `#[cfg(unix)]` | Run `cargo test --target x86_64-pc-windows-msvc` on a Windows host with a mock long-running command (e.g., `timeout /t 60`) and confirm timeout fires + process dies |
| Failure-box visual rendering | EXEC-03 (presentation) | ASCII rendering is visual — automated tests assert content presence but layout review is human | Run `cargo run -- --quiet` on a config that intentionally fails one command; eyeball the bordered box for column alignment and color codes |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (matcher.rs, commands.rs, output.rs, tests/execution_integration.rs, Cargo.toml deps)
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-04-27
