## Why

Planning moved from GSD (`.planning/`) to OpenSpec. Phases 1–6 of the v1 roadmap are shipped but their behavior only existed as phase plans and verification reports. This change records that shipped behavior as the initial `openspec/specs/` baseline so future changes have a source of truth to diff against.

## What Changes

- Capture current, verified behavior of phases 1–6 as specs (no code changes).
- Remaining v1 roadmap phases become separate changes: `add-signal-handling` (Phase 7), `add-terminal-ui` (Phase 8), `add-lint-staged-compat` (Phase 9).
- `.planning/` is retired; its history stays in git.

## Capabilities

### New Capabilities
- `config-loading`: discovery, parsing, settings cascade, and validation of TOML/JSON/package.json config (CFG-01..09)
- `git-staging`: repository detection, staged/unstaged file listing, patch backup/restore/cleanup (GIT-01..07)
- `file-matching`: glob matching of changed files to config patterns (EXEC-06, EXEC-07)
- `command-execution`: running matched commands — batching, ordering, failure propagation, timeouts (EXEC-01..03, EXEC-05)
- `cli`: command-line flags, `--version`, `--help` (CLI-01..03)
- `output-reporting`: success/failure/cancel output, quiet mode, error exit (ERR-01)

### Modified Capabilities
None.

## Impact

Docs only. No code, API, or dependency changes.

## Known gaps (current behavior, not requirements)

Found while writing the baseline; intentionally NOT specified so they are not locked in. Candidates for a future change:
- An unparseable `timeout` string silently means "no timeout" instead of a config error.
- A `!negation` pattern key cannot exclude files matched by a sibling key; negation-only keys are rejected.
- JSON structured-format detection ignores `path_format`, so `{"path_format": "absolute", "*.rs": [...]}` fails with "no patterns defined".
- Unknown keys inside a group are rejected in TOML but ignored in JSON.
- Pattern iteration order inside a group is not guaranteed.

## Deferred (v2 backlog, from REQUIREMENTS.md)

- DIST-01..03: npm platform binary packages (SWC pattern), cross-platform CI matrix, automated npm publish.
- ADV-01: git-stash based full save/restore (beyond patch backup).
- ADV-02 (argv chunking) and ADV-03 (path format) already shipped — covered by `command-execution`.
