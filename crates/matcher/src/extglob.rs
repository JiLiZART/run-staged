//! picomatch-style extended globs: groups `(a|b)` and extglobs `@(..)`,
//! `?(..)`, `+(..)`, `*(..)`, `!(..)`, alongside `*`, `**`, `?`, `[...]` and
//! `{a,b}`. A pattern is parsed into nodes and translated to an anchored
//! regex. `fancy-regex` is used because `!(..)` needs a negative lookahead.

use fancy_regex::Regex;

/// Whether `pattern` uses group syntax (an unescaped `(`) and so must be
/// compiled here rather than by globset.
pub fn is_extended(pattern: &str) -> bool {
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '(' => return true,
            _ => {}
        }
    }
    false
}

/// Compiles an already match-base-normalized pattern. The error is the
/// detail for an invalid-pattern message.
pub fn compile(pattern: &str) -> Result<Regex, String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut parser = Parser { chars, pos: 0 };
    let nodes = parser.sequence(Context::Top)?;
    let regex = format!("^{}$", emit_seq(&nodes, ""));
    Regex::new(&regex).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    /// `(a|b)`, `@(a|b)`, `{a,b}`: exactly one.
    One,
    /// `?(a|b)`
    Optional,
    /// `+(a|b)`
    OneOrMore,
    /// `*(a|b)`
    ZeroOrMore,
    /// `!(a|b)`
    Not,
}

#[derive(Debug)]
enum Node {
    Literal(char),
    /// `*`: any run of characters within a segment.
    Star,
    /// `?`: one character within a segment.
    AnyChar,
    /// `**` followed by `/`: zero or more whole directories.
    GlobstarDir,
    /// `**` at the end of the pattern: anything, across segments.
    GlobstarRest,
    /// `[...]`, already in regex form.
    Class(String),
    Group(Op, Vec<Vec<Node>>),
}

#[derive(Clone, Copy, PartialEq)]
enum Context {
    Top,
    /// Inside `(...)`: `|` separates alternatives, `)` ends.
    Paren,
    /// Inside `{...}`: `,` separates alternatives, `}` ends.
    Brace,
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    /// Parses until the end of input or a delimiter that ends `ctx`
    /// (left unconsumed).
    fn sequence(&mut self, ctx: Context) -> Result<Vec<Node>, String> {
        let mut nodes = Vec::new();
        while let Some(c) = self.peek(0) {
            match (c, ctx) {
                ('|' | ')', Context::Paren) | (',' | '}', Context::Brace) => break,
                (')', _) => {
                    return Err("unexpected \")\" - use \\) to match a literal parenthesis".into());
                }
                ('|', _) => {
                    return Err(
                        "unexpected \"|\" outside a group - use \\| to match it literally".into(),
                    );
                }
                _ => {}
            }
            let next = self.peek(1);
            let op = match (c, next) {
                ('@', Some('(')) => Some(Op::One),
                ('?', Some('(')) => Some(Op::Optional),
                ('+', Some('(')) => Some(Op::OneOrMore),
                ('*', Some('(')) => Some(Op::ZeroOrMore),
                ('!', Some('(')) => Some(Op::Not),
                _ => None,
            };
            if let Some(op) = op {
                self.pos += 2;
                nodes.push(self.group(op)?);
                continue;
            }
            self.pos += 1;
            let node = match c {
                '\\' => {
                    let escaped = self.peek(0).ok_or("trailing \"\\\" escapes nothing")?;
                    self.pos += 1;
                    Node::Literal(escaped)
                }
                '(' => self.group(Op::One)?,
                '{' => self.brace()?,
                '[' => self.class()?,
                '?' => Node::AnyChar,
                '*' if next == Some('*') => {
                    self.pos += 1;
                    let starts_segment = matches!(nodes.last(), None | Some(Node::Literal('/')));
                    match self.peek(0) {
                        Some('/') if starts_segment => {
                            self.pos += 1;
                            Node::GlobstarDir
                        }
                        None if starts_segment && ctx == Context::Top => Node::GlobstarRest,
                        _ => Node::Star,
                    }
                }
                '*' => Node::Star,
                other => Node::Literal(other),
            };
            nodes.push(node);
        }
        Ok(nodes)
    }

    /// After `(` (or `@(`, `?(`, ...): alternatives separated by `|` up to `)`.
    fn group(&mut self, op: Op) -> Result<Node, String> {
        let mut alternatives = Vec::new();
        loop {
            alternatives.push(self.sequence(Context::Paren)?);
            match self.peek(0) {
                Some('|') => self.pos += 1,
                Some(')') => {
                    self.pos += 1;
                    break;
                }
                _ => {
                    return Err(
                        "missing closing \")\" - use \\( or \\) to match a literal parenthesis"
                            .into(),
                    );
                }
            }
        }
        if alternatives.iter().all(Vec::is_empty) {
            return Err("empty group \"()\"".into());
        }
        Ok(Node::Group(op, alternatives))
    }

    /// After `{`: alternatives separated by `,` up to `}`.
    fn brace(&mut self) -> Result<Node, String> {
        let mut alternatives = Vec::new();
        loop {
            alternatives.push(self.sequence(Context::Brace)?);
            match self.peek(0) {
                Some(',') => self.pos += 1,
                Some('}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err("missing closing \"}\" - use \\{ to match a literal brace".into()),
            }
        }
        Ok(Node::Group(Op::One, alternatives))
    }

    /// After `[`: a character class up to `]`, translated to regex syntax.
    fn class(&mut self) -> Result<Node, String> {
        let mut out = String::from("[");
        if matches!(self.peek(0), Some('!' | '^')) {
            out.push('^');
            self.pos += 1;
        }
        let mut first = true;
        loop {
            let c = self
                .peek(0)
                .ok_or("missing closing \"]\" - use \\[ to match a literal bracket")?;
            self.pos += 1;
            match c {
                ']' if !first => break,
                '\\' => {
                    let escaped = self.peek(0).ok_or("trailing \"\\\" escapes nothing")?;
                    self.pos += 1;
                    out.push('\\');
                    out.push(escaped);
                }
                // Special inside regex classes (incl. set operators && ~~ --).
                '[' | ']' | '&' | '~' | '^' => {
                    out.push('\\');
                    out.push(c);
                }
                _ => out.push(c),
            }
            first = false;
        }
        out.push(']');
        Ok(Node::Class(out))
    }
}

/// Regex for `nodes`, where `after` is the regex of everything that follows
/// them (used by `!(..)`, whose lookahead covers the rest of the pattern).
fn emit_seq(nodes: &[Node], after: &str) -> String {
    let mut parts = Vec::with_capacity(nodes.len());
    let mut suffix = after.to_string();
    for node in nodes.iter().rev() {
        let part = emit_node(node, &suffix);
        suffix = format!("{part}{suffix}");
        parts.push(part);
    }
    parts.reverse();
    parts.concat()
}

fn emit_node(node: &Node, after: &str) -> String {
    match node {
        Node::Literal(c) => fancy_regex::escape(&c.to_string()).into_owned(),
        Node::Star => "[^/]*".into(),
        Node::AnyChar => "[^/]".into(),
        Node::GlobstarDir => "(?:[^/]*/)*".into(),
        Node::GlobstarRest => ".*".into(),
        Node::Class(class) => class.clone(),
        Node::Group(op, alternatives) => {
            let alts = alternatives
                .iter()
                .map(|alt| emit_seq(alt, after))
                .collect::<Vec<_>>()
                .join("|");
            match op {
                Op::One => format!("(?:{alts})"),
                Op::Optional => format!("(?:{alts})?"),
                Op::OneOrMore => format!("(?:{alts})+"),
                Op::ZeroOrMore => format!("(?:{alts})*"),
                // Any text within the segment unless, together with the
                // rest of the pattern, it would match an alternative.
                Op::Not => format!("(?:(?!(?:{alts}){after}$)[^/]*?)"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(pattern: &str, path: &str) -> bool {
        compile(pattern).unwrap().is_match(path).unwrap()
    }

    fn error(pattern: &str) -> String {
        compile(pattern).unwrap_err()
    }

    #[test]
    fn detects_unescaped_paren_only() {
        assert!(is_extended("*.(js|ts)"));
        assert!(is_extended("!(*.test).ts"));
        assert!(!is_extended("app/\\(auth\\)/*.tsx"));
        assert!(!is_extended("*.{js,ts}"));
    }

    #[test]
    fn plain_group_and_at() {
        for p in ["**/*.(js|ts)", "**/*.@(js|ts)"] {
            assert!(matches(p, "a/b.ts"), "{p}");
            assert!(matches(p, "b.js"), "{p}");
            assert!(!matches(p, "b.tsx"), "{p}");
        }
    }

    #[test]
    fn optional_one_or_more_zero_or_more() {
        assert!(matches("index?(.test).ts", "index.ts"));
        assert!(matches("index?(.test).ts", "index.test.ts"));
        assert!(!matches("index?(.test).ts", "index.test.test.ts"));
        assert!(matches("+(a|b).txt", "abba.txt"));
        assert!(!matches("+(a|b).txt", ".txt"));
        assert!(matches("x*(ab)y", "xy"));
        assert!(matches("x*(ab)y", "xababy"));
        assert!(!matches("x*(ab)y", "xaby2"));
    }

    #[test]
    fn negation() {
        assert!(matches("!(*.test).ts", "a.ts"));
        assert!(!matches("!(*.test).ts", "a.test.ts"));
        assert!(matches("**/!(*.min).js", "src/app.js"));
        assert!(!matches("**/!(*.min).js", "vendor/lib.min.js"));
    }

    #[test]
    fn nesting_and_braces_inside_groups() {
        assert!(matches("*.(c|m)@(js|ts)", "a.mts"));
        assert!(matches("*.(json|{yml,yaml})", "a.yaml"));
        assert!(!matches("*.(json|{yml,yaml})", "a.toml"));
    }

    #[test]
    fn groups_stay_within_a_segment() {
        assert!(matches("src/(a|b)/*.ts", "src/a/x.ts"));
        assert!(!matches("src/(a|b)/*.ts", "src/a/b/x.ts"));
        assert!(!matches("*(a|b)", "a/b"));
    }

    #[test]
    fn globstar_and_classes() {
        assert!(matches("**/*.(ts)", "x.ts"));
        assert!(matches("**/*.(ts)", "a/b/c/x.ts"));
        assert!(matches("src/**", "src/a/b.ts"));
        assert!(matches("[ab]*.(ts)", "b1.ts"));
        assert!(!matches("[!ab]*.(ts)", "b1.ts"));
        assert!(matches("[]x].(md)", "].md"));
    }

    #[test]
    fn escapes_are_literal() {
        assert!(matches("a\\(b\\).(md)", "a(b).md"));
        assert!(matches("a\\|b.(md)", "a|b.md"));
        assert!(matches("x.(a\\)b)", "x.a)b"));
    }

    #[test]
    fn errors() {
        assert!(error("*.(js|ts").contains("missing closing \")\""));
        assert!(error("*.(js|ts").contains("\\("), "hint mentions escaping");
        assert!(error("*.js)").contains("unexpected \")\""));
        assert!(error("a|b(c)").contains("outside a group"));
        assert!(error("*.()").contains("empty group"));
        assert!(error("(x)[ab").contains("missing closing \"]\""));
        assert!(error("(x){a,b").contains("missing closing \"}\""));
        assert!(error("(x)\\").contains("escapes nothing"));
    }
}
