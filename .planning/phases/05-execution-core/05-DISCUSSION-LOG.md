# Phase 5: Execution Core - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-25
**Phase:** 05-execution-core
**Areas discussed:** Glob library & matching, Command spawn & file-arg passing, Output capture & reporting, Failure & exit-code semantics, Timeout enforcement, --allow-empty scope, Per-file vs batch invocation, Restore-on-failure wiring

---

## Glob library & matching

### Q1: Which glob library?
| Option | Description | Selected |
|--------|-------------|----------|
| globset | Used by ripgrep. Compiles many patterns into a single set matcher. | ✓ |
| glob | Classic Rust glob crate. No batch compilation. | |
| globwalk | Walks fs with glob patterns — not what we need. | |

**User's choice:** globset (recommended)

### Q2: matchBase semantics?
| Option | Description | Selected |
|--------|-------------|----------|
| Auto-prefix `**/` to slashless patterns | `*.ts` → `**/*.ts` internally. Slashed patterns stay literal. | ✓ |
| globset literal_separator(false) | Lets `*` cross separators natively. | |
| Match against basename only | Strip path before matching slashless. | |

**User's choice:** Auto-prefix `**/` (recommended)

### Q3: Negation patterns?
| Option | Description | Selected |
|--------|-------------|----------|
| Yes — leading `!`, include+exclude sets | lint-staged supports this. | ✓ |
| No — defer to a future phase | | |

**User's choice:** Yes (recommended)

### Q4: Case sensitivity?
| Option | Description | Selected |
|--------|-------------|----------|
| Always case-sensitive | Matches lint-staged + git default. | ✓ |
| Follow OS default | macOS/Windows insensitive, Linux sensitive. | |
| Configurable per-group | | |

**User's choice:** Always case-sensitive (recommended)

---

## Command spawn & file-arg passing

### Q1: Command parsing & spawn strategy?
| Option | Description | Selected |
|--------|-------------|----------|
| shlex parse + direct exec | No shell. Faster, safer. | ✓ |
| Always sh -c / cmd /C | Supports pipes, redirects. Cross-platform issues. | |
| Auto-detect shell metachars | Magical, hard to predict. | |

**User's choice:** shlex + direct exec (recommended)

### Q2: How file paths are appended?
| Option | Description | Selected |
|--------|-------------|----------|
| Positional append only | `cmd files...` | |
| `{files}` placeholder only | `cmd --files {files}` | |
| Both — placeholder overrides, else positional append | Maximum flexibility. | ✓ |

**User's choice:** Both (placeholder + positional)

### Q3: path_format — include now or defer?
| Option | Description | Selected |
|--------|-------------|----------|
| Include now (Relative default) | Add to Group + parser this phase. | ✓ |
| Defer — always relative to git root | Cleaner Phase 5 scope. | |

**User's choice:** Include now (recommended)

### Q4: PATH lookup?
| Option | Description | Selected |
|--------|-------------|----------|
| Use PATH (default Command::new) | ENOENT → CommandNotFound. | ✓ |
| Require absolute paths | Too strict. | |

**User's choice:** Use PATH (recommended)

---

## Output capture & reporting

### Q1: stdout/stderr handling?
| Option | Description | Selected |
|--------|-------------|----------|
| Capture per-command, print on failure only | Buffer in memory, dump on failure. | ✓ |
| Stream live, interleaved | | |
| Inherit stdio | | |

**User's choice:** Capture + dump on failure (recommended)

### Q2: Successful run output?
| Option | Description | Selected |
|--------|-------------|----------|
| One-line summary per command | `✓ group/pattern: cmd (Ns, K files)` | ✓ |
| Nothing — silent | | |
| Full command output even on success | | |

**User's choice:** One-line summary (recommended)

### Q3: Failure output format?
| Option | Description | Selected |
|--------|-------------|----------|
| Boxed/bordered section per failed cmd | cmd + exit code + captured stderr. | ✓ |
| Plain concatenated dump | | |

**User's choice:** Boxed (recommended)

### Q4: --quiet behavior?
| Option | Description | Selected |
|--------|-------------|----------|
| Suppresses success summary, errors still print | | ✓ |
| Reserved for Phase 8 — ignore in Phase 5 | | |

**User's choice:** Suppress success summary (recommended)

---

## Failure & exit-code semantics

### Q1: Sibling commands in same group on failure?
| Option | Description | Selected |
|--------|-------------|----------|
| Respect group continue_on_error | Phase 3 D-08 alignment. | ✓ |
| Always let in-flight finish | | |
| Always cancel siblings | | |

**User's choice:** Respect group continue_on_error (recommended)

### Q2: Subsequent groups after a failed group?
| Option | Description | Selected |
|--------|-------------|----------|
| Respect global continue_on_error | Phase 3 D-09 alignment. | ✓ |
| Always run all groups | | |

**User's choice:** Respect global continue_on_error (recommended)

### Q3: Exit code with continue_on_error=true?
| Option | Description | Selected |
|--------|-------------|----------|
| Always non-zero on any failure | continue_on_error never downgrades exit code. | ✓ |
| continue_on_error=true → exit 0 | Dangerous; CI would miss failures. | |

**User's choice:** Always non-zero (recommended)

### Q4: Parallelism cap?
| Option | Description | Selected |
|--------|-------------|----------|
| Unlimited (tokio spawn all) | OS scheduler handles contention. | ✓ |
| Cap to num_cpus | | |
| Configurable cap | | |

**User's choice:** Unlimited (recommended)

---

## Timeout enforcement

### Q1: Timeout precedence?
| Option | Description | Selected |
|--------|-------------|----------|
| Group overrides global; none = no enforcement | Already aligned with Phase 3 cascade. | ✓ |
| Always require a timeout (5min default) | | |
| Per-group only — ignore global | | |

**User's choice:** Group overrides global (recommended)

### Q2: Granularity?
| Option | Description | Selected |
|--------|-------------|----------|
| Per spawn | Each invocation gets full timeout. | ✓ |
| Group-wide budget | | |

**User's choice:** Per spawn (recommended)

### Q3: Kill semantics (Unix)?
| Option | Description | Selected |
|--------|-------------|----------|
| SIGTERM, then SIGKILL after 2s grace | Polite shutdown. | ✓ |
| Immediate SIGKILL | | |
| tokio kill_on_drop only | | |

**User's choice:** SIGTERM-then-SIGKILL with 2s grace (recommended)

### Q4: Kill semantics (Windows)?
| Option | Description | Selected |
|--------|-------------|----------|
| TerminateProcess directly | Skip grace window. | ✓ |
| Ctrl+C event then TerminateProcess | Flaky on Windows. | |

**User's choice:** TerminateProcess directly (recommended)

---

## --allow-empty scope

### Q1: What does --allow-empty suppress?
| Option | Description | Selected |
|--------|-------------|----------|
| Both NoStagedFiles AND NoFilesMatched | Hook is allowed to be a no-op. | ✓ |
| Only NoFilesMatched | | |
| Only NoStagedFiles | | |

**User's choice:** Both (recommended)

### Q2: --unstaged + --allow-empty + clean worktree?
| Option | Description | Selected |
|--------|-------------|----------|
| Same suppression — exit 0 silently | Symmetric with staged path. | ✓ |
| Different behavior | | |

**User's choice:** Same suppression (recommended)

### Q3: Print info message on suppressed empty?
| Option | Description | Selected |
|--------|-------------|----------|
| Silent exit | Flag means "shut up about it." | ✓ |
| Print to stderr, exit 0 | | |

**User's choice:** Silent exit (recommended)

### Q4: Skip backup when no work to do?
| Option | Description | Selected |
|--------|-------------|----------|
| Yes — skip backup/restore/cleanup entirely | Saves I/O. | ✓ |
| Always run backup | Simpler control flow. | |

**User's choice:** Skip backup on no-op (recommended)

---

## Per-file vs batch invocation

### Q1: Per-file concurrency within group?
| Option | Description | Selected |
|--------|-------------|----------|
| Honor group execution_order | parallel/sequential applies to per-file too. | ✓ |
| Always sequential when per_file=true | | |
| Always parallel when per_file=true | | |

**User's choice:** Honor group execution_order (recommended)

### Q2: ARG_MAX handling for batch mode?
| Option | Description | Selected |
|--------|-------------|----------|
| Detect ARG_MAX, chunk batch invocations | Required for monorepos. | ✓ |
| Pass everything; fail on E2BIG | | |
| Defer chunking | | |

**User's choice:** Auto-chunk (recommended)

### Q3: Batch arg-passing without {files} placeholder?
| Option | Description | Selected |
|--------|-------------|----------|
| Single positional batch | `cmd file1 file2 ...` | ✓ |
| Repeated --file flag | | |
| Newline-delimited stdin | | |

**User's choice:** Positional batch (recommended)

### Q4: Empty file list within group?
| Option | Description | Selected |
|--------|-------------|----------|
| Skip command entirely | Don't spawn — risk of whole-repo lint. | ✓ |
| Spawn with empty list | | |

**User's choice:** Skip (recommended)

---

## Restore-on-failure wiring

### Q1: Restore policy?
| Option | Description | Selected |
|--------|-------------|----------|
| On any failure: restore + cleanup; success: cleanup | Closes lib.rs:54 TODO. | ✓ |
| Only restore when tools modified files | More complex. | |
| Never restore | Loses lint-staged safety guarantee. | |

**User's choice:** Always restore on failure (recommended)

### Q2: Restore scope?
| Option | Description | Selected |
|--------|-------------|----------|
| Worktree only — preserve user's index | Matches Phase 4 backup semantics. | ✓ |
| Worktree + index hard reset | | |

**User's choice:** Worktree only (recommended)

### Q3: If restore() itself fails?
| Option | Description | Selected |
|--------|-------------|----------|
| AppError::RestoreFailed; leave patch on disk | User can manually `git apply`. | ✓ |
| Best-effort: warn + continue + delete | | |
| Panic + abort | | |

**User's choice:** Surface error, preserve patch (recommended)

### Q4: Order of restore vs cleanup?
| Option | Description | Selected |
|--------|-------------|----------|
| Restore first → cleanup always (unless restore failed) | Single guard pattern. | ✓ |
| Cleanup only on success path | | |

**User's choice:** Restore first → cleanup (recommended)

---

## Claude's Discretion

- Internal module layout (split vs flat)
- TaskState/CommandStatus/Message minimal shapes for Phase 5
- Test layout and count
- Exact AppError variant message wording
- Choice of signal-sending crate on Unix (`nix` vs `libc` vs tokio-native)

## Deferred Ideas

- EXEC-04 Ctrl+C — Phase 7
- TUI rendering — Phase 8
- --config / --version / --help — Phase 6
- Configurable max_concurrent
- case_insensitive per-group
- Stdin file passing
- Auto-detect shell metachars
