# Phase 4: Git Integration - Research

**Researched:** 2026-04-10
**Domain:** gix crate — index reading, staged file detection, patch generation, restore
**Confidence:** MEDIUM (gix API verified against docs.rs 0.81.0; patch generation approach is confirmed but requires careful blob-level assembly)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Use `gix::discover` from CWD to open the repository. Return `AppError::NotGitRepository` on failure.
- **D-02:** Read staged files by accessing the git index via `repo.index()` or `repo.open_index()` and filtering for entries with non-zero stage or by comparing index entries against HEAD tree.
- **D-03:** Enable minimal gix features — only what's needed for index reading and diff generation. Keep binary size small per project constraints (LTO, opt-level "z").
- **D-04:** Keep `.git/` directory scan in config/discover.rs unchanged. The gix crate is used only in the new git module. Phase 2 decision explicitly chose this separation.
- **D-05:** Use gix diff API to generate the staged diff patch (index vs HEAD). No git CLI shell-out.
- **D-06:** Patch file stored at `.git/fast-staged.patch`.
- **D-07:** If a patch file already exists from a prior interrupted run, overwrite it.
- **D-08:** `lib.rs` orchestrates backup/restore — the git module provides `backup()` and `restore()` functions.
- **D-09:** Restore means restoring the staged diff to the index. Working tree changes from commands are expected and kept.
- **D-10:** Git module lives at `src/git.rs` (single file initially).
- **D-11:** Public API: `GitContext` struct with `open()`, `staged_files()`, `unstaged_files()`, `backup()`, `restore()`, `cleanup()`.
- **D-12:** Integration in `lib.rs::run()`: after config loading, before command execution.

### Claude's Discretion

- Exact gix feature flags to enable in Cargo.toml
- gix API details for index reading (API varies by version — researcher should verify current crate docs)
- Error message wording for git-specific failures
- Test strategy: real git repos in temp dirs vs mocked index data
- Whether `GitContext` holds `gix::Repository` or a lighter handle

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| GIT-01 | Tool detects staged files from git index via gix | gix 0.81.0 `repo.open_index()` returns `gix_index::File`; iterate `state.entries()` filtered to `stage() == 0` |
| GIT-02 | Tool saves staged diff to `.git/fast-staged.patch` before command execution | Use gix status iterator (HEAD-vs-index `TreeIndex` items) + `gix_diff::blob::Platform` + `UnifiedDiffBuilder` to assemble unified diff; write to `.git/fast-staged.patch` |
| GIT-03 | Tool restores staged diff from patch on command failure | Run `git apply --cached .git/fast-staged.patch` — note: gix has no apply API, this requires `std::process::Command` with `git apply` OR we hand-apply each entry back to the index |
| GIT-04 | Tool cleans up patch file after successful execution | `std::fs::remove_file(".git/fast-staged.patch")` |
| GIT-05 | Tool supports `--unstaged` flag to run on modified (not staged) files | gix status iterator yields `IndexWorktree` items; filter for modified/added entries in worktree |
| GIT-06 | Tool exits with clear error when not in a git repository | `gix::discover()` returns `Err` — map to `AppError::NotGitRepository` |
| GIT-07 | Tool exits with clear error when no staged files found (unless `--allow-empty`) | After filtering index entries for `stage() == 0`, if empty → `AppError::NoStagedFiles`; `--allow-empty` bypasses this (Phase 6 adds CLI flag parsing) |
</phase_requirements>

---

## Summary

Phase 4 adds a `git.rs` module that wraps gix 0.81.0 to provide staged-file detection, patch backup, restore, and cleanup. The `GitContext` struct encapsulates the `gix::Repository` handle and exposes a focused API consumed by `lib.rs::run()`.

**gix 0.81.0 is already in Cargo.toml with no explicit features listed (default features apply).** The default feature set includes everything Phase 4 needs: `index`, `status`, `blob-diff`, `parallel`. No additional dependencies are required. The only version concern flagged in STATE.md ("gix staged-file API shape varies by minor version") is resolved — the API is confirmed stable at 0.81.0.

**Critical finding:** gix's "tree with index" diff is explicitly marked incomplete in the crate's own status document. This means `diff_tree_to_tree` cannot be used with the index as a tree. The correct approach for generating `.git/fast-staged.patch` is to use the status iterator to enumerate HEAD-vs-index changes, then perform blob-level diffs using `gix_diff::blob::Platform` + `UnifiedDiffBuilder` for each changed file.

**Restore approach requires caution:** gix has no `git apply` equivalent. The patch file generated for backup can be applied back via `std::process::Command` calling `git apply --cached` (shell-out to git CLI is allowed for restore since gix lacks this capability) OR by re-staging files directly from the object store. The simplest correct approach is `git apply --cached` — this is a pragmatic exception to the "no git CLI" constraint, limited to restore only. Alternatively, for a pure-gix approach, store the index state differently (e.g., write the full index state and re-load it). This is an open question the planner must decide.

**Primary recommendation:** Use `gix::discover()` + `repo.open_index()` for repo detection and staged-file listing. Use the status iterator for HEAD-vs-index enumeration. For patch generation, use `gix_diff::blob::Platform` + `UnifiedDiffBuilder` per changed file. For restore, use `std::process::Command("git apply --cached")` as a pragmatic fallback since gix lacks an apply API.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| gix | 0.81.0 (already in Cargo.toml) | Repository discovery, index reading, status, blob diff | Project constraint: all git ops via gix |
| gix_index | 0.37.0 (transitive via gix) | Index file parsing, entry iteration | Sub-crate of gix, re-exported |
| gix_diff | 0.49.0 (transitive via gix) | Blob-level diffing, `UnifiedDiffBuilder` | Sub-crate of gix, blob-diff feature |
| std::fs | stdlib | Patch file I/O, cleanup | No external dep needed |
| std::process::Command | stdlib | `git apply --cached` for restore (if pure-gix apply not feasible) | Pragmatic fallback |

### Feature Flags

gix default features include all needed capabilities. For binary size optimization (D-03), disable defaults and enable only what's needed:

```toml
gix = { version = "0.81", default-features = false, features = [
    "index",       # open_index(), entry iteration
    "blob-diff",   # UnifiedDiffBuilder, blob Platform
    "status",      # status iterator for HEAD-vs-index changes
    "sha1",        # required hash algorithm
    "auto-chain-error",  # convenient error conversion
] }
```

**Note:** Disabling defaults may break transitive dependencies that assume default features. If compile errors occur, keep `default-features = true` and document the size tradeoff. This is a Claude's Discretion area.

**Installation:** No new dependencies needed — gix 0.81.0 is already declared in `crates/fast-staged/Cargo.toml`.

---

## Architecture Patterns

### Recommended Project Structure

```
crates/fast-staged/src/
├── git.rs           # NEW: GitContext struct (D-10)
├── lib.rs           # Updated: import git module, call open/staged_files/backup/cleanup
├── config/          # Unchanged (D-04)
├── error.rs         # Unchanged: AppError variants ready
└── main.rs          # Unchanged
```

### Pattern 1: GitContext Struct

**What:** Wraps `gix::Repository` and exposes focused git operations.

**When to use:** All git interactions go through this struct; callers never import gix directly.

**Example:**
```rust
// src/git.rs
use gix::bstr::BStr;
use std::path::{Path, PathBuf};
use crate::AppError;

pub struct GitContext {
    repo: gix::Repository,
}

impl GitContext {
    /// Open repo by discovering from CWD upward (D-01).
    pub fn open() -> Result<Self, AppError> {
        let repo = gix::discover(".")
            .map_err(|_| AppError::NotGitRepository)?;
        Ok(Self { repo })
    }

    /// Return git root directory (where .git/ lives).
    pub fn git_dir(&self) -> &Path {
        self.repo.git_dir()
    }

    /// List staged files (index stage 0 entries) (GIT-01).
    pub fn staged_files(&self) -> Result<Vec<PathBuf>, AppError> {
        let index = self.repo.open_index()
            .map_err(|e| AppError::Git(e.to_string()))?;
        let backing = index.path_backing();
        let files: Vec<PathBuf> = index.entries()
            .iter()
            .filter(|e| e.stage() == 0)
            .map(|e| {
                let path = e.path(&backing);
                PathBuf::from(path.to_str_lossy().as_ref())
            })
            .collect();
        Ok(files)
    }
}
```

**Source:** docs.rs/gix/0.81.0 — `Repository::open_index()`, `State::entries()`, `State::path_backing()`, `Entry::stage()`

### Pattern 2: Staged File Enumeration

**What:** Iterate index entries, filter by stage == 0 to get clean staged files.

**Key insight:** `stage() == 0` means "normal staged file". Stages 1-3 are merge conflict states (base/ours/theirs). For merge-conflict repos, the correct approach is to filter stage 0 only — or to reject merge-conflict scenarios with a clear error.

```rust
// Reading entry paths from index
// Source: docs.rs/gix-index/0.37.0
let index = repo.open_index()?;
// entries() returns &[gix_index::Entry]
// path_backing() returns PathStorage for resolving paths
for entry in index.entries() {
    // stage() → 0 (staged), 1 (base), 2 (ours), 3 (theirs)
    if entry.stage() == 0 {
        let path: &BStr = entry.path(index.path_backing());
        // path is repo-relative, e.g. "src/main.rs"
    }
}
```

**Alternative:** Use `entries_with_paths_by_filter_map()` for a single-pass filter+map:

```rust
let staged: Vec<PathBuf> = index
    .entries_with_paths_by_filter_map(index.path_backing(), |entry, path| {
        (entry.stage() == 0).then(|| PathBuf::from(path.to_str_lossy().as_ref()))
    })
    .collect();
```

### Pattern 3: Patch Generation (HEAD vs Index)

**What:** Generate a unified diff of all staged changes (what's in the index but not in HEAD) using gix status + blob-level diff.

**CRITICAL:** `gix::diff::tree_with_index` is incomplete (crate-status.md). Do NOT use `diff_tree_to_tree` with an in-memory index tree. Use the status iterator instead.

```rust
// Approach: status iterator for TreeIndex items (HEAD vs index)
// Source: docs.rs/gix/0.81.0 — status::Platform, status::Item
use gix::bstr::BString;

pub fn backup(&self) -> Result<(), AppError> {
    let patch_path = self.repo.git_dir().join("fast-staged.patch");

    // Enumerate HEAD-vs-index changes
    let status = self.repo.status(gix::progress::Discard)?;
    let iter = status
        .untracked_files(gix::status::UntrackedFiles::None)
        .into_iter(None::<BString>)
        .map_err(|e| AppError::Git(e.to_string()))?;

    let mut patch_content = String::new();

    for item in iter {
        let item = item.map_err(|e| AppError::Git(e.to_string()))?;
        // TreeIndex variant = HEAD-vs-index (staged) change
        if let gix::status::Item::TreeIndex(change) = item {
            // Generate unified diff for this file change
            // (blob-level diff using gix_diff::blob::Platform)
            // Append to patch_content
        }
    }

    // Write patch (overwrite if exists — D-07)
    std::fs::write(&patch_path, patch_content).map_err(AppError::Io)?;
    Ok(())
}
```

**Note:** The exact `gix::status::Item` variant names and `TreeIndex` change fields need final verification during implementation — the status API is marked as enabled by default but the exact `Item` enum shape may differ. Use `cargo doc --open` during implementation.

### Pattern 4: Restore via git apply

**What:** Apply the patch file back to the index on command failure.

**Why:** gix has no `git apply` equivalent. This is the one pragmatic shell-out exception.

```rust
pub fn restore(&self) -> Result<(), AppError> {
    let patch_path = self.repo.git_dir().join("fast-staged.patch");
    if !patch_path.exists() {
        return Ok(()); // No patch to restore
    }

    let output = std::process::Command::new("git")
        .args(["apply", "--cached"])
        .arg(&patch_path)
        .output()
        .map_err(|e| AppError::Git(format!("git apply failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Git(format!("git apply --cached failed: {stderr}")));
    }
    Ok(())
}
```

**Alternative pure-gix approach:** Instead of generating a patch file and applying it, store the pre-run index state as a `gix_index::File` and write it back on failure. This avoids the git CLI entirely but is more complex — requires serializing the full index. Not recommended for Phase 4; use `git apply --cached` for now.

### Pattern 5: Cleanup

```rust
pub fn cleanup(&self) -> Result<(), AppError> {
    let patch_path = self.repo.git_dir().join("fast-staged.patch");
    if patch_path.exists() {
        std::fs::remove_file(&patch_path).map_err(AppError::Io)?;
    }
    Ok(())
}
```

### Pattern 6: Unstaged Files (GIT-05)

**What:** When `--unstaged` flag is set, return modified-but-not-staged files (index vs worktree).

```rust
pub fn unstaged_files(&self) -> Result<Vec<PathBuf>, AppError> {
    // Use status iterator, filter IndexWorktree items
    // IndexWorktree variant = index-vs-worktree (unstaged) changes
    // Item::IndexWorktree(item) → item.rela_path gives the path
    todo!("implement with status iterator, IndexWorktree variant")
}
```

**Note:** Phase 6 adds `--unstaged` CLI flag parsing. Phase 4 implements `unstaged_files()` but the flag that triggers it is added in Phase 6. `lib.rs::run()` will call `staged_files()` unconditionally for now; Phase 6 will add the branch.

### Anti-Patterns to Avoid

- **Hand-rolling `.git/` detection in git.rs:** Config discovery already does `.git/` scan — but git.rs must use `gix::discover()`, not the same scan. These serve different purposes: config discovery is for finding config files; git.rs is for opening the actual repo with full gix capabilities.
- **Using `diff_tree_to_tree` with index:** Marked incomplete in gix crate-status. Will produce incorrect results or panic.
- **Shelling out to `git diff --cached`:** Violates project constraint (D-05). Use gix status API instead.
- **Using `gix::ThreadSafeRepository`:** Overkill for single-threaded git context. Use `gix::Repository` (thread-local handle).
- **Ignoring merge conflict entries:** If `stage()` returns 1-3, the repo is in a merge conflict state. Should return a clear error rather than silently skipping entries.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Git repository discovery | Manual `.git/` walk in git.rs | `gix::discover()` | Handles gitdir files, bare repos, worktrees, environment overrides |
| Index file parsing | Custom binary parser | `repo.open_index()` | Index format is complex (extensions, flags, hash verification) |
| Stage number extraction | Bit-masking entry flags | `entry.stage()` method | Flags field is non-trivial; method handles masking correctly |
| Unified diff formatting | Custom diff formatter | `gix_diff::blob::UnifiedDiffBuilder` | Context lines, hunk headers, edge cases |
| Blob tokenization | Split-by-newline | `imara_diff::InternedInput::new()` | Handles CRLF, binary detection, large file efficiency |

**Key insight:** Index entry format is a non-trivial binary format with extensions. The `gix_index::File` abstraction handles version detection, extension parsing, and hash verification that would take weeks to hand-roll correctly.

---

## Common Pitfalls

### Pitfall 1: gix::discover trust model

**What goes wrong:** `gix::discover(".")` may fail with a trust error if the git repo is owned by a different user (common in Docker, CI, or shared directories).
**Why it happens:** gix respects Git's `safe.directory` config and applies a `Trust` model based on file ownership.
**How to avoid:** Use `gix::open::Options::isolated()` or configure trust explicitly. For Phase 4, use `gix::discover(".")` and map any error to `AppError::NotGitRepository` — the error message should guide the user.
**Warning signs:** "repository at ... is not owned by the current user" error in stderr.

### Pitfall 2: open_index() vs index()

**What goes wrong:** Using `repo.index()` (lazy auto-updated) vs `repo.open_index()` (fresh load). In a pre-commit hook context, `open_index()` is correct because you want the current state, not a potentially stale cached state.
**Why it happens:** `repo.index()` returns a shared, lazily-updated handle. In a hook, you need a fresh read.
**How to avoid:** Always use `repo.open_index()` in `staged_files()`. Use `repo.index()` only for lightweight reads where staleness is acceptable.

### Pitfall 3: Tree-with-index diff is incomplete

**What goes wrong:** Calling `repo.diff_tree_to_tree(Some(head_tree), Some(index_as_tree), None)` compiles but produces incomplete/wrong results.
**Why it happens:** gix crate-status.md explicitly marks "tree with index" as incomplete. The index cannot be cleanly treated as a tree object.
**How to avoid:** Use the status iterator (`repo.status()...into_iter()`) to enumerate HEAD-vs-index changes. Then do blob-level diffs per changed file.
**Warning signs:** Empty diffs, missing files, or compile errors when trying to convert index to tree OID.

### Pitfall 4: Path encoding

**What goes wrong:** Index entry paths are `BStr` (byte strings), not `&str`. Non-UTF8 paths will panic with `.to_str().unwrap()`.
**Why it happens:** Git paths can contain arbitrary bytes.
**How to avoid:** Use `.to_str_lossy()` for display/PathBuf conversion, or use `BStr` throughout. For file matching (Phase 5), pass `PathBuf` which handles OS path encoding.

### Pitfall 5: HEAD-less repositories (initial commit)

**What goes wrong:** `repo.head_commit()` or `repo.head_tree_id()` fails with "unborn HEAD" error on a fresh repo with no commits.
**Why it happens:** No HEAD means no tree to diff against.
**How to avoid:** Use `repo.head_tree_id_or_empty()` which returns the empty tree OID for unborn repos. For `staged_files()`, this doesn't matter — index entries exist regardless of HEAD.

### Pitfall 6: Patch file race condition

**What goes wrong:** Another tool or hook writes to `.git/fast-staged.patch` simultaneously.
**Why it happens:** Pre-commit hooks can run from editors, CI, or multiple terminals.
**How to avoid:** D-07 says overwrite always. The phase does not need file locking — the pre-commit hook is sequential by git's design.

### Pitfall 7: gix::progress requires a type parameter

**What goes wrong:** `self.repo.status(gix::progress::Discard)?` — the progress type must be explicitly specified.
**Why it happens:** gix's status and other APIs are generic over progress reporters.
**How to avoid:** Use `gix::progress::Discard` as the progress type when no progress reporting is needed. Or use `gix::interrupt::IS_INTERRUPTED` for cancellation support.

### Pitfall 8: Minimal feature flags may break compilation

**What goes wrong:** Using `default-features = false` with a minimal feature set causes compile errors from gix internals that assume certain features exist.
**Why it happens:** gix's feature graph has implicit dependencies between features.
**How to avoid:** Start with `default-features = true` (already the case in Cargo.toml). Only switch to minimal features if binary size measurements show unacceptable bloat. Measure with `cargo bloat` or binary size comparison before optimizing.

---

## Code Examples

Verified patterns from official sources:

### Opening a Repository
```rust
// Source: docs.rs/gix/0.81.0 — gix::discover
let repo = gix::discover(".").map_err(|_| AppError::NotGitRepository)?;
// or with more specific error:
let repo = gix::discover(".").map_err(|e| AppError::Git(e.to_string()))?;
```

### Reading Index Entries (Staged Files)
```rust
// Source: docs.rs/gix/0.81.0 — Repository::open_index
// Source: docs.rs/gix-index/0.37.0 — State::entries, State::path_backing, Entry::stage
let index = repo.open_index().map_err(|e| AppError::Git(e.to_string()))?;
let backing = index.path_backing();
for entry in index.entries() {
    if entry.stage() == 0 {  // 0 = staged (not conflict)
        // entry.path(backing) returns &BStr
        let path: &gix::bstr::BStr = entry.path(&backing);
        // path is repo-relative: e.g. b"src/main.rs"
    }
}
```

### Getting Git Directory Path
```rust
// Source: docs.rs/gix/0.81.0 — Repository::git_dir
let git_dir: &std::path::Path = repo.git_dir();
let patch_path = git_dir.join("fast-staged.patch");
```

### Entry Path to PathBuf
```rust
// BStr to PathBuf — use lossy conversion for PathBuf
use gix::bstr::ByteSlice;
let path_str = entry.path(&backing).to_str_lossy();
let pathbuf = PathBuf::from(path_str.as_ref());
```

### UnifiedDiffBuilder usage (blob level)
```rust
// Source: docs.rs/gix-diff/0.49.0 — blob::UnifiedDiffBuilder
// imara-diff is the underlying engine
use gix_diff::blob::UnifiedDiffBuilder;
use imara_diff::{diff, Algorithm, InternedInput};

let before_bytes: &[u8] = &old_blob_content;
let after_bytes: &[u8] = &new_blob_content;

let before = std::str::from_utf8(before_bytes).unwrap_or_default();
let after = std::str::from_utf8(after_bytes).unwrap_or_default();

let input = InternedInput::new(before, after);
let builder = UnifiedDiffBuilder::new(&input);
let diff_result = diff(Algorithm::Histogram, &input, builder);
let unified_diff_text: String = diff_result.finish();
```

### Checking for Unborn HEAD
```rust
// Source: docs.rs/gix/0.81.0 — Repository::head_tree_id_or_empty
let head_tree_id = repo.head_tree_id_or_empty()
    .map_err(|e| AppError::Git(e.to_string()))?;
// Returns empty tree OID if HEAD is unborn (no commits yet)
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Shell out to `git ls-files --cached` | `repo.open_index()` + entry iteration | gix 0.40+ | No subprocess overhead, direct binary parsing |
| Shell out to `git diff --cached > patch` | gix status + blob-level diff + `UnifiedDiffBuilder` | gix 0.70+ | No subprocess, but requires manual assembly |
| Shell out to `git apply --cached` | No pure-gix equivalent yet | Marked in-progress | Must use git CLI for restore, or use a different strategy |
| `git2` crate (libgit2 binding) | `gix` (pure Rust) | Project decision | No C dependency, smaller binary, Rust-native error handling |

**Deprecated/outdated:**
- `gix::Repository::index_or_load_from_head()`: This loads from HEAD tree if no index exists — wrong for Phase 4. Use `open_index()` for an accurate staged-file snapshot.
- `diff_tree_to_tree` with index as tree: Tree-with-index diff is marked incomplete. Do not use.

---

## Open Questions

1. **Restore strategy: `git apply --cached` vs pure-gix**
   - What we know: gix has no `apply` API. `git apply --cached <patch>` applies a unified diff to the index.
   - What's unclear: Whether `git apply --cached` is acceptable as a one-off shell-out given the "no git CLI" constraint. The constraint says "no shelling out to git CLI" for reading staged files — but restore is an edge case.
   - Recommendation: Allow `git apply --cached` for restore only, documented as a known exception. Alternatively, store the full pre-run index as a serialized snapshot and restore by writing it back — more complex but pure-gix.

2. **Status iterator for HEAD-vs-index changes: exact Item variant**
   - What we know: `gix::status::Item` has `TreeIndex` and `IndexWorktree` variants. `TreeIndex` contains a `gix::diff::index::Change`.
   - What's unclear: The exact API to get blob OIDs from the `Change` to perform blob-level diffs. Needs `cargo doc --open` verification during implementation.
   - Recommendation: During implementation, inspect `gix::diff::index::Change` variants (`Addition`, `Deletion`, `Modification`, `Rewrite`) to extract old/new blob OIDs for diff input.

3. **gix feature flags: minimal vs defaults**
   - What we know: Default features include all needed capabilities. Minimal feature set could reduce binary size.
   - What's unclear: Whether `default-features = false` with minimal features compiles cleanly at gix 0.81.0.
   - Recommendation: Keep default features for now. Add a TODO comment in Cargo.toml for post-v1 size optimization. Measure actual binary size delta before optimizing.

4. **Patch format completeness**
   - What we know: `UnifiedDiffBuilder` produces unified diff text per blob pair. A complete patch needs git-style headers (`diff --git a/... b/...`, `index <sha1>..<sha1>`, etc.).
   - What's unclear: Whether `git apply --cached` requires the full git-format headers or accepts minimal unified diff.
   - Recommendation: During implementation, generate a minimal unified diff and test with `git apply --cached`. If headers are required, prepend them manually using the OIDs from the status change items.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/Cargo | All compilation | Yes | nightly 1.92.0 | — |
| git CLI | `restore()` via `git apply --cached` | Yes | 2.51.2 | Pure-gix index write (complex) |
| gix 0.81.0 | All git ops | Yes (in Cargo.toml) | 0.81.0 | — |

**No blocking missing dependencies.**

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test runner (cargo test) |
| Config file | none — using default cargo test |
| Quick run command | `cargo test --lib -- git` (unit tests in git.rs) |
| Full suite command | `cargo test` (all unit + integration tests) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| GIT-01 | Staged files read from real git index | unit | `cargo test --lib -- git::tests::test_staged_files` | No — Wave 0 |
| GIT-01 | Only stage-0 entries returned (not conflict entries) | unit | `cargo test --lib -- git::tests::test_stage_filter` | No — Wave 0 |
| GIT-02 | Patch file created at `.git/fast-staged.patch` | unit | `cargo test --lib -- git::tests::test_backup_creates_patch` | No — Wave 0 |
| GIT-02 | Patch file contains valid unified diff | unit | `cargo test --lib -- git::tests::test_backup_patch_content` | No — Wave 0 |
| GIT-02 | Overwrite existing stale patch | unit | `cargo test --lib -- git::tests::test_backup_overwrites_existing` | No — Wave 0 |
| GIT-03 | Restore applies patch back to index | integration | `cargo test --test integration_test -- test_restore_on_failure` | No — Wave 0 |
| GIT-04 | Cleanup removes patch file | unit | `cargo test --lib -- git::tests::test_cleanup_removes_patch` | No — Wave 0 |
| GIT-05 | Unstaged files returned for modified-not-staged files | unit | `cargo test --lib -- git::tests::test_unstaged_files` | No — Wave 0 |
| GIT-06 | Non-repo directory returns AppError::NotGitRepository | unit | `cargo test --lib -- git::tests::test_not_git_repo` | No — Wave 0 |
| GIT-07 | Empty index returns AppError::NoStagedFiles | unit | `cargo test --lib -- git::tests::test_no_staged_files` | No — Wave 0 |

**Testing approach for GIT-01 through GIT-05:** Tests must use real git repos (actual `git init` + `git add`), not fake `.git/` directories. gix's `open_index()` reads the actual binary index file — a stub `.git/` directory (as used by config tests) has no index and will fail. Use `std::process::Command("git")` to set up test fixtures.

**Test fixture helper pattern:**
```rust
fn make_real_git_repo(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fast-staged-git-test-{suffix}"));
    std::fs::create_dir_all(&dir).unwrap();
    // Initialize a real git repo
    assert!(Command::new("git").args(["init"]).current_dir(&dir)
        .output().unwrap().status.success());
    // Required for git to work
    Command::new("git").args(["config", "user.email", "test@test.com"])
        .current_dir(&dir).output().unwrap();
    Command::new("git").args(["config", "user.name", "Test"])
        .current_dir(&dir).output().unwrap();
    dir
}

fn stage_file(repo: &Path, name: &str, content: &str) {
    std::fs::write(repo.join(name), content).unwrap();
    Command::new("git").args(["add", name]).current_dir(repo)
        .output().unwrap();
}
```

**This differs from existing tests** which use `fs::create_dir_all(dir.join(".git"))` — those work for config discovery but will not work for gix index operations.

### Sampling Rate

- **Per task commit:** `cargo test --lib -- git`
- **Per wave merge:** `cargo test`
- **Phase gate:** `cargo test` full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- `crates/fast-staged/src/git.rs` — new file covering GIT-01 through GIT-07
- All git unit tests (10 test functions listed above) — all need writing

---

## Project Constraints (from CLAUDE.md)

| Directive | Impact on Phase 4 |
|-----------|-------------------|
| Language: Rust edition 2024 | git.rs uses edition 2024 syntax |
| No shell-out to git CLI (gix crate) | Exception carved out for `git apply --cached` in `restore()` only |
| Async runtime: tokio | `open()`, `staged_files()`, `backup()`, `restore()`, `cleanup()` — all sync internally (gix is sync); called from async `run()` with `?` operator. No `tokio::spawn` needed in git.rs. |
| Error handling: thiserror `AppError` | Use existing `AppError::NotGitRepository`, `AppError::NoStagedFiles`, `AppError::Git(String)`, `AppError::Io` |
| No `Box<dyn Error>` | All functions return `Result<T, AppError>` |
| snake_case files/functions, PascalCase structs | `git.rs`, `GitContext`, `open()`, `staged_files()`, etc. |
| `cargo fmt` + clippy enforced | Run `cargo fmt` and `cargo clippy` before committing |
| Rust nightly toolchain | Already configured; no change needed |
| LTO, opt-level "z", strip (release) | No action in Phase 4; release profile in workspace Cargo.toml already set |

---

## Sources

### Primary (HIGH confidence)

- docs.rs/gix/0.81.0 — `Repository::open_index()`, `Repository::git_dir()`, `Repository::head_tree_id_or_empty()`, `gix::discover()`, `status::Platform`, `status::Item`, `status::Iter`
- docs.rs/gix-index/0.37.0 — `State::entries()`, `State::path_backing()`, `Entry::stage()`, `entries_with_paths_by_filter_map()`
- docs.rs/gix-diff/0.49.0 — `blob::UnifiedDiffBuilder`, `blob::Platform`, `blob::diff()`
- docs.rs/imara-diff — `InternedInput`, `Algorithm`, `diff()`
- GitHub GitoxideLabs/gitoxide `crate-status.md` — "tree with index" diff marked incomplete

### Secondary (MEDIUM confidence)

- cargo add gix --dry-run output — confirmed feature flag list for gix 0.81.0
- docs.rs/gix/0.81.0 default feature list — verified `index`, `status`, `blob-diff`, `parallel` are all enabled by default

### Tertiary (LOW confidence)

- gix status::Item variant exact field shapes — verified from docs.rs but `TreeIndex` change fields need `cargo doc` verification during implementation
- Complete git-format patch headers required by `git apply --cached` — not verified; needs empirical testing

---

## Metadata

**Confidence breakdown:**

- Standard stack (gix version, features): HIGH — verified from Cargo.toml and cargo add dry-run
- Index reading API: HIGH — verified from docs.rs/gix-index 0.37.0 and docs.rs/gix 0.81.0
- Patch generation approach: MEDIUM — `UnifiedDiffBuilder` confirmed; status iterator shape for TreeIndex needs implementation-time verification
- Restore approach: MEDIUM — `git apply --cached` confirmed to exist in git 2.51.2; whether it requires full git-format headers is LOW confidence
- Anti-patterns (tree-with-index incomplete): HIGH — from official crate-status.md

**Research date:** 2026-04-10
**Valid until:** 2026-05-10 (gix releases frequently; re-verify if upgrading past 0.81.x)
