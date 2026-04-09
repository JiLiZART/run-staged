# Codebase Structure

**Analysis Date:** 2026-04-09

## Directory Layout

```
fast-staged/
├── crates/                  # Rust Cargo workspace members
│   └── fast-staged/         # The sole crate (binary + future lib)
│       ├── Cargo.toml       # Crate manifest (no dependencies yet)
│       └── src/
│           └── main.rs      # Binary entry point (stub)
├── .cargo/
│   └── config.toml          # Cargo build flags (release optimization, Windows CRT static)
├── .github/
│   └── workflows/
│       └── CI.yml           # CI: lint, multi-platform build, NAPI-RS publish to npm
├── .husky/
│   └── pre-commit           # Runs yarn lint-staged before each commit
├── .planning/
│   └── codebase/            # GSD mapper output (this file)
├── .yarn/
│   └── releases/            # Yarn Berry binary
├── Cargo.toml               # Workspace manifest with release profile optimizations
├── .yarnrc.yml              # Yarn configuration
├── .taplo.toml              # TOML formatter config
├── .prettierignore          # Prettier ignore rules
├── .editorconfig            # Editor formatting baseline
├── .gitattributes           # Git line-ending rules
├── .gitignore               # Ignored files
├── ROADMAP.md               # Feature roadmap (source of truth for planned features)
└── TODO.md                  # Detailed implementation plans per feature
```

## Directory Purposes

**`crates/`:**
- Purpose: Cargo workspace members — all Rust source code lives here
- Contains: One crate (`cli`) today; intended to expand as the project grows
- Key files: `crates/fast-staged/Cargo.toml`, `crates/fast-staged/src/main.rs`

**`crates/fast-staged/src/`:**
- Purpose: All Rust source for the CLI binary (and future library)
- Current state: Single `main.rs` stub
- Planned modules (not yet created): `lib.rs`, `config.rs`, `commands.rs`, `error.rs`, `renderer.rs`, `git.rs`

**`.cargo/`:**
- Purpose: Workspace-level Cargo configuration
- Key file: `.cargo/config.toml` — sets Windows CRT static linking, nightly `-Zlocation-detail=none` and `-Zfmt-debug=none` flags for binary size reduction

**`.github/workflows/`:**
- Purpose: CI/CD via GitHub Actions
- Key file: `CI.yml` — runs Oxlint + cargo fmt/clippy on lint job; builds NAPI-RS `.node` + `.wasm` artifacts for Windows, Linux GNU, Linux musl, WASM; tests on Node 20 & 22; publishes to npm on version-tagged commits

**`.husky/`:**
- Purpose: Git hooks managed by Husky
- Key file: `pre-commit` — runs `yarn lint-staged`

**`.planning/codebase/`:**
- Purpose: GSD codebase mapping documents consumed by `/gsd:plan-phase` and `/gsd:execute-phase`
- Generated: Yes (by GSD mapper)
- Committed: Yes

## Key File Locations

**Entry Points:**
- `crates/fast-staged/src/main.rs`: Binary entry point — currently a stub

**Workspace Configuration:**
- `Cargo.toml`: Workspace definition + release profile (LTO, size optimization, strip symbols)
- `.cargo/config.toml`: Build flags for cross-compilation and binary size

**CI/CD:**
- `.github/workflows/CI.yml`: Full pipeline — lint → build (4 targets) → test → publish to npm

**Roadmap / Intent:**
- `ROADMAP.md`: High-level feature list
- `TODO.md`: Per-feature implementation plans with planned function signatures, struct shapes, and code snippets

## Naming Conventions

**Files:**
- Rust source modules: `snake_case.rs` (e.g., `main.rs`, planned: `config.rs`, `executor.rs`, `error.rs`)
- Config files: kebab-case with dot prefix (`.fast-staged.toml`, `fast-staged.toml`)
- CI files: `UPPERCASE.yml` (e.g., `CI.yml`)

**Directories:**
- Rust crates: lowercase (`cli`)
- Hidden tool directories: dot-prefixed (`.cargo`, `.husky`, `.github`, `.yarn`, `.planning`)

**Rust Identifiers (planned conventions from TODO.md):**
- Functions: `snake_case` (e.g., `find_config_file`, `execute_commands`, `run_single_command`)
- Structs: `PascalCase` (e.g., `TaskState`, `Config`, `Group`, `AppError`)
- Enums: `PascalCase` with `PascalCase` variants (e.g., `CommandStatus::Running`, `ExecutionOrder::Parallel`)
- Traits: `PascalCase` (e.g., `StatusDisplay`)

## Where to Add New Code

**New Rust module (e.g., config parsing):**
- Implementation: `crates/fast-staged/src/<module_name>.rs`
- Declare in: `crates/fast-staged/src/lib.rs` (once created) with `pub mod <module_name>;`

**New dependency:**
- Add to: `crates/fast-staged/Cargo.toml` under `[dependencies]`
- For workspace-shared deps (when more crates added): `Cargo.toml` under `[workspace.dependencies]`

**New CLI subcommand or flag:**
- Entry: `crates/fast-staged/src/main.rs` (or a dedicated `cli.rs` module using `clap`)

**New test:**
- Unit tests: inline in the relevant source file using `#[cfg(test)] mod tests { ... }`
- Integration tests: `crates/fast-staged/tests/` directory (not yet created)

**New config file format support:**
- Config discovery: extend `find_config_file()` in `crates/fast-staged/src/config.rs`

**New CI target:**
- Add matrix entry to the `build` job in `.github/workflows/CI.yml`

## Special Directories

**`.planning/`:**
- Purpose: GSD planning artifacts (codebase maps, phase plans)
- Generated: Partially (codebase maps are generated; phase plans are authored)
- Committed: Yes

**`target/`:**
- Purpose: Cargo build output (not present in listing — generated at build time)
- Generated: Yes
- Committed: No (in `.gitignore`)

**`.yarn/releases/`:**
- Purpose: Yarn Berry self-contained binary for reproducible installs
- Generated: No (checked in)
- Committed: Yes

---

*Structure analysis: 2026-04-09*
