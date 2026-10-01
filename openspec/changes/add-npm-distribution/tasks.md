## 1. Packages

- [ ] 1.1 Add `npm/platforms/<platform>/package.json` for all eight targets (`@run-staged/<platform>`, with `os`, `cpu`, `libc` (Linux only), `files: ["bin"]`, `preferUnplugged: true`, version `0.0.0-dev`). Verify: `npm pack --dry-run` in each directory lists only `package.json` and `bin/`.
- [ ] 1.2 Add `npm/cli/package.json` (`@run-staged/cli`, `bin: { "run-staged": "bin/run-staged.js" }`, `engines.node >= 18`, all eight `optionalDependencies`, `publishConfig.access = public`, no install scripts). Verify: `npm pack --dry-run` shows the launcher and README, and a script check confirms there are no `*install` keys.
- [ ] 1.3 Add `npm/scripts/set-version.mjs`, which stamps the version into every package and every `optionalDependencies` entry. Verify: running it with `1.2.3` makes `git diff` show only version fields.

## 2. Launcher

- [ ] 2.1 Resolution: `RUN_STAGED_BINARY`, then platform key, libc detection with fallback to the other Linux package, and `require.resolve` of the platform package. Verify: `node --test` unit tests for the platform-key mapping, libc fallback, and the override.
- [ ] 2.2 Hand-off: `process.execve` when available, otherwise `spawn` with `stdio: 'inherit'`, signal forwarding, and exit-code / signal re-raise. Verify: `node --test` with a stub binary checks exit code 3, the arguments, and death by SIGTERM, on Node 20 (spawn path) and Node 24 (execve path).
- [ ] 2.3 Unsupported or missing platform package: print the detected platform, the supported list, and both fixes, then exit 1. Verify: a `node --test` case with an unresolvable package.

## 3. Smoke test

- [ ] 3.1 `npm/scripts/smoke-test.mjs`: pack `@run-staged/cli` and the host platform package (with a release binary), and install them into temporary npm, pnpm, Yarn 1, and Yarn 4 (node-modules and PnP) projects. Verify: `<pm> run-staged --version` prints the stamped version in each project.
- [ ] 3.2 Add the commit and passthrough scenarios: an `--install` hook and a husky-style hook, each with a failing command, plus exit code, arguments, SIGTERM (Unix), `--ignore-scripts`, and `--omit=optional`. Verify: each assertion passes.
- [ ] 3.3 Run the smoke test in CI on Linux, macOS, and Windows for pull requests that touch `npm/`, `apps/`, or `crates/`. Verify: the CI jobs are green.

## 4. Release workflow

- [ ] 4.1 Add a `release.yml` build matrix for the eight targets (native runners for macOS and Windows, `cargo-zigbuild` for Linux gnu (glibc 2.17) and musl), uploading the binaries as artifacts. Verify: a `workflow_dispatch` dry run produces eight artifacts, each checked with `--version` or `file`.
- [ ] 4.2 Add a publish job that `needs` every build: check the tag against the version, publish the crates in dependency order, stamp the npm versions, publish the platform packages with `--provenance`, then `@run-staged/cli`, then a GitHub release. Verify: a dry run (`cargo publish --dry-run`, `npm publish --dry-run`) shows the right order and that `@run-staged/cli` is skipped when one build is forced to fail.

## 5. Docs

- [ ] 5.1 Write `npm/cli/README.md` and a root README section on migrating from lint-staged (install, config conversion, husky or `--install`, Yarn PnP note, `RUN_STAGED_BINARY`). Verify: the README is included in `npm pack --dry-run` and every command in it has been run by hand once.
