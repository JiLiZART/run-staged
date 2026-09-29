## 1. Progress events

- [ ] 1.1 Define progress events (started, finished with outcome) and emit them from the executor over an mpsc channel
- [ ] 1.2 Keep `execute_all` usable without a listener (plain-text path)

## 2. Renderer

- [ ] 2.1 View model: groups → commands with status and start time
- [ ] 2.2 ratatui inline viewport: spinner, `<group>/<pattern>: <command>`, elapsed time, final status icon
- [ ] 2.3 ~100ms tick loop driven by events and timer
- [ ] 2.4 After exit, print failure boxes and cancelled lines using existing output code

## 3. Terminal lifecycle

- [ ] 3.1 Enable TUI only when stdout is a TTY and `--quiet` is not set
- [ ] 3.2 Panic hook restoring terminal before chaining to previous hook
- [ ] 3.3 Map Ctrl+C key event to the cancellation flag; restore terminal on interrupt and error paths

## 4. Verification

- [ ] 4.1 Render tests with ratatui `TestBackend` for running, success, failure, timeout, cancelled states
- [ ] 4.2 Process-level test: piped stdout produces plain-text output (no escape sequences)
- [ ] 4.3 Unit test: panic hook restores terminal state
