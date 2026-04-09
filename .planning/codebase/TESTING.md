# Testing Patterns

**Analysis Date:** 2026-04-09

## Project State

This project is at initial scaffolding stage. No tests exist yet. The only source file is
`crates/cli/src/main.rs` — a 3-line Hello World stub. All testing infrastructure below
describes what is configured or planned, not what has been written.

## Test Framework

**Rust unit/integration tests:**
- Framework: built-in Rust `#[test]` / `#[tokio::test]`
- Runner: `cargo test`
- No external Rust testing crate configured yet
- Expected addition: `tokio` test runtime for async tests once async code is introduced

**JavaScript binding tests:**
- Runner: configured via `yarn test` (referenced throughout CI)
- Framework: not yet configured — no `package.json`, no `jest.config.*`, no `vitest.config.*` found
- These tests validate the compiled NAPI `.node` or `.wasm` bindings
- Run after build artifacts are produced (CI gates: `test-macOS-windows-binding`, `test-linux-binding`, `test-wasi`)

**Run Commands:**
```bash
cargo test                  # Run all Rust unit and integration tests
cargo test -- --nocapture   # Run tests with stdout visible
yarn test                   # Run JS binding tests (requires built .node artifact)
cargo clippy                # Static analysis (used as quality gate in CI)
cargo fmt -- --check        # Format check (used as quality gate in CI)
```

## Test File Organization

**Rust unit tests:**
- Convention: inline `#[cfg(test)]` module at the bottom of each source file
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_something() { ... }
  }
  ```
- Integration tests: `crates/cli/tests/` directory (not yet created)

**JavaScript binding tests:**
- Expected location: project root or a `__tests__/` directory (not yet created)
- Naming convention: not yet established

## Test Structure

**Planned unit test targets** (derived from TODO.md):
- `load_config()` — happy path with valid TOML, JSON, package.json section
- `find_config_file()` — priority order: `.fast-staged.toml` > `fast-staged.toml` > `.fast-staged.json` > `fast-staged.json` > `package.json`
- `match_files_to_commands()` — glob pattern matching, group assignment
- `execute_commands()` — parallel vs sequential execution, timeout handling
- `AppError` variants — error message formatting

**Async test pattern** (planned, when tokio is added):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[tokio::test]
    async fn test_execute_commands_parallel() {
        // ...
    }
}
```

## Mocking

**Framework:** Not yet determined. No mock crate in `Cargo.toml`.
- Likely candidates once implemented: `mockall` for trait mocking, `tempfile` for filesystem tests

**Planned test isolation patterns** (from design docs):
- Use temporary directories for config file tests
- Use a real git repo fixture or mock `gix` calls for staged file tests
- Use `Arc<AtomicBool>` cancellation flags in tests to verify graceful shutdown

**What to mock:**
- `gix` repository access (avoid requiring a real git repo in unit tests)
- File system operations in config loading tests
- External command execution in `execute_commands()` tests

**What NOT to mock:**
- TOML/JSON parsing (test with real config strings)
- Error message formatting (test actual `AppError` Display output)

## Fixtures and Factories

**Test Data:**
- Config fixture strings should be defined inline as `const` or `static` within test modules
- Expected pattern:
  ```rust
  const VALID_CONFIG: &str = r#"
  timeout = "1sec"

  [group1.patterns]
  "*.rs" = ["cargo fmt"]
  "#;
  ```

**Location:**
- Inline constants in `#[cfg(test)]` modules for unit tests
- `crates/cli/tests/fixtures/` for integration test files (not yet created)

## Coverage

**Requirements:** None enforced — no coverage tool configured.

**Recommended addition:**
```bash
cargo tarpaulin --out Html   # Once tarpaulin is added as dev dependency
```

## Test Types

**Unit Tests:**
- Scope: individual functions (config parsing, pattern matching, error creation)
- Location: inline `#[cfg(test)]` in each `.rs` source file

**Integration Tests:**
- Scope: full CLI invocation against a real git repo
- Location: `crates/cli/tests/` (planned, not yet created)
- Requires: staged git files, valid config file on disk

**E2E / Binding Tests:**
- Framework: JavaScript test runner (`yarn test`) against compiled `.node` binary
- Runs in CI only after a successful `build` job
- Tested targets: Windows x86_64, Linux GNU, Linux musl (via Docker), WASI

## CI Testing Pipeline

Tests are gated in `.github/workflows/CI.yml`:

1. **lint** job — runs before any tests:
   - `yarn lint` (oxlint)
   - `cargo fmt -- --check`
   - `cargo clippy`

2. **build** job — compiles NAPI binaries for 4 targets

3. **test-macOS-windows-binding** — `yarn test` on Windows with Node 20 and 22

4. **test-linux-binding** — `yarn test` inside Docker containers for GNU and musl with Node 20 and 22

5. **test-wasi** — `yarn test` with `NAPI_RS_FORCE_WASI=1` env var

6. **publish** — only runs after all lint and test jobs pass

## Common Patterns

**Error Testing (planned):**
```rust
#[test]
fn test_config_not_found_error() {
    let result = load_config();
    assert!(matches!(result, Err(AppError::ConfigNotFound { .. })));
}
```

**Async Testing (planned):**
```rust
#[tokio::test]
async fn test_command_timeout() {
    let result = execute_with_timeout("sleep 10", Duration::from_millis(100)).await;
    assert!(matches!(result, Err(AppError::Timeout)));
}
```

**Signal/Cancellation Testing (planned):**
```rust
#[tokio::test]
async fn test_cancellation_flag() {
    let flag = Arc::new(AtomicBool::new(false));
    flag.store(true, Ordering::Relaxed);
    let result = execute_commands(vec![], flag).await;
    assert!(matches!(result, Err(AppError::Cancelled)));
}
```

---

*Testing analysis: 2026-04-09*
