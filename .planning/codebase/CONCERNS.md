# Codebase Concerns

**Analysis Date:** 2026-04-09

## Tech Debt

**Project is an empty stub — no implementation exists:**
- Issue: The entire application logic has not been written. The only Rust source file is a `Hello, world!` placeholder.
- Files: `crates/cli/src/main.rs`
- Impact: Nothing is functional. No config loading, no git integration, no staged file detection, no command execution, no TUI.
- Fix approach: Implement all features described in `TODO.md` and `ROADMAP.md` from scratch.

**`crates/cli/Cargo.toml` has no dependencies:**
- Issue: The crate has `[dependencies]` declared but empty. Every dependency needed for the real implementation (tokio, ratatui, gix, serde, thiserror, clap, etc.) is absent.
- Files: `crates/cli/Cargo.toml`
- Impact: No async runtime, no TUI, no git access, no config parsing — nothing will build beyond Hello World.
- Fix approach: Add all required dependencies before writing any feature code.

**Workspace `Cargo.lock` is gitignored:**
- Issue: `.gitignore` contains `Cargo.lock`. For a binary application (not a library), the lockfile should be committed to guarantee reproducible builds.
- Files: `.gitignore` (line 119)
- Impact: Different developers and CI builds may resolve different dependency versions, causing hard-to-reproduce bugs.
- Fix approach: Remove `Cargo.lock` from `.gitignore` for the binary workspace. Library crates should omit it; binary workspaces should commit it.

**CI workflow is mismatched to the actual project type:**
- Issue: `.github/workflows/CI.yml` is a NAPI-RS Node.js native addon workflow (references `yarn build`, `napi create-npm-dirs`, `.node`/`.wasm` artifacts, `NPM_TOKEN` publishing to npm). The codebase contains no NAPI-RS code — it is a plain Rust binary.
- Files: `.github/workflows/CI.yml`
- Impact: Every CI run will fail at the build step because there are no NAPI-RS bindings and no `package.json` build script. Publish step is unreachable.
- Fix approach: Replace the workflow with a standard Rust binary CI: `cargo build --release`, `cargo test`, `cargo clippy`, `cargo fmt --check`. Remove all NAPI-RS, yarn build, artifact upload, and npm publish steps.

**`package.json` / Node.js tooling present with no apparent purpose:**
- Issue: `.yarnrc.yml`, `.yarn/releases/yarn-4.13.0.cjs`, `.husky/pre-commit`, `.prettierignore`, and renovate.json all reference a Node.js/Yarn ecosystem. The pre-commit hook runs `yarn lint-staged`. There is no `package.json` in the repository root.
- Files: `.yarnrc.yml`, `.husky/pre-commit`, `.yarn/releases/yarn-4.13.0.cjs`, `.prettierignore`
- Impact: `yarn install` and `yarn lint-staged` (called in the pre-commit hook) will fail for any developer who clones the repo, because `package.json` is missing. The husky hook is broken on first checkout.
- Fix approach: Either add a `package.json` defining the Node.js tooling (oxlint, lint-staged, prettier) or remove all Node.js tooling and replace the pre-commit hook with a Rust-native equivalent (`cargo fmt`, `cargo clippy`).

**Unstable Rust compiler flags used in `.cargo/config.toml`:**
- Issue: The build config applies `-Zlocation-detail=none`, `-Zfmt-debug=none`, and `-Zunstable-options` globally. These are nightly-only flags. The CI installs stable toolchain (`dtolnay/rust-toolchain@stable`), causing a contradiction.
- Files: `.cargo/config.toml`
- Impact: `cargo build` and `cargo clippy` will fail on stable toolchain due to unrecognized `-Z` flags.
- Fix approach: Either switch the CI to a nightly toolchain, or remove the `-Z` flags from the global `[build]` section and only apply them conditionally (e.g., via a nightly profile or a separate release script).

**`[target.release]` section in `.cargo/config.toml` is invalid:**
- Issue: The section `[target.release]` is not a valid cargo config key. Valid sections are `[profile.release]` (in `Cargo.toml`) or `[target.<triple>]` for target-specific flags. `-Cpanic=immediate-abort` under `[target.release]` is silently ignored.
- Files: `.cargo/config.toml` (line 8)
- Impact: The intended `panic=immediate-abort` optimization for release builds does not apply, silently.
- Fix approach: Move `panic = "abort"` into `[profile.release]` in the workspace `Cargo.toml` (it is already declared there correctly). Remove the bogus `[target.release]` section from `.cargo/config.toml`.

**IDEA project files committed to repository:**
- Issue: `.idea/` directory with JetBrains IDE files is committed (`.idea/.gitignore` itself only ignores some internal files, not the directory).
- Files: `.idea/` (entire directory)
- Impact: Developer-specific IDE configuration pollutes the repository. Settings like `.idea/php.xml` suggest copy-paste from a PHP project.
- Fix approach: Add `.idea/` to `.gitignore` and remove the directory from git history.

**`.idea/php.xml` indicates copy-paste from a different project:**
- Issue: JetBrains PHP plugin configuration is committed in a pure Rust project.
- Files: `.idea/php.xml`
- Impact: Cosmetic noise; suggests the project skeleton was copied from a PHP project without cleanup.
- Fix approach: Delete `.idea/php.xml` and add `.idea/` to `.gitignore`.

## Known Bugs

**Pre-commit hook will fail on every commit:**
- Symptoms: Running `git commit` calls `.husky/pre-commit` which invokes `yarn lint-staged`. With no `package.json` present, yarn exits with an error, blocking all commits.
- Files: `.husky/pre-commit`
- Trigger: Any `git commit` attempt.
- Workaround: Use `git commit --no-verify` (bypasses hooks) or temporarily disable the hook.

## Security Considerations

**NPM_TOKEN secret expected by CI but project may not own an npm package:**
- Risk: The CI publish step writes `NPM_TOKEN` to `~/.npmrc`. If a fork runs the workflow or the secret is misconfigured, it could publish unexpected packages under any npm account associated with the token.
- Files: `.github/workflows/CI.yml` (lines 279–290)
- Current mitigation: Publish only triggers on commits matching a semver pattern in the commit message.
- Recommendations: Since the project is not yet a NAPI-RS package, remove the publish step entirely until the package identity and scope are established.

## Performance Bottlenecks

**No performance concerns applicable — no implementation exists.**

## Fragile Areas

**All planned features documented only in markdown, not in code:**
- Files: `TODO.md`, `ROADMAP.md`
- Why fragile: The entire implementation plan exists only in documentation. There is no skeleton code, no module structure, no types, no tests. Any deviation from the plan during implementation has no guardrails.
- Safe modification: Treat `TODO.md` as the authoritative implementation spec. Follow the item-by-item plan described there.
- Test coverage: Zero — no tests exist.

## Scaling Limits

**Not applicable — no implementation exists to evaluate.**

## Dependencies at Risk

**No Rust dependencies declared yet:**
- Risk: `crates/cli/Cargo.toml` has an empty `[dependencies]` section. All dependencies referenced in `TODO.md` (gix, tokio, ratatui, thiserror, clap, serde, toml, serde_json, parse_duration) need to be evaluated for compatibility and security before adding.
- Impact: Nothing works without them; adding them all at once without auditing may introduce transitive vulnerabilities.
- Migration plan: Add dependencies incrementally per feature, run `cargo audit` after each addition.

**Pinned Yarn release bundled in repository:**
- Risk: `.yarn/releases/yarn-4.13.0.cjs` (a 9MB binary) is committed. If a security issue is found in this Yarn version, the file must be manually replaced.
- Impact: Repository size bloat; stale toolchain.
- Migration plan: Upgrade via `yarn set version stable` when needed.

## Missing Critical Features

**Entire application is unimplemented:**
- Problem: `crates/cli/src/main.rs` only prints "Hello, world!". No git integration, no config loading, no pattern matching, no command execution, no TUI.
- Blocks: The tool cannot be used for its stated purpose (running lint-staged commands on git staged files).

**No error handling infrastructure:**
- Problem: No `AppError` enum, no typed errors, no `Result`-returning functions beyond `main`.
- Blocks: Any feature implementation must establish this foundation first (described in `TODO.md` under "Add checks and readable errors").

**No signal handling / graceful exit:**
- Problem: No Ctrl+C handling. Any running child processes would be orphaned on exit.
- Blocks: Safe use in pre-commit hooks where interrupts are common.

**No CLI argument parsing:**
- Problem: No `--config`, `--quiet`, `--unstaged`, or `--allow-empty` flags exist.
- Blocks: Users cannot customize behavior or integrate into non-standard workflows.

## Test Coverage Gaps

**Zero test coverage:**
- What's not tested: Everything — config loading, git integration, pattern matching, command execution, TUI rendering, error paths, signal handling.
- Files: `crates/cli/src/main.rs` (only file, no tests)
- Risk: Any implementation will ship with no regression safety net.
- Priority: High — establish at minimum unit tests for config parsing and file-pattern matching before other features.

---

*Concerns audit: 2026-04-09*
