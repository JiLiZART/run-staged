## MODIFIED Requirements

### Requirement: Batch and per-file modes
With `per_file = false` (default), the tool SHALL run each command once per pattern with all matched files. It SHALL split the files into several invocations when the list would exceed the group's `chunk_size`, or when the argument list would exceed 75% of the platform argument-length limit, whichever limit is smaller. File order SHALL be preserved. With `per_file = true`, the tool SHALL run each command once per file, and `chunk_size` SHALL be ignored. Chunked invocations of one command SHALL follow the group's `execution_order`.

#### Scenario: Batch mode
- **WHEN** `per_file` is false and `a.rs` and `b.rs` match `*.rs = ["rustfmt"]`
- **THEN** a single `rustfmt a.rs b.rs` process runs

#### Scenario: Per-file mode
- **WHEN** `per_file` is true and `a.rs` and `b.rs` match
- **THEN** `rustfmt a.rs` and `rustfmt b.rs` run as separate processes

#### Scenario: Oversized file list
- **WHEN** matched paths exceed the argument-length budget
- **THEN** the command runs several times, with consecutive chunks that cover every file exactly once

#### Scenario: Configured chunk size
- **WHEN** `chunk_size = 2` and `a.js`, `b.js`, `c.js` match `*.js = ["eslint"]`
- **THEN** `eslint a.js b.js` and `eslint c.js` run as separate invocations
