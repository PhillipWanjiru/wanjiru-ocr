//! A scanner that separates the code of a Rust file from its comments and strings.
//!
//! The checks look for the keyword `unsafe` and for `SAFETY:` comments; a regular expression over
//! raw text would find them inside string literals and miss them in nested comments. This scanner
//! returns, for every line, the code with comments and the contents of strings and characters
//! blanked out (the quotes stay), and the text of the comments on that line.

/// One line of a source file, split into code and comment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    /// The code on the line; comments and the insides of literals are spaces.
    pub code: String,
    /// The comment text on the line, including the `//` or `///` markers.
    pub comment: String,
}

impl Line {
    /// A comment and no code.
    pub fn is_comment_only(&self) -> bool {
        self.code.trim().is_empty() && !self.comment.trim().is_empty()
    }

    /// An attribute and nothing else: `#[...]` or `#![...]`.
    pub fn is_attribute(&self) -> bool {
        let code = self.code.trim();
        code.starts_with("#[") || code.starts_with("#![")
    }
}

#[derive(Clone, Copy)]
enum State {
    Code,
    LineComment,
    BlockComment(u32),
    Text { raw_hashes: Option<usize> },
}

struct Scanner {
    chars: Vec<char>,
    pos: usize,
    lines: Vec<Line>,
    current: Line,
}

impl Scanner {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn end_line(&mut self) {
        self.lines.push(std::mem::take(&mut self.current));
    }

    /// Emit one character of a literal's inside as blank.
    fn blank(&mut self, c: char) {
        if c == '\n' {
            self.end_line();
        } else {
            self.current.code.push(' ');
        }
    }

    /// `'` starts a character literal when a closing quote follows one character or one escape.
    fn char_literal_length(&self) -> Option<usize> {
        match self.peek(1)? {
            '\\' => {
                // '\n', '\'', '\\', '\u{1F600}', '\x7f'
                let mut i = 3;
                while let Some(c) = self.peek(i) {
                    if c == '\'' {
                        return Some(i + 1);
                    }
                    if c == '\n' || i > 12 {
                        return None;
                    }
                    i += 1;
                }
                None
            }
            '\'' | '\n' => None,
            _ => (self.peek(2) == Some('\'')).then_some(3),
        }
    }

    /// A string prefix (`r`, `b`, `br`, `c`, `cr`, then `#`s, then `"`) at the cursor: its length and
    /// the number of `#`s of a raw string.
    fn string_prefix(&self) -> Option<(usize, Option<usize>)> {
        let before_is_identifier = self
            .pos
            .checked_sub(1)
            .and_then(|i| self.chars.get(i))
            .is_some_and(|c| c.is_alphanumeric() || *c == '_');
        if before_is_identifier {
            return None;
        }
        let mut i = 0;
        let mut raw = false;
        loop {
            match self.peek(i) {
                Some('b' | 'c') if i == 0 => i += 1,
                Some('r') if i < 2 && !raw => {
                    raw = true;
                    i += 1;
                }
                _ => break,
            }
        }
        if raw {
            let mut hashes = 0;
            while self.peek(i + hashes) == Some('#') {
                hashes += 1;
            }
            (self.peek(i + hashes) == Some('"')).then_some((i + hashes + 1, Some(hashes)))
        } else if i > 0 && self.peek(i) == Some('"') {
            Some((i + 1, None))
        } else {
            None
        }
    }
}

/// Split `source` into lines of code and comment.
pub fn scan(source: &str) -> Vec<Line> {
    let mut s = Scanner {
        chars: source.chars().collect(),
        pos: 0,
        lines: Vec::new(),
        current: Line::default(),
    };
    let mut state = State::Code;
    while let Some(c) = s.peek(0) {
        match state {
            State::Code => {
                if c == '\n' {
                    s.end_line();
                    s.pos += 1;
                } else if c == '/' && s.peek(1) == Some('/') {
                    state = State::LineComment;
                } else if c == '/' && s.peek(1) == Some('*') {
                    s.current.comment.push_str("/*");
                    s.pos += 2;
                    state = State::BlockComment(1);
                } else if c == '"' {
                    s.current.code.push('"');
                    s.pos += 1;
                    state = State::Text { raw_hashes: None };
                } else if let Some((length, raw_hashes)) = matches!(c, 'r' | 'b' | 'c')
                    .then(|| s.string_prefix())
                    .flatten()
                {
                    for _ in 0..length {
                        let ch = s.chars[s.pos];
                        s.current.code.push(ch);
                        s.pos += 1;
                    }
                    state = State::Text { raw_hashes };
                } else if c == '\'' {
                    if let Some(length) = s.char_literal_length() {
                        s.current.code.push('\'');
                        for _ in 0..length - 2 {
                            s.current.code.push(' ');
                        }
                        s.current.code.push('\'');
                        s.pos += length;
                    } else {
                        s.current.code.push('\'');
                        s.pos += 1;
                    }
                } else {
                    s.current.code.push(c);
                    s.pos += 1;
                }
            }
            State::LineComment => {
                if c == '\n' {
                    state = State::Code;
                } else {
                    s.current.comment.push(c);
                    s.pos += 1;
                }
            }
            State::BlockComment(depth) => {
                if c == '/' && s.peek(1) == Some('*') {
                    s.current.comment.push_str("/*");
                    s.pos += 2;
                    state = State::BlockComment(depth + 1);
                } else if c == '*' && s.peek(1) == Some('/') {
                    s.current.comment.push_str("*/");
                    s.pos += 2;
                    state = if depth == 1 {
                        State::Code
                    } else {
                        State::BlockComment(depth - 1)
                    };
                } else if c == '\n' {
                    s.end_line();
                    s.pos += 1;
                } else {
                    s.current.comment.push(c);
                    s.pos += 1;
                }
            }
            State::Text { raw_hashes } => match raw_hashes {
                None if c == '\\' => {
                    s.current.code.push(' ');
                    s.pos += 1;
                    if let Some(next) = s.peek(0) {
                        s.blank(next);
                        s.pos += 1;
                    }
                }
                None if c == '"' => {
                    s.current.code.push('"');
                    s.pos += 1;
                    state = State::Code;
                }
                Some(hashes) if c == '"' && (1..=hashes).all(|k| s.peek(k) == Some('#')) => {
                    s.current.code.push('"');
                    for _ in 0..hashes {
                        s.current.code.push('#');
                    }
                    s.pos += 1 + hashes;
                    state = State::Code;
                }
                _ => {
                    s.blank(c);
                    s.pos += 1;
                }
            },
        }
    }
    s.end_line();
    s.lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(source: &str) -> Vec<String> {
        scan(source).into_iter().map(|l| l.code).collect()
    }

    fn comments(source: &str) -> Vec<String> {
        scan(source).into_iter().map(|l| l.comment).collect()
    }

    #[test]
    fn comments_and_code_are_separated() {
        let lines = scan("let a = 1; // unsafe here\n/// docs\nfn f() {}");
        assert_eq!(lines[0].code, "let a = 1; ");
        assert_eq!(lines[0].comment, "// unsafe here");
        assert!(lines[1].is_comment_only());
        assert_eq!(lines[2].code, "fn f() {}");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn block_comments_nest_and_span_lines() {
        let source = "a /* x /* unsafe */ y\nstill */ b";
        assert_eq!(code(source), ["a ", " b"]);
        assert!(comments(source)[0].contains("unsafe"));
    }

    #[test]
    fn strings_are_blanked_but_keep_their_quotes_and_lines() {
        let source = "let s = \"unsafe { \\\" }\";\nlet t = \"two\nlines unsafe\";\nx";
        let c = code(source);
        assert!(!c.concat().contains("unsafe"), "{c:?}");
        assert_eq!(c[0].matches('"').count(), 2);
        assert_eq!(c.len(), 4);
        assert_eq!(c[3], "x");
    }

    #[test]
    fn raw_byte_and_c_strings_are_blanked() {
        let source = "a = r#\"unsafe \" still\"#; b = br\"unsafe\"; c = b\"unsafe\"; d = c\"unsafe\"; e = r\"x\"; end";
        let c = code(source);
        assert!(!c[0].contains("unsafe"), "{c:?}");
        assert!(c[0].ends_with("end"), "{c:?}");
    }

    #[test]
    fn identifiers_ending_in_r_are_not_string_prefixes() {
        // `r` after an identifier character is part of the identifier; the quote then starts a string.
        let c = code("var\"unsafe\"")[0].clone();
        assert!(c.starts_with("var\""), "{c}");
        assert!(!c.contains("unsafe"), "{c}");
        // an identifier that merely begins with `r` or `b` is not a prefix either
        assert_eq!(code("return b + c")[0], "return b + c");
    }

    #[test]
    fn character_literals_and_lifetimes_are_told_apart() {
        let c = code(
            "let q = '\"'; let l: &'a str = x; let u = '\\u{1F600}'; let e = '\\''; unsafe {}",
        );
        assert!(c[0].contains("&'a str"), "{c:?}");
        assert!(c[0].contains("unsafe {}"), "{c:?}");
        // the double quote inside the character literal did not open a string
        assert!(c[0].ends_with("unsafe {}"), "{c:?}");
    }

    #[test]
    fn a_lifetime_followed_by_a_char_is_still_a_lifetime() {
        let c = code("fn f<'a>(x: &'a u8) -> &'a u8 { x }");
        assert_eq!(c[0], "fn f<'a>(x: &'a u8) -> &'a u8 { x }");
    }

    #[test]
    fn an_empty_file_is_one_empty_line() {
        assert_eq!(scan(""), [Line::default()]);
    }

    #[test]
    fn attributes_and_blank_lines_are_recognised() {
        let lines = scan("#![forbid(unsafe_code)]\n\n#[inline]\n// c\n");
        assert!(lines[0].is_attribute());
        assert_eq!(lines[1], Line::default());
        assert!(lines[2].is_attribute());
        assert!(lines[3].is_comment_only());
    }
}
