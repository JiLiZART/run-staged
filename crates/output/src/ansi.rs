//! Escape-sequence sanitizer for captured command output.
//!
//! Commands may print colors (SGR, `ESC [ … m`), but also cursor moves, screen
//! clears, window titles (OSC), and bare carriage returns. Inside a failure box
//! or a one-line output tail only colors are safe to keep.

/// What to do with color (SGR) sequences; every other escape and C0 control
/// except `\n` and `\t` is always removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sgr {
    Keep,
    Strip,
}

/// Removes unsafe escape sequences and control characters from `input`.
pub fn sanitize(input: &str, sgr: Sgr) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => match chars.peek() {
                // CSI: ESC [ params… final-byte (0x40..=0x7e)
                Some('[') => {
                    chars.next();
                    let mut seq = String::from("\x1b[");
                    for c in chars.by_ref() {
                        seq.push(c);
                        if ('\x40'..='\x7e').contains(&c) {
                            break;
                        }
                    }
                    if sgr == Sgr::Keep && seq.ends_with('m') {
                        out.push_str(&seq);
                    }
                }
                // OSC: ESC ] … terminated by BEL or ESC \
                Some(']') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                // Any other escape: ESC, intermediates (0x20..=0x2f), final byte.
                Some(_) => {
                    for c in chars.by_ref() {
                        if !('\x20'..='\x2f').contains(&c) {
                            break;
                        }
                    }
                }
                None => {}
            },
            '\n' | '\t' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{sanitize, Sgr};

    #[test]
    fn table() {
        let cases: &[(&str, &str, &str)] = &[
            // input, keep-SGR, strip-all
            ("plain", "plain", "plain"),
            (
                "\x1b[31merror\x1b[0m: bad",
                "\x1b[31merror\x1b[0m: bad",
                "error: bad",
            ),
            ("\x1b[1;38;5;196mx\x1b[m", "\x1b[1;38;5;196mx\x1b[m", "x"),
            ("a\x1b[2Kb", "ab", "ab"),                      // erase line
            ("a\x1b[3Ab", "ab", "ab"),                      // cursor up
            ("\x1b[2J\x1b[Hscreen", "screen", "screen"),    // clear + home
            ("\x1b]0;title\x07text", "text", "text"),       // OSC with BEL
            ("\x1b]8;;http://x\x1b\\link", "link", "link"), // OSC with ST
            ("10%\r50%", "10%50%", "10%50%"),               // bare CR removed
            ("bell\x07", "bell", "bell"),
            ("tab\tnew\nline", "tab\tnew\nline", "tab\tnew\nline"),
            ("ünïcödé ✔ 日本", "ünïcödé ✔ 日本", "ünïcödé ✔ 日本"),
            ("\x1b(Bx", "x", "x"), // charset designation
            ("dangling\x1b", "dangling", "dangling"),
        ];
        for (input, keep, strip) in cases {
            assert_eq!(sanitize(input, Sgr::Keep), *keep, "keep: {input:?}");
            assert_eq!(sanitize(input, Sgr::Strip), *strip, "strip: {input:?}");
        }
    }
}
