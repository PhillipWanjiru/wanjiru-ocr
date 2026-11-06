//! The JSON value type and a strict reader (RFC 8259).
//!
//! Strictness is the point: this reader is the front door for untrusted documents, and a result that
//! is hashed or signed must mean the same thing to every reader. It therefore refuses what lenient
//! readers accept: duplicate keys, numbers that overflow, lone surrogates, a byte-order mark, comments,
//! trailing commas and leading zeros. Nesting deeper than [`MAX_DEPTH`] is refused instead of
//! exhausting the stack.

use std::collections::BTreeSet;

use wanjiru_types::{ErrorKind, Result, WnjError, codes};

/// Deepest nesting of arrays and objects the reader accepts.
pub const MAX_DEPTH: usize = 128;

/// A JSON value. Numbers are IEEE-754 doubles, as in ECMAScript and RFC 8785; objects keep the order
/// of their members (the canonical writer sorts them) and never contain a key twice.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number.
    Number(f64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object, as members in source order.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// A string value.
    #[must_use]
    pub fn string(text: impl Into<String>) -> Self {
        Self::String(text.into())
    }

    /// A number from an `f32`, written with the shortest decimal that reads back to the same `f32`
    /// (so `0.1_f32` becomes `0.1`, not `0.10000000149011612`). The decimal is read as an `f64` and
    /// then narrowed, which rounds twice; the result is checked, and the exact `f64` value of the
    /// `f32` is used if the short decimal did not survive. A scan of every seventh `f32` bit pattern
    /// (6.1 x 10^8 values) found no case that needs the fallback, so it is a guard, not a code path.
    #[must_use]
    pub fn from_f32(x: f32) -> Self {
        let short = format!("{x}").parse::<f64>().ok();
        #[allow(clippy::cast_possible_truncation, reason = "checked by the comparison")]
        let exact = match short {
            Some(v) if (v as f32).to_bits() == x.to_bits() => v,
            _ => f64::from(x),
        };
        Self::Number(exact)
    }

    /// A number from a count. Counts in this workspace stay far below 2^53, where `f64` is exact.
    #[must_use]
    pub fn count(n: u32) -> Self {
        Self::Number(f64::from(n))
    }

    /// An array of numbers.
    #[must_use]
    pub fn numbers(values: &[f64]) -> Self {
        Self::Array(values.iter().map(|v| Self::Number(*v)).collect())
    }

    /// The member `key` of an object; `None` for other values or a missing key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The string, if this is one.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// The number, if this is one.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(x) => Some(*x),
            _ => None,
        }
    }

    /// The boolean, if this is one.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The elements, if this is an array.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The members, if this is an object.
    #[must_use]
    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Self::Object(members) => Some(members),
            _ => None,
        }
    }

    /// The JSON Schema type name of this value (`"integer"` is reported as `"number"`).
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }

    /// Equality as JSON Schema defines it for `enum` and `const`: numbers by value, objects without
    /// regard to member order.
    #[must_use]
    pub fn json_equal(&self, other: &Json) -> bool {
        match (self, other) {
            (Self::Object(a), Self::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| other.get(k).is_some_and(|w| v.json_equal(w)))
                    && b.iter().all(|(k, _)| self.get(k).is_some())
            }
            (Self::Array(a), Self::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.json_equal(y))
            }
            _ => self == other,
        }
    }
}

/// Parse JSON text. See the module documentation for what is refused.
pub fn parse(text: &str) -> Result<Json> {
    let mut parser = Parser {
        text,
        bytes: text.as_bytes(),
        pos: 0,
    };
    parser.skip_whitespace();
    let value = parser.value(0)?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        return Err(parser.error("unexpected data after the JSON value"));
    }
    Ok(value)
}

/// Parse JSON from bytes: they must be UTF-8 and must not start with a byte-order mark.
pub fn parse_bytes(bytes: &[u8]) -> Result<Json> {
    let text = std::str::from_utf8(bytes).map_err(|e| {
        WnjError::new(
            codes::JSON_SYNTAX,
            ErrorKind::InvalidInput,
            format!(
                "input is not UTF-8: invalid byte at offset {}",
                e.valid_up_to()
            ),
        )
    })?;
    parse(text)
}

struct Parser<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, message: &str) -> WnjError {
        self.error_at(self.pos, message)
    }

    fn error_at(&self, pos: usize, message: &str) -> WnjError {
        let before = self.text.get(..pos).unwrap_or("");
        let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        let column = before.get(line_start..).map_or(0, |s| s.chars().count()) + 1;
        WnjError::new(
            codes::JSON_SYNTAX,
            ErrorKind::InvalidInput,
            format!("{message} at byte {pos} (line {line}, column {column})"),
        )
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn describe_next(&self) -> String {
        match self.text.get(self.pos..).and_then(|s| s.chars().next()) {
            Some(c) if c.is_control() => format!("U+{:04X}", u32::from(c)),
            Some(c) => format!("'{c}'"),
            None => "end of input".to_owned(),
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json> {
        match self.peek() {
            Some(b'n') => self.literal("null", Json::Null),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'"') => self.string().map(Json::String),
            Some(b'[') => self.array(depth),
            Some(b'{') => self.object(depth),
            Some(b'-' | b'0'..=b'9') => self.number().map(Json::Number),
            _ => Err(self.error(&format!("expected a value, found {}", self.describe_next()))),
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.error(&format!("expected `{word}`")))
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        self.pos - start
    }

    fn number(&mut self) -> Result<f64> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.error("expected a digit in a number")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if self.digits() == 0 {
                return Err(self.error("expected a digit after the decimal point"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.error("expected a digit in the exponent"));
            }
        }
        let literal = &self.text[start..self.pos];
        match literal.parse::<f64>() {
            Ok(x) if x.is_finite() => Ok(x),
            _ => Err(WnjError::new(
                codes::JSON_NUMBER,
                ErrorKind::InvalidInput,
                format!("number {literal} is outside the range of a double (offset {start})"),
            )),
        }
    }

    fn hex4(&mut self) -> Result<u32> {
        let digits = self
            .bytes
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.error("truncated \\u escape"))?;
        let mut unit = 0_u32;
        for &b in digits {
            let digit = char::from(b)
                .to_digit(16)
                .ok_or_else(|| self.error("invalid hex digit in \\u escape"))?;
            unit = unit * 16 + digit;
        }
        self.pos += 4;
        Ok(unit)
    }

    fn escape(&mut self, out: &mut String) -> Result<()> {
        let Some(b) = self.peek() else {
            return Err(self.error("unterminated escape"));
        };
        self.pos += 1;
        match b {
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            b'/' => out.push('/'),
            b'b' => out.push('\u{8}'),
            b'f' => out.push('\u{c}'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'u' => {
                let start = self.pos - 2;
                let unit = self.hex4()?;
                let scalar = match unit {
                    0xD800..=0xDBFF => {
                        if self.bytes.get(self.pos..self.pos + 2) != Some(b"\\u") {
                            return Err(self.error_at(start, "lone high surrogate"));
                        }
                        self.pos += 2;
                        let low = self.hex4()?;
                        if !(0xDC00..=0xDFFF).contains(&low) {
                            return Err(
                                self.error_at(start, "high surrogate not followed by a low one")
                            );
                        }
                        0x1_0000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
                    }
                    0xDC00..=0xDFFF => return Err(self.error_at(start, "lone low surrogate")),
                    _ => unit,
                };
                out.push(
                    char::from_u32(scalar)
                        .ok_or_else(|| self.error_at(start, "invalid scalar value"))?,
                );
            }
            _ => return Err(self.error_at(self.pos - 1, "invalid escape sequence")),
        }
        Ok(())
    }

    fn string(&mut self) -> Result<String> {
        // The caller has seen the opening quote.
        self.pos += 1;
        let mut out = String::new();
        let mut run_start = self.pos;
        loop {
            let Some(b) = self.peek() else {
                return Err(self.error("unterminated string"));
            };
            match b {
                b'"' => {
                    out.push_str(&self.text[run_start..self.pos]);
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    out.push_str(&self.text[run_start..self.pos]);
                    self.pos += 1;
                    self.escape(&mut out)?;
                    run_start = self.pos;
                }
                0..=0x1F => return Err(self.error("unescaped control character in a string")),
                _ => self.pos += 1,
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json> {
        if depth >= MAX_DEPTH {
            return Err(self.error(&format!("nesting deeper than {MAX_DEPTH}")));
        }
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip_whitespace();
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                _ => {
                    return Err(self.error(&format!(
                        "expected ',' or ']' in an array, found {}",
                        self.describe_next()
                    )));
                }
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json> {
        if depth >= MAX_DEPTH {
            return Err(self.error(&format!("nesting deeper than {MAX_DEPTH}")));
        }
        self.pos += 1;
        let mut members: Vec<(String, Json)> = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Object(members));
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err(self.error(&format!(
                    "expected a string key in an object, found {}",
                    self.describe_next()
                )));
            }
            let key_pos = self.pos;
            let key = self.string()?;
            if !seen.insert(key.clone()) {
                return Err(WnjError::new(
                    codes::JSON_DUPLICATE_KEY,
                    ErrorKind::InvalidInput,
                    format!("duplicate object key {key:?} at byte {key_pos}"),
                ));
            }
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(self.error(&format!(
                    "expected ':' after a key, found {}",
                    self.describe_next()
                )));
            }
            self.pos += 1;
            self.skip_whitespace();
            let value = self.value(depth + 1)?;
            members.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip_whitespace();
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Object(members));
                }
                _ => {
                    return Err(self.error(&format!(
                        "expected ',' or '}}' in an object, found {}",
                        self.describe_next()
                    )));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str) -> Json {
        parse(text).unwrap_or_else(|e| panic!("{text:?} should parse: {e}"))
    }

    fn code_of(text: &str) -> String {
        parse(text)
            .expect_err(&format!("{text:?} should be rejected"))
            .code()
            .to_string()
    }

    #[test]
    fn scalars_and_containers() {
        assert_eq!(ok("null"), Json::Null);
        assert_eq!(ok(" true "), Json::Bool(true));
        assert_eq!(ok("false"), Json::Bool(false));
        assert_eq!(ok("-12.5e2"), Json::Number(-1250.0));
        assert_eq!(ok("\"a\\n\\u00e9\\ud83d\\ude00\""), Json::string("a\né😀"));
        assert_eq!(ok("[]"), Json::Array(vec![]));
        assert_eq!(ok("{}"), Json::Object(vec![]));
        let v = ok(r#"{"b":[1,2,{"c":null}],"a":"x"}"#);
        assert_eq!(v.get("a").and_then(Json::as_str), Some("x"));
        assert_eq!(
            v.get("b").and_then(Json::as_array).map(<[Json]>::len),
            Some(3)
        );
        assert_eq!(v.as_object().map(<[(String, Json)]>::len), Some(2));
        // Member order is kept as written.
        assert_eq!(v.as_object().expect("object")[0].0, "b");
    }

    #[test]
    fn refuses_what_lenient_readers_accept() {
        for text in [
            "",
            " ",
            "nul",
            "tru",
            "01",
            "-",
            "+1",
            ".5",
            "5.",
            "1e",
            "1e+",
            "--1",
            "NaN",
            "Infinity",
            "[1,]",
            "[,1]",
            "{\"a\":1,}",
            "{,}",
            "{'a':1}",
            "{\"a\" 1}",
            "{\"a\":}",
            "[1 2]",
            "\"abc",
            "\"\\x41\"",
            "\"\\u12\"",
            "\"\\u12g4\"",
            "\"\t\"",
            "\"\n\"",
            "// c\n1",
            "/* c */1",
            "1 2",
            "[1] x",
            "\u{feff}1",
            "{\"a\":1}}",
            "[[]",
            "\"\\ud800\"",
            "\"\\udc00\"",
            "\"\\ud800\\u0041\"",
            "\"\\ud800x\"",
        ] {
            assert_eq!(code_of(text), "WNJ-E1001", "{text:?}");
        }
    }

    #[test]
    fn duplicate_keys_overflowing_numbers_and_depth() {
        assert_eq!(code_of(r#"{"a":1,"a":2}"#), "WNJ-E1002");
        // The same key spelled through an escape is still the same key.
        assert_eq!(code_of(r#"{"a":1,"\u0061":2}"#), "WNJ-E1002");
        // Duplicates are checked per object, not globally.
        assert!(parse(r#"{"a":{"a":1},"b":{"a":2}}"#).is_ok());
        assert_eq!(code_of("1e999"), "WNJ-E1007");
        assert_eq!(code_of("-1e999"), "WNJ-E1007");
        assert!(
            parse("1e-999").is_ok(),
            "underflow to zero is representable"
        );
        let deep_ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(&deep_ok).is_ok());
        let too_deep = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
        assert_eq!(code_of(&too_deep), "WNJ-E1001");
        // An adversarial input of a million brackets must fail fast, not overflow the stack.
        assert_eq!(code_of(&"[".repeat(1_000_000)), "WNJ-E1001");
    }

    #[test]
    fn errors_name_the_position() {
        let message = parse("{\n  \"a\": tru\n}")
            .expect_err("bad literal")
            .to_string();
        assert!(message.contains("line 2"), "{message}");
        assert!(message.contains("column 8"), "{message}");
        let message = parse("{\"é\": x}").expect_err("bad value").to_string();
        assert!(
            message.contains("column 7"),
            "columns count characters: {message}"
        );
    }

    #[test]
    fn bytes_must_be_utf8() {
        assert!(parse_bytes(b"{\"a\":1}").is_ok());
        assert_eq!(
            parse_bytes(&[b'"', 0xFF, b'"'])
                .expect_err("not UTF-8")
                .code()
                .to_string(),
            "WNJ-E1001"
        );
    }

    #[test]
    fn f32_numbers_are_written_short_and_read_back_exactly() {
        for x in [
            0.1_f32,
            1.0,
            -2.5,
            1234.5678,
            1e-7,
            3.4e38,
            f32::MIN_POSITIVE,
            16_777_217.0,
        ] {
            let Json::Number(v) = Json::from_f32(x) else {
                panic!("number expected");
            };
            #[allow(clippy::cast_possible_truncation, reason = "the point of the test")]
            let back = v as f32;
            assert_eq!(back.to_bits(), x.to_bits(), "{x}");
        }
        assert_eq!(Json::from_f32(0.1), Json::Number(0.1));
    }

    #[test]
    fn json_equality_ignores_member_order() {
        let a = ok(r#"{"x":1,"y":[1,2],"z":{"p":true}}"#);
        let b = ok(r#"{"z":{"p":true},"y":[1,2],"x":1.0}"#);
        assert!(a.json_equal(&b));
        assert_ne!(a, b, "structural equality is order sensitive");
        assert!(!a.json_equal(&ok(r#"{"x":1,"y":[2,1],"z":{"p":true}}"#)));
        assert!(!a.json_equal(&ok(r#"{"x":1,"y":[1,2]}"#)));
    }
}
