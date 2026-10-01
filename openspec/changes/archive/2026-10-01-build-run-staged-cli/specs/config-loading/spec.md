## MODIFIED Requirements

### Requirement: Config discovery order
The tool SHALL search for configuration starting in the current working directory and walking up to the git root. In each directory it SHALL check these candidates in order, and SHALL use the first one found:
1. `.run-staged.toml`
2. `.run-staged.json`
3. `run-staged.toml`
4. `run-staged.json`
5. `package.json` containing a `run-staged` key

#### Scenario: Higher-priority file wins
- **WHEN** a directory contains both `.run-staged.toml` and `run-staged.json`
- **THEN** the tool loads `.run-staged.toml`

#### Scenario: Walk up to parent directory
- **WHEN** the tool runs in a subdirectory with no config and the repository root contains `run-staged.toml`
- **THEN** the tool loads the root `run-staged.toml`

#### Scenario: package.json without fast-staged key is ignored
- **WHEN** the only candidate is a `package.json` that has no `run-staged` key
- **THEN** the file is skipped and discovery continues

#### Scenario: Old MVP file names are not discovered
- **WHEN** the only config in the repository is `fast-staged.toml`
- **THEN** the tool reports that no configuration was found

#### Scenario: No config found
- **WHEN** no config file exists between the working directory and the git root
- **THEN** the tool prints `error: configuration file not found` and exits with code 1

### Requirement: Explicit config path
When `--config <path>` is given, the tool SHALL load that file and SHALL NOT run discovery. A relative path SHALL be resolved against the current working directory. The format SHALL be chosen by file name: `package.json` reads the `run-staged` key, `*.toml` is TOML, and anything else is JSON.

#### Scenario: Explicit config bypasses discovery
- **WHEN** the tool runs with `--config nested/custom.toml` and that file exists
- **THEN** the tool loads `nested/custom.toml`, even though discovery would not find it

#### Scenario: Explicit config missing
- **WHEN** the tool runs with `--config` pointing at a file that does not exist
- **THEN** the tool prints `error: configuration file not found` and exits with code 1

### Requirement: JSON config shapes
JSON configs (`*.json` files and the `package.json` `run-staged` value) SHALL accept the structured shape described above and a flat glob-to-commands mapping such as `{"*.rs": ["cargo fmt"]}`, which becomes the `default` group with built-in settings. A root with any of `timeout`, `execution_order`, `continue_on_error`, `per_file`, `path_format`, `read_only`, `chunk_size`, or `patterns` SHALL be read as structured.

#### Scenario: Flat JSON mapping
- **WHEN** `run-staged.json` contains `{"*.rs": ["cargo fmt"]}`
- **THEN** staged `.rs` files run `cargo fmt` with default settings

#### Scenario: package.json section
- **WHEN** `package.json` contains `"run-staged": {"*.ts": ["eslint"]}` and no other config exists
- **THEN** staged `.ts` files run `eslint`

#### Scenario: Structured JSON with only new keys
- **WHEN** `run-staged.json` contains `{"read_only": true, "lint": {"patterns": {"*.js": ["eslint"]}}}`
- **THEN** it is read as structured, and group `lint` is read-only

### Requirement: Config structure
A config SHALL accept these root keys: `timeout`, `execution_order`, `continue_on_error`, `per_file`, `path_format`, `read_only`, `chunk_size`, and `patterns` (a map of glob to list of commands). Any other table (TOML) or object (JSON) at the root SHALL be treated as a named group that accepts the same keys. Root `patterns` SHALL form a group named `default`.

#### Scenario: Named groups in TOML
- **WHEN** a TOML config defines `[lint]` with `patterns = { "*.rs" = ["cargo fmt"] }`
- **THEN** a group named `lint` runs `cargo fmt` for matching `.rs` files

#### Scenario: Named group without patterns is skipped
- **WHEN** a named group has no `patterns` or an empty `patterns` map
- **THEN** that group is ignored

#### Scenario: New group keys accepted
- **WHEN** a group sets `read_only = true` and `chunk_size = 10`
- **THEN** the config loads without a validation error

#### Scenario: Invalid chunk size
- **WHEN** a group sets `chunk_size = 0`
- **THEN** loading fails with a config validation error that names the group and the `chunk_size` key

### Requirement: Settings cascade
Each group setting SHALL resolve to the group's value if set, else the root value if set, else the built-in default:
- `timeout`: none
- `execution_order`: `parallel`
- `continue_on_error`: `false`
- `per_file`: `false`
- `path_format`: `relative`
- `read_only`: `false`
- `chunk_size`: none (unlimited)

`timeout` SHALL accept human-readable durations such as `"5s"` or `"300ms"`. A zero duration (for example `"0s"` or `"0sec"`) SHALL mean no timeout.

#### Scenario: Group overrides root
- **WHEN** root sets `timeout = "10s"` and group `lint` sets `timeout = "30s"`
- **THEN** commands in `lint` use a 30 second timeout

#### Scenario: Root value inherited
- **WHEN** root sets `execution_order = "sequential"` and a group sets nothing
- **THEN** that group runs sequentially

#### Scenario: Zero timeout disables timeout
- **WHEN** root sets `timeout = "0sec"` and a group sets no timeout
- **THEN** that group's commands run without a time limit
