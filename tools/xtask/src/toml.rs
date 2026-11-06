//! The part of TOML that Cargo manifests and `deny.toml` use.
//!
//! Tables (`[a.b]`, `[[a]]`), keys (bare, quoted, dotted), strings (basic, literal, multi-line),
//! booleans, numbers (kept as text), arrays and inline tables, with comments anywhere. Dates and
//! times are read as text too; nothing here needs their value. Anything else is an error with a line
//! number, never a guess.

use std::collections::BTreeMap;

/// A TOML value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A string, basic or literal.
    String(String),
    /// `true` or `false`.
    Bool(bool),
    /// A number or a date, as written.
    Raw(String),
    /// `[...]`
    Array(Vec<Value>),
    /// `{ ... }`, or the sub-tables a dotted key creates.
    Table(Table),
}

/// A table: keys in sorted order.
pub type Table = BTreeMap<String, Value>;

impl Value {
    /// The text of a string value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// The elements of an array value.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The members of a table value.
    pub fn as_table(&self) -> Option<&Table> {
        match self {
            Self::Table(table) => Some(table),
            _ => None,
        }
    }

    /// The truth value of a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

/// A parsed document: the top-level table, with every `[section]` nested under its dotted path.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    /// The root table.
    pub root: Table,
}

impl Document {
    /// The value at a dotted path such as `["workspace", "dependencies"]`.
    pub fn get(&self, path: &[&str]) -> Option<&Value> {
        let (last, parents) = path.split_last()?;
        let mut table = &self.root;
        for key in parents {
            table = table.get(*key)?.as_table()?;
        }
        table.get(*last)
    }

    /// The table at a dotted path.
    pub fn table(&self, path: &[&str]) -> Option<&Table> {
        self.get(path)?.as_table()
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    line: usize,
}

/// A number, `inf`, `nan` or a date-time, as TOML spells them: no spaces, except the one between a
/// date and a time.
fn is_number_or_date(text: &str) -> bool {
    let plain = |t: &str| {
        let first = t.chars().next();
        (first.is_some_and(|c| c.is_ascii_digit() || c == '+' || c == '-')
            || matches!(t, "inf" | "nan"))
            && t.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-' | '.' | ':'))
    };
    match text.split_once(' ') {
        None => plain(text),
        Some((date, time)) => {
            date.len() == 10
                && date.as_bytes().iter().enumerate().all(|(i, b)| {
                    if matches!(i, 4 | 7) {
                        *b == b'-'
                    } else {
                        b.is_ascii_digit()
                    }
                })
                && time.starts_with(|c: char| c.is_ascii_digit())
                && plain(time)
        }
    }
}

/// Parse a document.
pub fn parse(text: &str) -> Result<Document, String> {
    let mut parser = Parser {
        chars: text.chars().collect(),
        pos: 0,
        line: 1,
    };
    parser.document()
}

impl Parser {
    fn error<T>(&self, message: &str) -> Result<T, String> {
        Err(format!("line {}: {message}", self.line))
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn skip_blanks(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.pos += 1;
        }
    }

    fn skip_comment(&mut self) {
        if self.peek() == Some('#') {
            while !matches!(self.peek(), None | Some('\n')) {
                self.pos += 1;
            }
        }
    }

    /// Whitespace, comments and newlines: between statements and inside arrays.
    fn skip_space(&mut self) {
        loop {
            match self.peek() {
                Some(' ' | '\t' | '\r' | '\n') => {
                    self.bump();
                }
                Some('#') => self.skip_comment(),
                _ => break,
            }
        }
    }

    fn end_of_statement(&mut self) -> Result<(), String> {
        self.skip_blanks();
        self.skip_comment();
        match self.peek() {
            None => Ok(()),
            Some('\r') => {
                self.bump();
                self.end_of_statement()
            }
            Some('\n') => {
                self.bump();
                Ok(())
            }
            Some(c) => self.error(&format!("unexpected `{c}` after a value")),
        }
    }

    fn document(&mut self) -> Result<Document, String> {
        let mut root = Table::new();
        let mut current: Vec<String> = Vec::new();
        loop {
            self.skip_space();
            let Some(c) = self.peek() else { break };
            if c == '[' {
                let array = self.peek_at(1) == Some('[');
                self.pos += if array { 2 } else { 1 };
                self.skip_blanks();
                let path = self.key_path(']')?;
                if self.bump() != Some(']') {
                    return self.error("a table header is not closed");
                }
                if array && self.bump() != Some(']') {
                    return self.error("an array-of-tables header is not closed");
                }
                current = path;
                if array {
                    // Only the last element is kept: nothing here reads the others.
                    if let Some(last) = current.last_mut() {
                        last.push_str("[]");
                    }
                }
                self.open_table(&mut root, &current, array)?;
                self.end_of_statement()?;
            } else {
                let path = self.key_path('=')?;
                if self.bump() != Some('=') {
                    return self.error("a key needs `=`");
                }
                self.skip_blanks();
                let value = self.value()?;
                let table = self.open_table(&mut root, &current, false)?;
                self.insert(table, &path, value)?;
                self.end_of_statement()?;
            }
        }
        Ok(Document { root })
    }

    /// The table at `path`, created on the way; with `fresh`, the last one starts empty again, which is
    /// how a second `[[x]]` replaces the first.
    fn open_table<'t>(
        &self,
        root: &'t mut Table,
        path: &[String],
        fresh: bool,
    ) -> Result<&'t mut Table, String> {
        let mut table = root;
        for (depth, key) in path.iter().enumerate() {
            let last = depth + 1 == path.len();
            let entry = table
                .entry(key.clone())
                .or_insert_with(|| Value::Table(Table::new()));
            if last && fresh {
                *entry = Value::Table(Table::new());
            }
            match entry {
                Value::Table(inner) => table = inner,
                _ => return self.error(&format!("`{key}` is a value and also a table")),
            }
        }
        Ok(table)
    }

    fn insert(&self, table: &mut Table, path: &[String], value: Value) -> Result<(), String> {
        let Some((last, parents)) = path.split_last() else {
            return self.error("a key is empty");
        };
        let mut table = table;
        for key in parents {
            let entry = table
                .entry(key.clone())
                .or_insert_with(|| Value::Table(Table::new()));
            match entry {
                Value::Table(inner) => table = inner,
                _ => return self.error(&format!("`{key}` is a value and also a table")),
            }
        }
        if table.insert(last.clone(), value).is_some() {
            return self.error(&format!("the key `{last}` is defined twice"));
        }
        Ok(())
    }

    /// Keys joined by dots, up to (not including) `stop`.
    fn key_path(&mut self, stop: char) -> Result<Vec<String>, String> {
        let mut path = Vec::new();
        loop {
            self.skip_blanks();
            let key = match self.peek() {
                Some('"') => self.basic_string()?,
                Some('\'') => self.literal_string()?,
                _ => {
                    let start = self.pos;
                    while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    {
                        self.pos += 1;
                    }
                    if self.pos == start {
                        return self.error("a key is expected");
                    }
                    self.chars[start..self.pos].iter().collect()
                }
            };
            path.push(key);
            self.skip_blanks();
            match self.peek() {
                Some('.') => {
                    self.pos += 1;
                }
                Some(c) if c == stop => return Ok(path),
                _ => return self.error(&format!("expected `.` or `{stop}` after a key")),
            }
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        match self.peek() {
            Some('"') => {
                if self.peek_at(1) == Some('"') && self.peek_at(2) == Some('"') {
                    self.multi_line('"').map(Value::String)
                } else {
                    self.basic_string().map(Value::String)
                }
            }
            Some('\'') => {
                if self.peek_at(1) == Some('\'') && self.peek_at(2) == Some('\'') {
                    self.multi_line('\'').map(Value::String)
                } else {
                    self.literal_string().map(Value::String)
                }
            }
            Some('[') => self.array(),
            Some('{') => self.inline_table(),
            Some(_) => {
                let start = self.pos;
                while matches!(self.peek(), Some(c) if !matches!(c, ',' | ']' | '}' | '#' | '\n' | '\r'))
                {
                    self.pos += 1;
                }
                let text: String = self.chars[start..self.pos].iter().collect();
                let text = text.trim();
                match text {
                    "" => self.error("a value is expected"),
                    "true" => Ok(Value::Bool(true)),
                    "false" => Ok(Value::Bool(false)),
                    _ if is_number_or_date(text) => Ok(Value::Raw(text.to_owned())),
                    _ => self.error(&format!("`{text}` is not a TOML value")),
                }
            }
            None => self.error("a value is expected"),
        }
    }

    fn basic_string(&mut self) -> Result<String, String> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => return self.error("a string is not closed"),
                Some('"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some('\\') => {
                    self.pos += 1;
                    out.push(self.escape()?);
                }
                Some(c) => {
                    self.pos += 1;
                    out.push(c);
                }
            }
        }
    }

    fn escape(&mut self) -> Result<char, String> {
        match self.bump() {
            Some('n') => Ok('\n'),
            Some('t') => Ok('\t'),
            Some('r') => Ok('\r'),
            Some('b') => Ok('\u{8}'),
            Some('f') => Ok('\u{c}'),
            Some('"') => Ok('"'),
            Some('\\') => Ok('\\'),
            Some(marker @ ('u' | 'U')) => {
                let digits = if marker == 'u' { 4 } else { 8 };
                let mut code = 0_u32;
                for _ in 0..digits {
                    let Some(digit) = self.bump().and_then(|c| c.to_digit(16)) else {
                        return self.error("a unicode escape needs hexadecimal digits");
                    };
                    code = code * 16 + digit;
                }
                char::from_u32(code).map_or_else(|| self.error("not a unicode scalar value"), Ok)
            }
            _ => self.error("unknown escape in a string"),
        }
    }

    fn literal_string(&mut self) -> Result<String, String> {
        self.pos += 1;
        let start = self.pos;
        loop {
            match self.peek() {
                None | Some('\n') => return self.error("a string is not closed"),
                Some('\'') => {
                    self.pos += 1;
                    return Ok(self.chars[start..self.pos - 1].iter().collect());
                }
                Some(_) => self.pos += 1,
            }
        }
    }

    /// `"""..."""` or `'''...'''`; a newline right after the opening quotes is dropped.
    fn multi_line(&mut self, quote: char) -> Result<String, String> {
        self.pos += 3;
        if self.peek() == Some('\n') {
            self.bump();
        }
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return self.error("a multi-line string is not closed"),
                Some(c)
                    if c == quote
                        && self.peek_at(1) == Some(quote)
                        && self.peek_at(2) == Some(quote) =>
                {
                    self.pos += 3;
                    return Ok(out);
                }
                Some('\\') if quote == '"' => {
                    self.bump();
                    if self.peek() == Some('\n') {
                        // a line-ending backslash trims the whitespace that follows
                        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
                            self.bump();
                        }
                    } else {
                        out.push(self.escape()?);
                    }
                }
                Some(c) => {
                    self.bump();
                    out.push(c);
                }
            }
        }
    }

    fn array(&mut self) -> Result<Value, String> {
        self.bump();
        let mut items = Vec::new();
        loop {
            self.skip_space();
            match self.peek() {
                Some(']') => {
                    self.bump();
                    return Ok(Value::Array(items));
                }
                None => return self.error("an array is not closed"),
                Some(_) => {}
            }
            items.push(self.value()?);
            self.skip_space();
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(']') => {}
                _ => return self.error("expected `,` or `]` in an array"),
            }
        }
    }

    fn inline_table(&mut self) -> Result<Value, String> {
        self.bump();
        let mut table = Table::new();
        loop {
            self.skip_space();
            match self.peek() {
                Some('}') => {
                    self.bump();
                    return Ok(Value::Table(table));
                }
                None => return self.error("an inline table is not closed"),
                Some(_) => {}
            }
            let path = self.key_path('=')?;
            self.bump();
            self.skip_blanks();
            let value = self.value()?;
            self.insert(&mut table, &path, value)?;
            self.skip_space();
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some('}') => {}
                _ => return self.error("expected `,` or `}` in an inline table"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Document {
        parse(text).unwrap_or_else(|e| panic!("{e}\n{text}"))
    }

    #[test]
    fn reads_a_cargo_manifest() {
        let d = doc(r#"
[package]
name = "wanjiru-imgproc"   # a comment
description = "Image buffers: resample, grey"
version.workspace = true

[lints]
workspace = true

[dependencies]
wanjiru-types = { workspace = true }
wanjiru-math.workspace = true
serde = { version = "1", features = ["derive"], optional = true }

[dev-dependencies.proptest]
version = "1"
"#);
        assert_eq!(
            d.get(&["package", "name"]).and_then(Value::as_str),
            Some("wanjiru-imgproc")
        );
        assert_eq!(
            d.get(&["package", "version", "workspace"])
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            d.get(&["lints", "workspace"]).and_then(Value::as_bool),
            Some(true)
        );
        let deps = d.table(&["dependencies"]).expect("dependencies");
        assert_eq!(
            deps.keys().map(String::as_str).collect::<Vec<_>>(),
            ["serde", "wanjiru-math", "wanjiru-types"]
        );
        assert_eq!(
            d.get(&["dependencies", "wanjiru-math", "workspace"])
                .and_then(Value::as_bool),
            Some(true)
        );
        let serde = d
            .get(&["dependencies", "serde"])
            .and_then(Value::as_table)
            .expect("serde");
        assert_eq!(
            serde
                .get("features")
                .and_then(Value::as_array)
                .map(<[Value]>::len),
            Some(1)
        );
        assert_eq!(
            d.get(&["dev-dependencies", "proptest", "version"])
                .and_then(Value::as_str),
            Some("1")
        );
    }

    #[test]
    fn reads_multi_line_arrays_with_comments_and_trailing_commas() {
        let d = doc("allow = [\n    \"MIT\",   # permissive\n    \"Apache-2.0\",\n]\nn = 3\n");
        let allow = d.get(&["allow"]).and_then(Value::as_array).expect("array");
        let names: Vec<&str> = allow.iter().filter_map(Value::as_str).collect();
        assert_eq!(names, ["MIT", "Apache-2.0"]);
        assert_eq!(d.get(&["n"]), Some(&Value::Raw("3".to_owned())));
    }

    #[test]
    fn reads_strings_of_every_kind() {
        let d = doc(
            "a = 'C:\\path'\nb = \"tab\\there \\u00e9\"\nc = \"\"\"\nline one\nline two\"\"\"\n",
        );
        assert_eq!(d.get(&["a"]).and_then(Value::as_str), Some("C:\\path"));
        assert_eq!(d.get(&["b"]).and_then(Value::as_str), Some("tab\there é"));
        assert_eq!(
            d.get(&["c"]).and_then(Value::as_str),
            Some("line one\nline two")
        );
    }

    #[test]
    fn numbers_and_dates_are_kept_as_written() {
        let d = doc(
            "a = 1_000\nb = -2.5e3\nc = 1979-05-27\nd = 1979-05-27 07:32:00Z\ne = inf\nf = 0x1F\n",
        );
        for key in ["a", "b", "c", "d", "e", "f"] {
            assert!(matches!(d.get(&[key]), Some(Value::Raw(_))), "{key}");
        }
        assert_eq!(
            d.get(&["d"]),
            Some(&Value::Raw("1979-05-27 07:32:00Z".to_owned()))
        );
    }

    #[test]
    fn quoted_and_dotted_keys_nest() {
        let d = doc("[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n");
        assert!(d.table(&["target", "cfg(unix)", "dependencies"]).is_some());
    }

    #[test]
    fn arrays_of_tables_keep_the_last_element() {
        let d = doc("[[bin]]\nname = \"a\"\n[[bin]]\nname = \"b\"\n");
        assert_eq!(d.get(&["bin[]", "name"]).and_then(Value::as_str), Some("b"));
    }

    #[test]
    fn refuses_what_it_does_not_understand_with_a_line_number() {
        for (text, line) in [
            ("a = \n", 1),
            ("ok = 1\nb = \"open\n", 2),
            ("[x\n", 1),
            ("a = 1\na = 2\n", 2),
            ("a = [1, 2\n", 2),
            ("a = 1 junk\n", 1),
            ("a = bare\n", 1),
            ("a = [1, two]\n", 1),
        ] {
            let error = parse(text).expect_err(text);
            assert!(
                error.starts_with(&format!("line {line}:")),
                "{text:?}: {error}"
            );
        }
    }

    #[test]
    fn reads_the_manifests_of_this_repository() {
        for path in [
            "../../Cargo.toml",
            "../../deny.toml",
            "../../clippy.toml",
            "Cargo.toml",
        ] {
            let text = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path),
            )
            .unwrap_or_else(|e| panic!("{path}: {e}"));
            parse(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
        }
    }
}
