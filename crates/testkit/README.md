# run-staged-testkit

Test-only harness that runs the real `run-staged` binary against throwaway git
repositories. Not published.

## Fixtures

A fixture is a directory in `apps/run-staged/tests/fixtures/<name>/` with up to
three layers:

| Layer        | What happens                                              |
|--------------|-----------------------------------------------------------|
| `committed/` | written, `git add -A`, committed as the initial commit     |
| `staged/`    | written over the tree, then `git add -A`                   |
| `unstaged/`  | written last, not staged                                   |

No `committed/` means a repository without commits. `.gitkeep` files are
skipped, so an empty layer can be checked in.

To add one: create the directory and its layers, then in a test:

```rust
use run_staged_testkit::fixture_repo;

let repo = fixture_repo!("basic-js");
let r = repo.runner().args(["--quiet"]).run();
assert!(r.success(), "{}", r.snapshot(&repo));
```

Or build one in code:

```rust
let repo = run_staged_testkit::Repo::builder()
    .committed("a.js", "old")
    .staged("a.js", "new")
    .staged("run-staged.toml", "[lint]\npatterns = { \"*.js\" = [\"fixture-tool --append x\"] }\n")
    .git_config("core.hooksPath", ".githooks")
    .file(".githooks/pre-commit", "#!/bin/sh\n")
    .build();
```

## Isolation

Every repository lives in its own temp dir. System and user git config are
ignored (`GIT_CONFIG_NOSYSTEM`, an empty `GIT_CONFIG_GLOBAL`, `HOME` in the temp
dir), and author, dates, and branch (`main`) are fixed. `GIT_DIR`,
`GIT_WORK_TREE`, and `GIT_INDEX_FILE` from the parent are removed.

Set `RUN_STAGED_KEEP_FIXTURES=1` to keep the temp dirs; their paths are printed.

## fixture-tool

A portable stand-in for linters and formatters, put first on the PATH of the
process under test. Every call appends `{"args": [...], "cwd": "<rel>"}` to
`<git-dir>/fixture-tool.log` (read with `repo.invocations()`).

| Option             | Effect                              |
|--------------------|-------------------------------------|
| `--sleep <ms>`     | sleep first                          |
| `--append <text>`  | append to each file argument         |
| `--replace <text>` | replace each file argument's content |
| `--delete`         | delete each file argument            |
| `--stdout <text>`  | print to stdout                      |
| `--stderr <text>`  | print to stderr                      |
| `--exit <n>`       | exit code (default 0)                |
| `--print-env <NAME>` | print `NAME=value` or `NAME=<unset>` to stdout (repeatable) |
| `--copy-git-file <name>` | copy `<git-dir>/<name>` to `<git-dir>/fixture-tool.<name>` first (to observe mid-run state) |

## Runner options

- `.dir("sub")`: working directory relative to the repo root.
- `.env(k, v)`: extra environment.
- `.no_git()`: drop every PATH entry that contains a `git` executable.
- `.timeout(d)`: kill and fail after `d` (default 30s).
- `.hold_index_lock(Some(d))` / `.hold_index_lock(None)`: create `<index>.lock`
  before the run and release it after `d`, or never. `RunResult::lock_touched`
  reports whether the process changed it.
- `repo.enable_commit_a_index()`: simulate `git commit -a` (`.git/index.lock`
  holds the staged state and is passed as `GIT_INDEX_FILE`).

## Snapshots

`RunResult::snapshot(&repo)` renders exit code, stdout, and stderr with
durations as `<duration>`, the repo path as `<repo>`, `/` separators, and LF
line endings. Use it with `insta::assert_snapshot!`.
