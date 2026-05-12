---
status: complete
quick_id: 260512-3d4
slug: fix-phase-999-1-dogfood-matcher-bug
completed: 2026-04-29
files_modified:
  - crates/fast-staged/src/git.rs
commits:
  - ed7fbb8: "test(git): regression — staged_files returning committed files"
  - 067a64d: "fix(git): staged_files returns only HEAD-vs-index diff (Phase 999.1)"
---

# What Changed

`staged_files()` in `crates/fast-staged/src/git.rs` rewritten to return only files whose index entry differs from HEAD.

**Before:** iterated all stage-0 index entries — in a repo with prior commits returns every tracked file.

**After:**
- HEAD exists: shells out to `git diff --cached --name-only -z --diff-filter=ACMR`
- HEAD unborn (fresh repo): falls back to stage-0 iteration via `staged_files_unborn_head()`

# Why

Phase 999.1 dogfood bug: `staged_files()` returned all tracked `.rs` files when HEAD exists, causing `cargo fmt` to be invoked on committed-but-unchanged files and breaking every `git commit`.

# Verification

Regression test `test_staged_files_ignores_committed_files`: fails on old impl, passes on fix.

`cargo test -p fast-staged`: 140 passed, 0 failed. `cargo fmt --check` + `cargo clippy -D warnings`: clean.

## Manual dogfood evidence

Staged only a `.md` file, ran `./target/debug/fast-staged`:

```
$ ./target/debug/fast-staged 2>&1; echo "EXIT=$?"
error: no staged files matched any configured patterns
EXIT=1
```

No `cargo fmt` in output. No `.rs` files in stderr. Phase 999.1: Resolved 2026-04-29.
