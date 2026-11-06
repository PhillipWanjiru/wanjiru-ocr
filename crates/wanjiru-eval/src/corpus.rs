//! Corpus files: ground truth, engine outputs and baselines, as JSON Lines text.
//!
//! The harness does no file I/O (layer L4); callers pass the contents of the files. Three kinds of
//! file exist and all of them carry one JSON object per line:
//!
//! * **truth**: `{"id": "...", "class": "invoice", "text": "...", "fields": {"total_amount": "1234.50"}}`.
//!   `text` is the reference transcription, lines joined by `\n`, and may be left out; then the
//!   document does not take part in the character error rate. `fields` maps a field name to its
//!   value in canonical form. Unknown keys are errors.
//! * **predictions** and **baseline**: complete DocIR documents (`docir check` validates them), one
//!   per line.
//!
//! Documents are paired by `id`. A truth document without a prediction is a total failure, never a
//! document that is quietly left out: leaving out the documents the engine failed on is the easiest
//! way to inflate every score. A prediction whose id is not in the truth is an error, because it is
//! almost always a mismatch of corpus versions.

use std::collections::{BTreeMap, BTreeSet};

use wanjiru_docir::{DocClass, Document, parse};
use wanjiru_types::{ErrorKind, Result, WnjError, codes};

/// One ground-truth record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Truth {
    /// Identifier, unique within the corpus.
    pub id: String,
    /// The class of the document; a contract judges the documents of its own class.
    pub class: DocClass,
    /// The reference transcription, if the document has one.
    pub text: Option<String>,
    /// Field name to canonical value.
    pub fields: BTreeMap<String, String>,
}

/// A truth document together with what the engine and the baseline produced for it.
#[derive(Copy, Clone, Debug)]
pub struct Case<'a> {
    /// The ground truth.
    pub truth: &'a Truth,
    /// The engine's output, `None` if it produced none.
    pub prediction: Option<&'a Document>,
    /// The baseline's output, `None` if there is no baseline run or it produced none.
    pub baseline: Option<&'a Document>,
}

/// A parsed corpus: all truth records, the engine's documents and, optionally, a baseline's.
#[derive(Clone, Debug)]
pub struct Corpus {
    truths: Vec<Truth>,
    predictions: BTreeMap<String, Document>,
    baseline: Option<BTreeMap<String, Document>>,
}

fn record_error(line: usize, message: impl Into<String>) -> WnjError {
    WnjError::new(
        codes::CORPUS_RECORD_INVALID,
        ErrorKind::InvalidInput,
        message,
    )
    .at(format!("line {line}"))
}

/// Add the line number to an error from a lower layer, keeping its code and kind.
fn at_line(error: &WnjError, line: usize) -> WnjError {
    let path = match error.path() {
        Some(inner) => format!("line {line} {inner}"),
        None => format!("line {line}"),
    };
    WnjError::new(error.code(), error.kind(), error.message()).at(path)
}

/// The non-blank lines of `text` with their one-based line numbers; `\r\n` is accepted.
fn records(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split('\n')
        .enumerate()
        .map(|(i, line)| (i + 1, line.strip_suffix('\r').unwrap_or(line)))
        .filter(|(_, line)| !line.trim().is_empty())
}

fn truth_record(line: usize, text: &str) -> Result<Truth> {
    let value = parse(text).map_err(|e| at_line(&e, line))?;
    let Some(members) = value.as_object() else {
        return Err(record_error(line, "a truth record is a JSON object"));
    };
    let (mut id, mut class, mut reference, mut fields) = (None, None, None, BTreeMap::new());
    for (key, member) in members {
        let string = |what: &str| -> Result<String> {
            member
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| record_error(line, format!("`{what}` is a string")))
        };
        match key.as_str() {
            "id" => id = Some(string("id")?),
            "class" => {
                let name = string("class")?;
                class = Some(DocClass::parse(&name).ok_or_else(|| {
                    record_error(
                        line,
                        format!("unknown class `{name}`; classes are invoice, receipt, id, other"),
                    )
                })?);
            }
            "text" => reference = Some(string("text")?),
            "fields" => {
                let Some(entries) = member.as_object() else {
                    return Err(record_error(line, "`fields` is an object of strings"));
                };
                for (name, v) in entries {
                    let Some(value) = v.as_str() else {
                        return Err(record_error(
                            line,
                            format!("the value of field `{name}` is a string"),
                        ));
                    };
                    if value.is_empty() {
                        return Err(record_error(
                            line,
                            format!(
                                "the value of field `{name}` is not empty; leave an absent field out"
                            ),
                        ));
                    }
                    fields.insert(name.clone(), value.to_owned());
                }
            }
            other => {
                return Err(record_error(
                    line,
                    format!("unknown key `{other}`; known keys: id, class, text, fields"),
                ));
            }
        }
    }
    let id = id.ok_or_else(|| record_error(line, "missing `id`"))?;
    if id.is_empty() || id.chars().count() > 256 {
        return Err(record_error(line, "`id` has 1 to 256 characters"));
    }
    let class = class.ok_or_else(|| record_error(line, "missing `class`"))?;
    Ok(Truth {
        id,
        class,
        text: reference,
        fields,
    })
}

/// Parse the truth file; the records are returned in id order.
///
/// # Errors
///
/// `WNJ-E4006` for a malformed record, `WNJ-E4003` if two records share an id, and the `WNJ-E100x`
/// codes for text that is not strict JSON.
pub fn parse_truth(text: &str) -> Result<Vec<Truth>> {
    let mut by_id: BTreeMap<String, (usize, Truth)> = BTreeMap::new();
    for (line, record) in records(text) {
        let truth = truth_record(line, record)?;
        if let Some((first, _)) = by_id.get(&truth.id) {
            return Err(WnjError::new(
                codes::CORPUS_DUPLICATE_ID,
                ErrorKind::InvalidInput,
                format!(
                    "the id `{}` is used twice (first on line {first})",
                    truth.id
                ),
            )
            .at(format!("line {line}")));
        }
        by_id.insert(truth.id.clone(), (line, truth));
    }
    Ok(by_id.into_values().map(|(_, truth)| truth).collect())
}

/// Parse a file of DocIR documents, one per line, keyed by document id. Every id must be in
/// `known`.
///
/// # Errors
///
/// The DocIR error codes for a document that does not parse or validate (with the line number in the
/// path), `WNJ-E4003` for a repeated id, `WNJ-E4007` for an id that `known` does not contain.
pub fn parse_documents(text: &str, known: &BTreeSet<&str>) -> Result<BTreeMap<String, Document>> {
    let mut documents: BTreeMap<String, (usize, Document)> = BTreeMap::new();
    for (line, record) in records(text) {
        let document = Document::from_json_str(record).map_err(|e| at_line(&e, line))?;
        if !known.contains(document.id.as_str()) {
            return Err(WnjError::new(
                codes::CORPUS_UNKNOWN_ID,
                ErrorKind::InvalidInput,
                format!(
                    "the document `{}` is not in the ground truth; the files are from different corpus versions",
                    document.id
                ),
            )
            .at(format!("line {line}")));
        }
        if let Some((first, _)) = documents.get(&document.id) {
            return Err(WnjError::new(
                codes::CORPUS_DUPLICATE_ID,
                ErrorKind::InvalidInput,
                format!(
                    "the id `{}` is used twice (first on line {first})",
                    document.id
                ),
            )
            .at(format!("line {line}")));
        }
        documents.insert(document.id.clone(), (line, document));
    }
    Ok(documents
        .into_iter()
        .map(|(id, (_, document))| (id, document))
        .collect())
}

impl Corpus {
    /// Parse the three files.
    ///
    /// # Errors
    ///
    /// See [`parse_truth`] and [`parse_documents`].
    pub fn parse(truth: &str, predictions: &str, baseline: Option<&str>) -> Result<Self> {
        let truths = parse_truth(truth)?;
        let known: BTreeSet<&str> = truths.iter().map(|t| t.id.as_str()).collect();
        let predictions = parse_documents(predictions, &known)?;
        let baseline = baseline
            .map(|text| parse_documents(text, &known))
            .transpose()?;
        Ok(Self {
            truths,
            predictions,
            baseline,
        })
    }

    /// Whether a baseline run was supplied.
    #[must_use]
    pub const fn has_baseline(&self) -> bool {
        self.baseline.is_some()
    }

    /// The truth documents of `class`, in id order, each with the outputs paired to it.
    ///
    /// # Errors
    ///
    /// `WNJ-E4004` if the truth holds no document of the class.
    pub fn cases(&self, class: DocClass) -> Result<Vec<Case<'_>>> {
        let cases: Vec<Case<'_>> = self
            .truths
            .iter()
            .filter(|t| t.class == class)
            .map(|truth| Case {
                truth,
                prediction: self.predictions.get(&truth.id),
                baseline: self.baseline.as_ref().and_then(|b| b.get(&truth.id)),
            })
            .collect();
        if cases.is_empty() {
            Err(WnjError::new(
                codes::CORPUS_EMPTY,
                ErrorKind::InvalidInput,
                format!(
                    "the ground truth holds no document of class `{}`",
                    class.as_str()
                ),
            ))
        } else {
            Ok(cases)
        }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! Builders for small DocIR documents used by the tests of this crate.

    use wanjiru_docir::{Json, to_canonical_string};

    fn q(text: &str) -> String {
        to_canonical_string(&Json::string(text)).expect("strings are always representable")
    }

    /// A field of a test document: name, kind, value, accepted or sent to review.
    pub type FieldSpec<'a> = (&'a str, &'a str, &'a str, bool);

    /// A valid one-page DocIR document as JSON text on one line.
    pub fn document(
        id: &str,
        class: &str,
        lines: &[&str],
        fields: &[FieldSpec<'_>],
        total_ms: Option<f64>,
    ) -> String {
        let lines_json: Vec<String> = lines
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let y = 10 * i;
                format!(
                    r#"{{"id":"l{i}","text":{},"polygon":[[0,{y}],[100,{y}],[100,{}],[0,{}]],"confidence":{{"value":0.9,"calibrated":false}},"words":[]}}"#,
                    q(text),
                    y + 8,
                    y + 8
                )
            })
            .collect();
        let line_refs = if lines.is_empty() { "[]" } else { r#"["l0"]"# };
        let fields_json: Vec<String> = fields
            .iter()
            .map(|(name, kind, value, accept)| {
                format!(
                    r#"{{"name":{},"kind":"{kind}","value":{},"confidence":{{"value":0.9,"calibrated":false}},"decision":"{}","evidence":[],"lines":{line_refs}}}"#,
                    q(name),
                    q(value),
                    if *accept { "accept" } else { "review" }
                )
            })
            .collect();
        let timing = total_ms.map_or_else(String::new, |t| format!(r#""total":{t}"#));
        format!(
            r#"{{"schema":"wanjiru.docir/0","id":{},"class":"{class}","source":{{"sha256":"{}","media_type":"image/png"}},"pages":[{{"index":0,"size":{{"w":100,"h":200}},"chain":{{"from":"source","steps":[]}},"lines":[{}]}}],"fields":[{}],"run":{{"engine":"test 0","config_sha256":"{}","timing_ms":{{{timing}}}}}}}"#,
            q(id),
            "0".repeat(64),
            lines_json.join(","),
            fields_json.join(","),
            "1".repeat(64),
        )
    }

    /// A truth record as JSON text on one line.
    pub fn truth(id: &str, class: &str, text: Option<&str>, fields: &[(&str, &str)]) -> String {
        let mut members = vec![
            format!(r#""id":{}"#, q(id)),
            format!(r#""class":"{class}""#),
        ];
        if let Some(t) = text {
            members.push(format!(r#""text":{}"#, q(t)));
        }
        let fields: Vec<String> = fields
            .iter()
            .map(|(k, v)| format!("{}:{}", q(k), q(v)))
            .collect();
        members.push(format!(r#""fields":{{{}}}"#, fields.join(",")));
        format!("{{{}}}", members.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{document, truth};
    use super::*;

    #[test]
    fn truth_records_are_read_in_id_order() {
        let text = format!(
            "{}\r\n\n  \n{}\n",
            truth("b", "receipt", None, &[("total", "1.00")]),
            truth("a", "invoice", Some("Hello\nWorld"), &[]),
        );
        let truths = parse_truth(&text).expect("valid");
        assert_eq!(truths.len(), 2);
        assert_eq!(truths[0].id, "a");
        assert_eq!(truths[0].class, DocClass::Invoice);
        assert_eq!(truths[0].text.as_deref(), Some("Hello\nWorld"));
        assert_eq!(truths[1].fields["total"], "1.00");
        assert_eq!(truths[1].text, None);
    }

    #[test]
    fn malformed_truth_names_the_line() {
        let ok = truth("a", "invoice", None, &[]);
        let cases = [
            (
                r#"{"id":"x","class":"invoice","extra":1}"#,
                codes::CORPUS_RECORD_INVALID,
            ),
            (r#"{"id":"x"}"#, codes::CORPUS_RECORD_INVALID),
            (r#"{"class":"invoice"}"#, codes::CORPUS_RECORD_INVALID),
            (
                r#"{"id":"x","class":"cheque"}"#,
                codes::CORPUS_RECORD_INVALID,
            ),
            (
                r#"{"id":"","class":"invoice"}"#,
                codes::CORPUS_RECORD_INVALID,
            ),
            (
                r#"{"id":"x","class":"invoice","fields":{"a":5}}"#,
                codes::CORPUS_RECORD_INVALID,
            ),
            (
                r#"{"id":"x","class":"invoice","fields":{"a":""}}"#,
                codes::CORPUS_RECORD_INVALID,
            ),
            (
                r#"{"id":"x","class":"invoice","fields":[]}"#,
                codes::CORPUS_RECORD_INVALID,
            ),
            (
                r#"{"id":1,"class":"invoice"}"#,
                codes::CORPUS_RECORD_INVALID,
            ),
            (r"[1]", codes::CORPUS_RECORD_INVALID),
            (
                r#"{"id":"x","id":"y","class":"invoice"}"#,
                codes::JSON_DUPLICATE_KEY,
            ),
            (r#"{"id":"x","class":"invoice""#, codes::JSON_SYNTAX),
        ];
        for (bad, code) in cases {
            let e = parse_truth(&format!("{ok}\n{bad}\n")).expect_err(bad);
            assert_eq!(e.code(), code, "{bad}: {e}");
            assert!(
                e.path().is_some_and(|p| p.starts_with("line 2")),
                "{bad}: {:?}",
                e.path()
            );
        }
    }

    #[test]
    fn duplicate_ids_are_refused() {
        let a = truth("a", "invoice", None, &[]);
        let e = parse_truth(&format!("{a}\n{a}\n")).expect_err("duplicate");
        assert_eq!(e.code(), codes::CORPUS_DUPLICATE_ID);
        assert_eq!(e.path(), Some("line 2"));
        assert!(e.message().contains("line 1"), "{e}");
        let known: BTreeSet<&str> = ["a"].into();
        let d = document("a", "invoice", &["x"], &[], None);
        let e = parse_documents(&format!("{d}\n{d}\n"), &known).expect_err("duplicate");
        assert_eq!(e.code(), codes::CORPUS_DUPLICATE_ID);
    }

    #[test]
    fn predictions_for_unknown_documents_are_refused() {
        let known: BTreeSet<&str> = ["a", "b"].into();
        let text = format!(
            "{}\n{}\n",
            document("a", "invoice", &["x"], &[], None),
            document("zzz", "invoice", &["x"], &[], None)
        );
        let e = parse_documents(&text, &known).expect_err("unknown");
        assert_eq!(e.code(), codes::CORPUS_UNKNOWN_ID);
        assert_eq!(e.path(), Some("line 2"));
    }

    #[test]
    fn an_invalid_prediction_keeps_its_docir_code_and_gains_a_line_number() {
        let known: BTreeSet<&str> = ["a"].into();
        let good = document("a", "invoice", &["x"], &[], None);
        // A document that is not JSON at all, and one that breaks the schema.
        let e = parse_documents("{oops\n", &known).expect_err("syntax");
        assert_eq!(e.code(), codes::JSON_SYNTAX);
        let schema_breaking = good.replace("\"index\":0", "\"index\":3");
        let e = parse_documents(&format!("\n{schema_breaking}\n"), &known).expect_err("schema");
        assert_eq!(e.code(), codes::SCHEMA_VIOLATION);
        assert!(
            e.path().is_some_and(|p| p.starts_with("line 2 ")),
            "{:?}",
            e.path()
        );
    }

    #[test]
    fn cases_pair_by_id_and_filter_by_class() {
        let truth_text = [
            truth("inv-1", "invoice", Some("a"), &[]),
            truth("inv-2", "invoice", Some("b"), &[]),
            truth("rec-1", "receipt", Some("c"), &[]),
        ]
        .join("\n");
        let predictions = [
            document("inv-1", "invoice", &["a"], &[], Some(5.0)),
            document("rec-1", "receipt", &["c"], &[], None),
        ]
        .join("\n");
        let baseline = document("inv-2", "invoice", &["b"], &[], None);
        let corpus = Corpus::parse(&truth_text, &predictions, Some(&baseline)).expect("valid");
        assert!(corpus.has_baseline());
        let cases = corpus.cases(DocClass::Invoice).expect("invoices");
        let summary: Vec<_> = cases
            .iter()
            .map(|c| {
                (
                    c.truth.id.as_str(),
                    c.prediction.is_some(),
                    c.baseline.is_some(),
                )
            })
            .collect();
        assert_eq!(summary, [("inv-1", true, false), ("inv-2", false, true)]);
        let e = corpus.cases(DocClass::Id).expect_err("none");
        assert_eq!(e.code(), codes::CORPUS_EMPTY);
        let corpus = Corpus::parse(&truth_text, &predictions, None).expect("valid");
        assert!(!corpus.has_baseline());
    }
}
