## ADDED Requirements

### Requirement: lint-staged config discovery
During discovery, after checking all run-staged candidates in a directory, the tool SHALL check these lint-staged candidates in order: `package.json` containing a `lint-staged` key, `package.yaml` or `package.yml` containing a `lint-staged` key, `.lintstagedrc`, `.lintstagedrc.json`, `.lintstagedrc.yaml`, `.lintstagedrc.yml`. A lint-staged config SHALL be read as a flat glob-to-commands mapping where each value is a command string or an array of command strings. Extensionless `.lintstagedrc` SHALL be parsed as JSON, falling back to YAML. `--config <path>` SHALL accept any of these file names with the same parsing rules.

#### Scenario: JSON lint-staged config
- **WHEN** the only config is `.lintstagedrc.json` containing `{"*.ts": "eslint --fix"}`
- **THEN** staged `.ts` files run `eslint --fix`

#### Scenario: YAML lint-staged config
- **WHEN** the only config is `.lintstagedrc.yml` mapping `"*.css"` to `["stylelint", "prettier --write"]`
- **THEN** staged `.css` files run both commands

#### Scenario: Extensionless rc in YAML
- **WHEN** the only config is `.lintstagedrc` containing the YAML line `"*.md": prettier --write`
- **THEN** staged `.md` files run `prettier --write`

#### Scenario: run-staged config takes priority
- **WHEN** a directory contains both `run-staged.toml` and `.lintstagedrc.json`
- **THEN** the tool loads `run-staged.toml`

#### Scenario: package.json lint-staged key
- **WHEN** `package.json` has a `lint-staged` key and no other config exists
- **THEN** the tool uses the `lint-staged` mapping

#### Scenario: package.yaml lint-staged key
- **WHEN** `package.yaml` has a `lint-staged` key and no other config exists
- **THEN** the tool uses the `lint-staged` mapping

#### Scenario: package.json without lint-staged key falls through
- **WHEN** `package.json` has no `lint-staged` key and `.lintstagedrc.json` exists in the same directory
- **THEN** the tool loads `.lintstagedrc.json`

### Requirement: JavaScript lint-staged configs rejected
When discovery reaches a directory with no JSON/YAML config but a JavaScript or TypeScript lint-staged config (`.lintstagedrc.js`, `.lintstagedrc.cjs`, `.lintstagedrc.mjs`, `.lintstagedrc.ts`, `.lintstagedrc.cts`, `.lintstagedrc.mts`, `lint-staged.config.js`, `lint-staged.config.cjs`, `lint-staged.config.mjs`, `lint-staged.config.ts`, `lint-staged.config.cts`, `lint-staged.config.mts`), the tool SHALL exit with code 1 and an invalid configuration error naming the file, stating JavaScript/TypeScript configs are not supported, and suggesting conversion to `.lintstagedrc.json` or `run-staged.toml`.

#### Scenario: JS config only
- **WHEN** the only config is `lint-staged.config.js`
- **THEN** the tool exits with code 1 and the error names `lint-staged.config.js` and suggests a static config

#### Scenario: TS config only
- **WHEN** the only config is `.lintstagedrc.ts`
- **THEN** the tool exits with code 1 and the error names `.lintstagedrc.ts`
