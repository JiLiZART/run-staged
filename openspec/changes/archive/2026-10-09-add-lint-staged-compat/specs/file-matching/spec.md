## MODIFIED Requirements

### Requirement: Glob syntax
Patterns SHALL support `*`, `**`, `?`, character classes `[...]`, and alternation `{a,b}`. Patterns SHALL also support picomatch groups and extglobs, each matching within a single path segment (never across `/`):
- `(a|b)` and `@(a|b)`: exactly one of the alternatives
- `?(a|b)`: zero or one occurrence
- `+(a|b)`: one or more occurrences
- `*(a|b)`: zero or more occurrences
- `!(a|b)`: any text that, together with the rest of its path segment, does not match an alternative followed by that rest

Alternatives may themselves contain glob syntax, including nested groups. `\(`, `\)`, `\|`, `\{`, `\}`, `\[` and `\]` SHALL match the literal character. An unescaped `(` always starts a group, in every config format. `*` and `?` SHALL NOT match `/`; only `**` matches across directories. Matching SHALL be case-sensitive, and `*`, `?` and groups SHALL match names starting with `.`. An invalid glob, including an unbalanced `(`, `[` or `{`, SHALL fail with `error: invalid configuration: invalid glob pattern "<pattern>": <detail>`.

#### Scenario: Alternation
- **WHEN** the pattern is `*.{js,ts}` and `a.js` and `b.ts` are staged
- **THEN** both files match

#### Scenario: Case-sensitive
- **WHEN** the pattern is `*.ts` and `FOO.TS` is staged
- **THEN** `FOO.TS` does not match

#### Scenario: Star stays within a directory
- **WHEN** the pattern is `src/*.ts` and `src/a.ts` and `src/lib/b.ts` are staged
- **THEN** `src/a.ts` matches and `src/lib/b.ts` does not

#### Scenario: Group alternation at any depth
- **WHEN** the pattern is `*.(js|ts|tsx|jsx)` and `packages/diir/subdir/somedir/file.test.ts`, `src/a.jsx` and `README.md` are staged
- **THEN** `packages/diir/subdir/somedir/file.test.ts` and `src/a.jsx` match and `README.md` does not

#### Scenario: Extglob one-or-more
- **WHEN** the pattern is `+(a|b).txt` and `ab.txt`, `bba.txt` and `c.txt` are staged
- **THEN** `ab.txt` and `bba.txt` match and `c.txt` does not

#### Scenario: Extglob optional
- **WHEN** the pattern is `index?(.test).ts` and `index.ts`, `index.test.ts` and `index.spec.ts` are staged
- **THEN** `index.ts` and `index.test.ts` match and `index.spec.ts` does not

#### Scenario: Extglob negation
- **WHEN** the pattern is `!(*.test).ts` and `a.ts`, `a.test.ts` and `lib/b.ts` are staged
- **THEN** `a.ts` and `lib/b.ts` match and `a.test.ts` does not

#### Scenario: Groups stay within a segment
- **WHEN** the pattern is `src/(a|b)/*.ts` and `src/a/x.ts` and `src/a/b/x.ts` are staged
- **THEN** `src/a/x.ts` matches and `src/a/b/x.ts` does not

#### Scenario: Escaped parentheses are literal
- **WHEN** the pattern is `app/\(auth\)/*.tsx` and `app/(auth)/page.tsx` and `app/auth/page.tsx` are staged
- **THEN** `app/(auth)/page.tsx` matches and `app/auth/page.tsx` does not

#### Scenario: Unbalanced group rejected
- **WHEN** the pattern is `*.(js|ts`
- **THEN** the tool exits with code 1 and prints `error: invalid configuration: invalid glob pattern "*.(js|ts": ...`

### Requirement: Negation-only pattern rejected
A pattern key starting with `!` SHALL be rejected with an invalid configuration error stating the pattern has only negation and nothing can match. A key starting with `!(` SHALL NOT be treated as negation: it is a negated extglob (see "Glob syntax") and matches like any other pattern.

#### Scenario: Negation-only key
- **WHEN** a group has the pattern key `!*.md`
- **THEN** the tool exits with code 1 and an invalid configuration error naming the group and pattern

#### Scenario: Leading negated extglob is not negation
- **WHEN** a group has the pattern key `!(*.test).ts` and `a.ts` is staged
- **THEN** `a.ts` matches and no negation error is raised
