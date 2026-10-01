## Context

`build-run-staged-cli` produces the `apps/run-staged` binary, and its hook script already checks `./node_modules/.bin/run-staged`. Today the repository has no npm packaging. `.github/workflows/CI.yml` only runs fmt, clippy, and test. The unscoped npm name `run-staged` is an unrelated 0.0.0 placeholder, and the `@run-staged` scope has no packages. For motivation, see proposal.md.

## Goals / Non-Goals

**Goals:**
- `npm i -D @run-staged/cli` gives a working `run-staged` command with no Rust, no scripts, and no network beyond the registry.
- The launcher adds as little as possible: resolve one path, then hand over to the binary.
- The release is atomic from the user's point of view: `@run-staged/cli` is never published without its binaries.

**Non-Goals:**
- Replacing the launcher with the native binary at install time (the esbuild postinstall trick). That needs install scripts, which the spec forbids.
- Supporting 32-bit, FreeBSD, or Android targets. Add them when someone asks.
- A JS API. The package is a CLI only.

## Decisions

**Optional dependencies per platform (the SWC and Biome pattern).** The package manager installs only the package whose `os`, `cpu`, and `libc` match.
- Alternative considered: one fat package with all binaries. Rejected because it is about 8× the download.
- Alternative considered: postinstall download. Rejected because it breaks with `--ignore-scripts`, offline mirrors, and proxies.

**Thin launcher: `npm/cli/bin/run-staged.js`, plain CommonJS, no dependencies, about 60 lines.**
1. **Resolve.** Use `RUN_STAGED_BINARY` if it is set. Otherwise use `require.resolve('@run-staged/<platform>/package.json')` and take `bin/run-staged[.exe]` next to it. This works in node-modules layouts and in Yarn PnP. libc detection: `process.report.getReport().header.glibcVersionRuntime` is present on glibc; if detection fails, the launcher tries the other Linux package.
2. **Replace the process where possible.** When `process.execve` exists (Node 22.15+ / 23.11+, Unix), call `process.execve(bin, [bin, ...argv], process.env)`. Node's process becomes the binary, so stdio, signals, and the exit status are the binary's own, with no relay code and no extra process.
3. **Otherwise spawn.** On Windows and older Node: `child_process.spawn(bin, argv, { stdio: 'inherit', windowsHide: false })`.
   - Forward `SIGINT`, `SIGTERM`, and `SIGHUP` to the child with `child.kill(sig)`. Terminal SIGINT already reaches the child through the process group, so the launcher's handler only stops Node from dying first.
   - On `exit(code, signal)`: if `signal` is set, remove the handlers and `process.kill(process.pid, signal)`; otherwise call `process.exit(code)`.
   - Spawn errors (`ENOENT`, `EACCES`) print the missing-platform message.
- Alternative considered: `spawnSync`. Rejected because it cannot forward SIGTERM sent to the launcher alone.
- Alternative considered: a native shim as the bin entry. Rejected because npm bin entries must be files inside the main package, which would put the platform problem back.

**Package sources are checked in, versions are stamped at release.**
- `npm/cli/package.json` and `npm/platforms/<platform>/package.json` are committed with version `0.0.0-dev`.
- Platform packages set `preferUnplugged: true` (so Yarn PnP unpacks the binary) and `files: ["bin"]`.
- `npm/scripts/set-version.mjs <version>` rewrites every `version` and the `optionalDependencies`.
- CI copies each built binary into `npm/platforms/<platform>/bin/`.
- Alternative considered: generating the packages from a template. Rejected because eight small checked-in files are easier to review.

**Release workflow (`release.yml`, on tag `v*`).**
1. Check that the tag matches the `apps/run-staged/Cargo.toml` version.
2. Build matrix:
   - macOS runner: `aarch64-apple-darwin`, `x86_64-apple-darwin`.
   - Windows runner: `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`, with the existing static-CRT config.
   - Ubuntu with `cargo-zigbuild`: the gnu targets with a glibc 2.17 floor, and the musl targets.
3. Each job uploads its binary as an artifact.
4. A `publish` job that `needs` every build: stamp the versions, publish the eight platform packages with `--provenance --access public`, then `@run-staged/cli`, then a GitHub release with the binaries. Because `needs` gates on every build, a failed target blocks the main publish.
5. crates.io publishing (the library crates in dependency order, then `run-staged`) runs in the same workflow before npm, so both registries carry the same version.

**Smoke test (`npm/scripts/smoke-test.mjs`, in CI on Linux, macOS, and Windows for the host platform).**
1. Pack `@run-staged/cli` and the host's platform package, with a release binary.
2. Create temporary npm, pnpm, Yarn 1, and Yarn 4 (node-modules and PnP) projects that install the tarballs through `overrides` / `resolutions`.
3. Assert that `<pm> run-staged --version` prints the stamped version.
4. In the npm project, also assert exit-code passthrough, argument passthrough, SIGTERM passthrough (Unix), and commit abort through both the `--install` hook and a husky-style hook.

## Risks / Trade-offs

- [The `libc` field is ignored by older npm (before 10.4) or Yarn 1, so both Linux packages get installed] → Harmless: the launcher still picks by detected libc. The cost is extra download size on those clients.
- [Node startup adds about 40ms per commit] → Acceptable next to lint-staged's startup. `process.execve` removes the second process where available. Users who care can use the Cargo binary or `RUN_STAGED_BINARY`.
- [`process.execve` is experimental in Node] → Feature-detected, with spawn as the fallback. The smoke test covers both paths by running on two Node versions.
- [Yarn PnP has no `node_modules/.bin`, so the `--install` hook falls back to the global PATH] → The docs point PnP users to a `yarn run-staged` husky hook, as the spec requires.
- [A Windows arm64 cross-build fails on the hosted runner] → The release fails as a whole (atomic by spec). Fix the build or drop the target from the list in a patch release.

## Migration Plan

Users migrating from lint-staged:
1. `npm rm lint-staged && npm i -D @run-staged/cli`.
2. Convert `.lintstagedrc` to `run-staged.toml`. Once `add-lint-staged-compat` lands, this step becomes optional.
3. Replace `npx lint-staged` with `npx run-staged` in the husky hook, or run `npx run-staged --install`.

Rollback for a bad release: `npm deprecate @run-staged/cli@X.Y.Z` and publish a patch release. Published versions are never unpublished.

## Open Questions

- Should we also ask npm support for the unscoped `run-staged` name (the current holder is an empty 0.0.0 placeholder)? If it is granted later, publish an unscoped alias that depends on `@run-staged/cli`. Nothing in this design changes.
- Trusted publishing (OIDC) or an `NPM_TOKEN` secret? This is a repository admin choice and does not change the workflow's structure.
