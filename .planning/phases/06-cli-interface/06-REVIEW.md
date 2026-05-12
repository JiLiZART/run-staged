---
phase: 06
phase_name: cli-interface
depth: standard
files_reviewed: 4
status: issues_found
findings:
  critical: 0
  warning: 2
  info: 3
  total: 5
generated_at: 2026-05-12
---

# Code Review: Phase 06 — cli-interface

## Summary
Phase 06 cleanly swaps the hand-rolled argv block for clap derive and threads `Option<PathBuf>` into `lib::run()`. All eight locked decisions (D-01..D-08) are honored. Test coverage is solid for CLI-01/02/03. Issues found are minor — mostly test-robustness and small UX polish items, no correctness bugs.

## Findings

### WR-01: `expect("cannot determine current directory")` panics with a confusing message
**File:** `crates/fast-staged/src/main.rs:33`
**Severity:** Warning

The `.expect(...)` here will panic-print a Rust backtrace (`thread 'main' panicked at ...`) rather than the project's `error: ...` convention used four lines below at `eprintln!("error: {e}")`. While `std::env::current_dir()` failing is extremely rare (deleted CWD, permission stripped mid-process), in a pre-commit hook context it is plausible enough that a panicking binary would confuse users.

Suggested fix — match the binary's existing error style:
```rust
let cwd = match std::env::current_dir() {
    Ok(c) => c,
    Err(e) => {
        eprintln!("error: cannot determine current directory: {e}");
        process::exit(1);
    }
};
let config_path = cli.config.map(|p| if p.is_relative() { cwd.join(p) } else { p });
```

Not a blocker — the existing code is functionally correct — but it is the only `.expect` in either production file and inconsistent with the rest of the error-handling style.

### WR-02: `config_explicit_loads` test adds `--allow-empty` that weakens its assertion
**File:** `crates/fast-staged/tests/execution_integration.rs:329`
**Severity:** Warning

The test stages `a.txt`, writes a config matching `*.txt`, and invokes with both `--config <abs>` AND `--allow-empty`. The `--allow-empty` is superfluous here: a file is staged AND matched, so neither `NoStagedFiles` nor `NoFilesMatched` is reachable. Worse, including `--allow-empty` weakens the regression guarantee — if a future change causes silent no-op exits when explicit config IS parsed but yields zero groups, this test would still pass green.

Suggested fix: drop `--allow-empty` from the args:
```rust
let out = run_binary(&dir, &["--config", explicit.to_str().unwrap()]);
```
The test still passes (file matches pattern, `echo` succeeds), and now any future regression to "explicit config loads but yields no executions" would surface as a non-zero exit.

The plan does call this flag out, but the rationale (defensive padding) doesn't survive scrutiny: an exit-0 assertion under `--allow-empty` is satisfied by every code path that returns `Ok(())` early. The test is currently still meaningful (auto-discovery would NOT find `nested/custom.toml` so a non-honored `--config` would error with ConfigNotFound and fail the test), but it could be stronger.

### IN-01: `version_flag` exact-equality assertion is brittle across clap upgrades
**File:** `crates/fast-staged/tests/execution_integration.rs:266`
**Severity:** Info

`stdout.trim() == "fast-staged X.Y.Z"` is exact-match. clap 4 today emits exactly that; future clap versions could theoretically add a trailing newline or extra metadata. The neighboring `unknown_flag_rejected` test is explicitly defensive with three substring alternatives — `version_flag` is not. Consider `stdout.contains(&expected)` for symmetry with `help_flag` and `unknown_flag_rejected`, which both use substring checks. The 06-02 SUMMARY already flags clap-version-dependent strings as a known maintenance vector — this one isn't documented.

### IN-02: `unreachable!` in `failure_to_app_error` is correct but undocumented invariant
**File:** `crates/fast-staged/src/lib.rs:121`
**Severity:** Info

```rust
CommandOutcome::Success { .. } => unreachable!("only failures should reach here"),
```
This is safe today because the caller pre-filters via `.find(|o| o.is_failure())`. The invariant is implicit. A short doc-comment on `failure_to_app_error` like "Precondition: outcome must be a failure variant" or asserting via `debug_assert!(outcome.is_failure())` at entry would make the contract explicit for future maintainers. Not a Phase 06 regression — this code predates the phase — flagging here for awareness.

### IN-03: `--allow-empty` flag name uses underscore in field, kebab in flag — verify auto-conversion in tests
**File:** `crates/fast-staged/src/main.rs:22`
**Severity:** Info

`#[arg(long)]` on field `allow_empty` auto-derives `--allow-empty` (clap default kebab conversion). The `help_flag` test correctly asserts `"--allow-empty"` substring is present, locking this behavior in. No action needed — calling out for posterity that this is the one place where the Plan's Pitfall 3 (do NOT write `long = "allow_empty"`) was successfully avoided.

## Strengths

- D-01..D-08 are all faithfully implemented; the signature in `lib.rs:24-29` matches the locked shape exactly.
- D-04 CWD anchoring lives in `main.rs`, not `lib::run` — keeping the library free of env side-effects, which matches the documented design intent.
- `cli.config.map(...)` + `config_path.as_deref()` is idiomatic `Option<PathBuf>` plumbing; no needless clones or allocations.
- Doc-comment on `pub async fn run` was updated to document the new `config_path` parameter and cite the relevant decisions (D-02, D-08) — good traceability hygiene.
- `unknown_flag_rejected` defends against clap version drift with three substring alternatives, and the 06-02 SUMMARY explicitly calls this out as a future-upgrade signal.
- Tests use uniquely-suffixed temp dirs (e.g. `e2e-cli-explicit-config`, `e2e-cli-missing-config`, `e2e-cli-quiet`) so parallel `cargo test` execution stays contention-free.
- `config_missing_exits_nonzero` asserts exit code 1 (not 2) AND matches the `AppError::ConfigNotFound` display string — locks down the D-02/D-03 distinction between "clap parse error" (exit 2) and "our domain error" (exit 1).
- Test helpers (`make_real_git_repo`, `stage_file`, `write_config`, `run_binary`) are reused cleanly; no copy-paste duplication for the 6 new tests.
- No `unwrap()` / `expect()` in lib.rs production paths. The one `.expect` in main.rs (WR-01) is the only nit.
- No new dependencies added — clap was already in `Cargo.toml` from a prior phase.
