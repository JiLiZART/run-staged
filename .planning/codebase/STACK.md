# Technology Stack

**Analysis Date:** 2026-04-09

## Languages

**Primary:**
- Rust (edition 2024) - All application logic in `crates/fast-staged/src/`

**Secondary:**
- None currently present in source; `.prettierignore` references JS artifacts (wasi), suggesting future WASM/JS output may be planned

## Runtime

**Environment:**
- Native binary (compiled Rust)
- Target: cross-platform; explicit Windows MSVC static CRT config in `.cargo/config.toml`

**Package Manager:**
- Cargo (Rust toolchain) — workspace at `Cargo.toml`
- Lockfile: `Cargo.lock` — NOT present yet (project is in skeleton state)
- Yarn 4.13.0 also present (`.yarn/releases/yarn-4.13.0.cjs`, `.yarnrc.yml`) — used for dev tooling (husky, lint-staged) not for the Rust binary itself

## Frameworks

**Core:**
- None declared yet — `crates/fast-staged/Cargo.toml` has an empty `[dependencies]` section

**Planned (documented in `TODO.md` and `ROADMAP.md`):**
- `tokio` (async runtime, `features = ["full"]`) — async command execution, signal handling
- `ratatui` — terminal UI rendering
- `gix` — git operations (staged file listing, stash, diff)
- `clap` — CLI argument parsing (`--config`, `--quiet`, `--unstaged`, `--allow-empty`)
- `thiserror` — typed error enum (`AppError`)
- `toml` — TOML config parsing
- `serde_json` — JSON config parsing (`fast-staged.json`, `package.json` section)
- `parse_duration` — human-readable timeout parsing (e.g., `"1sec"`, `"5s"`)

**Build/Dev:**
- Taplo — TOML formatter, config at `.taplo.toml`
- Prettier — JS/general formatter, ignore rules at `.prettierignore`
- Husky — git hooks, config at `.husky/pre-commit`
- lint-staged — staged file linting, invoked via husky pre-commit hook

## Key Dependencies

**Critical (planned):**
- `gix` — core git interaction (staged files, stash, patch diff)
- `ratatui` — full TUI rendering with live task progress
- `tokio` — async runtime for parallel command execution

**Infrastructure:**
- `thiserror` — structured error types
- `toml` + `serde_json` — multi-format config loading

## Configuration

**Environment:**
- No environment variables required at runtime
- Tool reads project-local config files in this priority order (planned):
  1. `.fast-staged.toml`
  2. `fast-staged.toml`
  3. `.fast-staged.json`
  4. `fast-staged.json`
  5. `"fast-staged"` key inside `package.json`

**Build:**
- `Cargo.toml` (workspace) — release profile: `lto = true`, `opt-level = "z"`, `strip = true`, `panic = "abort"`, `codegen-units = 1`
- `.cargo/config.toml` — Windows MSVC static CRT; unstable `rustflags` for size/debug-info stripping (`-Zlocation-detail=none`, `-Zfmt-debug=none`)
- `.taplo.toml` — TOML formatter settings (aligned entries, indented tables, reordered keys)
- `.editorconfig` — 2-space indent, LF line endings, UTF-8

## Platform Requirements

**Development:**
- Rust toolchain (nightly implied by `-Z` flags in `.cargo/config.toml`)
- Yarn 4 (for dev tooling: husky, lint-staged)
- Taplo CLI (TOML formatting)

**Production:**
- Compiled native binary
- Designed to run as a git pre-commit hook (replaces/complements lint-staged/nano-staged)
- No server or network access required

---

*Stack analysis: 2026-04-09*
