//! Cross-checks against independent implementations.
//!
//! The vectors in `tests/golden/` come from `tools/oracles/make_docir_vectors.py`. The references are
//! Node.js (ECMAScript number formatting and string escaping, UTF-16 key order: the definition of
//! RFC 8785), Python's `json` and `jsonschema` packages, NumPy, and hashlib. Nothing here shares
//! code with the crate under test.

use wanjiru_docir::{
    Chain, Confidence, Decision, DocClass, Document, Evidence, Field, FieldKind, Line, Page, Run,
    SourceInfo, Word, canonical_sha256, format_number, parse, schema, to_canonical_string,
};
use wanjiru_types::{Point, Polygon, Size, Source};

fn unhex(text: &str) -> String {
    if text == "-" {
        return String::new();
    }
    let bytes: Vec<u8> = (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex byte"))
        .collect();
    String::from_utf8(bytes).expect("UTF-8")
}

#[test]
fn number_formatting_agrees_with_ecmascript() {
    let mut count = 0;
    for line in include_str!("golden/numbers.txt").lines() {
        let (bits, expected) = line.split_once(' ').expect("two columns");
        let x = f64::from_bits(u64::from_str_radix(bits, 16).expect("16 hex digits"));
        assert_eq!(format_number(x).expect("finite"), expected, "bits {bits}");
        count += 1;
    }
    assert!(count >= 10_000, "vectors read: {count}");
}

#[test]
fn json_reader_and_canonical_writer_agree_with_python_and_node() {
    let (mut accepted, mut rejected) = (0, 0);
    for line in include_str!("golden/json_cases.txt").lines() {
        let mut parts = line.splitn(3, ' ');
        let verdict = parts.next().expect("verdict");
        let text = unhex(parts.next().expect("text"));
        let parsed = parse(&text);
        match verdict {
            "ok" => {
                let expected = unhex(parts.next().expect("canonical"));
                let value = parsed.unwrap_or_else(|e| panic!("{text:?} should parse: {e}"));
                assert_eq!(
                    to_canonical_string(&value).expect("canonical"),
                    expected,
                    "input {text:?}"
                );
                accepted += 1;
            }
            "err" => {
                assert!(parsed.is_err(), "{text:?} should be rejected");
                rejected += 1;
            }
            other => panic!("unknown verdict {other}"),
        }
    }
    assert!(accepted >= 500 && rejected >= 300, "{accepted} {rejected}");
}

#[test]
fn schema_validator_agrees_with_jsonschema_and_the_reader_is_never_more_lenient() {
    let (mut models, mut valid, mut invalid) = (0, 0, 0);
    for line in include_str!("golden/schema_cases.txt").lines() {
        let mut parts = line.splitn(3, ' ');
        let kind = parts.next().expect("kind");
        let canonical = parts.next().expect("canonical");
        let text = parts.next().expect("document");
        let value = parse(text).expect("generated documents are valid JSON");
        let violations = schema::check(&value).expect("embedded schema works");
        match kind {
            "model" => {
                assert!(violations.is_empty(), "{violations:?} in {text}");
                // A document the generator built to satisfy every rule is read, rewritten and
                // hashed, and its canonical text equals Node's canonicalisation of the input text.
                let document =
                    Document::from_json_str(text).unwrap_or_else(|e| panic!("{e}: {text}"));
                assert_eq!(
                    document.to_canonical_string().expect("canonical"),
                    unhex(canonical),
                    "{text}"
                );
                let again =
                    Document::from_json(&document.to_json().expect("json")).expect("round trip");
                assert_eq!(again, document);
                models += 1;
            }
            "schema-valid" => {
                assert!(
                    violations.is_empty(),
                    "jsonschema accepts, we report {violations:?}: {text}"
                );
                valid += 1;
            }
            "schema-invalid" => {
                assert!(
                    !violations.is_empty(),
                    "jsonschema rejects, we accept: {text}"
                );
                // Whatever the schema rejects, the strict reader rejects too.
                assert!(
                    Document::from_json(&value).is_err(),
                    "reader accepts what the schema rejects: {text}"
                );
                invalid += 1;
            }
            other => panic!("unknown kind {other}"),
        }
    }
    assert!(
        models >= 200 && valid >= 80 && invalid >= 250,
        "{models} {valid} {invalid}"
    );
}

#[test]
fn chains_agree_with_numpy_matrix_arithmetic() {
    let mut points = 0;
    for line in include_str!("golden/chains.jsonl").lines() {
        let case = parse(line).expect("json");
        let chain = Chain::from_json(case.get("chain").expect("chain")).expect("valid chain");
        let inverse = chain.inverse().expect("invertible");
        let pairs = |key: &str| -> Vec<(f64, f64)> {
            case.get(key)
                .and_then(|v| v.as_array())
                .expect(key)
                .iter()
                .map(|p| {
                    let p = p.as_array().expect("pair");
                    (p[0].as_f64().expect("x"), p[1].as_f64().expect("y"))
                })
                .collect()
        };
        let (start, forward, back) = (pairs("points"), pairs("forward"), pairs("back"));
        for ((p, f), b) in start.iter().zip(&forward).zip(&back) {
            let close = |got: (f64, f64), want: (f64, f64)| {
                let scale = 1.0 + want.0.abs().max(want.1.abs());
                (got.0 - want.0).abs() <= 1e-9 * scale && (got.1 - want.1).abs() <= 1e-9 * scale
            };
            let got = chain.map_point(p.0, p.1).expect("finite");
            assert!(close(got, *f), "forward {p:?}: {got:?} vs {f:?}");
            let got_back = inverse.map_point(f.0, f.1).expect("finite");
            assert!(close(got_back, *b), "inverse {f:?}: {got_back:?} vs {b:?}");
            points += 1;
        }
        // The chain survives its own JSON round trip.
        assert_eq!(Chain::from_json(&chain.to_json()).expect("again"), chain);
    }
    assert!(points >= 1_000, "points checked: {points}");
}

fn polygon(points: &[(f32, f32)]) -> Polygon<Source> {
    Polygon::new(points.iter().map(|&(x, y)| Point::new(x, y)).collect()).expect("valid polygon")
}

fn rectangle(x0: f32, y0: f32, x1: f32, y1: f32) -> Polygon<Source> {
    polygon(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
}

/// The sample invoice of `tests/golden/sample-invoice.json`, built in code.
fn sample_invoice() -> Document {
    let mut chain = Chain::new("source").expect("known frame");
    chain
        .push(
            "page",
            wanjiru_docir::Transform::Affine([0.5, 0.0, 0.0, 0.5, 0.0, 0.0]),
        )
        .expect("affine step");
    chain
        .push(
            "line",
            wanjiru_docir::Transform::Homography([
                1.0, 0.0, -10.0, 0.0, 1.0, -20.0, 0.00001, 0.0, 1.0,
            ]),
        )
        .expect("homography step");
    let score = |v: f64| Confidence::score(v).expect("in range");
    Document {
        id: "inv-0001".to_owned(),
        class: DocClass::Invoice,
        source: SourceInfo {
            sha256: include_str!("golden/sample-source.sha256")
                .trim()
                .to_owned(),
            media_type: "image/png".to_owned(),
        },
        pages: vec![Page {
            index: 0,
            size: Size::new(1654.0, 2339.0),
            chain,
            lines: vec![
                Line {
                    id: "l0".to_owned(),
                    text: "Rechnung Nr. 2026-001".to_owned(),
                    polygon: rectangle(100.5, 120.0, 620.25, 152.5),
                    confidence: score(0.98),
                    words: vec![Word {
                        text: "Rechnung".to_owned(),
                        polygon: rectangle(100.5, 120.0, 260.0, 152.5),
                        confidence: score(0.99),
                    }],
                },
                Line {
                    id: "l1".to_owned(),
                    text: "Gesamtbetrag 1 234,50 € — fällig am 2026-11-30 😀".to_owned(),
                    polygon: rectangle(100.5, 1900.0, 900.75, 1940.0),
                    confidence: score(0.973_123_456_789_012_3),
                    words: Vec::new(),
                },
            ],
        }],
        fields: vec![
            Field {
                name: "total_amount".to_owned(),
                kind: FieldKind::Amount,
                value: "1234.50".to_owned(),
                currency: Some("EUR".to_owned()),
                confidence: score(0.97),
                decision: Decision::Accept,
                evidence: vec![Evidence {
                    page: 0,
                    polygon: rectangle(420.0, 1900.0, 600.5, 1940.0),
                    text: "1 234,50".to_owned(),
                }],
                lines: vec!["l1".to_owned()],
            },
            Field {
                name: "issue_date".to_owned(),
                kind: FieldKind::Date,
                value: "2026-11-30".to_owned(),
                currency: None,
                confidence: score(0.61),
                decision: Decision::Review,
                evidence: Vec::new(),
                lines: vec!["l1".to_owned()],
            },
        ],
        run: Run {
            engine: "wanjiru 0.1.0".to_owned(),
            config_sha256: include_str!("golden/sample-config.sha256")
                .trim()
                .to_owned(),
            timing_ms: [
                ("decode", 1.25),
                ("recognise", 0.000_000_1),
                ("total", 12.5),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
        },
    }
}

#[test]
fn the_sample_invoice_is_the_same_document_in_code_and_in_json() {
    let text = include_str!("golden/sample-invoice.json");
    let from_file = Document::from_json_str(text).expect("sample parses and validates");
    let built = sample_invoice();
    assert_eq!(from_file, built);
    let canonical = include_str!("golden/sample-invoice.canonical.json");
    assert_eq!(built.to_canonical_string().expect("canonical"), canonical);
    assert_eq!(
        from_file.to_canonical_string().expect("canonical"),
        canonical
    );
    // The hash is hashlib's, over exactly those bytes.
    let expected = include_str!("golden/sample-invoice.sha256").trim();
    assert_eq!(built.canonical_sha256().expect("hash"), expected);
    assert_eq!(
        canonical_sha256(&parse(canonical).expect("canonical text is JSON")).expect("hash"),
        expected
    );
    // Timings change with the machine; the content hash does not.
    let mut slower = built.clone();
    slower.run.timing_ms.insert("total".to_owned(), 99.0);
    assert_ne!(slower.canonical_sha256().expect("hash"), expected);
    assert_eq!(
        slower.content_sha256().expect("hash"),
        include_str!("golden/sample-invoice.content.sha256").trim()
    );
    assert_eq!(
        built.content_sha256().expect("hash"),
        slower.content_sha256().expect("hash")
    );
}

#[test]
fn documents_that_break_a_rule_are_not_serialised() {
    let mut broken = sample_invoice();
    broken.fields[0].lines = vec!["no-such-line".to_owned()];
    let error = broken.to_json().expect_err("dangling reference");
    assert_eq!(error.path(), Some("/fields/0/lines/0"));
    let mut broken = sample_invoice();
    broken.pages[0].lines[1].id = "l0".to_owned();
    assert!(broken.canonical_sha256().is_err());
}

#[test]
fn an_unknown_schema_version_has_its_own_error() {
    let text =
        include_str!("golden/sample-invoice.json").replace("wanjiru.docir/0", "wanjiru.docir/1");
    let value = parse(&text).expect("json");
    let error = Document::from_json(&value).expect_err("future version");
    assert_eq!(error.code().to_string(), "WNJ-E1006");
    let other =
        include_str!("golden/sample-invoice.json").replace("wanjiru.docir/0", "somebody.else/0");
    let error = Document::from_json(&parse(&other).expect("json")).expect_err("foreign schema");
    assert_eq!(error.code().to_string(), "WNJ-E1003");
}
