//! A validator for the subset of JSON Schema (draft 2020-12) that the DocIR schema uses.
//!
//! Supported keywords: `$ref` (local pointers only), `type`, `enum`, `const`, `required`,
//! `properties`, `additionalProperties`, `items`, `minItems`, `maxItems`, `minLength`, `maxLength`,
//! `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `allOf`, `anyOf`, `oneOf`, and the
//! annotations `$schema`, `$id`, `$defs`, `$comment`, `title`, `description`. There is no regular
//! expression engine, so `pattern` is not supported. A schema that uses any other keyword is
//! rejected by [`check_schema`] instead of being silently weakened.
//!
//! The validator is cross-checked against the `jsonschema` package of Python on the fixtures in
//! `tests/golden/` (`tools/oracles/make_docir_vectors.py`).

use std::borrow::Cow;
use std::fmt::Write as _;

use wanjiru_types::{ErrorKind, Result, WnjError, codes};

use crate::canonical::format_number;
use crate::json::{self, Json};

/// The DocIR v0 JSON Schema, embedded at build time from `schema/docir-v0.schema.json`.
pub const DOCIR_SCHEMA: &str = include_str!("../schema/docir-v0.schema.json");

/// Every keyword the validator understands.
pub const SUPPORTED_KEYWORDS: &[&str] = &[
    "$schema",
    "$id",
    "$defs",
    "$ref",
    "$comment",
    "title",
    "description",
    "type",
    "enum",
    "const",
    "required",
    "properties",
    "additionalProperties",
    "items",
    "minItems",
    "maxItems",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "allOf",
    "anyOf",
    "oneOf",
];

/// How many violations are collected before the validator stops reporting more.
pub const MAX_VIOLATIONS: usize = 100;

const MAX_REF_DEPTH: usize = 32;

/// One place where an instance does not satisfy the schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    /// JSON Pointer (RFC 6901) of the offending value; the empty string is the document itself.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

fn pointer_escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Run `body` with `/segment` appended to the pointer. Nothing is allocated unless the key needs escaping.
fn with_segment<R>(path: &mut String, segment: &str, body: impl FnOnce(&mut String) -> R) -> R {
    let length = path.len();
    path.push('/');
    if segment.contains(['~', '/']) {
        path.push_str(&pointer_escape(segment));
    } else {
        path.push_str(segment);
    }
    let result = body(path);
    path.truncate(length);
    result
}

/// Like [`with_segment`], for an array index.
fn with_index<R>(path: &mut String, index: usize, body: impl FnOnce(&mut String) -> R) -> R {
    let length = path.len();
    let _ = write!(path, "/{index}");
    let result = body(path);
    path.truncate(length);
    result
}

fn number_text(x: f64) -> String {
    format_number(x).unwrap_or_else(|_| x.to_string())
}

fn lookup<'a>(members: &'a [(String, Json)], keyword: &str) -> Option<&'a Json> {
    members.iter().find(|(k, _)| k == keyword).map(|(_, v)| v)
}

fn type_matches(name: &str, instance: &Json) -> bool {
    match (name, instance) {
        ("null", Json::Null)
        | ("boolean", Json::Bool(_))
        | ("number", Json::Number(_))
        | ("string", Json::String(_))
        | ("array", Json::Array(_))
        | ("object", Json::Object(_)) => true,
        ("integer", Json::Number(x)) => x.is_finite() && x.fract() == 0.0,
        _ => false,
    }
}

fn describe(instance: &Json) -> &'static str {
    match instance {
        Json::Number(x) if x.is_finite() && x.fract() == 0.0 => "integer",
        other => other.type_name(),
    }
}

/// Resolve a local reference such as `#/$defs/point` against `root`.
fn resolve<'a>(root: &'a Json, reference: &str) -> Option<&'a Json> {
    let rest = reference.strip_prefix('#')?;
    if rest.is_empty() {
        return Some(root);
    }
    let mut node = root;
    for token in rest.strip_prefix('/')?.split('/') {
        let key = if token.contains('~') {
            Cow::Owned(token.replace("~1", "/").replace("~0", "~"))
        } else {
            Cow::Borrowed(token)
        };
        node = match node {
            Json::Object(_) => node.get(&key)?,
            Json::Array(items) => items.get(key.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(node)
}

struct Validator<'a> {
    root: &'a Json,
    found: Vec<Violation>,
}

impl Validator<'_> {
    fn report(&mut self, path: &str, message: String) {
        if self.found.len() < MAX_VIOLATIONS {
            self.found.push(Violation {
                path: path.to_owned(),
                message,
            });
        }
    }

    /// Run a sub-schema on the side: its violations are returned, not reported.
    fn trial(
        &mut self,
        schema: &Json,
        instance: &Json,
        path: &mut String,
        depth: usize,
    ) -> Vec<Violation> {
        let saved = std::mem::take(&mut self.found);
        self.check(schema, instance, path, depth);
        std::mem::replace(&mut self.found, saved)
    }

    fn check(&mut self, schema: &Json, instance: &Json, path: &mut String, depth: usize) {
        let members = match schema {
            Json::Bool(true) => return,
            Json::Bool(false) => {
                self.report(path, "no value is allowed here".to_owned());
                return;
            }
            Json::Object(members) => members,
            _ => {
                self.report(
                    path,
                    "the schema is neither an object nor a boolean".to_owned(),
                );
                return;
            }
        };
        for (keyword, argument) in members {
            match keyword.as_str() {
                "$ref" => self.reference(argument, instance, path, depth),
                "type" => self.type_keyword(argument, instance, path),
                "enum" => {
                    let allowed = argument.as_array().unwrap_or(&[]);
                    if !allowed.iter().any(|v| v.json_equal(instance)) {
                        self.report(
                            path,
                            "the value is not one of the allowed values".to_owned(),
                        );
                    }
                }
                "const" => {
                    if !argument.json_equal(instance) {
                        self.report(path, "the value is not the required constant".to_owned());
                    }
                }
                "allOf" => {
                    for alternative in argument.as_array().unwrap_or(&[]) {
                        self.check(alternative, instance, path, depth);
                    }
                }
                "anyOf" => self.any_or_one_of(argument, instance, path, depth, false),
                "oneOf" => self.any_or_one_of(argument, instance, path, depth, true),
                _ => {}
            }
        }
        match instance {
            Json::Number(x) => self.number_keywords(members, *x, path),
            Json::String(s) => self.string_keywords(members, s, path),
            Json::Array(items) => self.array_keywords(members, items, path, depth),
            Json::Object(entries) => self.object_keywords(members, entries, path, depth),
            _ => {}
        }
    }

    fn reference(&mut self, argument: &Json, instance: &Json, path: &mut String, depth: usize) {
        let Some(pointer) = argument.as_str() else {
            self.report(path, "$ref must be a string".to_owned());
            return;
        };
        if depth >= MAX_REF_DEPTH {
            self.report(
                path,
                format!("references nested deeper than {MAX_REF_DEPTH} (a cycle?)"),
            );
            return;
        }
        let root = self.root;
        match resolve(root, pointer) {
            Some(target) => self.check(target, instance, path, depth + 1),
            None => self.report(path, format!("unresolved reference {pointer}")),
        }
    }

    fn type_keyword(&mut self, argument: &Json, instance: &Json, path: &str) {
        let matches = match argument {
            Json::String(name) => type_matches(name, instance),
            Json::Array(items) => items
                .iter()
                .filter_map(Json::as_str)
                .any(|name| type_matches(name, instance)),
            _ => false,
        };
        if !matches {
            let names: Vec<&str> = match argument {
                Json::String(name) => vec![name.as_str()],
                Json::Array(items) => items.iter().filter_map(Json::as_str).collect(),
                _ => Vec::new(),
            };
            self.report(
                path,
                format!(
                    "expected {}, found {}",
                    names.join(" or "),
                    describe(instance)
                ),
            );
        }
    }

    fn any_or_one_of(
        &mut self,
        argument: &Json,
        instance: &Json,
        path: &mut String,
        depth: usize,
        exactly_one: bool,
    ) {
        let alternatives = argument.as_array().unwrap_or(&[]);
        let mut matching = 0_usize;
        let mut closest: Option<Vec<Violation>> = None;
        for alternative in alternatives {
            let violations = self.trial(alternative, instance, path, depth);
            if violations.is_empty() {
                matching += 1;
            } else if closest
                .as_ref()
                .is_none_or(|best| violations.len() < best.len())
            {
                closest = Some(violations);
            }
        }
        let keyword = if exactly_one { "oneOf" } else { "anyOf" };
        if matching == 0 {
            let hint = closest
                .as_ref()
                .and_then(|v| v.first())
                .map_or(String::new(), |v| {
                    format!("; closest alternative: {}: {}", v.path, v.message)
                });
            self.report(
                path,
                format!(
                    "matches none of the {} alternatives of {keyword}{hint}",
                    alternatives.len()
                ),
            );
        } else if exactly_one && matching > 1 {
            self.report(
                path,
                format!("matches {matching} alternatives of oneOf; exactly one is required"),
            );
        }
    }

    fn number_keywords(&mut self, members: &[(String, Json)], x: f64, path: &str) {
        let bound = |keyword: &str| lookup(members, keyword).and_then(Json::as_f64);
        if let Some(limit) = bound("minimum").filter(|limit| x < *limit) {
            self.report(
                path,
                format!(
                    "{} is less than the minimum {}",
                    number_text(x),
                    number_text(limit)
                ),
            );
        }
        if let Some(limit) = bound("maximum").filter(|limit| x > *limit) {
            self.report(
                path,
                format!(
                    "{} is greater than the maximum {}",
                    number_text(x),
                    number_text(limit)
                ),
            );
        }
        if let Some(limit) = bound("exclusiveMinimum").filter(|limit| x <= *limit) {
            self.report(
                path,
                format!(
                    "{} is not greater than {}",
                    number_text(x),
                    number_text(limit)
                ),
            );
        }
        if let Some(limit) = bound("exclusiveMaximum").filter(|limit| x >= *limit) {
            self.report(
                path,
                format!("{} is not less than {}", number_text(x), number_text(limit)),
            );
        }
    }

    fn string_keywords(&mut self, members: &[(String, Json)], text: &str, path: &str) {
        let length = text.chars().count() as f64;
        if let Some(limit) = lookup(members, "minLength")
            .and_then(Json::as_f64)
            .filter(|l| length < *l)
        {
            self.report(
                path,
                format!(
                    "the string is shorter than {} characters",
                    number_text(limit)
                ),
            );
        }
        if let Some(limit) = lookup(members, "maxLength")
            .and_then(Json::as_f64)
            .filter(|l| length > *l)
        {
            self.report(
                path,
                format!(
                    "the string is longer than {} characters",
                    number_text(limit)
                ),
            );
        }
    }

    fn array_keywords(
        &mut self,
        members: &[(String, Json)],
        items: &[Json],
        path: &mut String,
        depth: usize,
    ) {
        let length = items.len() as f64;
        if let Some(limit) = lookup(members, "minItems")
            .and_then(Json::as_f64)
            .filter(|l| length < *l)
        {
            self.report(
                path,
                format!("the array has fewer than {} items", number_text(limit)),
            );
        }
        if let Some(limit) = lookup(members, "maxItems")
            .and_then(Json::as_f64)
            .filter(|l| length > *l)
        {
            self.report(
                path,
                format!("the array has more than {} items", number_text(limit)),
            );
        }
        if let Some(schema) = lookup(members, "items") {
            for (index, item) in items.iter().enumerate() {
                with_index(path, index, |p| {
                    self.check(schema, item, p, depth);
                });
            }
        }
    }

    fn object_keywords(
        &mut self,
        members: &[(String, Json)],
        entries: &[(String, Json)],
        path: &mut String,
        depth: usize,
    ) {
        if let Some(required) = lookup(members, "required").and_then(Json::as_array) {
            for name in required.iter().filter_map(Json::as_str) {
                if !entries.iter().any(|(k, _)| k == name) {
                    self.report(path, format!("missing required property {name:?}"));
                }
            }
        }
        let properties = lookup(members, "properties")
            .and_then(Json::as_object)
            .unwrap_or(&[]);
        let additional = lookup(members, "additionalProperties");
        for (key, value) in entries {
            let declared = properties.iter().find(|(k, _)| k == key);
            if let Some((_, schema)) = declared {
                with_segment(path, key, |p| self.check(schema, value, p, depth));
            } else if let Some(schema) = additional {
                match schema {
                    Json::Bool(true) => {}
                    Json::Bool(false) => self.report(path, format!("unexpected property {key:?}")),
                    other => with_segment(path, key, |p| self.check(other, value, p, depth)),
                }
            }
        }
    }
}

/// Validate `instance` against `schema` and collect up to [`MAX_VIOLATIONS`] violations. The schema is
/// assumed to have passed [`check_schema`].
#[must_use]
pub fn validate_against(schema: &Json, instance: &Json) -> Vec<Violation> {
    let mut validator = Validator {
        root: schema,
        found: Vec::new(),
    };
    validator.check(schema, instance, &mut String::new(), 0);
    validator.found
}

fn schema_error(path: &str, message: impl Into<String>) -> WnjError {
    WnjError::new(
        codes::SCHEMA_VIOLATION,
        ErrorKind::Internal,
        format!("invalid schema: {}", message.into()),
    )
    .at(if path.is_empty() {
        "/".to_owned()
    } else {
        path.to_owned()
    })
}

fn is_non_negative_integer(value: &Json) -> bool {
    value.as_f64().is_some_and(|x| x >= 0.0 && x.fract() == 0.0)
}

/// Check that a schema uses only supported keywords, with well-formed arguments and local references
/// that resolve. A schema that fails this check could be validated "successfully" while ignoring
/// part of its own text, so [`docir_schema`] runs it on every load.
pub fn check_schema(schema: &Json) -> Result<()> {
    check_node(schema, schema, "")
}

fn check_node(root: &Json, node: &Json, path: &str) -> Result<()> {
    let members = match node {
        Json::Bool(_) => return Ok(()),
        Json::Object(members) => members,
        _ => return Err(schema_error(path, "a schema is an object or a boolean")),
    };
    for (keyword, argument) in members {
        let here = format!("{path}/{}", pointer_escape(keyword));
        if !SUPPORTED_KEYWORDS.contains(&keyword.as_str()) {
            return Err(schema_error(
                &here,
                format!("keyword {keyword:?} is not supported"),
            ));
        }
        match keyword.as_str() {
            "type" => {
                let names: Vec<&Json> = match argument {
                    Json::Array(items) => items.iter().collect(),
                    other => vec![other],
                };
                let valid = [
                    "null", "boolean", "integer", "number", "string", "array", "object",
                ];
                if names.is_empty()
                    || !names
                        .iter()
                        .all(|n| n.as_str().is_some_and(|s| valid.contains(&s)))
                {
                    return Err(schema_error(
                        &here,
                        "type names a JSON Schema type or an array of them",
                    ));
                }
            }
            "required" => {
                if !argument
                    .as_array()
                    .is_some_and(|a| a.iter().all(|v| v.as_str().is_some()))
                {
                    return Err(schema_error(&here, "required is an array of strings"));
                }
            }
            "enum" => {
                if argument.as_array().is_none_or(<[Json]>::is_empty) {
                    return Err(schema_error(&here, "enum is a non-empty array"));
                }
            }
            "minLength" | "maxLength" | "minItems" | "maxItems" => {
                if !is_non_negative_integer(argument) {
                    return Err(schema_error(
                        &here,
                        format!("{keyword} is a non-negative integer"),
                    ));
                }
            }
            "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" => {
                if argument.as_f64().is_none() {
                    return Err(schema_error(&here, format!("{keyword} is a number")));
                }
            }
            "properties" | "$defs" => {
                let Some(entries) = argument.as_object() else {
                    return Err(schema_error(
                        &here,
                        format!("{keyword} is an object of schemas"),
                    ));
                };
                for (name, sub) in entries {
                    check_node(root, sub, &format!("{here}/{}", pointer_escape(name)))?;
                }
            }
            "items" | "additionalProperties" => check_node(root, argument, &here)?,
            "allOf" | "anyOf" | "oneOf" => {
                let Some(alternatives) = argument.as_array().filter(|a| !a.is_empty()) else {
                    return Err(schema_error(
                        &here,
                        format!("{keyword} is a non-empty array of schemas"),
                    ));
                };
                for (i, sub) in alternatives.iter().enumerate() {
                    check_node(root, sub, &format!("{here}/{i}"))?;
                }
            }
            "$ref" => {
                let resolvable = argument.as_str().and_then(|r| resolve(root, r)).is_some();
                if !resolvable {
                    return Err(schema_error(
                        &here,
                        "$ref is a local pointer that resolves, such as #/$defs/name",
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// The embedded DocIR v0 schema, parsed and checked.
pub fn docir_schema() -> Result<Json> {
    let schema = json::parse(DOCIR_SCHEMA).map_err(|e| {
        WnjError::new(
            codes::SCHEMA_VIOLATION,
            ErrorKind::Internal,
            format!("the embedded DocIR schema does not parse: {}", e.message()),
        )
    })?;
    check_schema(&schema)?;
    Ok(schema)
}

/// Validate `instance` against the DocIR v0 schema and return every violation found (empty when the
/// instance is valid). `Err` only if the embedded schema itself is broken.
pub fn check(instance: &Json) -> Result<Vec<Violation>> {
    Ok(validate_against(&docir_schema()?, instance))
}

/// Validate `instance` against the DocIR v0 schema; the error names the first violation.
pub fn validate(instance: &Json) -> Result<()> {
    let violations = check(instance)?;
    match violations.first() {
        None => Ok(()),
        Some(first) => {
            let count = if violations.len() >= MAX_VIOLATIONS {
                format!("{MAX_VIOLATIONS}+")
            } else {
                violations.len().to_string()
            };
            Err(WnjError::new(
                codes::SCHEMA_VIOLATION,
                ErrorKind::InvalidInput,
                format!("{count} schema violation(s); first: {}", first.message),
            )
            .at(if first.path.is_empty() {
                "/".to_owned()
            } else {
                first.path.clone()
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;

    fn verdict(schema: &str, instance: &str) -> Vec<Violation> {
        let schema = parse(schema).expect("schema parses");
        check_schema(&schema).expect("schema is supported");
        validate_against(&schema, &parse(instance).expect("instance parses"))
    }

    fn valid(schema: &str, instance: &str) -> bool {
        verdict(schema, instance).is_empty()
    }

    #[test]
    fn types_including_integer_and_unions() {
        assert!(valid(r#"{"type":"string"}"#, r#""x""#));
        assert!(!valid(r#"{"type":"string"}"#, "1"));
        assert!(valid(r#"{"type":"integer"}"#, "3"));
        assert!(valid(r#"{"type":"integer"}"#, "3.0"));
        assert!(!valid(r#"{"type":"integer"}"#, "3.5"));
        assert!(valid(r#"{"type":"number"}"#, "3.5"));
        assert!(valid(r#"{"type":["string","null"]}"#, "null"));
        assert!(!valid(r#"{"type":["string","null"]}"#, "true"));
        assert!(valid(r#"{"type":"object"}"#, "{}"));
        assert!(!valid(r#"{"type":"array"}"#, "{}"));
        assert!(valid(r#"{"type":"boolean"}"#, "false"));
    }

    #[test]
    fn enum_const_and_numeric_bounds() {
        assert!(valid(r#"{"enum":["a",1,{"x":[1,2]}]}"#, r#"{"x":[1,2]}"#));
        assert!(valid(r#"{"enum":["a",1]}"#, "1.0"));
        assert!(!valid(r#"{"enum":["a",1]}"#, r#""b""#));
        assert!(valid(r#"{"const":"v0"}"#, r#""v0""#));
        assert!(!valid(r#"{"const":"v0"}"#, r#""v1""#));
        let range = r#"{"type":"number","minimum":0,"maximum":1}"#;
        assert!(valid(range, "0") && valid(range, "1") && valid(range, "0.5"));
        assert!(!valid(range, "-0.1") && !valid(range, "1.0000001"));
        let open = r#"{"exclusiveMinimum":0,"exclusiveMaximum":10}"#;
        assert!(valid(open, "5") && !valid(open, "0") && !valid(open, "10"));
        // Bounds say nothing about values of other types (only `type` does).
        assert!(valid(r#"{"minimum":0,"maximum":1}"#, r#""text""#));
        assert!(!valid(range, r#""text""#));
    }

    #[test]
    fn string_and_array_sizes_count_characters_and_items() {
        let s = r#"{"minLength":2,"maxLength":3}"#;
        assert!(valid(s, r#""é😀""#), "two characters, five bytes");
        assert!(!valid(s, r#""a""#) && !valid(s, r#""abcd""#));
        let a = r#"{"minItems":1,"maxItems":2,"items":{"type":"integer"}}"#;
        assert!(valid(a, "[1]") && valid(a, "[1,2]"));
        assert!(!valid(a, "[]") && !valid(a, "[1,2,3]") && !valid(a, r#"[1,"x"]"#));
    }

    #[test]
    fn objects_required_properties_and_additional_properties() {
        let schema = r#"{"type":"object","required":["a"],"properties":{"a":{"type":"integer"},"b":{"type":"string"}},"additionalProperties":false}"#;
        assert!(valid(schema, r#"{"a":1}"#) && valid(schema, r#"{"a":1,"b":"x"}"#));
        assert!(!valid(schema, r#"{"b":"x"}"#));
        assert!(!valid(schema, r#"{"a":"x"}"#));
        assert!(!valid(schema, r#"{"a":1,"c":0}"#));
        let typed = r#"{"additionalProperties":{"type":"number","minimum":0}}"#;
        assert!(valid(typed, r#"{"x":1,"y":2}"#) && !valid(typed, r#"{"x":-1}"#));
        let violations = verdict(schema, r#"{"a":"x","c":0}"#);
        assert_eq!(violations.len(), 2);
        assert!(violations.iter().any(|v| v.path == "/a"));
        assert!(
            violations
                .iter()
                .any(|v| v.message.contains("unexpected property"))
        );
    }

    #[test]
    fn references_composition_and_paths() {
        let schema = r##"{"$defs":{"p":{"type":"array","minItems":2,"maxItems":2,"items":{"type":"number"}}},
                          "type":"array","items":{"$ref":"#/$defs/p"}}"##;
        assert!(valid(schema, "[[1,2],[3,4]]"));
        let violations = verdict(schema, r#"[[1,2],[3,"x"]]"#);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].path, "/1/1");
        let one = r#"{"oneOf":[{"type":"string"},{"type":"integer"}]}"#;
        assert!(valid(one, r#""x""#) && valid(one, "1") && !valid(one, "1.5"));
        let overlapping = r#"{"oneOf":[{"type":"number"},{"type":"integer"}]}"#;
        assert!(
            valid(overlapping, "1.5") && !valid(overlapping, "1"),
            "1 matches both"
        );
        let any = r#"{"anyOf":[{"type":"number"},{"type":"integer"}]}"#;
        assert!(valid(any, "1"));
        let all = r#"{"allOf":[{"type":"number"},{"minimum":3}]}"#;
        assert!(valid(all, "3") && !valid(all, "2"));
        // Object member names that need escaping appear escaped in the pointer.
        let tricky = verdict(
            r#"{"properties":{"a/b~c":{"type":"string"}}}"#,
            r#"{"a/b~c":1}"#,
        );
        assert_eq!(tricky[0].path, "/a~1b~0c");
    }

    #[test]
    fn unsupported_or_malformed_schemas_are_rejected_up_front() {
        for bad in [
            r#"{"pattern":"^a$"}"#,
            r#"{"type":"text"}"#,
            r#"{"required":"a"}"#,
            r#"{"minLength":-1}"#,
            r#"{"minimum":"0"}"#,
            r##"{"$ref":"#/$defs/missing"}"##,
            r#"{"properties":{"a":{"patternProperties":{}}}}"#,
            r#"{"oneOf":[]}"#,
            "3",
        ] {
            let schema = parse(bad).expect("parses as JSON");
            assert!(check_schema(&schema).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_embedded_schema_is_supported_and_resolves() {
        let schema = docir_schema().expect("embedded schema");
        assert_eq!(
            schema.get("$id").and_then(Json::as_str),
            Some("urn:wanjiru:docir:0")
        );
        let defs = schema
            .get("$defs")
            .and_then(Json::as_object)
            .expect("$defs");
        assert!(defs.len() >= 15);
    }

    #[test]
    fn validate_reports_the_first_violation_with_its_path() {
        let err = validate(&parse(r#"{"schema":"wanjiru.docir/0"}"#).expect("parses"))
            .expect_err("incomplete");
        assert_eq!(err.code().to_string(), "WNJ-E1003");
        assert!(err.message().contains("violation"), "{err}");
        assert_eq!(err.path(), Some("/"));
    }
}
