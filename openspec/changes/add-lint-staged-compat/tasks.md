## 1. Discovery

- [ ] 1.1 Append lint-staged candidates after run-staged candidates in each directory of the discovery walk
- [ ] 1.2 Detect JS config names (`.lintstagedrc.js/.cjs/.mjs`, `lint-staged.config.js/.cjs/.mjs`) and return an invalid configuration error with a conversion hint
- [ ] 1.3 `--config` accepts lint-staged file names with the same parsing rules

## 2. Parsing

- [ ] 2.1 Add a maintained serde YAML crate; parse `.yaml`/`.yml` and extensionless `.lintstagedrc` (JSON first, then YAML)
- [ ] 2.2 Accept a string command value as a one-element command list
- [ ] 2.3 Read `package.json["lint-staged"]` as a flat mapping

## 3. Verification

- [ ] 3.1 Unit tests: each lint-staged file name, string and array values, priority vs run-staged configs
- [ ] 3.2 Process-level test: repo with only `.lintstagedrc.json` runs its command
- [ ] 3.3 Process-level test: repo with only `lint-staged.config.js` exits 1 with the JS error
