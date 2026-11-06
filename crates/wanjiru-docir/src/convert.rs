//! Reading and writing documents as JSON.
//!
//! Reading is strict: a member that is not part of the schema is an error, not something to ignore,
//! because a result that silently drops what it does not understand cannot be trusted to have been
//! understood. Every error carries the JSON Pointer of the offending value.

use std::collections::BTreeMap;

use wanjiru_types::{ErrorKind, Point, Polygon, Result, Size, Source, WnjError, codes};

use crate::canonical::{canonical_sha256, to_canonical_string};
use crate::confidence::Confidence;
use crate::json::{self, Json};
use crate::model::{
    Decision, DocClass, Document, Evidence, Field, FieldKind, Line, Page, Run, SCHEMA_ID,
    SCHEMA_PREFIX, SourceInfo, Word,
};
use crate::schema;
use crate::transform::{Chain, Transform};

fn error(code: wanjiru_types::ErrorCode, path: &str, message: impl Into<String>) -> WnjError {
    WnjError::new(code, ErrorKind::InvalidInput, message).at(if path.is_empty() {
        "/"
    } else {
        path
    })
}

fn violation(path: &str, message: impl Into<String>) -> WnjError {
    error(codes::SCHEMA_VIOLATION, path, message)
}

fn child(path: &str, segment: impl std::fmt::Display) -> String {
    let segment = segment.to_string().replace('~', "~0").replace('/', "~1");
    format!("{path}/{segment}")
}

/// A JSON object being read: members are taken by name, and [`Obj::finish`] rejects any left over.
struct Obj<'a> {
    members: &'a [(String, Json)],
    taken: Vec<bool>,
    path: String,
}

impl<'a> Obj<'a> {
    fn new(value: &'a Json, path: &str) -> Result<Self> {
        match value {
            Json::Object(members) => Ok(Self {
                members,
                taken: vec![false; members.len()],
                path: path.to_owned(),
            }),
            other => Err(violation(
                path,
                format!("expected an object, found {}", other.type_name()),
            )),
        }
    }

    fn take(&mut self, key: &str) -> Option<&'a Json> {
        let index = self.members.iter().position(|(k, _)| k == key)?;
        self.taken[index] = true;
        Some(&self.members[index].1)
    }

    fn require(&mut self, key: &str) -> Result<&'a Json> {
        self.take(key)
            .ok_or_else(|| violation(&self.path, format!("missing required member {key:?}")))
    }

    fn string(&mut self, key: &str) -> Result<String> {
        let value = self.require(key)?;
        value.as_str().map(str::to_owned).ok_or_else(|| {
            violation(
                &child(&self.path, key),
                format!("expected a string, found {}", value.type_name()),
            )
        })
    }

    fn number(&mut self, key: &str) -> Result<f64> {
        let value = self.require(key)?;
        value.as_f64().ok_or_else(|| {
            violation(
                &child(&self.path, key),
                format!("expected a number, found {}", value.type_name()),
            )
        })
    }

    fn index(&mut self, key: &str) -> Result<u32> {
        let x = self.number(key)?;
        if x.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&x) {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "range and integrality checked"
            )]
            Ok(x as u32)
        } else {
            Err(violation(
                &child(&self.path, key),
                format!("expected a non-negative integer, found {x}"),
            ))
        }
    }

    fn array(&mut self, key: &str) -> Result<&'a [Json]> {
        let value = self.require(key)?;
        value.as_array().ok_or_else(|| {
            violation(
                &child(&self.path, key),
                format!("expected an array, found {}", value.type_name()),
            )
        })
    }

    fn object(&mut self, key: &str) -> Result<Obj<'a>> {
        let value = self.require(key)?;
        Obj::new(value, &child(&self.path, key))
    }

    fn finish(self) -> Result<()> {
        match self.taken.iter().position(|t| !t) {
            None => Ok(()),
            Some(i) => Err(violation(
                &self.path,
                format!("unexpected member {:?}", self.members[i].0),
            )),
        }
    }
}

// ---------------------------------------------------------------- geometry

fn f32_from(value: &Json, path: &str) -> Result<f32> {
    let x = value.as_f64().ok_or_else(|| {
        violation(
            path,
            format!("expected a number, found {}", value.type_name()),
        )
    })?;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "range checked on the next line"
    )]
    let y = x as f32;
    if y.is_finite() {
        Ok(y)
    } else {
        Err(violation(
            path,
            format!("{x} is outside the range of a 32-bit float"),
        ))
    }
}

fn polygon_to_json(polygon: &Polygon<Source>) -> Json {
    Json::Array(
        polygon
            .points()
            .iter()
            .map(|p| Json::Array(vec![Json::from_f32(p.x), Json::from_f32(p.y)]))
            .collect(),
    )
}

fn polygon_from_json(value: &Json, path: &str) -> Result<Polygon<Source>> {
    let items = value.as_array().ok_or_else(|| {
        violation(
            path,
            format!("expected an array of points, found {}", value.type_name()),
        )
    })?;
    let mut points = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let here = child(path, i);
        let pair = item
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or_else(|| violation(&here, "a point is an array of two numbers"))?;
        points.push(Point::new(
            f32_from(&pair[0], &child(&here, 0))?,
            f32_from(&pair[1], &child(&here, 1))?,
        ));
    }
    Polygon::new(points).map_err(|e| violation(path, e.message().to_owned()))
}

fn confidence_to_json(c: Confidence) -> Json {
    Json::Object(vec![
        ("value".to_owned(), Json::Number(c.value())),
        ("calibrated".to_owned(), Json::Bool(c.is_calibrated())),
    ])
}

fn confidence_from_json(value: &Json, path: &str) -> Result<Confidence> {
    let mut obj = Obj::new(value, path)?;
    let number = obj.number("value")?;
    let calibrated = obj.require("calibrated")?;
    let calibrated = calibrated
        .as_bool()
        .ok_or_else(|| violation(&child(path, "calibrated"), "expected a boolean"))?;
    obj.finish()?;
    Confidence::new(number, calibrated).map_err(|e| {
        error(
            codes::CONFIDENCE_INVALID,
            &child(path, "value"),
            e.message().to_owned(),
        )
    })
}

// ---------------------------------------------------------------- chains

fn chain_to_json(chain: &Chain) -> Json {
    let steps = chain
        .steps()
        .iter()
        .map(|step| {
            Json::Object(vec![
                ("to".to_owned(), Json::string(step.to())),
                ("kind".to_owned(), Json::string(step.transform().kind())),
                (
                    "matrix".to_owned(),
                    Json::numbers(step.transform().matrix()),
                ),
            ])
        })
        .collect();
    Json::Object(vec![
        ("from".to_owned(), Json::string(chain.from_frame())),
        ("steps".to_owned(), Json::Array(steps)),
    ])
}

fn chain_from_json(value: &Json, path: &str) -> Result<Chain> {
    let mut obj = Obj::new(value, path)?;
    let from = obj.string("from")?;
    let steps = obj.array("steps")?;
    obj.finish()?;
    let at = |code_path: &str, e: WnjError| {
        error(codes::CHAIN_INVALID, code_path, e.message().to_owned())
    };
    let mut chain = Chain::new(&from).map_err(|e| at(&child(path, "from"), e))?;
    for (i, step) in steps.iter().enumerate() {
        let here = child(&child(path, "steps"), i);
        let mut step_obj = Obj::new(step, &here)?;
        let to = step_obj.string("to")?;
        let kind = step_obj.string("kind")?;
        let matrix_json = step_obj.array("matrix")?;
        step_obj.finish()?;
        let mut matrix = Vec::with_capacity(matrix_json.len());
        for (j, element) in matrix_json.iter().enumerate() {
            matrix.push(element.as_f64().ok_or_else(|| {
                violation(&child(&child(&here, "matrix"), j), "expected a number")
            })?);
        }
        let transform = Transform::from_kind(&kind, &matrix).map_err(|e| at(&here, e))?;
        chain.push(&to, transform).map_err(|e| at(&here, e))?;
    }
    Ok(chain)
}

impl Chain {
    /// The chain as JSON: `{"from": ..., "steps": [{"to": ..., "kind": ..., "matrix": [...]}, ...]}`.
    #[must_use]
    pub fn to_json(&self) -> Json {
        chain_to_json(self)
    }

    /// Read a chain from its JSON form; the error is `WNJ-E1004` for a chain that is not connected or
    /// has a singular step, and `WNJ-E1003` for a value of the wrong shape.
    pub fn from_json(value: &Json) -> Result<Self> {
        chain_from_json(value, "")
    }
}

// ---------------------------------------------------------------- document parts

fn word_to_json(word: &Word) -> Json {
    Json::Object(vec![
        ("text".to_owned(), Json::string(&word.text)),
        ("polygon".to_owned(), polygon_to_json(&word.polygon)),
        ("confidence".to_owned(), confidence_to_json(word.confidence)),
    ])
}

fn word_from_json(value: &Json, path: &str) -> Result<Word> {
    let mut obj = Obj::new(value, path)?;
    let text = obj.string("text")?;
    let polygon = polygon_from_json(obj.require("polygon")?, &child(path, "polygon"))?;
    let confidence = confidence_from_json(obj.require("confidence")?, &child(path, "confidence"))?;
    obj.finish()?;
    Ok(Word {
        text,
        polygon,
        confidence,
    })
}

fn line_to_json(line: &Line) -> Json {
    Json::Object(vec![
        ("id".to_owned(), Json::string(&line.id)),
        ("text".to_owned(), Json::string(&line.text)),
        ("polygon".to_owned(), polygon_to_json(&line.polygon)),
        ("confidence".to_owned(), confidence_to_json(line.confidence)),
        (
            "words".to_owned(),
            Json::Array(line.words.iter().map(word_to_json).collect()),
        ),
    ])
}

fn line_from_json(value: &Json, path: &str) -> Result<Line> {
    let mut obj = Obj::new(value, path)?;
    let id = obj.string("id")?;
    let text = obj.string("text")?;
    let polygon = polygon_from_json(obj.require("polygon")?, &child(path, "polygon"))?;
    let confidence = confidence_from_json(obj.require("confidence")?, &child(path, "confidence"))?;
    let words_path = child(path, "words");
    let words = obj
        .array("words")?
        .iter()
        .enumerate()
        .map(|(i, w)| word_from_json(w, &child(&words_path, i)))
        .collect::<Result<Vec<_>>>()?;
    obj.finish()?;
    Ok(Line {
        id,
        text,
        polygon,
        confidence,
        words,
    })
}

fn page_to_json(page: &Page) -> Json {
    Json::Object(vec![
        ("index".to_owned(), Json::count(page.index)),
        (
            "size".to_owned(),
            Json::Object(vec![
                ("w".to_owned(), Json::from_f32(page.size.w)),
                ("h".to_owned(), Json::from_f32(page.size.h)),
            ]),
        ),
        ("chain".to_owned(), chain_to_json(&page.chain)),
        (
            "lines".to_owned(),
            Json::Array(page.lines.iter().map(line_to_json).collect()),
        ),
    ])
}

fn page_from_json(value: &Json, path: &str) -> Result<Page> {
    let mut obj = Obj::new(value, path)?;
    let index = obj.index("index")?;
    let mut size_obj = obj.object("size")?;
    let size_path = child(path, "size");
    let w = f32_from(size_obj.require("w")?, &child(&size_path, "w"))?;
    let h = f32_from(size_obj.require("h")?, &child(&size_path, "h"))?;
    size_obj.finish()?;
    let chain = chain_from_json(obj.require("chain")?, &child(path, "chain"))?;
    let lines_path = child(path, "lines");
    let lines = obj
        .array("lines")?
        .iter()
        .enumerate()
        .map(|(i, l)| line_from_json(l, &child(&lines_path, i)))
        .collect::<Result<Vec<_>>>()?;
    obj.finish()?;
    Ok(Page {
        index,
        size: Size::new(w, h),
        chain,
        lines,
    })
}

fn evidence_to_json(evidence: &Evidence) -> Json {
    Json::Object(vec![
        ("page".to_owned(), Json::count(evidence.page)),
        ("polygon".to_owned(), polygon_to_json(&evidence.polygon)),
        ("text".to_owned(), Json::string(&evidence.text)),
    ])
}

fn evidence_from_json(value: &Json, path: &str) -> Result<Evidence> {
    let mut obj = Obj::new(value, path)?;
    let page = obj.index("page")?;
    let polygon = polygon_from_json(obj.require("polygon")?, &child(path, "polygon"))?;
    let text = obj.string("text")?;
    obj.finish()?;
    Ok(Evidence {
        page,
        polygon,
        text,
    })
}

fn field_to_json(field: &Field) -> Json {
    let mut members = vec![
        ("name".to_owned(), Json::string(&field.name)),
        ("kind".to_owned(), Json::string(field.kind.as_str())),
        ("value".to_owned(), Json::string(&field.value)),
    ];
    if let Some(currency) = &field.currency {
        members.push(("currency".to_owned(), Json::string(currency)));
    }
    members.push((
        "confidence".to_owned(),
        confidence_to_json(field.confidence),
    ));
    members.push(("decision".to_owned(), Json::string(field.decision.as_str())));
    members.push((
        "evidence".to_owned(),
        Json::Array(field.evidence.iter().map(evidence_to_json).collect()),
    ));
    members.push((
        "lines".to_owned(),
        Json::Array(field.lines.iter().map(Json::string).collect()),
    ));
    Json::Object(members)
}

fn spelled<T: Copy>(
    obj: &mut Obj<'_>,
    key: &str,
    parse: fn(&str) -> Option<T>,
    allowed: &str,
) -> Result<T> {
    let text = obj.string(key)?;
    parse(&text).ok_or_else(|| {
        violation(
            &child(&obj.path, key),
            format!("{text:?} is not one of {allowed}"),
        )
    })
}

fn field_from_json(value: &Json, path: &str) -> Result<Field> {
    let mut obj = Obj::new(value, path)?;
    let name = obj.string("name")?;
    let kind = spelled(
        &mut obj,
        "kind",
        FieldKind::parse,
        "amount, date, iban, mrz, text",
    )?;
    let text = obj.string("value")?;
    let currency = match obj.take("currency") {
        None => None,
        Some(v) => Some(
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| violation(&child(path, "currency"), "expected a string"))?,
        ),
    };
    let confidence = confidence_from_json(obj.require("confidence")?, &child(path, "confidence"))?;
    let decision = spelled(&mut obj, "decision", Decision::parse, "accept, review")?;
    let evidence_path = child(path, "evidence");
    let evidence = obj
        .array("evidence")?
        .iter()
        .enumerate()
        .map(|(i, e)| evidence_from_json(e, &child(&evidence_path, i)))
        .collect::<Result<Vec<_>>>()?;
    let lines_path = child(path, "lines");
    let lines = obj
        .array("lines")?
        .iter()
        .enumerate()
        .map(|(i, l)| {
            l.as_str()
                .map(str::to_owned)
                .ok_or_else(|| violation(&child(&lines_path, i), "expected a string"))
        })
        .collect::<Result<Vec<_>>>()?;
    obj.finish()?;
    Ok(Field {
        name,
        kind,
        value: text,
        currency,
        confidence,
        decision,
        evidence,
        lines,
    })
}

fn run_to_json(run: &Run) -> Json {
    Json::Object(vec![
        ("engine".to_owned(), Json::string(&run.engine)),
        ("config_sha256".to_owned(), Json::string(&run.config_sha256)),
        (
            "timing_ms".to_owned(),
            Json::Object(
                run.timing_ms
                    .iter()
                    .map(|(k, v)| (k.clone(), Json::Number(*v)))
                    .collect(),
            ),
        ),
    ])
}

fn run_from_json(value: &Json, path: &str) -> Result<Run> {
    let mut obj = Obj::new(value, path)?;
    let engine = obj.string("engine")?;
    let config_sha256 = obj.string("config_sha256")?;
    let timing = obj.require("timing_ms")?;
    let timing_path = child(path, "timing_ms");
    let Json::Object(entries) = timing else {
        return Err(violation(
            &timing_path,
            format!("expected an object, found {}", timing.type_name()),
        ));
    };
    let mut timing_ms = BTreeMap::new();
    for (stage, ms) in entries {
        let x = ms
            .as_f64()
            .ok_or_else(|| violation(&child(&timing_path, stage), "expected a number"))?;
        timing_ms.insert(stage.clone(), x);
    }
    obj.finish()?;
    Ok(Run {
        engine,
        config_sha256,
        timing_ms,
    })
}

// ---------------------------------------------------------------- document

impl Document {
    /// The document as a JSON value. Validates first: an invalid document is never serialised.
    pub fn to_json(&self) -> Result<Json> {
        self.validate()?;
        Ok(Json::Object(vec![
            ("schema".to_owned(), Json::string(SCHEMA_ID)),
            ("id".to_owned(), Json::string(&self.id)),
            ("class".to_owned(), Json::string(self.class.as_str())),
            (
                "source".to_owned(),
                Json::Object(vec![
                    ("sha256".to_owned(), Json::string(&self.source.sha256)),
                    (
                        "media_type".to_owned(),
                        Json::string(&self.source.media_type),
                    ),
                ]),
            ),
            (
                "pages".to_owned(),
                Json::Array(self.pages.iter().map(page_to_json).collect()),
            ),
            (
                "fields".to_owned(),
                Json::Array(self.fields.iter().map(field_to_json).collect()),
            ),
            ("run".to_owned(), run_to_json(&self.run)),
        ]))
    }

    /// Read a document from a JSON value, strictly, and validate it.
    pub fn from_json(value: &Json) -> Result<Self> {
        let mut obj = Obj::new(value, "")?;
        let schema_id = obj.string("schema")?;
        if schema_id != SCHEMA_ID {
            let code = if schema_id.starts_with(SCHEMA_PREFIX) {
                codes::SCHEMA_VERSION
            } else {
                codes::SCHEMA_VIOLATION
            };
            return Err(error(
                code,
                "/schema",
                format!("this build reads {SCHEMA_ID}, the document declares {schema_id:?}"),
            ));
        }
        let id = obj.string("id")?;
        let class = spelled(
            &mut obj,
            "class",
            DocClass::parse,
            "invoice, receipt, id, other",
        )?;
        let mut source_obj = obj.object("source")?;
        let source = SourceInfo {
            sha256: source_obj.string("sha256")?,
            media_type: source_obj.string("media_type")?,
        };
        source_obj.finish()?;
        let pages = obj
            .array("pages")?
            .iter()
            .enumerate()
            .map(|(i, p)| page_from_json(p, &child("/pages", i)))
            .collect::<Result<Vec<_>>>()?;
        let fields = obj
            .array("fields")?
            .iter()
            .enumerate()
            .map(|(i, f)| field_from_json(f, &child("/fields", i)))
            .collect::<Result<Vec<_>>>()?;
        let run = run_from_json(obj.require("run")?, "/run")?;
        obj.finish()?;
        let document = Self {
            id,
            class,
            source,
            pages,
            fields,
            run,
        };
        document.validate()?;
        Ok(document)
    }

    /// Parse JSON text, check it against the schema, and read it. The schema check runs first, so its
    /// complaints are about the text as written; the reader then applies the checks the schema cannot
    /// express.
    pub fn from_json_str(text: &str) -> Result<Self> {
        let value = json::parse(text)?;
        schema::validate(&value)?;
        Self::from_json(&value)
    }

    /// The canonical JSON text (RFC 8785): the form that is hashed and signed.
    pub fn to_canonical_string(&self) -> Result<String> {
        to_canonical_string(&self.to_json()?)
    }

    /// SHA-256 of the canonical text, 64 lower-case hexadecimal digits.
    pub fn canonical_sha256(&self) -> Result<String> {
        canonical_sha256(&self.to_json()?)
    }

    /// SHA-256 of the canonical text with `run.timing_ms` emptied: the same input, configuration and
    /// build give the same value however long they took. This is the hash the determinism tests compare.
    pub fn content_sha256(&self) -> Result<String> {
        let mut value = self.to_json()?;
        if let Json::Object(top) = &mut value {
            for (key, member) in top.iter_mut() {
                if key != "run" {
                    continue;
                }
                if let Json::Object(run) = member {
                    for (run_key, run_member) in run.iter_mut() {
                        if run_key == "timing_ms" {
                            *run_member = Json::Object(Vec::new());
                        }
                    }
                }
            }
        }
        canonical_sha256(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::to_canonical_string;
    use crate::json::parse;
    use wanjiru_math::rng::Rng;

    const SAMPLE: &str = include_str!("../tests/golden/sample-invoice.json");

    fn at_mut<'a>(value: &'a mut Json, path: &[&str]) -> &'a mut Json {
        let mut node = value;
        for step in path {
            node = match node {
                Json::Object(members) => {
                    &mut members
                        .iter_mut()
                        .find(|(k, _)| k == step)
                        .unwrap_or_else(|| panic!("no member {step}"))
                        .1
                }
                Json::Array(items) => &mut items[step.parse::<usize>().expect("index")],
                other => panic!("cannot descend into {other:?}"),
            };
        }
        node
    }

    fn broken(path: &[&str], replacement: Json) -> WnjError {
        let mut value = parse(SAMPLE).expect("sample parses");
        *at_mut(&mut value, path) = replacement;
        Document::from_json(&value).expect_err("should be refused")
    }

    #[test]
    fn the_reader_names_the_value_it_refuses() {
        let cases: Vec<(&[&str], Json, &str, &str)> = vec![
            (
                &["pages", "0", "lines", "0", "polygon"],
                parse("[[1,2],[3,4]]").expect("json"),
                "WNJ-E1003",
                "/pages/0/lines/0/polygon",
            ),
            (
                &["pages", "0", "lines", "0", "confidence", "value"],
                Json::Number(1.5),
                "WNJ-E1005",
                "/pages/0/lines/0/confidence/value",
            ),
            (
                &["pages", "0", "chain", "steps", "0", "to"],
                Json::string("plane"),
                "WNJ-E1004",
                "/pages/0/chain/steps/0",
            ),
            (
                &["pages", "0", "chain", "steps", "1", "matrix"],
                parse("[1,2,3]").expect("json"),
                "WNJ-E1004",
                "/pages/0/chain/steps/1",
            ),
            (
                &["pages", "0", "index"],
                Json::Number(0.5),
                "WNJ-E1003",
                "/pages/0/index",
            ),
            (
                &["fields", "0", "kind"],
                Json::string("Amount"),
                "WNJ-E1003",
                "/fields/0/kind",
            ),
            (
                &["fields", "1", "lines", "0"],
                Json::Number(7.0),
                "WNJ-E1003",
                "/fields/1/lines/0",
            ),
            (
                &["run", "timing_ms", "total"],
                Json::string("fast"),
                "WNJ-E1003",
                "/run/timing_ms/total",
            ),
            (
                &["pages", "0", "size", "w"],
                Json::Number(1e300),
                "WNJ-E1003",
                "/pages/0/size/w",
            ),
        ];
        for (path, replacement, code, pointer) in cases {
            let error = broken(path, replacement);
            assert_eq!(error.code().to_string(), code, "{error}");
            assert_eq!(error.path(), Some(pointer), "{error}");
        }
    }

    #[test]
    fn unknown_and_missing_members_are_refused() {
        let mut value = parse(SAMPLE).expect("sample parses");
        if let Json::Object(members) = at_mut(&mut value, &["fields", "0"]) {
            members.push(("surprise".to_owned(), Json::Null));
        }
        let error = Document::from_json(&value).expect_err("unknown member");
        assert!(error.message().contains("surprise"), "{error}");
        assert_eq!(error.path(), Some("/fields/0"));

        let mut value = parse(SAMPLE).expect("sample parses");
        if let Json::Object(members) = at_mut(&mut value, &["run"]) {
            members.retain(|(k, _)| k != "engine");
        }
        let error = Document::from_json(&value).expect_err("missing member");
        assert!(error.message().contains("engine"), "{error}");
        assert_eq!(error.path(), Some("/run"));
    }

    const POOL: [char; 18] = [
        'a',
        'Z',
        '7',
        ' ',
        '"',
        '\\',
        '/',
        '\n',
        '\t',
        '\u{0}',
        '\u{7f}',
        'é',
        '€',
        '\u{2028}',
        '\u{fb33}',
        '😀',
        '\u{10ffff}',
        '日',
    ];

    fn text(rng: &mut Rng, max: usize) -> String {
        (0..rng.below_usize(max + 1))
            .map(|_| POOL[rng.below_usize(POOL.len())])
            .collect()
    }

    fn finite_f32(rng: &mut Rng) -> f32 {
        loop {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "the low 32 bits are the point"
            )]
            let x = f32::from_bits(rng.next_u64() as u32);
            if x.is_finite() {
                return x;
            }
        }
    }

    fn polygon(rng: &mut Rng) -> Polygon<Source> {
        let count = 3 + rng.below_usize(3);
        let points = (0..count)
            .map(|_| {
                // Half of the coordinates are "ordinary" pixel values, half are arbitrary floats.
                if rng.below(2) == 0 {
                    Point::new(finite_f32(rng), finite_f32(rng))
                } else {
                    #[allow(clippy::cast_possible_truncation, reason = "f32 storage")]
                    let ordinary =
                        |r: &mut Rng| ((r.next_f64() * 4000.0 * 100.0).round() / 100.0) as f32;
                    Point::new(ordinary(rng), ordinary(rng))
                }
            })
            .collect();
        Polygon::new(points).expect("finite")
    }

    fn confidence(rng: &mut Rng) -> Confidence {
        Confidence::new(rng.next_f64(), rng.below(2) == 0).expect("in range")
    }

    fn chain(rng: &mut Rng) -> Chain {
        const FRAMES: [&str; 4] = ["page", "line", "pdf", "normalised"];
        let mut chain = Chain::new("source").expect("known");
        for _ in 0..rng.below_usize(4) {
            let mut to = FRAMES[rng.below_usize(FRAMES.len())];
            while to == chain.to_frame() {
                to = FRAMES[rng.below_usize(FRAMES.len())];
            }
            let scale = 0.5 + rng.next_f64();
            let step = if rng.below(2) == 0 {
                Transform::Affine([scale, 0.01, -0.01, scale, rng.next_f64() * 100.0, 5.0])
            } else {
                Transform::Homography([scale, 0.0, 3.0, 0.0, scale, 4.0, 1e-5, 0.0, 1.0])
            };
            chain.push(to, step).expect("valid step");
        }
        chain
    }

    fn random_document(rng: &mut Rng) -> Document {
        let mut line_ids = Vec::new();
        let mut pages = Vec::new();
        for index in 0..=rng.below_usize(2) {
            let lines = (0..rng.below_usize(4))
                .map(|_| {
                    let id = format!("l{}", line_ids.len());
                    line_ids.push(id.clone());
                    Line {
                        id,
                        text: text(rng, 20),
                        polygon: polygon(rng),
                        confidence: confidence(rng),
                        words: (0..rng.below_usize(3))
                            .map(|_| Word {
                                text: text(rng, 6),
                                polygon: polygon(rng),
                                confidence: confidence(rng),
                            })
                            .collect(),
                    }
                })
                .collect();
            pages.push(Page {
                index: u32::try_from(index).expect("small"),
                size: Size::new(
                    1.0 + finite_f32(rng).abs().min(1e6),
                    1.0 + finite_f32(rng).abs().min(1e6),
                ),
                chain: chain(rng),
                lines,
            });
        }
        let kinds = FieldKind::ALL;
        let fields = (0..rng.below_usize(4))
            .map(|n| {
                let kind = kinds[rng.below_usize(kinds.len())];
                Field {
                    name: format!("field_{n}"),
                    kind,
                    value: format!("v{}", text(rng, 12)),
                    currency: (kind == FieldKind::Amount && rng.below(2) == 0)
                        .then(|| "EUR".to_owned()),
                    confidence: confidence(rng),
                    decision: Decision::ALL[rng.below_usize(Decision::ALL.len())],
                    evidence: (0..rng.below_usize(3))
                        .map(|_| Evidence {
                            page: u32::try_from(rng.below_usize(pages.len())).expect("small"),
                            polygon: polygon(rng),
                            text: text(rng, 8),
                        })
                        .collect(),
                    lines: line_ids
                        .iter()
                        .filter(|_| rng.below(3) == 0)
                        .cloned()
                        .collect(),
                }
            })
            .collect();
        Document {
            id: format!("doc-{}", text(rng, 30)),
            class: DocClass::ALL[rng.below_usize(DocClass::ALL.len())],
            source: SourceInfo {
                sha256: format!(
                    "{:016x}{:016x}{:016x}{:016x}",
                    rng.next_u64(),
                    rng.next_u64(),
                    rng.next_u64(),
                    rng.next_u64()
                ),
                media_type: "image/png".to_owned(),
            },
            pages,
            fields,
            run: Run {
                engine: "wanjiru test".to_owned(),
                config_sha256: "0".repeat(64),
                timing_ms: [("total".to_owned(), rng.next_f64() * 1000.0)]
                    .into_iter()
                    .collect(),
            },
        }
    }

    #[test]
    fn random_documents_survive_the_round_trip_unchanged() {
        let mut rng = Rng::seed_from_u64(20_261_031);
        for case in 0..3_000 {
            let document = random_document(&mut rng);
            let json = document
                .to_json()
                .unwrap_or_else(|e| panic!("case {case}: {e}"));
            // Independent of the reader: the schema accepts what the writer produced.
            let violations = schema::check(&json).expect("schema");
            assert!(violations.is_empty(), "case {case}: {violations:?}");
            let canonical = to_canonical_string(&json).expect("canonical");
            let back =
                Document::from_json_str(&canonical).unwrap_or_else(|e| panic!("case {case}: {e}"));
            assert_eq!(back, document, "case {case}");
            assert_eq!(
                back.to_canonical_string().expect("canonical"),
                canonical,
                "case {case}"
            );
            assert_eq!(
                back.content_sha256().expect("hash"),
                document.content_sha256().expect("hash")
            );
        }
    }
}
