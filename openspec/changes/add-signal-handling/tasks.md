## 1. Interrupt handling

- [ ] 1.1 Spawn a signal listener in `run()` (tokio ctrl_c; SIGTERM on Unix) that sets the shared cancellation flag
- [ ] 1.2 In `run_one`, select on child exit vs cancellation and call `terminate()` when cancelled
- [ ] 1.3 Second signal sets a force flag so `terminate()` kills without grace period
- [ ] 1.4 Map interrupt to `AppError::Cancelled`, route through restore → cleanup, exit 130 in `main.rs`

## 2. Error hints

- [ ] 2.1 Add hints to `AppError` messages: not a git repository, configuration file not found, command not found, command timed out
- [ ] 2.2 Replace `expect` on `current_dir()` in `main.rs` with `error:` output and exit 1
- [ ] 2.3 Drop superfluous `--allow-empty` from `config_explicit_loads` test (06-REVIEW WR-02)

## 3. Verification

- [ ] 3.1 Unix process-level test: run binary with `sleep 30` command, send SIGINT, assert exit 130, child gone, index unchanged, patch removed
- [ ] 3.2 Unit tests for hint text on each hinted error
- [ ] 3.3 Add `windows-latest` to the CI test matrix
