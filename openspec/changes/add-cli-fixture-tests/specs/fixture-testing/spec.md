## Purpose

Defines the end-to-end test harness that runs the compiled `run-staged` binary against temporary git repositories built from declarative fixtures, with identical behavior on Linux, macOS, and Windows.

## ADDED Requirements

### Requirement: Fixture directory layout
A fixture SHALL be a directory under `apps/run-staged/tests/fixtures/<name>/` with up to three optional subdirectories:
- `committed/`: files written and committed as the initial commit.
- `staged/`: files written over the working tree after that commit and then staged.
- `unstaged/`: files written after staging and not staged.

Nested paths SHALL be kept. A fixture with no `committed/` directory SHALL produce a repository with no commits.

#### Scenario: Staged over committed
- **WHEN** a fixture has `committed/a.js` and `staged/a.js` with different content
- **THEN** HEAD holds the committed content, the index holds the staged content, and `run-staged` sees `a.js` as staged

#### Scenario: Partially staged file
- **WHEN** a fixture has `staged/a.js` and `unstaged/a.js` with different content
- **THEN** the index holds the staged content and the working tree holds the unstaged content

#### Scenario: Repository without commits
- **WHEN** a fixture has only `staged/`
- **THEN** the repository has no commits and every staged file is reported as staged

### Requirement: Programmatic fixtures
Tests SHALL also be able to build a repository in code, with the same three layers, plus git config entries and extra files outside the three layers (for example, a pre-existing hook). Both kinds of fixture SHALL produce the same kind of repository handle.

#### Scenario: Inline fixture with git config
- **WHEN** a test builds a repository in code with `core.hooksPath = .githooks`
- **THEN** `git config core.hooksPath` in that repository prints `.githooks`

### Requirement: Isolated repositories
Every fixture repository SHALL be created in a fresh temporary directory. System and user git configuration SHALL NOT be read. Author, committer, commit dates, and the default branch (`main`) SHALL be fixed. The same environment SHALL apply to the `run-staged` process under test. The temporary directory SHALL be deleted after the test, unless `RUN_STAGED_KEEP_FIXTURES=1` is set; in that case its path SHALL be printed.

#### Scenario: User git config ignored
- **WHEN** the developer's global git config sets `core.autocrlf = true` and `core.hooksPath`
- **THEN** fixture repositories and the binary under test see neither setting

#### Scenario: Parallel tests do not collide
- **WHEN** two tests use the same fixture at the same time
- **THEN** each gets its own repository and neither sees the other's changes

### Requirement: Running the binary
The harness SHALL run the `run-staged` binary built from the current workspace. It SHALL run it with the fixture repository (or a given subdirectory) as the working directory, with the given arguments and extra environment variables. It SHALL capture the exit code, stdout, and stderr. The harness SHALL put `fixture-tool` on the PATH of the process under test, ahead of any other entry. It SHALL offer an option that removes every directory containing a `git` executable from that PATH. The harness SHALL fail the test if the binary runs longer than a configurable limit (default 30 seconds).

#### Scenario: Run in subdirectory
- **WHEN** a test runs the binary in `packages/web` of a fixture repository
- **THEN** config discovery starts from `packages/web`

#### Scenario: Run without git
- **WHEN** a test runs the binary with the no-git option
- **THEN** a command inside the run that tries to execute `git` fails with not found

#### Scenario: Hung binary
- **WHEN** the binary does not exit within the limit
- **THEN** the process is killed and the test fails with a timeout message

### Requirement: Lock helpers
The harness SHALL let a test create the index lock file (`<index>.lock`) before a run, and remove it either after a given delay while the binary is running, or never. It SHALL let a test set `GIT_INDEX_FILE` for the run to an alternate index file built from the fixture's staged state. It SHALL report whether the lock file's content or mtime changed during the run.

#### Scenario: Lock released during run
- **WHEN** a test holds `.git/index.lock` and asks for it to be released after 1 second
- **THEN** the lock file disappears about 1 second after the binary starts

#### Scenario: Commit -a style index
- **WHEN** a test asks for a `git commit -a` style environment
- **THEN** `.git/index.lock` holds the staged state and the binary runs with `GIT_INDEX_FILE` pointing at it

### Requirement: Fake command tool
The harness SHALL provide a `fixture-tool` executable that works the same on every supported OS. Options decide what it does, and every invocation SHALL:
- append one record (its arguments, and its working directory relative to the repository root) to an invocation log in the repository's git directory;
- optionally sleep for a given number of milliseconds;
- optionally, for each file argument, append given text, replace the content, or delete the file;
- optionally write given text to stdout or stderr;
- optionally print the values of named environment variables to stdout, as `NAME=value` or `NAME=<unset>`;
- optionally copy a named file from the git directory to `fixture-tool.<name>` in the git directory before anything else, so tests can see state that only exists mid-run;
- exit with a given code (default 0).

#### Scenario: Formatter stand-in
- **WHEN** a fixture config runs `fixture-tool --append "// fmt"` on `a.js`
- **THEN** `a.js` ends with `// fmt` and the invocation log contains one record with argument `a.js`

#### Scenario: Failing linter stand-in
- **WHEN** a fixture config runs `fixture-tool --stderr "lint error" --exit 1`
- **THEN** the tool writes `lint error` to stderr and exits 1

### Requirement: Repository assertions
The harness SHALL let tests read the following after a run: the index content of a path, the working-tree content of a path, the list of staged paths, whether a path exists in the git directory (for example `run-staged.index`), and the parsed invocation log in order of completion.

#### Scenario: Assert staged content
- **WHEN** a formatter stand-in changed `a.js` and the run succeeded
- **THEN** the test can read the index content of `a.js` and compare it with the formatted text

### Requirement: Output normalization
Before comparison, the harness SHALL make captured output stable:
- durations become `<duration>`;
- the fixture repository's absolute path becomes `<repo>`;
- Windows path separators inside repository paths become `/`;
- CRLF line endings become LF.

Tests SHALL compare normalized output with stored snapshots.

#### Scenario: Stable success line
- **WHEN** the binary prints `✓ default/*.js: fixture-tool (12ms, 1 file)`
- **THEN** the normalized output is `✓ default/*.js: fixture-tool (<duration>, 1 file)`

### Requirement: Cross-platform suite
The fixture suite SHALL pass on Linux, macOS, and Windows. A test that checks platform-specific behavior (such as signals or file modes) SHALL be marked as limited to its platform instead of failing on the others.

#### Scenario: Windows run
- **WHEN** `cargo test -p run-staged` runs on Windows
- **THEN** all tests not limited to another platform pass without `sh` or other Unix tools on PATH
