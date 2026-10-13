## Context

`crates/matcher` compiles each pattern with globset after match-base normalization (a pattern without `/` becomes `**/<pattern>`), and `*`, `**` and `**/*` take a fast path that skips compilation. globset has no groups or extglobs: `(`, `|` and `)` are literal, so `*.(js|ts)` compiles fine and matches nothing. globset also drops empty brace alternatives (`*.{,test.}ts` doesn't match `b.ts`), so extglobs can't be rewritten into braces.

lint-staged 17 matches with `picomatch(pattern, { dot: true, matchBase: !pattern.includes('/'), posixSlashes: true, strictBrackets: true })`, against paths relative to the config's directory. Every scenario in `specs/file-matching/spec.md` was checked against picomatch 4 with exactly these options.

Config discovery lives in `crates/config/src/discover.rs`. JSON parsing is in `parse_json.rs`, which already accepts the flat glob-to-commands shape.

## Goals / Non-Goals

**Goals:**
- Patterns from real lint-staged configs match the same files they match in lint-staged, for the syntax in the spec.
- No behavior change for patterns without an unescaped `(`. They keep going through globset unchanged.
- Bad patterns fail loudly at config load, never by silently matching nothing.

**Non-Goals:**
- A general picomatch port: no POSIX classes, no `{1..3}` ranges, no `nocase`/`basename` options.
- Per-package configs that match relative to their own directory.
- JS/TS configs (rejected with an error, per project decision).

## Decisions

**D1. Route by syntax: globset stays the default, and a small extended-glob compiler handles patterns with groups.**
A pattern containing an unescaped `(` goes to a new `extglob` module in `crates/matcher`. Everything else keeps today's globset path, so existing configs and tests are untouched.
- Alternative 1: a picomatch port crate (`satch`, `picomatch-rs`, `ferralk-glob`). All are 0.x or under two months old, with a few hundred to 4 000 downloads. Rejected: matching is the core of the tool, and none of these has a track record.
- Alternative 2: replace globset entirely with the new compiler. Rejected: it adds regression risk to every existing pattern for no user-visible gain.

**D2. The compiler translates the glob into a regex, the way picomatch does, and builds it with `fancy-regex`.**
Translation, per path segment:

| Glob | Regex |
|:---|:---|
| `*` | `[^/]*` |
| `?` | `[^/]` |
| `**` as a whole segment | `(?:[^/]*(?:/|$))*` |
| `[...]` | the same class, kept within the segment |
| `{a,b}` | `(?:a|b)` |
| `(a|b)` / `@(a|b)` | `(?:a|b)` |
| `?(a|b)` | `(?:a|b)?` |
| `+(a|b)` | `(?:a|b)+` |
| `*(a|b)` | `(?:a|b)*` |
| `!(a|b)` | `(?:(?!(?:a|b)<rest-of-segment>$)[^/]*?)` |

Alternatives are compiled recursively, so nesting works. Everything else is escaped as a literal, and the result is anchored with `^…$`. `!(...)` needs negative lookahead, which `regex` doesn't support. `fancy-regex` adds lookaround and hands every pattern without it to the `regex` crate, which is already in the tree through gix. It is the one new matching dependency: mature (about 250M downloads), with `regex`/`regex-syntax` as its main dependencies.
- Alternative: a hand-written backtracking matcher with no new dependency. Rejected: more code to get right than a regex translation, and `!(...)` combined with `*` backtracking is exactly where hand-written matchers go wrong.

**D3. Match-base and dotfiles work the same on both paths.**
The existing normalization (no `/` → prefix `**/`) runs before routing, so `*.(js|ts)` becomes `**/*.(js|ts)` and matches at any depth. `*`, `?` and groups match a leading `.`, as globset already does today and as lint-staged's `dot: true` requires.

**D3b. globset gets `literal_separator(true)`.**
Checking the globset path against picomatch's recorded results showed that `*` crossed `/` there (`.*` matched `.github/workflows/ci.yml`). Turning on the separator rule makes both paths agree with picomatch on every recorded case.

**D4. Parse errors become the existing invalid-pattern error.**
The compiler tracks `(`, `[` and `{` nesting with escape handling. Any unclosed delimiter, or a `|` outside a group, gives `MatchError::InvalidPattern`, which already prints `error: invalid configuration: invalid glob pattern "<pattern>": <detail>`. An empty group `()` is an error too.

**D5. lint-staged discovery reuses the existing candidate walk.**
The lint-staged names are appended after the run-staged names in each directory, in lint-staged's own order. `package.json` and `package.yaml`/`package.yml` count only when they have a `lint-staged` key; otherwise discovery moves on. The JS/TS names are checked last, and only produce the rejection error when no static config was found in that directory.
- YAML: pick a maintained serde-compatible crate at implementation time, because `serde_yaml` is archived. `serde_yaml_ng` and `serde_norway` are the current forks. Choose by maintenance activity, and record the choice in the commit message.

**D6. Command values:** a string `"eslint --fix"` becomes `["eslint --fix"]`. Arrays are used as they are. Any other value type is a config error naming the key.

## Risks / Trade-offs

- [The BREAKING change: a literal `(` in existing run-staged patterns now starts a group] → called out in the proposal. The error message for an unbalanced group suggests escaping, as picomatch does ("use `\)` to match literal characters"). There is no published release yet.
- [The translated regex differs from picomatch in an edge case] → a table test checks each spec scenario, plus a corpus of 20–30 patterns taken from popular lint-staged configs, against expected results recorded from picomatch 4. The test is generated once with `bench/node_modules/picomatch` and committed as data, so CI needs no Node.
- [`!(...)` regexes with lookahead can backtrack badly on pathological input] → paths are short (a few hundred bytes), and fancy-regex has a backtrack limit that gives an error instead of hanging. That error counts as a non-match; the matcher has no warning channel, and staged paths are far too short to reach the limit.
- [Extended patterns compile slower than globset] → only patterns containing `(` pay that cost, once per run. Measured: a group pattern costs the same as the equivalent brace pattern (32.1 vs 32.4 ms at 10 files); both pay the regex engine's ~3 ms first compile, which a later change can avoid for simple extension alternations.
