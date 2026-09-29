# config-loading Specification

## Purpose
Discover, parse, and validate fast-staged configuration from TOML, JSON, or a `package.json` section, and resolve effective per-group settings.

## Requirements

### Requirement: Config discovery order
The tool SHALL search for configuration starting in the current working directory and walking up to the git root. In each directory it SHALL check, in order, `.fast-staged.toml`, `.fast-staged.json`, `fast-staged.toml`, `fast-staged.json`, and `package.json` containing a `fast-staged` key, and SHALL use the first file found.

#### Scenario: Higher-priority file wins
- **WHEN** a directory contains both `.fast-staged.toml` and `fast-staged.json`
- **THEN** the tool loads `.fast-staged.toml`

#### Scenario: Walk up to parent directory
- **WHEN** the tool runs in a subdirectory with no config and the repository root contains `fast-staged.toml`
- **THEN** the tool loads the root `fast-staged.toml`

#### Scenario: package.json without fast-staged key is ignored
- **WHEN** the only candidate is a `package.json` that has no `fast-staged` key
- **THEN** the file is skipped and discovery continues

#### Scenario: No config found
- **WHEN** no config file exists between the working directory and the git root
- **THEN** the tool prints `error: configuration file not found` and exits with code 1

### Requirement: Explicit config path
When `--config <path>` is given, the tool SHALL load that file and SHALL NOT run discovery. A relative path SHALL be resolved against the current working directory. The format SHALL be chosen by file name: `package.json` reads the `fast-staged` key, `*.toml` is TOML, anything else is JSON.

#### Scenario: Explicit config bypasses discovery
- **WHEN** the tool runs with `--config nested/custom.toml` and that file exists
- **THEN** the tool loads `nested/custom.toml` even though discovery would not find it

#### Scenario: Explicit config missing
- **WHEN** the tool runs with `--config` pointing at a file that does not exist
- **THEN** the tool prints `error: configuration file not found` and exits with code 1

### Requirement: Config structure
A config SHALL accept root keys `timeout`, `execution_order`, `continue_on_error`, `per_file`, `path_format`, and `patterns` (a map of glob to list of commands). Any other table (TOML) or object (JSON) at the root SHALL be treated as a named group accepting the same six keys. Root `patterns` SHALL form a group named `default`.

#### Scenario: Named groups in TOML
- **WHEN** a TOML config defines `[lint]` with `patterns = { "*.rs" = ["cargo fmt"] }`
- **THEN** a group named `lint` runs `cargo fmt` for matching `.rs` files

#### Scenario: Named group without patterns is skipped
- **WHEN** a named group has no `patterns` or an empty `patterns` map
- **THEN** that group is ignored

### Requirement: Group declaration order
Groups SHALL be processed in the order they are declared in the config file, with the root `default` group first.

#### Scenario: Declaration order preserved
- **WHEN** a config declares groups `zeta` then `alpha`
- **THEN** `zeta` runs before `alpha`

### Requirement: Settings cascade
Each group setting SHALL resolve to the group's value if set, else the root value if set, else the built-in default: `timeout` none, `execution_order` `parallel`, `continue_on_error` `false`, `per_file` `false`, `path_format` `relative`. `timeout` SHALL accept human-readable durations such as `"5s"` or `"300ms"`.

#### Scenario: Group overrides root
- **WHEN** root sets `timeout = "10s"` and group `lint` sets `timeout = "30s"`
- **THEN** commands in `lint` use a 30 second timeout

#### Scenario: Root value inherited
- **WHEN** root sets `execution_order = "sequential"` and a group sets nothing
- **THEN** that group runs sequentially

### Requirement: JSON config shapes
JSON configs (`*.json` files and the `package.json` `fast-staged` value) SHALL accept the structured shape described above and a flat glob-to-commands mapping such as `{"*.rs": ["cargo fmt"]}`, which becomes the `default` group with built-in settings. A root with any of `timeout`, `execution_order`, `continue_on_error`, `per_file`, or `patterns` SHALL be read as structured.

#### Scenario: Flat JSON mapping
- **WHEN** `fast-staged.json` contains `{"*.rs": ["cargo fmt"]}`
- **THEN** staged `.rs` files run `cargo fmt` with default settings

#### Scenario: package.json section
- **WHEN** `package.json` contains `"fast-staged": {"*.ts": ["eslint"]}` and no other config exists
- **THEN** staged `.ts` files run `eslint`

### Requirement: Config validation errors
The tool SHALL reject invalid configuration with `error: invalid configuration: <path>: <detail>` and exit code 1. Invalid cases SHALL include: syntax errors, `execution_order` other than `parallel`/`sequential`, `path_format` other than `relative`/`absolute`, unknown keys inside a TOML group, and a config with no patterns in any group (`no patterns defined`).

#### Scenario: Syntax error
- **WHEN** the config file is not valid TOML
- **THEN** the tool prints an `invalid configuration` error naming the file and exits with code 1

#### Scenario: Bad enum value
- **WHEN** a group sets `execution_order = "random"`
- **THEN** the error says `unknown execution_order: "random", expected "parallel" or "sequential"`

#### Scenario: Typo in TOML group key
- **WHEN** a TOML group sets `continue_on_eror = true`
- **THEN** the tool reports an invalid configuration error for that group
