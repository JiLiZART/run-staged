## 1. Discovery

- [x] 1.1 Append lint-staged candidates after the run-staged candidates in each directory of the discovery walk, in order `package.json["lint-staged"]`, `package.yaml`/`package.yml` `lint-staged` key, `.lintstagedrc`, `.lintstagedrc.json`, `.lintstagedrc.yaml`, `.lintstagedrc.yml`; a manifest without the key falls through. Verify with unit tests in `crates/config` for each name, the order, and the run-staged-first priority
- [x] 1.2 Detect JS/TS config names (`.lintstagedrc.{js,cjs,mjs,ts,cts,mts}`, `lint-staged.config.{js,cjs,mjs,ts,cts,mts}`) when no static config exists in that directory, and return an invalid configuration error naming the file with a conversion hint. Verify with a unit test per extension family
- [x] 1.3 Make `--config` accept every lint-staged file name with the same parsing rules. Verify with a unit test for `--config .lintstagedrc.yml` and `--config package.yaml`

## 2. Parsing

- [x] 2.1 Add a maintained serde YAML crate (chose `serde-saphyr` 1.3: `serde_yaml_ng` and `serde_norway` were last released in 2024, `serde-saphyr` in 2026-09, pure Rust; `serde_yml` excluded as flagged unmaintained; record in the commit message); parse `.yaml`/`.yml`, `package.yaml`/`package.yml`, and extensionless `.lintstagedrc` (JSON first, then YAML). Verify with unit tests for each, including a YAML-only `.lintstagedrc`
- [x] 2.2 Accept a string command value as a one-element command list; reject other value types with an error naming the key. Verify with unit tests for string, array and number values
- [x] 2.3 Read `package.json["lint-staged"]` as a flat mapping. Verify with a unit test

## 3. Extended globs (crates/matcher)

- [x] 3.1 Record the expected results: run a one-off Node script against `bench/node_modules/picomatch` with lint-staged's options (`dot`, `matchBase` for patterns without `/`, `strictBrackets`) over the spec scenarios plus 20–30 patterns from real lint-staged configs; commit the results as `crates/matcher/tests/picomatch_cases.json` (pattern, path, expected). Verify the file loads in a Rust test
  - 38 paths × 34 patterns from picomatch 4.0.7, stored per pattern (5 KB); generator script kept out of the repo, its output records the picomatch version and options
- [x] 3.2 Add an `extglob` module: a parser for `*`, `**`, `?`, `[...]`, `{a,b}`, `(a|b)`, `@()`, `?()`, `+()`, `*()` and `!()` with escapes and nesting, translated to an anchored regex per design D2 and compiled with `fancy-regex`; unbalanced or empty groups return `MatchError::InvalidPattern`. Verify with unit tests for each operator, nesting, escapes, and every error case
- [x] 3.3 Route patterns containing an unescaped `(` to `extglob` after match-base normalization, leaving the globset path and the match-all fast path unchanged. Verify that every case in `picomatch_cases.json` passes and that all existing matcher tests pass unchanged
  - The parity test also exercises the globset path; it exposed `*` crossing `/` there (`.*` matched `.github/workflows/ci.yml`). Fixed with `literal_separator(true)`; spec, proposal (BREAKING) and design (D3b) updated. Leading `!(` routes to extglob, so the negation requirement got an exception (delta spec)
- [x] 3.4 Point the invalid-pattern detail for an unbalanced `(` at escaping (`use \( or \) to match a literal parenthesis`). Verify with a unit test on the message

## 4. Verification

- [x] 4.1 Process-level test: a repo with only `.lintstagedrc.json` containing `{"*.(js|ts|tsx|jsx)": "fixture-tool"}` runs the command on `packages/diir/subdir/somedir/file.test.ts` and not on `README.md`
- [x] 4.2 Process-level test: a repo with only `lint-staged.config.js` exits 1 with the JS/TS error naming the file
- [x] 4.3 Process-level test: `*.(js|ts` in `run-staged.toml` exits 1 with `invalid glob pattern "*.(js|ts"`
- [x] 4.4 Performance check: on the 10-file fixture, a group pattern `*.(js|ts|css|md)` costs no more than the equivalent brace pattern `*.{js,ts,css,md}` (target changed from "≤ 30 ms" by decision on 2026-10-09: both pay the regex engine's first compile)
  - Met: interleaved hyperfine, 10-file fixture, 60 runs: `*` 28.9 ± 2.4 ms, `*.{js,ts,css,md}` (existing globset path) 32.4 ± 2.2, `*.(js|ts|css|md)` 32.1 ± 3.1. Groups cost the same as braces: the ~3 ms is the regex engine's first compile, paid by any alternation pattern. Follow-up (separate change): expand simple alternations into extension globs so globset skips the regex engine (~3 ms for both braces and groups)
- [x] 4.5 Update the README config section with the lint-staged file names, the new glob syntax, and the escaping note for literal parentheses; run `cargo fmt -- --check` on touched files, `cargo clippy --workspace -- -D warnings` and `cargo test --workspace`, and verify all pass
