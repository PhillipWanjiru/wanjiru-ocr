//! A small JSON reader, enough for the output of `cargo metadata`.

/// A JSON value. Object members keep their order.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// `null`
    Null,
    /// `true` or `false`
    Bool(bool),
    /// A number.
    Number(f64),
    /// A string.
    String(String),
    /// `[...]`
    Array(Vec<Json>),
    /// `{...}`
    Object(Vec<(String, Json)>),
}

impl Json {
    /// The member `key` of an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The text of a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// The elements of an array.
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    text: &'a str,
    pos: usize,
}

/// Parse a complete JSON text.
pub fn parse(text: &str) -> Result<Json, String> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        text,
        pos: 0,
    };
    let value = reader.value(0)?;
    reader.skip_space();
    if reader.pos != reader.bytes.len() {
        return Err(reader.error("unexpected text after the value"));
    }
    Ok(value)
}

impl Reader<'_> {
    fn error(&self, message: &str) -> String {
        format!("JSON error at byte {}: {message}", self.pos)
    }

    fn skip_space(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, literal: &str, value: Json) -> Result<Json, String> {
        if self.text[self.pos..].starts_with(literal) {
            self.pos += literal.len();
            Ok(value)
        } else {
            Err(self.error("not a JSON value"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > 128 {
            return Err(self.error("nested too deeply"));
        }
        self.skip_space();
        match self.bytes.get(self.pos) {
            Some(b'n') => self.expect("null", Json::Null),
            Some(b't') => self.expect("true", Json::Bool(true)),
            Some(b'f') => self.expect("false", Json::Bool(false)),
            Some(b'"') => self.string().map(Json::String),
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_space();
                    if self.bytes.get(self.pos) == Some(&b']') {
                        self.pos += 1;
                        return Ok(Json::Array(items));
                    }
                    if !items.is_empty() {
                        if self.bytes.get(self.pos) != Some(&b',') {
                            return Err(self.error("expected `,` or `]`"));
                        }
                        self.pos += 1;
                    }
                    items.push(self.value(depth + 1)?);
                }
            }
            Some(b'{') => {
                self.pos += 1;
                let mut members: Vec<(String, Json)> = Vec::new();
                loop {
                    self.skip_space();
                    if self.bytes.get(self.pos) == Some(&b'}') {
                        self.pos += 1;
                        return Ok(Json::Object(members));
                    }
                    if !members.is_empty() {
                        if self.bytes.get(self.pos) != Some(&b',') {
                            return Err(self.error("expected `,` or `}`"));
                        }
                        self.pos += 1;
                        self.skip_space();
                    }
                    if self.bytes.get(self.pos) != Some(&b'"') {
                        return Err(self.error("expected a member name"));
                    }
                    let key = self.string()?;
                    self.skip_space();
                    if self.bytes.get(self.pos) != Some(&b':') {
                        return Err(self.error("expected `:`"));
                    }
                    self.pos += 1;
                    let value = self.value(depth + 1)?;
                    members.push((key, value));
                }
            }
            Some(b'-' | b'0'..=b'9') => {
                let start = self.pos;
                while matches!(
                    self.bytes.get(self.pos),
                    Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
                ) {
                    self.pos += 1;
                }
                self.text[start..self.pos]
                    .parse::<f64>()
                    .map(Json::Number)
                    .map_err(|_| self.error("not a number"))
            }
            _ => Err(self.error("not a JSON value")),
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .text
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.error("a \\u escape is cut short"))?;
        let code = u32::from_str_radix(digits, 16)
            .map_err(|_| self.error("a \\u escape needs four hexadecimal digits"))?;
        self.pos += 4;
        Ok(code)
    }

    fn string(&mut self) -> Result<String, String> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(&byte) = self.bytes.get(self.pos) else {
                return Err(self.error("a string is not closed"));
            };
            match byte {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let escape = self.bytes.get(self.pos).copied();
                    self.pos += 1;
                    match escape {
                        Some(b'"') => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'/') => out.push('/'),
                        Some(b'b') => out.push('\u{8}'),
                        Some(b'f') => out.push('\u{c}'),
                        Some(b'n') => out.push('\n'),
                        Some(b'r') => out.push('\r'),
                        Some(b't') => out.push('\t'),
                        Some(b'u') => {
                            let first = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&first) {
                                if !self.text[self.pos..].starts_with("\\u") {
                                    return Err(
                                        self.error("a high surrogate needs a low surrogate")
                                    );
                                }
                                self.pos += 2;
                                let second = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&second) {
                                    return Err(
                                        self.error("a high surrogate needs a low surrogate")
                                    );
                                }
                                0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                            } else {
                                first
                            };
                            out.push(
                                char::from_u32(code)
                                    .ok_or_else(|| self.error("not a unicode scalar value"))?,
                            );
                        }
                        _ => return Err(self.error("unknown escape")),
                    }
                }
                0..=0x1F => return Err(self.error("a control character in a string")),
                _ => {
                    let c = self.text[self.pos..]
                        .chars()
                        .next()
                        .ok_or_else(|| self.error("bad UTF-8"))?;
                    out.push(c);
                    self.pos += c.len_utf8();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_kind_of_value() {
        let j = parse(
            r#" {"a": [1, -2.5e3, true, false, null], "b": {"c": "d\n\u00e9\ud83d\ude00"}} "#,
        )
        .expect("json");
        assert_eq!(
            j.get("a").and_then(Json::as_array).map(<[Json]>::len),
            Some(5)
        );
        assert_eq!(
            j.get("b").and_then(|b| b.get("c")).and_then(Json::as_str),
            Some("d\né😀")
        );
        assert_eq!(
            j.get("a").and_then(Json::as_array).and_then(|a| a.get(1)),
            Some(&Json::Number(-2500.0))
        );
    }

    #[test]
    fn refuses_malformed_text() {
        for text in [
            "",
            "{",
            "[1,]",
            "{\"a\" 1}",
            "\"open",
            "nul",
            "[1] x",
            "\"\\ud83d\"",
            "\"\u{1}\"",
            "{\"a\":1,}",
        ] {
            assert!(parse(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn deep_nesting_is_an_error_not_a_crash() {
        let text = "[".repeat(1000) + &"]".repeat(1000);
        assert!(parse(&text).is_err());
    }
}
