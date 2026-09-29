# file-matching Specification

## Purpose
Match the changed file list against each group's glob patterns to decide which commands run on which files.

## Requirements

### Requirement: Match-base semantics
A pattern without `/` SHALL match a file's name at any directory depth. A pattern containing `/` SHALL match paths relative to the repository root.

#### Scenario: Slashless pattern matches nested file
- **WHEN** the pattern is `*.ts` and `src/foo/bar.ts` is staged
- **THEN** `src/foo/bar.ts` matches

#### Scenario: Slash pattern anchored at root
- **WHEN** the pattern is `src/*.ts` and `nested/src/bar.ts` is staged
- **THEN** `nested/src/bar.ts` does not match

### Requirement: Glob syntax
Patterns SHALL support `*`, `**`, `?`, character classes `[...]`, and alternation `{a,b}`. Matching SHALL be case-sensitive. An invalid glob SHALL fail with `error: invalid configuration: invalid glob pattern "<pattern>": <detail>`.

#### Scenario: Alternation
- **WHEN** the pattern is `*.{js,ts}` and `a.js` and `b.ts` are staged
- **THEN** both files match

#### Scenario: Case-sensitive
- **WHEN** the pattern is `*.ts` and `FOO.TS` is staged
- **THEN** `FOO.TS` does not match

### Requirement: Negation-only pattern rejected
A pattern key starting with `!` SHALL be rejected with an invalid configuration error stating the pattern has only negation and nothing can match.

#### Scenario: Negation-only key
- **WHEN** a group has the pattern key `!*.md`
- **THEN** the tool exits with code 1 and an invalid configuration error naming the group and pattern

### Requirement: Unmatched patterns and groups
Patterns that match no files SHALL NOT run their commands, and groups with no matching patterns SHALL be skipped. When no file matches any pattern, the tool SHALL print `error: no staged files matched any configured patterns` and exit with code 1, unless `--allow-empty` is given, in which case it SHALL exit 0 silently.

#### Scenario: Only non-matching files staged
- **WHEN** only `README.md` is staged and the config only has `*.rs` patterns
- **THEN** no command runs and the tool exits with code 1 and the no-match message

#### Scenario: No match with allow-empty
- **WHEN** no staged file matches and `--allow-empty` is given
- **THEN** no command runs and the tool exits 0 silently
