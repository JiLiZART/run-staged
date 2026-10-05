## 1. Scaffold

- [x] 1.1 Create `bench/run.sh` (executable, `set -euo pipefail`) with option parsing (`--tools`, `--scenarios`, `--files`, `--runs`, `--warmup`, `--verify`), `RUN_STAGED_BIN` override, and prerequisite checks for `hyperfine`, `jq`, `git`, `rsync`; verify `PATH=/usr/bin:/bin bench/run.sh` exits non-zero naming `hyperfine`
- [x] 1.2 Add `bench/package.json` pinning lint-staged, nano-staged, lefthook; add `bench/node_modules` and `bench/results/` to `.gitignore`; add `bench/.npmrc` with `ignore-scripts=true`; verify `cd bench && npm install` creates no `lefthook.yml` in the repo and produces `node_modules/.bin/{lint-staged,nano-staged,lefthook}`

## 2. Fixtures

- [x] 2.1 Implement fixture builder: pristine repo per (scenario, file count) under `$TMPDIR/run-staged-bench/`, staged files split across 4 extensions; verify `git -C <pristine> diff --cached --name-only | wc -l` equals the requested count
- [x] 2.2 Implement partial-staging fixture (staged edit + extra unstaged edit containing `UNSTAGED_MARKER`); verify `git diff` in pristine shows the marker and `git diff --cached` does not
- [x] 2.3 Commit all tool configs into the fixture base commit and implement the `--prepare` reset (`rsync -a --delete pristine/ work/`); verify the project repo's `git status` is unchanged after a full run

## 3. Tool adapters

- [x] 3.1 Add tool detection + version capture + "skipped: <tool> not found" notice; verify a run with a bogus tool on the filter prints the notice and exits 0
- [x] 3.2 Add config writer + invocation for run-staged (release build) and verify it exits 0 on the overhead fixture
- [x] 3.3 Add lint-staged and nano-staged adapters; verify each exits 0 on overhead and concurrency fixtures
- [x] 3.4 Add lefthook adapter (`parallel: true`); verify exit 0 on both fixtures
- [x] 3.5 Add pre-commit and prek adapters (`repo: local`, `language: system`); verify exit 0 on both fixtures
- [x] 3.6 Add hk and stagelint adapters (confirm flags via `--help`); verify exit 0 on both fixtures
- [x] 3.7 Add overcommit adapter (install hook in pristine, invoke hook script); verify exit 0 on both fixtures
- [x] 3.8 Add `--verify` mode (each tool once per scenario, untimed, stdin closed, prints failures); verify `bench/run.sh --verify` reports `ok` for every installed tool

## 4. Scenarios

- [x] 4.1 Wire overhead + scaling scenarios through hyperfine (`-N --warmup 3 --runs 10 --reference run-staged --sort mean-time`, JSON + markdown export); verify `results/scaling-{10,100,1000,10000}.json` exist
- [x] 4.2 Wire concurrency scenario (4 tasks × `sleep 0.5`); verify run-staged mean is < 1.0 s
- [x] 4.3 Wire partial-staging timing plus the untimed probe check writing `results/unstaged.tsv` (hides / preserved / probe not run); verify by pointing run-staged at a command that deletes the marker and seeing it flagged as lost

## 5. Report

- [x] 5.1 Generate `bench/RESULTS.md` via `bench/report.sh`: date, OS, CPU model/cores, tool versions, per-scenario tables from hyperfine JSON, unstaged-check table; verify file renders with run-staged as 1.00 baseline
- [x] 5.2 Add concurrency columns (ideal 0.5 s, sequential 2.0 s, parallelism factor = 2.0 / mean, sequential marker) via `jq`; verify values for run-staged look sane (factor > 2)
- [x] 5.3 Write `bench/README.md` (install commands per tool, how to run, per-tool invocation/parallel/hides table, noise advice) and link it from the root `README.md`; verify links resolve

## 6. Validate

- [x] 6.1 Run the full suite with all installable tools on the maintainer machine; verify exit 0, `RESULTS.md` complete, project `git status` clean
- [x] 6.2 Run `shellcheck bench/run.sh` and `openspec validate add-competitor-benchmarks --strict`; verify both pass
