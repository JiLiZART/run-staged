# Benchmarks

`run.sh` compares run-staged with other staged-file runners on identical throwaway git repos and
writes [`RESULTS.md`](RESULTS.md).

| Scenario | What it measures |
|:---|:---|
| `overhead` | No-op command (`true`) on 100 staged files: startup, git reads, matching, spawn |
| `scaling` | `overhead` at 10 / 100 / 1 000 / 10 000 staged files |
| `concurrency` | 4 tasks on disjoint globs, each `sleep 0.5`: ideal 0.5 s, sequential 2.0 s |
| `partial-staging` | 100 staged files that also carry unstaged edits: cost of hiding/restoring them, plus a check that each tool kept them |

Every tool is called directly (not through `git commit`), in its default safety mode, with the
equivalent config written into the fixture's base commit. Fixtures live in
`$TMPDIR/run-staged-bench` and are reset with `rsync` before every timed run; the project repo is
never touched.

## Install

Required: `hyperfine`, `jq`, `git`, `rsync`.

```sh
brew install hyperfine jq                # or your package manager

# competitors - install whichever you want; missing ones are skipped
(cd bench && npm install)                # lint-staged, nano-staged, lefthook (pinned)
uv tool install pre-commit               # or: pipx install pre-commit
brew install prek hk lefthook            # or: cargo install prek / see hk docs
cargo install stagelint
gem install --user-install overcommit    # then add `ruby -e 'puts Gem.user_dir'`/bin to PATH
```

`bench/.npmrc` sets `ignore-scripts=true` so lefthook's postinstall doesn't write a
`lefthook.yml` into this repo.

## Run

```sh
bench/run.sh                                # everything, 3 warmups + 10 runs each
bench/run.sh --verify                       # run each tool once per scenario, show failures
bench/run.sh --tools run-staged,lint-staged --scenarios concurrency
bench/run.sh --files 10,100 --runs 3        # quick pass
RUN_STAGED_BIN=/path/to/run-staged bench/run.sh   # use a prebuilt binary instead of cargo build
bench/report.sh > bench/RESULTS.md          # re-render from bench/results/
```

Raw hyperfine JSON lands in `bench/results/` (gitignored).

## Tool notes

| Tool | Invocation | Parallel | Hides unstaged edits |
|:---|:---|:---|:---|
| run-staged | `run-staged --quiet` | yes, within a group | no - never re-stages partially staged files instead |
| lint-staged | `lint-staged --quiet` | yes, across globs | yes |
| nano-staged | `nano-staged` | yes | yes |
| lefthook | `lefthook run pre-commit` | `parallel: true` | yes |
| pre-commit | `pre-commit run` | no, hooks run in order | yes |
| prek | `prek run` | hooks with equal `priority` | yes |
| hk | `hk run pre-commit` | yes | no (default config) |
| stagelint | `stagelint` | yes | yes (three-way merge) |
| overcommit | `.git/hooks/pre-commit` | yes | yes |

The "hides" column is measured on every run (see the partial-staging table in `RESULTS.md`), so
trust that over this table if they disagree. pre-commit prints a warning about the unknown
`priority` key, which is there for prek.

## Getting stable numbers

Plug in the laptop, close browsers/IDEs/indexers, and don't use the machine while it runs. Check
the `±` column: a σ above ~10% of the mean means the run was noisy.
