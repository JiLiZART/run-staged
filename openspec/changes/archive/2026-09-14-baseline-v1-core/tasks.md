## 1. Foundation (GSD Phase 1, completed 2026-04-09)

- [x] 1.1 Crate renamed to fast-staged, dependencies declared, typed AppError enum, async main with exit codes
- [x] 1.2 NAPI-RS CI replaced with stable Rust fmt + clippy + test workflow

## 2. Config Core (GSD Phase 2)

- [x] 2.1 Config types, TOML/JSON parsing, validation, cascading defaults
- [x] 2.2 Config discovery (priority walk to git root) and package.json support

## 3. Advanced Config (GSD Phase 3, completed 2026-04-09)

- [x] 3.1 Declaration-order groups, per-group timeout / execution_order / continue_on_error / per_file / path_format

## 4. Git Integration (GSD Phase 4, completed 2026-04-11)

- [x] 4.1 GitContext with staged_files / unstaged_files
- [x] 4.2 Patch backup / restore / cleanup wired into run()
- [x] 4.3 --unstaged flag wired end-to-end
- [x] 4.4 Quick fix 999.1: staged_files returns only HEAD-vs-index changes (2026-04-29)

## 5. Execution Core (GSD Phase 5, completed 2026-04-29)

- [x] 5.1 Glob matcher with matchBase and negation guard
- [x] 5.2 Command primitives: shlex argv, {files} placement, ARG_MAX chunking
- [x] 5.3 execute_all orchestrator: per-group parallel/sequential, cancellation, cross-group failure gate
- [x] 5.4 Per-spawn timeout with SIGTERM → 2s grace → SIGKILL
- [x] 5.5 run() integration with restore-on-failure and output reporting

## 6. CLI Interface (GSD Phase 6, completed 2026-05-12)

- [x] 6.1 clap derive CLI; run() accepts explicit config path
- [x] 6.2 Process-level tests for --version, --help, unknown flag, --config, --quiet
