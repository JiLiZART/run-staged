# run-staged

Run linters and formatters on git-staged files, fast. A Rust replacement for
lint-staged with no Node.js runtime, no `git` process, and no husky.

```sh
cargo install run-staged
run-staged --install   # writes .git/hooks/pre-commit
```

## Config

`run-staged.toml` (also `.run-staged.toml`, `run-staged.json`, `.run-staged.json`,
or a `"run-staged"` key in `package.json`), found by walking up from the current
directory to the repository root:

```toml
[format]
patterns = { "*.{js,ts}" = ["prettier --write"] }

[lint]
read_only = true          # never re-stage what these commands change
timeout   = "30s"
patterns  = { "*.rs" = ["cargo clippy -- -D warnings"] }
```

Edits that commands make to fully staged files are staged again before the
commit. Partially staged files are left alone, with a warning.

## Output

In an interactive terminal, run-staged draws a live panel below your prompt:

```
╭ run-staged ───────────────────── 3 files · 1.21s ╮
│ ✔ Backed up original state  4ms                  │
│ ⠴ Running tasks for staged files…  1.20s         │
│   ▸ format  *.{js,ts}  2 files                   │
│     ⠴ prettier --write  1.20s                    │
│       │ checking src/b.ts                        │
│     ✔ eslint --fix  640ms                        │
│ ◌ Stage changes from tasks                       │
│ ◌ Clean up temporary files                       │
│ 3/7 ━━━━━━━━━━━━━━━──────────────────────────────│
╰──────────────────────────────────────────────────╯
```

- Each step and command shows a spinner and a timer while it runs, then `✔`,
  `✗` (failed), `⏱` (timed out), or `⊘` (cancelled).
- The dimmed `│` line under a running command is the last line it printed.
- The border turns green when everything succeeded and red when something
  failed. Failure details are printed below the panel in a red box (yellow for
  timeouts) that keeps the command's own colors.

The final panel stays in your scrollback. Everywhere else (pipes, CI, `--quiet`)
the output is plain text with no escape sequences.

### Colors for commands

Most tools turn colors off when their output is captured. So that failure boxes
keep colors, run-staged starts commands with `FORCE_COLOR=1` and
`CLICOLOR_FORCE=1` when stderr is a terminal. It never overrides values you set
yourself.

### Opting out

| You want                         | Do                       |
|----------------------------------|--------------------------|
| no colors anywhere (still animated) | `NO_COLOR=1 run-staged` |
| plain-text output                | `TERM=dumb run-staged`   |
| only failures, no panel          | `run-staged --quiet`     |

## Benchmarks

Comparison with lint-staged, nano-staged, lefthook, pre-commit, prek, hk,
stagelint and overcommit: see [bench/RESULTS.md](bench/RESULTS.md). To reproduce
them, see [bench/README.md](bench/README.md).
