## Why

Users migrating from lint-staged should be able to run run-staged against their existing config without rewriting it. This is roadmap Phase 9 (CFG-10, CFG-11). Reading the config file is not enough: lint-staged matches with picomatch, so configs commonly use syntax our globset matcher doesn't understand. `*.(js|ts|tsx|jsx)` is accepted today but **silently matches nothing**, because globset treats `(` and `|` as literal characters. A pattern like that has to match `packages/diir/subdir/somedir/file.test.ts`, just as it does in lint-staged.

## What Changes

- Discover lint-staged static configs at lower priority than run-staged configs, in lint-staged 17's own order: `package.json["lint-staged"]`, `package.yaml`/`package.yml` `lint-staged` key, `.lintstagedrc`, `.lintstagedrc.json`, `.lintstagedrc.yaml`, `.lintstagedrc.yml`.
- Accept lint-staged command values as a string or an array of strings.
- Detect JavaScript **and TypeScript** lint-staged configs (`.lintstagedrc.{js,cjs,mjs,ts,cts,mts}`, `lint-staged.config.{js,cjs,mjs,ts,cts,mts}`) and fail with a clear error suggesting a static config.
- **Glob syntax:** in addition to `* ** ? [...] {a,b}`, accept picomatch groups `(a|b)` and extglobs `@(..)`, `?(..)`, `+(..)`, `*(..)`, `!(..)`, in every config format. Match-base for patterns without `/` and dotfile matching stay as they are, and already agree with lint-staged's `matchBase`/`dot: true`.
- Unbalanced `(`, `[` or `{` becomes an invalid-pattern config error instead of silently never matching.
- **BREAKING:** `*` and `?` no longer match `/` in globset patterns (they did, because globset's `literal_separator` was off), so `src/*.ts` stops matching `src/lib/b.ts`, as in lint-staged; use `src/**/*.ts` for recursion. Found when checking run-staged against picomatch's recorded results.
- **BREAKING:** an unescaped `(` in a pattern now starts a group, as in lint-staged. A pattern meant to match literal parentheses, such as Next.js route groups `app/(auth)/**`, must escape them: `app/\(auth\)/**`. run-staged has no published release yet, so impact is limited to local configs.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `config-loading`: adds lint-staged config discovery (including `package.yaml`/`package.yml`) and JS/TS-config rejection.
- `file-matching`: "Glob syntax" adds groups and extglobs, and makes unbalanced delimiters an error.

## Impact

- Code: `crates/config/src/discover.rs` (candidate list), `crates/config/src/parse_json.rs` (string command values), a new YAML parse path in `crates/config`, and `crates/matcher` (an extended-glob compiler next to globset).
- Dependencies:
  - one YAML deserializer. `serde_yaml` is unmaintained, so pick a maintained serde-compatible crate and record the choice in the implementation commit;
  - `fancy-regex`, for `!(..)`, which needs negative lookahead.
- Out of scope:
  - JS/TS function configs (project-level decision);
  - lint-staged `--diff`/`--relative` CLI flags;
  - per-package lint-staged configs in monorepos, which match relative to each config's own directory;
  - picomatch features beyond groups and extglobs (POSIX classes like `[[:alpha:]]`, numeric ranges like `{1..3}`).
