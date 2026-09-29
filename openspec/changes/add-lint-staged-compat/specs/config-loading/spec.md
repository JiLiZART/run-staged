## ADDED Requirements

### Requirement: lint-staged config discovery
During discovery, after checking all fast-staged candidates in a directory, the tool SHALL check `.lintstagedrc`, `.lintstagedrc.json`, `.lintstagedrc.yaml`, `.lintstagedrc.yml`, and `package.json` containing a `lint-staged` key, in that order. A lint-staged config SHALL be read as a flat glob-to-commands mapping where each value is a command string or an array of command strings. Extensionless `.lintstagedrc` SHALL be parsed as JSON, falling back to YAML.

#### Scenario: JSON lint-staged config
- **WHEN** the only config is `.lintstagedrc.json` containing `{"*.ts": "eslint --fix"}`
- **THEN** staged `.ts` files run `eslint --fix`

#### Scenario: YAML lint-staged config
- **WHEN** the only config is `.lintstagedrc.yml` mapping `"*.css"` to `["stylelint", "prettier --write"]`
- **THEN** staged `.css` files run both commands

#### Scenario: fast-staged config takes priority
- **WHEN** a directory contains both `fast-staged.toml` and `.lintstagedrc.json`
- **THEN** the tool loads `fast-staged.toml`

#### Scenario: package.json lint-staged key
- **WHEN** `package.json` has a `lint-staged` key and no other config exists
- **THEN** the tool uses the `lint-staged` mapping

### Requirement: JavaScript lint-staged configs rejected
When discovery reaches a directory with no JSON/YAML config but a JavaScript lint-staged config (`.lintstagedrc.js`, `.lintstagedrc.cjs`, `.lintstagedrc.mjs`, `lint-staged.config.js`, `lint-staged.config.cjs`, `lint-staged.config.mjs`), the tool SHALL exit with code 1 and an invalid configuration error naming the file, stating JavaScript configs are not supported, and suggesting conversion to `.lintstagedrc.json` or `fast-staged.toml`.

#### Scenario: JS config only
- **WHEN** the only config is `lint-staged.config.js`
- **THEN** the tool exits with code 1 and the error names `lint-staged.config.js` and suggests a static config
