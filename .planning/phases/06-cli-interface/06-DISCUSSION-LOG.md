# Phase 6: CLI Interface - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-29
**Phase:** 06-cli-interface
**Areas discussed:** --config missing-path behavior, Unknown-flag handling

---

## Gray Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| --config missing-path behavior | If user passes --config /missing.toml, hard-fail with ConfigNotFound, or warn + fall back to auto-discovery? Affects error UX and CLI-01 contract semantics. | ✓ |
| Help/version text style | Default clap minimal vs custom long-form with examples + about + repo URL. Also: should --help include a config-format hint pointing at fast-staged.toml shape? | |
| Short flag aliases | Add -q (quiet), -c (config), -V (version), -h (help)? lint-staged compat / Unix convention vs strict long-only. Affects script ergonomics. | |
| Unknown-flag handling | Strict reject (clap default), or pass through to forward to spawned commands? lint-staged strips unknowns; we have no commands to forward to. Probably strict — but lock the call. | ✓ |

**Notes:** User selected the two areas with binding semantic implications and let Claude resolve help/version style and short-alias policy at its discretion.

---

## --config missing-path behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Hard fail (Recommended) | Print AppError::ConfigNotFound with the bad path + actionable hint, exit 1. User got a path wrong — silent fallback would mask the typo and run the wrong config. | ✓ |
| Warn + fallback | Print warning, fall back to auto-discovery from CWD. Lenient UX, but masks typos and hides which config actually loaded. | |
| Hard fail, but distinguish missing-vs-invalid | Two distinct errors: ConfigNotFound (path doesn't exist) vs ConfigInvalid (path exists but parse error). Different exit hints. Already partly modelled in AppError. | |

**User's choice:** Hard fail
**Notes:** Locked as D-02. The third option (distinguish missing vs invalid) was not selected, but Claude folded it into D-03 anyway because both `AppError` variants already exist and using them costs nothing — better-quality errors fall out naturally with no extra work.

---

## --config relative path resolution

| Option | Description | Selected |
|--------|-------------|----------|
| Relative to CWD (Recommended) | Standard Unix CLI behavior. `fast-staged --config fs.toml` resolves against `pwd`. Predictable for shell scripts, husky hooks, and CI. | ✓ |
| Relative to git root | Match auto-discovery semantics (Phase 2 walks CWD→git-root). Consistent with no-flag behavior, but unusual for explicit paths. | |
| CWD first, then git root fallback | Try CWD-relative; if missing, try git-root-relative before failing. Friendlier, but adds two failure paths and ambiguity over which file actually loaded. | |

**User's choice:** Relative to CWD
**Notes:** Locked as D-04. User picked the standard Unix-tool answer. Auto-discovery (no flag) keeps its CWD→git-root walk; only the explicit `--config <path>` form is CWD-anchored.

---

## Unknown / unrecognized flags

| Option | Description | Selected |
|--------|-------------|----------|
| Strict reject (Recommended) | clap's default — print error with similar-flag suggestion + usage hint, exit 2. Catches typos like `--quet` or `--allow_empty`. No commands to forward args to anyway. | ✓ |
| Allow trailing -- args | Reject unknown flags before `--`, but allow everything after `--` (positional ignore). Future-proof if we ever want `fast-staged -- <command-args>`. clap supports via `trailing_var_arg`. | |
| Warn but accept | Print to stderr but proceed. Permissive. Risks running with wrong/partial config silently. Generally bad CLI hygiene. | |

**User's choice:** Strict reject
**Notes:** Locked as D-05. clap's default behavior is exactly what we want — no extra config attributes needed on the `Parser` struct.

---

## Claude's Discretion

- **Help / version text style** (D-06): User did not select this area. Locked as default clap output with about line + version sourced from `Cargo.toml [package]`. No custom long-help in v1. Revisit on user feedback.
- **Short flag aliases** (D-07): User did not select this area. Locked as long-only for project flags. Only clap's automatic `-h` / `-V` (help / version) are kept because they are zero-cost and Unix-standard. No `-q`, `-c`, `-u`, `-e`. Reversible if scripting feedback demands.
- **`lib::run()` signature change** (D-08): Implied by CLI-01. Adds `config_path: Option<PathBuf>` as first parameter. Mechanical refactor; not a user-facing decision.

## Deferred Ideas

- Custom long-form `--help` with usage examples and config-format hint — revisit if/when users find default clap help insufficient.
- Short flag aliases for `--quiet` / `--config` / `--unstaged` — same trigger: scripting ergonomics feedback.
- `--stash`, `--revert`, `--fail-on-changes` flags — listed in PROJECT.md but not in CLI-01..03; belong in a later phase after signal handling solidifies the on-failure semantics.
- `fast-staged init` / `check` subcommands — explicitly out of scope; this phase keeps a single flat-flag invocation.
- `--version` enriched with git commit hash + build profile (via `built` or `vergen`) — own micro-phase if needed.
