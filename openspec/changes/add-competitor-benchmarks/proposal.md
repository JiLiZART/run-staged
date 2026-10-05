## Why

run-staged's core value is "faster than lint-staged", but nothing in the repo measures it. We need a reproducible benchmark that compares run-staged against lint-staged, nano-staged and the other staged-file runners across ecosystems (Rust, Go, JS, Python, Ruby), covering both raw startup/overhead speed and how well each tool runs work concurrently.

## What Changes

- Add a `bench/` directory with a self-contained benchmark harness (shell script driving `hyperfine`) — no Rust/JS harness code, nothing shipped in the binary or npm packages.
- Generate throwaway git fixture repos with N staged files (10 / 100 / 1 000 / 10 000) so every tool runs on identical input.
- Benchmark these tools, each configured to the equivalent job (glob → command on staged files):
  - Rust: **run-staged** (this repo, release build), **prek**, **hk**, **stagelint**
  - JS: **lint-staged**, **nano-staged**
  - Go (npm/gem distributed): **lefthook**
  - Python: **pre-commit**
  - Ruby: **overcommit**
- Scenarios:
  - **overhead** — no-op command (`true`), measures tool startup + git + matching cost
  - **scaling** — overhead scenario across file counts
  - **concurrency** — 4 groups/hooks each running `sleep 0.5`; ideal parallel wall time ≈ 0.5 s, sequential ≈ 2 s
  - **partial-staging** — files with unstaged hunks, measures stash/restore cost where the tool supports it
- Tools not installed are skipped with a notice, not a failure.
- Results written as hyperfine JSON + a generated markdown table in `bench/RESULTS.md`, including tool versions and machine info.

## Capabilities

### New Capabilities
- `benchmarks`: reproducible comparative benchmark of run-staged vs other staged-file runners — fixtures, tool matrix, scenarios (overhead, scaling, concurrency, partial staging) and result reporting.

### Modified Capabilities
<!-- none -->

## Impact

- New: `bench/` (script, per-tool config templates, results). No changes to crates or the `apps/run-staged` binary.
- Dev-only external tools: `hyperfine` (required), plus whichever competitors are installed (`npx`, `pipx`/`uvx`, `gem`, `brew`, `cargo`).
- Not run in CI by default (needs third-party installs and stable hardware); run manually.
