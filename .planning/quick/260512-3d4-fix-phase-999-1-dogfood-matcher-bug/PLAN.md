---
phase: quick
plan: 260512-3d4
type: execute
plan_id: quick-260512-3d4
task_type: bugfix
wave: 1
depends_on: []
files_modified:
  - crates/fast-staged/src/git.rs
autonomous: true
requirements: []
must_haves:
  truths:
    - "staged_files() in a repo with prior HEAD commits returns ONLY files whose index entry differs from HEAD (filter ACMR), not every tracked file"
    - "staged_files() in a fresh repo with no HEAD commit still returns all stage-0 entries (current behavior preserved)"
    - "staged_files() returns Err(AppError::NoStagedFiles) when the diff against HEAD is empty"
    - "Running ./target/debug/fast-staged with only a .md file staged exits with NoFilesMatched and never invokes cargo fmt"
    - "All existing tests in crates/fast-staged/src/git.rs still pass after the fix"
  artifacts:
    - path: "crates/fast-staged/src/git.rs"
      provides: "staged_files() backed by `git diff --cached --name-only -z --diff-filter=ACMR` when HEAD exists, with stage-0 fallback for unborn HEAD"
      contains_fn: "staged_files"
    - path: "crates/fast-staged/src/git.rs (mod tests)"
      provides: "Regression test asserting staged_files() ignores committed files"
      contains_fn: "test_staged_files_ignores_committed_files"
---

<objective>
Phase 999.1 dogfood bug: `staged_files()` returns every tracked file in repos with prior commits, not just files added/modified in the current pre-commit cycle. This causes the matcher to (correctly) match `.rs` files that the user did not stage, and `cargo fmt <positional file>` then fails (cargo fmt rejects positional args).

Root cause (already verified by orchestrator): `git.rs:49-74` iterates `index.entries()` filtered by `stage() == Stage::Unconflicted`. In a repo with prior commits, every tracked file is a stage-0 entry — the index ≠ the change set. Existing tests pass only because they all run in fresh repos with no HEAD (where index IS the change set).

Fix: shell out to `git diff --cached --name-only -z --diff-filter=ACMR` when HEAD exists; fall back to the existing stage-0 iteration when HEAD is unborn. This reuses the established pattern (`backup()` and `restore()` already shell out to `git`).

Purpose: unblock dogfooding the binary as a pre-commit hook on a non-empty repo (this very repo).
Output: One file modified (`crates/fast-staged/src/git.rs`) — ~30 LoC change + ~30 LoC regression test. No new dependencies.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
</execution_context>

<context>
@./CLAUDE.md
@crates/fast-staged/src/git.rs
@crates/fast-staged/src/error.rs
@crates/fast-staged/src/lib.rs
@crates/fast-staged/src/matcher.rs
@fast-staged.toml
@.planning/ROADMAP.md

<interfaces>
<!-- The only public API touched. Behavior changes; signature does NOT. -->

From crates/fast-staged/src/git.rs:
```rust
impl GitContext {
    pub fn open() -> Result<Self, AppError>;
    pub fn git_dir(&self) -> &Path;
    pub fn work_dir(&self) -> &Path;          // panics on bare repo (by design)
    pub fn staged_files(&self) -> Result<Vec<PathBuf>, AppError>;   // <- THIS CHANGES
    pub fn backup(&self) -> Result<(), AppError>;
    pub fn restore(&self) -> Result<(), AppError>;
    pub fn cleanup(&self) -> Result<(), AppError>;
    pub fn unstaged_files(&self) -> Result<Vec<PathBuf>, AppError>;
}
```

From crates/fast-staged/src/error.rs (variants used here):
```rust
AppError::NoStagedFiles                 // empty diff result
AppError::Git(String)                   // shell-out failure (matches backup/restore wording)
```

Sole consumer (crates/fast-staged/src/lib.rs:29):
```rust
let files_result = if unstaged { git_ctx.unstaged_files() } else { git_ctx.staged_files() };
// Err(AppError::NoStagedFiles) is already suppressed under --allow-empty.
```
No call sites change.
</interfaces>

<threat_model>
Trusting `git` binary on PATH for `git diff --cached`. This is the same trust posture already used by `backup()` (line 86, `git diff --cached`) and `restore()` (line 117, `git apply --cached`). No new threat surface: the binary already shells out to `git` in two places in this same file. We pin behavior to `--name-only -z --diff-filter=ACMR` (no path expansion, no glob shell handling) and pass `current_dir(work_dir)` so git resolves against the discovered repo, not an attacker-supplied CWD. Stderr is wrapped into `AppError::Git("git diff --cached failed: {stderr}")` — same format string as `backup()` for grep-able consistency.
</threat_model>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Add failing regression test for staged_files() in repo with prior commits</name>
  <files>crates/fast-staged/src/git.rs</files>

  <read_first>
- crates/fast-staged/src/git.rs lines 188-258 (existing test module helpers: CWD_LOCK, make_real_git_repo, stage_file, commit_all)
- crates/fast-staged/src/git.rs lines 325-397 (existing staged_files tests to mirror conventions)
  </read_first>

  <behavior>
- Test 1 — staged_files() in a repo with prior HEAD commits returns ONLY the newly staged file, not the previously committed one.
  Setup: real git repo, stage `committed.rs` with `fn main() {}`, commit it, then stage `new.md` with `# title`.
  Expect: `staged_files()` returns `vec![PathBuf::from("new.md")]` — does NOT contain `committed.rs`.
- The test MUST FAIL on the current implementation (which returns both files because both are stage-0 entries).
  </behavior>

  <action>
Append a new `#[test]` function to the `mod tests` block in `crates/fast-staged/src/git.rs`, immediately after `test_staged_files_only_returns_stage_zero_entries` (after line 397, before the `unstaged_files()` tests divider line 399).

Add this test verbatim:

```rust
    #[test]
    fn test_staged_files_ignores_committed_files() {
        let _lock = CWD_LOCK.lock().unwrap();
        // Reproducer for Phase 999.1 dogfood bug: in a repo with prior HEAD
        // commits, staged_files() must return ONLY files whose index entry
        // differs from HEAD — not every tracked file.
        let dir = make_real_git_repo("staged-ignores-committed");

        // Commit a .rs file so it becomes part of HEAD.
        stage_file(&dir, "committed.rs", "fn main() {}\n");
        commit_all(&dir, "initial commit");

        // Now stage a brand-new .md file. The committed .rs file is
        // unchanged in the worktree and unchanged in the index.
        stage_file(&dir, "new.md", "# title\n");

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let ctx = GitContext::open().unwrap();
        let files = ctx.staged_files().unwrap();

        let names: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        assert_eq!(
            files.len(),
            1,
            "expected exactly 1 staged file (new.md), got {:?}",
            names
        );
        assert_eq!(
            files[0],
            PathBuf::from("new.md"),
            "expected new.md, got {:?}",
            names
        );
        assert!(
            !names.contains(&"committed.rs".to_string()),
            "committed.rs must NOT be in staged_files() result (it is unchanged vs HEAD), got {:?}",
            names
        );

        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
```

After writing, run the single test to confirm it FAILS against the current (buggy) implementation:

```bash
cargo test -p fast-staged --lib git::tests::test_staged_files_ignores_committed_files -- --nocapture
```

Expected output: test fails because `files.len()` is 2 (both `committed.rs` and `new.md`).
  </action>

  <verify>
    <automated>cargo test -p fast-staged --lib git::tests::test_staged_files_ignores_committed_files -- --nocapture 2>&1 | tee /tmp/fs-test1.log; grep -E "(test result: FAILED|expected exactly 1 staged file)" /tmp/fs-test1.log</automated>
  </verify>

  <acceptance_criteria>
- `grep -n "fn test_staged_files_ignores_committed_files" crates/fast-staged/src/git.rs` returns a hit.
- Running `cargo test -p fast-staged --lib git::tests::test_staged_files_ignores_committed_files` exits NON-ZERO (test fails on current impl). This proves the test is a true regression test, not a tautology.
- The test uses `make_real_git_repo`, `stage_file`, and `commit_all` (existing helpers — no duplicated fixture code).
- No other code in `git.rs` is modified yet (production code change is Task 2).
  </acceptance_criteria>

  <done>
A new test `test_staged_files_ignores_committed_files` is appended to `crates/fast-staged/src/git.rs::tests`. Running it fails with the assertion "expected exactly 1 staged file (new.md), got [\"committed.rs\", \"new.md\"]" (or similar — exact ordering may vary). All OTHER existing tests in `crates/fast-staged/src/git.rs` still pass: `cargo test -p fast-staged --lib git::tests` shows 1 failure (the new one) and N passing.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Implement HEAD-vs-index fix in staged_files() — shell out to git diff --cached</name>
  <files>crates/fast-staged/src/git.rs</files>

  <read_first>
- crates/fast-staged/src/git.rs lines 49-74 (current `staged_files()` body to replace)
- crates/fast-staged/src/git.rs lines 86-109 (existing `backup()` — pattern for `std::process::Command::new("git")` + work_dir + AppError::Git wrapping; mirror its error wording exactly)
- crates/fast-staged/src/git.rs lines 524-545 (`test_backup_on_unborn_head_succeeds` — confirms unborn HEAD must remain a supported path)
- crates/fast-staged/src/error.rs (confirm `AppError::Git(String)` and `AppError::NoStagedFiles` are the only variants needed)
  </read_first>

  <behavior>
- Test 1 (regression, from Task 1): repo with prior commits + `committed.rs` committed + `new.md` newly staged ⇒ `staged_files() == vec!["new.md"]`. Now PASSES.
- Test 2 (preserved): `test_staged_files_returns_both_staged_files` — fresh repo (no HEAD), stage `a.txt` and `b.rs` ⇒ returns both. Still PASSES (unborn-HEAD fallback path).
- Test 3 (preserved): `test_staged_files_empty_returns_no_staged_files_error` — fresh repo, nothing staged ⇒ `Err(AppError::NoStagedFiles)`. Still PASSES.
- Test 4 (preserved): `test_staged_files_only_returns_stage_zero_entries` — fresh repo, one file staged ⇒ returns that one file. Still PASSES.
- Implicit: when HEAD exists and diff is empty (everything staged was reverted), returns `Err(AppError::NoStagedFiles)` — no Empty Vec leaks.
  </behavior>

  <action>
Replace the body of `staged_files()` in `crates/fast-staged/src/git.rs` (current lines 49-74). The function signature and doc-comment intent stay the same; update the doc comment to describe the new semantics.

**Step 2.1 — Update the doc comment (lines 42-48)** to:

```rust
    /// Return the list of files whose index entry differs from the HEAD tree
    /// entry (added / modified / renamed / copied — i.e. the set produced by
    /// `git diff --cached --name-only -z --diff-filter=ACMR`). Deleted entries
    /// are excluded: they don't exist on disk so linters/formatters cannot
    /// operate on them. This is the set a user just `git add`-ed for the
    /// current commit cycle, NOT every tracked file.
    ///
    /// In an unborn-HEAD repo (no commits yet) there is nothing to diff
    /// against, so we fall back to listing all stage-0 (Unconflicted) index
    /// entries — which in that case IS the change set.
    ///
    /// Uses `open_index()` (fresh read from disk) for the fallback path —
    /// required for correctness inside a pre-commit hook.
    ///
    /// Returns `AppError::NoStagedFiles` when the diff is empty (or, in the
    /// unborn-HEAD fallback, when there are no stage-0 entries), including
    /// when no index file has been created yet.
```

**Step 2.2 — Replace the function body** (current lines 49-74) with this implementation:

```rust
    pub fn staged_files(&self) -> Result<Vec<PathBuf>, AppError> {
        // Branch on HEAD existence. In an unborn-HEAD repo there is no tree
        // to diff against — fall back to iterating stage-0 index entries
        // (which IS the change set when there are no prior commits).
        if self.repo.head_id().is_err() {
            return self.staged_files_unborn_head();
        }

        // HEAD exists — shell out to `git diff --cached`. Same trust posture
        // as backup()/restore() in this file (Pattern 4 in research notes).
        // -z gives NUL-separated paths (binary-safe for non-UTF-8 filenames).
        // --diff-filter=ACMR: Added/Copied/Modified/Renamed only; D (deleted)
        // entries are excluded because they don't exist on disk.
        let work_dir = self
            .repo
            .workdir()
            .ok_or_else(|| AppError::Git("bare repositories are not supported".to_string()))?;

        let output = std::process::Command::new("git")
            .args([
                "diff",
                "--cached",
                "--name-only",
                "-z",
                "--diff-filter=ACMR",
            ])
            .current_dir(work_dir)
            .output()
            .map_err(|e| AppError::Git(format!("git diff --cached failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Git(format!("git diff --cached failed: {stderr}")));
        }

        // Parse NUL-separated output. Trailing NUL after the last entry is
        // normal — filter empties to drop it.
        let files: Vec<PathBuf> = output
            .stdout
            .split(|&b| b == 0)
            .filter(|chunk| !chunk.is_empty())
            .map(|chunk| {
                // Path bytes may not be valid UTF-8 on Unix. to_string_lossy
                // matches the existing path-handling convention in this file
                // (see line 65).
                let s = String::from_utf8_lossy(chunk);
                PathBuf::from(s.as_ref())
            })
            .collect();

        if files.is_empty() {
            return Err(AppError::NoStagedFiles);
        }

        Ok(files)
    }

    /// Unborn-HEAD fallback for `staged_files()`. Lists all stage-0 index
    /// entries — the original Phase 4 behavior, correct only when there is
    /// no HEAD to diff against.
    fn staged_files_unborn_head(&self) -> Result<Vec<PathBuf>, AppError> {
        let index = match self.repo.open_index() {
            Ok(idx) => idx,
            Err(_) => return Err(AppError::NoStagedFiles),
        };

        let backing = index.path_backing();

        let files: Vec<PathBuf> = index
            .entries()
            .iter()
            .filter(|e| e.stage() == Stage::Unconflicted)
            .map(|e| {
                let bstr = e.path_in(backing);
                PathBuf::from(bstr.to_str_lossy().as_ref())
            })
            .collect();

        if files.is_empty() {
            return Err(AppError::NoStagedFiles);
        }

        Ok(files)
    }
```

**Step 2.3 — Verify the unused imports.** The fallback still uses `gix::index::entry::Stage` and `gix::bstr::ByteSlice` (lines 3-4). Do NOT remove those — they're used in `staged_files_unborn_head()`.

**Step 2.4 — Format and lint:**

```bash
cargo fmt -p fast-staged
cargo clippy -p fast-staged --all-targets -- -D warnings
```
  </action>

  <verify>
    <automated>cargo fmt -p fast-staged -- --check && cargo clippy -p fast-staged --all-targets -- -D warnings && cargo test -p fast-staged --lib git::tests 2>&1 | tee /tmp/fs-test2.log; grep -E "test result: ok\. [0-9]+ passed; 0 failed" /tmp/fs-test2.log</automated>
  </verify>

  <acceptance_criteria>
- `grep -n "git diff --cached failed" crates/fast-staged/src/git.rs | wc -l` returns `3` (one in `staged_files`, two in `backup` — confirms consistent error wording, no copy-paste typos in the new variants).
- `grep -n "fn staged_files_unborn_head" crates/fast-staged/src/git.rs` returns exactly 1 hit.
- `grep -n '"--diff-filter=ACMR"' crates/fast-staged/src/git.rs` returns exactly 1 hit.
- `cargo test -p fast-staged --lib git::tests` exits 0; output contains the Task-1 regression test as PASSING (was failing before this task).
- `cargo test -p fast-staged --lib git::tests::test_staged_files_returns_both_staged_files` exits 0 (unborn-HEAD fallback still works).
- `cargo test -p fast-staged --lib git::tests::test_staged_files_empty_returns_no_staged_files_error` exits 0 (NoStagedFiles still mapped for empty fresh repo).
- `cargo clippy -p fast-staged --all-targets -- -D warnings` exits 0 (no new warnings).
- `cargo fmt -p fast-staged -- --check` exits 0.
- No new entries in `crates/fast-staged/Cargo.toml [dependencies]`.
  </acceptance_criteria>

  <done>
`staged_files()` now returns the HEAD-vs-index diff (filter ACMR) when HEAD exists and falls back to stage-0 iteration when HEAD is unborn. All four existing `staged_files` tests pass AND the new regression test `test_staged_files_ignores_committed_files` passes. `cargo fmt --check` and `cargo clippy -- -D warnings` are clean. Zero new dependencies.
  </done>
</task>

<task type="auto">
  <name>Task 3: Full test suite + manual dogfood verification</name>
  <files>(verification only — no source edits)</files>

  <read_first>
- fast-staged.toml (the dogfood config with `*.rs` + `**/*.rs` patterns)
- crates/fast-staged/src/lib.rs lines 21-50 (confirm `run()` returns NoFilesMatched, NOT a cargo fmt invocation, when no `.rs` is in the staged set)
  </read_first>

  <action>
**Step 3.1 — Full unit + integration test sweep:**

```bash
cargo test -p fast-staged 2>&1 | tee /tmp/fs-fulltest.log
```

Inspect the tail: every test target (`--lib`, integration tests under `tests/`) must show `test result: ok. N passed; 0 failed`. If anything fails that is unrelated to `git.rs`, STOP and report — do not paper over an unrelated regression.

**Step 3.2 — Build the debug binary:**

```bash
cargo build -p fast-staged 2>&1 | tail -20
```

Expect: `Compiling fast-staged …` → `Finished \`dev\` profile`. No errors.

**Step 3.3 — Manual dogfood reproducer.** Run inside THIS repository (which has prior commits, including `crates/fast-staged/src/commands.rs`), so the original Phase 999.1 bug would have triggered:

```bash
# Stage ONLY a markdown file — no .rs files.
echo "# dogfood" >> .planning/quick/260512-3d4-fix-phase-999-1-dogfood-matcher-bug/SCRATCH.md
git add .planning/quick/260512-3d4-fix-phase-999-1-dogfood-matcher-bug/SCRATCH.md

# What does staged-files-from-git see?
git diff --cached --name-only --diff-filter=ACMR
# Expect: ONLY the SCRATCH.md path. NOT commands.rs.

# Run the binary. Capture exit code AND output.
./target/debug/fast-staged; echo "EXIT=$?"
```

**Expected outcome:**
- `git diff --cached --name-only --diff-filter=ACMR` lists ONLY `SCRATCH.md`.
- `./target/debug/fast-staged` exits with `EXIT=1` (or whatever code matches `AppError::NoFilesMatched`) and prints the no-files-matched error.
- Output MUST NOT contain `cargo fmt` invocation or `cargo fmt` error text. `cargo fmt` must NOT run.
- The run completes in well under 1 second (no 1-second timeout, since no command ran).

**Step 3.4 — Cleanup the dogfood artifact** (don't commit the scratch file):

```bash
git restore --staged .planning/quick/260512-3d4-fix-phase-999-1-dogfood-matcher-bug/SCRATCH.md
rm .planning/quick/260512-3d4-fix-phase-999-1-dogfood-matcher-bug/SCRATCH.md
```

**Step 3.5 — Bonus sanity check** (positive case still works): stage a tiny `.rs` change somewhere innocuous and confirm `cargo fmt` still gets invoked correctly. If no safe target, skip this step and note it in the run report.
  </action>

  <verify>
    <automated>cargo test -p fast-staged 2>&1 | tail -5 | grep -E "test result: ok\.|0 failed"</automated>
  </verify>

  <acceptance_criteria>
- `cargo test -p fast-staged` final summary line reads `test result: ok. N passed; 0 failed; …` across every test binary (lib + each integration test). Run this assertion: `cargo test -p fast-staged 2>&1 | grep -c "0 failed" ` must equal the number of test binaries (≥1).
- `cargo build -p fast-staged` produces `target/debug/fast-staged` with exit 0.
- Manual dogfood run output: `./target/debug/fast-staged 2>&1 | grep -i "cargo fmt"` returns NOTHING (no `cargo fmt` invocation when only .md is staged).
- Manual dogfood run output: `./target/debug/fast-staged 2>&1 | grep -iE "no.*match|NoFilesMatched"` returns at least one hit (the no-files-matched error is surfaced to the user).
- Manual dogfood exit code is non-zero (the binary correctly signals "nothing to do" as an error per current semantics; if Phase 999 backlog later changes this to silent-exit, that's a separate task).
- The scratch `.md` file is unstaged and removed at the end so this plan's own commit is clean (only `crates/fast-staged/src/git.rs` changes).
  </acceptance_criteria>

  <done>
`cargo test -p fast-staged` is fully green. Manual dogfood reproducer with only a `.md` file staged: binary exits non-zero with NoFilesMatched and never runs `cargo fmt`. Phase 999.1 bug is resolved end-to-end (test layer + binary layer). The repo working tree is clean except for the intended change to `crates/fast-staged/src/git.rs`.
  </done>
</task>

</tasks>

<verification>
- `cargo fmt -p fast-staged -- --check` exits 0.
- `cargo clippy -p fast-staged --all-targets -- -D warnings` exits 0.
- `cargo test -p fast-staged` exits 0 with all test binaries reporting `0 failed`.
- New regression test `test_staged_files_ignores_committed_files` exists and passes.
- Manual dogfood: staging only `.md` produces NoFilesMatched, no `cargo fmt` invocation.
- `git diff --stat` after the plan is run shows ONLY `crates/fast-staged/src/git.rs` modified.
- No additions to `crates/fast-staged/Cargo.toml`.
</verification>

<success_criteria>
1. **Behavior:** `staged_files()` returns the HEAD-vs-index diff (ACMR filter) in repos with prior commits, AND preserves stage-0 fallback for unborn-HEAD repos.
2. **Test coverage:** New regression test reproduces the original bug (fails on old impl, passes on new impl) and the four pre-existing `staged_files` tests still pass.
3. **End-to-end:** Running the debug binary in THIS repo with only a `.md` file staged exits with NoFilesMatched — never invokes `cargo fmt`. This is the exact scenario from the Phase 999.1 backlog entry.
4. **No regressions:** Full `cargo test -p fast-staged` (lib + integration) is green. Clippy and rustfmt are clean.
5. **Minimal blast radius:** Exactly one source file modified (`crates/fast-staged/src/git.rs`). Zero new dependencies. No changes to public API surface (function signature unchanged; semantics tightened to match user expectations).
</success_criteria>

<output>
After completion, create `.planning/quick/260512-3d4-fix-phase-999-1-dogfood-matcher-bug/SUMMARY.md` capturing:
- Before/after snippets of `staged_files()`
- The regression test that locks the fix in
- Output of the dogfood reproducer (the `EXIT=…` line + absence of `cargo fmt`)
- A note for ROADMAP §Phase 999.1 to flip status from `Reproducible — re-runs hit timeout in 1s` to `Resolved <date>` once this lands.
</output>
