## Why

Most projects that use lint-staged install their tooling through npm, not Cargo. To be a drop-in replacement, run-staged has to arrive with one `npm install -D`, with no Rust toolchain, no compile step, and no install-time downloads. SWC, Biome, and esbuild have shown that shipping precompiled platform binaries as optional npm dependencies works well. The unscoped npm name `run-staged` is held by an unrelated placeholder package, so everything ships under the `@run-staged` scope.

## What Changes

- New npm package `@run-staged/cli`. It exposes the `run-staged` command through a thin JS launcher, contains no native code, and declares one optional dependency per platform.
- Eight platform packages `@run-staged/<platform>`, each holding the precompiled release binary and restricted to one platform with `os`, `cpu`, and `libc`: `darwin-arm64`, `darwin-x64`, `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `win32-x64-msvc`, `win32-arm64-msvc`.
- The launcher finds the installed platform binary and runs it with the same arguments, stdio, environment, exit code, and signal behavior. Where Node can replace its own process with the binary, it does. `RUN_STAGED_BINARY` overrides the binary path. An unsupported platform fails with a clear message.
- A release workflow: a version tag builds all eight targets, publishes the platform packages, then publishes `@run-staged/cli`. All packages use the crate's version.
- An npm smoke test: install the packed packages into throwaway npm, pnpm, and Yarn projects and run a real commit through a hook.
- Docs: migrating from lint-staged.

## Capabilities

### New Capabilities
- `npm-distribution`: package layout, platform selection, launcher behavior, versioning, and release.

### Modified Capabilities
None.

## Impact

- New files: `npm/cli/` (launcher and `package.json`), `npm/platforms/<platform>/package.json`, `npm/scripts/` (version sync, smoke test), `.github/workflows/release.yml`.
- CI: cross-compilation for Linux arm64 and musl targets with `cargo-zigbuild`. macOS and Windows targets build on native runners.
- Registry: needs the `@run-staged` npm org and a publish credential.
- Depends on `build-run-staged-cli`: the `apps/run-staged` binary and the hook script's `node_modules/.bin/run-staged` lookup.
- Out of scope: NAPI bindings, WASM builds, postinstall download, and the unscoped `run-staged` npm name.
