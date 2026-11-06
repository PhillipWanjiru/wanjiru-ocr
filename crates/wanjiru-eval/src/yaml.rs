//! A reader for the small subset of YAML that quality contracts use.
//!
//! Supported: block mappings and block sequences nested by indentation (spaces only), flow mappings
//! and flow sequences (`{min: 0.995, coverage: 0.95}`, `[1, 2]`) whose members are scalars or further
//! flow collections, plain and quoted scalars, and comments. Plain scalars are typed as in the YAML 1.2
//! core schema, except that only `true`, `false` and `null` (or `~`) are keywords.
//!
//! Refused with an error rather than guessed at: tabs for indentation, anchors, aliases, tags,
//! directives, block scalars (`|`, `>`), multi-line scalars, explicit keys (`?`), several documents,
//! duplicate keys, and nesting deeper than [`MAX_DEPTH`] (the limit of the JSON reader: a crafted file
//! must be an error, not a stack overflow). The result is a [`Json`] value, so the same strict checks and
//! the same canonical writer apply to contracts as to DocIR.
//!
//! `tools/oracles/` cross-checks the reader against PyYAML on the contracts of this repository.

use wanjiru_docir::{Json, MAX_DEPTH};
use wanjiru_types::{ErrorKind, Result, WnjError, codes};

fn error(line: usize, message: impl std::fmt::Display) -> WnjError {
    WnjError::new(
        codes::CONTRACT_PARSE,
        ErrorKind::InvalidInput,
        format!("line {line}: {message}"),
    )
}

fn too_deep(line: usize) -> WnjError {
    error(line, format!("nesting deeper than {MAX_DEPTH}"))
}

/// One significant line: its indentation, its text without indentation and comment, its number.
struct Line<'a> {
    indent: usize,
    text: &'a str,
    number: usize,
}

/// Remove a trailing comment: `#` at the start of the text or after whitespace, outside quotes. A
/// quote opens a quoted scalar only where a scalar can start: after whitespace, `[`, `{` or `,`.
fn strip_comment(text: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut previous: Option<char> = None;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if let Some(open) = quote {
            if c == '\\' && open == '"' {
                chars.next();
            } else if c == open {
                if open == '\'' && matches!(chars.peek(), Some((_, '\''))) {
                    chars.next();
                } else {
                    quote = None;
                }
            }
        } else {
            if c == '#' && previous.is_none_or(char::is_whitespace) {
                return &text[..i];
            }
            let starts_scalar =
                previous.is_none_or(|p| p.is_whitespace() || matches!(p, '[' | '{' | ','));
            if (c == '"' || c == '\'') && starts_scalar {
                quote = Some(c);
            }
        }
        previous = Some(c);
    }
    text
}

fn significant_lines(text: &str) -> Result<Vec<Line<'_>>> {
    let mut lines = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let body = strip_comment(raw);
        let trimmed = body.trim_end();
        if trimmed.trim_start().is_empty() {
            continue;
        }
        let indent = trimmed.len() - trimmed.trim_start_matches(' ').len();
        if trimmed[indent..].starts_with('\t') || trimmed[..indent].contains('\t') {
            return Err(error(number, "tabs are not allowed for indentation"));
        }
        let content = &trimmed[indent..];
        if content == "---" && lines.is_empty() {
            continue;
        }
        if content == "---" || content == "..." || content.starts_with("--- ") {
            return Err(error(number, "only one YAML document is supported"));
        }
        if content.starts_with('%') {
            return Err(error(number, "directives are not supported"));
        }
        lines.push(Line {
            indent,
            text: content,
            number,
        });
    }
    Ok(lines)
}

/// Parse YAML text into a [`Json`] value.
pub fn parse(text: &str) -> Result<Json> {
    let lines = significant_lines(text)?;
    if lines.is_empty() {
        return Ok(Json::Null);
    }
    let mut position = 0;
    let value = block(&lines, &mut position, lines[0].indent, 0)?;
    if position != lines.len() {
        return Err(error(lines[position].number, "unexpected indentation"));
    }
    Ok(value)
}

/// `depth` is the number of collections around this one, as in the JSON reader.
fn block(lines: &[Line<'_>], position: &mut usize, indent: usize, depth: usize) -> Result<Json> {
    let first = &lines[*position];
    if first.text == "-" || first.text.starts_with("- ") {
        sequence(lines, position, indent, depth)
    } else if find_key_separator(first.text).is_some() {
        mapping(lines, position, indent, depth)
    } else {
        // A bare scalar or flow collection on its own line, as the whole document.
        *position += 1;
        scalar_or_flow(first.text, first.number, depth)
    }
}

fn sequence(lines: &[Line<'_>], position: &mut usize, indent: usize, depth: usize) -> Result<Json> {
    if depth >= MAX_DEPTH {
        return Err(too_deep(lines[*position].number));
    }
    let mut items = Vec::new();
    while let Some(line) = lines.get(*position) {
        if line.indent != indent || !(line.text == "-" || line.text.starts_with("- ")) {
            break;
        }
        let rest = line.text[1..].trim_start();
        *position += 1;
        if rest.is_empty() {
            let Some(next) = lines.get(*position).filter(|n| n.indent > indent) else {
                return Err(error(line.number, "a sequence entry needs a value"));
            };
            let child_indent = next.indent;
            items.push(block(lines, position, child_indent, depth + 1)?);
        } else if find_key_separator(rest).is_some() && !rest.starts_with(['{', '[', '"', '\'']) {
            return Err(error(
                line.number,
                "mappings inside sequences are not supported",
            ));
        } else {
            items.push(scalar_or_flow(rest, line.number, depth + 1)?);
        }
    }
    Ok(Json::Array(items))
}

/// The byte offset of the `:` that separates a key from its value, if the text is `key: ...` or `key:`.
fn find_key_separator(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    if matches!(bytes.first(), Some(b'"' | b'\'')) {
        let quote = bytes[0];
        i = 1;
        while i < bytes.len() && bytes[i] != quote {
            if quote == b'"' && bytes[i] == b'\\' {
                i += 1;
            }
            i += 1;
        }
        i += 1;
        return (bytes.get(i) == Some(&b':') && matches!(bytes.get(i + 1), None | Some(b' ')))
            .then_some(i);
    }
    if matches!(bytes.first(), Some(b'{' | b'[')) {
        return None;
    }
    while i < bytes.len() {
        if bytes[i] == b':' && matches!(bytes.get(i + 1), None | Some(b' ')) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn mapping(lines: &[Line<'_>], position: &mut usize, indent: usize, depth: usize) -> Result<Json> {
    if depth >= MAX_DEPTH {
        return Err(too_deep(lines[*position].number));
    }
    let mut members: Vec<(String, Json)> = Vec::new();
    while let Some(line) = lines.get(*position) {
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(error(line.number, "unexpected indentation"));
        }
        let Some(colon) = find_key_separator(line.text) else {
            return Err(error(line.number, "expected `key: value`"));
        };
        let key = match scalar_or_flow(line.text[..colon].trim_end(), line.number, depth + 1)? {
            Json::String(s) => s,
            Json::Number(_) | Json::Bool(_) | Json::Null => {
                line.text[..colon].trim_end().to_owned()
            }
            _ => return Err(error(line.number, "a key is a scalar")),
        };
        if members.iter().any(|(k, _)| *k == key) {
            return Err(error(line.number, format!("duplicate key {key:?}")));
        }
        let rest = line.text[colon + 1..].trim_start();
        *position += 1;
        let value = if rest.is_empty() {
            match lines.get(*position) {
                Some(next) if next.indent > indent => {
                    let child_indent = next.indent;
                    block(lines, position, child_indent, depth + 1)?
                }
                // PyYAML and many other tools write a block sequence at the indentation of its key.
                Some(next)
                    if next.indent == indent
                        && (next.text == "-" || next.text.starts_with("- ")) =>
                {
                    sequence(lines, position, indent, depth + 1)?
                }
                // `key:` followed by a sibling or the end is an empty value, YAML's null.
                _ => Json::Null,
            }
        } else {
            scalar_or_flow(rest, line.number, depth + 1)?
        };
        members.push((key, value));
    }
    Ok(Json::Object(members))
}

fn scalar_or_flow(text: &str, line: usize, depth: usize) -> Result<Json> {
    let text = text.trim();
    let mut cursor = 0;
    let value = flow_value(text, &mut cursor, line, false, depth)?;
    if text[cursor..].trim().is_empty() {
        Ok(value)
    } else {
        Err(error(
            line,
            format!("unexpected text after the value: {:?}", &text[cursor..]),
        ))
    }
}

fn skip_spaces(text: &str, cursor: &mut usize) {
    while text[*cursor..].starts_with(' ') {
        *cursor += 1;
    }
}

fn flow_value(
    text: &str,
    cursor: &mut usize,
    line: usize,
    inside_flow: bool,
    depth: usize,
) -> Result<Json> {
    skip_spaces(text, cursor);
    let rest = &text[*cursor..];
    match rest.chars().next() {
        None => Ok(Json::Null),
        Some('[') => {
            if depth >= MAX_DEPTH {
                return Err(too_deep(line));
            }
            *cursor += 1;
            let mut items = Vec::new();
            loop {
                skip_spaces(text, cursor);
                if text[*cursor..].starts_with(']') {
                    *cursor += 1;
                    return Ok(Json::Array(items));
                }
                items.push(flow_value(text, cursor, line, true, depth + 1)?);
                skip_spaces(text, cursor);
                match text[*cursor..].chars().next() {
                    Some(',') => *cursor += 1,
                    Some(']') => {}
                    _ => return Err(error(line, "expected `,` or `]` in a flow sequence")),
                }
            }
        }
        Some('{') => {
            if depth >= MAX_DEPTH {
                return Err(too_deep(line));
            }
            *cursor += 1;
            let mut members: Vec<(String, Json)> = Vec::new();
            loop {
                skip_spaces(text, cursor);
                if text[*cursor..].starts_with('}') {
                    *cursor += 1;
                    return Ok(Json::Object(members));
                }
                let Json::String(key) = flow_value(text, cursor, line, true, depth + 1)? else {
                    return Err(error(line, "a flow mapping key is a string"));
                };
                skip_spaces(text, cursor);
                if !text[*cursor..].starts_with(':') {
                    return Err(error(line, "expected `:` after a key in a flow mapping"));
                }
                *cursor += 1;
                let value = flow_value(text, cursor, line, true, depth + 1)?;
                if members.iter().any(|(k, _)| *k == key) {
                    return Err(error(line, format!("duplicate key {key:?}")));
                }
                members.push((key, value));
                skip_spaces(text, cursor);
                match text[*cursor..].chars().next() {
                    Some(',') => *cursor += 1,
                    Some('}') => {}
                    _ => return Err(error(line, "expected `,` or `}` in a flow mapping")),
                }
            }
        }
        Some(quote @ ('"' | '\'')) => quoted(text, cursor, quote, line).map(Json::String),
        Some('&' | '*' | '!' | '|' | '>' | '@' | '`' | '%') => Err(error(
            line,
            "anchors, aliases, tags and block scalars are not supported",
        )),
        Some('?') if rest.starts_with("? ") => Err(error(line, "explicit keys are not supported")),
        Some(_) => {
            let end = if inside_flow {
                plain_end_in_flow(rest)
            } else if rest.contains(": ") || rest.ends_with(':') {
                return Err(error(line, "a mapping value is not allowed here"));
            } else {
                rest.len()
            };
            let word = rest[..end].trim_end();
            *cursor += end;
            Ok(plain_scalar(word))
        }
    }
}

/// Where a plain scalar inside a flow collection ends: at `,`, `]`, `}` or at a `:` that separates a key.
fn plain_end_in_flow(rest: &str) -> usize {
    let bytes = rest.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b',' | b']' | b'}' => return i,
            b':' if matches!(bytes.get(i + 1), None | Some(b' ' | b',' | b']' | b'}')) => return i,
            _ => {}
        }
    }
    rest.len()
}

fn quoted(text: &str, cursor: &mut usize, quote: char, line: usize) -> Result<String> {
    let mut out = String::new();
    let mut chars = text[*cursor + 1..].char_indices();
    while let Some((offset, c)) = chars.next() {
        if c == quote {
            if quote == '\'' && text[*cursor + 1 + offset + 1..].starts_with('\'') {
                out.push('\'');
                chars.next();
                continue;
            }
            *cursor += 1 + offset + 1;
            return Ok(out);
        }
        if quote == '"' && c == '\\' {
            let Some((_, escaped)) = chars.next() else {
                break;
            };
            out.push(match escaped {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '0' => '\0',
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                other => return Err(error(line, format!("unsupported escape \\{other}"))),
            });
        } else {
            out.push(c);
        }
    }
    Err(error(line, "unterminated quoted string"))
}

fn plain_scalar(word: &str) -> Json {
    match word {
        "true" => return Json::Bool(true),
        "false" => return Json::Bool(false),
        "null" | "~" | "" => return Json::Null,
        _ => {}
    }
    if is_yaml_number(word) {
        if let Ok(x) = word.parse::<f64>() {
            if x.is_finite() {
                return Json::Number(x);
            }
        }
    }
    Json::String(word.to_owned())
}

/// What the YAML 1.2 core schema types as a number: `[-+]?(\.[0-9]+|[0-9]+(\.[0-9]*)?)([eE][-+]?[0-9]+)?`.
/// A leading zero is not an octal marker here, and underscores are not digits.
fn is_yaml_number(word: &str) -> bool {
    let bytes = word.as_bytes();
    let mut i = 0;
    let digits = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    let whole = digits(i);
    i += whole;
    let mut fraction = 0;
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        fraction = digits(i);
        i += fraction;
    }
    if whole == 0 && fraction == 0 {
        return false;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let exponent = digits(i);
        if exponent == 0 {
            return false;
        }
        i += exponent;
    }
    i == bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(text: &str) -> Json {
        parse(text).unwrap_or_else(|e| panic!("{text:?}: {e}"))
    }

    fn json(text: &str) -> Json {
        wanjiru_docir::parse(text).expect("json")
    }

    #[test]
    fn nesting_is_bounded_like_the_json_reader() {
        // 128 nested collections are accepted and the 129th is refused, in flow style and in block style,
        // and a crafted file is an error and not a stack overflow.
        let flow = |brackets: usize| format!("a: {}{}", "[".repeat(brackets), "]".repeat(brackets));
        // the top-level mapping is the first of the 128
        assert!(parse(&flow(MAX_DEPTH - 1)).is_ok());
        let refused = parse(&flow(MAX_DEPTH)).expect_err("one level too deep");
        assert!(
            refused.to_string().contains("nesting deeper than 128"),
            "{refused}"
        );
        assert!(parse(&flow(200_000)).is_err());

        let block = |levels: usize| {
            let mut text = String::new();
            for level in 0..levels - 1 {
                text.push_str(&" ".repeat(level));
                text.push_str("k:\n");
            }
            text.push_str(&" ".repeat(levels - 1));
            text.push_str("k: 1\n");
            text
        };
        assert!(parse(&block(MAX_DEPTH)).is_ok());
        assert!(parse(&block(MAX_DEPTH + 1)).is_err());
        assert!(parse(&block(1_000)).is_err());

        let sequences = |levels: usize| {
            let mut text = String::new();
            for level in 0..levels {
                text.push_str(&" ".repeat(2 * level));
                text.push_str("-\n");
            }
            text.push_str(&" ".repeat(2 * levels));
            text.push_str("1\n");
            text
        };
        assert!(parse(&sequences(MAX_DEPTH)).is_ok());
        assert!(parse(&sequences(MAX_DEPTH + 1)).is_err());
    }

    #[test]
    fn the_blueprint_contract_parses() {
        let text = "\
contract: invoice-fields-v1
corpus: golden/invoices-2026-q3        # frozen, versioned, document-level split
metrics:
  field_exact_match:
    amount: {min: 0.995, coverage: 0.95}
    date:   {min: 0.995, coverage: 0.95}
    iban:   {min: 0.999, coverage: 0.90}
  ece_char: {max: 0.02}
  conformal: {alpha: 0.01, coverage_tolerance: 0.01}
  latency_ms: {p50_max: 1000, p95_max: 2500}
gates:
  regression_budget_cer_rel: 0.02       # block merge if CER worsens by more than 2% relative
  confidence_interval: bootstrap-by-document
";
        let expected = json(
            r#"{"contract":"invoice-fields-v1","corpus":"golden/invoices-2026-q3",
            "metrics":{"field_exact_match":{"amount":{"min":0.995,"coverage":0.95},"date":{"min":0.995,"coverage":0.95},
            "iban":{"min":0.999,"coverage":0.9}},"ece_char":{"max":0.02},"conformal":{"alpha":0.01,"coverage_tolerance":0.01},
            "latency_ms":{"p50_max":1000,"p95_max":2500}},
            "gates":{"regression_budget_cer_rel":0.02,"confidence_interval":"bootstrap-by-document"}}"#,
        );
        assert_eq!(ok(text), expected);
    }

    #[test]
    fn scalars_are_typed_like_yaml_1_2() {
        let value = ok(
            "a: 1\nb: -2.5\nc: 1e3\nd: .5\ne: true\nf: false\ng: null\nh: ~\ni:\nj: 0.1.2\nk: 12abc\nl: yes\nm: 007\n",
        );
        let expected = json(
            r#"{"a":1,"b":-2.5,"c":1000,"d":0.5,"e":true,"f":false,"g":null,"h":null,"i":null,"j":"0.1.2","k":"12abc","l":"yes","m":7}"#,
        );
        assert_eq!(value, expected);
    }

    #[test]
    fn quoted_scalars_comments_and_url_like_values() {
        let value = ok(
            "a: \"x # not a comment\"  # a comment\nb: 'it''s'\nc: \"tab\\there\"\nd: http://example.org/a#frag\ne: \"\"\n# full-line comment\n",
        );
        let expected = json(
            r#"{"a":"x # not a comment","b":"it's","c":"tab\there","d":"http://example.org/a#frag","e":""}"#,
        );
        assert_eq!(value, expected);
    }

    #[test]
    fn sequences_and_nesting() {
        let value = ok(
            "list:\n  - 1\n  - two\n  - [3, 4]\n  - {a: 5}\nempty:\nnested:\n  deeper:\n    - x\n  other: [a, b]\n",
        );
        let expected = json(
            r#"{"list":[1,"two",[3,4],{"a":5}],"empty":null,"nested":{"deeper":["x"],"other":["a","b"]}}"#,
        );
        assert_eq!(value, expected);
        assert_eq!(ok("- 1\n- 2\n"), json("[1,2]"));
        assert_eq!(ok("---\na: 1\n"), json(r#"{"a":1}"#));
        assert_eq!(ok(""), Json::Null);
        assert_eq!(ok("# only a comment\n"), Json::Null);
    }

    #[test]
    fn a_sequence_may_sit_at_the_indentation_of_its_key() {
        let value = ok("a:\n- 1\n- two\nb: x\nc:\n  d:\n  - y\n  e: 2\nf:\n- z\n");
        let expected = json(r#"{"a":[1,"two"],"b":"x","c":{"d":["y"],"e":2},"f":["z"]}"#);
        assert_eq!(value, expected);
        // A dash that follows a complete `key: value` pair is not a value of anything.
        assert!(parse("a: 1\n- x\n").is_err());
        assert!(parse("- x\nb: 1\n").is_err());
    }

    #[test]
    fn flow_collections_may_nest() {
        let value = ok("a: {b: [1, 2, {c: d}], e: \"x, y\"}\nz: []\ny: {}\n");
        let expected = json(r#"{"a":{"b":[1,2,{"c":"d"}],"e":"x, y"},"z":[],"y":{}}"#);
        assert_eq!(value, expected);
    }

    #[test]
    fn unsupported_constructs_are_errors() {
        for bad in [
            "a:\n\tb: 1\n",
            "a: &anchor 1\n",
            "a: *alias\n",
            "a: !!str 1\n",
            "a: |\n  text\n",
            "a: >\n  text\n",
            "? a\n: b\n",
            "a: 1\n---\nb: 2\n",
            "%YAML 1.2\na: 1\n",
            "a: 1\na: 2\n",
            "a: {b: 1, b: 2}\n",
            "a: [1, 2\n",
            "a: {b: 1\n",
            "a: \"open\n",
            "a: 1\n  b: 2\n",
            "- a: 1\n",
            "a: 1 2: 3\n",
            "a: [1] x\n",
        ] {
            let error = parse(bad).expect_err(&format!("{bad:?} should be refused"));
            assert_eq!(error.code().to_string(), "WNJ-E4001", "{bad:?}");
        }
    }

    #[test]
    fn errors_name_the_line() {
        let error = parse("a: 1\nb: [1, 2\nc: 3\n").expect_err("unterminated");
        assert!(error.message().starts_with("line 2:"), "{error}");
    }
}
