//! The evaluation report: a typed result, its deterministic JSON form and a text summary.
//!
//! The JSON form (`wanjiru.eval-report/0`) is canonical (RFC 8785) and ends with a member
//! `report_sha256`, the SHA-256 of the canonical text of everything else. It carries no timestamp, no
//! path and no host name, so the same inputs, settings and version of the harness give the same bytes
//! on every machine; two reports are equal if and only if their hashes are. Numbers that are not
//! finite (an unbounded interval end) are written as `null`.

use wanjiru_docir::{Json, canonical_sha256, to_canonical_string};
use wanjiru_types::Result;

use crate::contract::{Contract, FieldGate};
use crate::fields::Tally;
use crate::gates::{Gate, Verdict, overall};
use crate::intervals::{Interval, Proportion, Quantile, Resampling};

/// The schema identifier written into every report.
pub const SCHEMA_ID: &str = "wanjiru.eval-report/0";

/// SHA-256 digests of the input texts, so that a report names exactly what it was computed from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Digests {
    /// The contract text.
    pub contract: String,
    /// The truth file.
    pub truth: String,
    /// The predictions file.
    pub predictions: String,
    /// The baseline file, if one was given.
    pub baseline: Option<String>,
}

/// How many documents took part.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Counts {
    /// Truth documents of the contract's class.
    pub truth: usize,
    /// Of those, documents the engine produced output for.
    pub predicted: usize,
    /// Of those, documents the baseline produced output for (`None` without a baseline).
    pub baseline: Option<usize>,
    /// Of those, documents with a reference transcription.
    pub with_reference: usize,
}

/// The character error rate over the documents with a reference transcription.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CerSummary {
    /// Documents taking part.
    pub documents: usize,
    /// Reference characters, summed.
    pub reference_chars: u64,
    /// Edit operations, summed.
    pub edits: u64,
    /// The rate and its interval.
    pub interval: Interval,
}

/// One gated field.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldSummary {
    /// The contract's thresholds.
    pub gate: FieldGate,
    /// The counts.
    pub tally: Tally,
    /// Correct among accepted.
    pub accuracy: Proportion,
    /// Accepted among trials.
    pub coverage: Proportion,
}

/// Latency per document, from `run.timing_ms.total`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LatencySummary {
    /// Median.
    pub p50: Quantile,
    /// 95th percentile.
    pub p95: Quantile,
}

/// The change of the error count against the baseline.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Regression {
    /// Edits of the baseline.
    pub base_edits: u64,
    /// Edits of the engine.
    pub new_edits: u64,
    /// `new / base - 1` with its interval.
    pub change: Interval,
}

/// Everything a run of the harness found.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// The contract the run was judged by.
    pub contract: Contract,
    /// Interval settings.
    pub resampling: Resampling,
    /// What it was computed from.
    pub digests: Digests,
    /// Document counts.
    pub counts: Counts,
    /// The character error rate, if at least two documents have reference text.
    pub cer: Option<CerSummary>,
    /// The gated fields, in name order.
    pub fields: Vec<FieldSummary>,
    /// Latency, if any output carried a timing.
    pub latency: Option<LatencySummary>,
    /// The comparison with the baseline, if there is one.
    pub regression: Option<Regression>,
    /// The gates, in a fixed order.
    pub gates: Vec<Gate>,
}

#[allow(clippy::cast_precision_loss, reason = "counts stay far below 2^53")]
fn count(n: u64) -> Json {
    Json::Number(n as f64)
}

#[allow(clippy::cast_precision_loss, reason = "counts stay far below 2^53")]
fn size(n: usize) -> Json {
    Json::Number(n as f64)
}

fn number(x: f64) -> Json {
    if x.is_finite() {
        Json::Number(x)
    } else {
        Json::Null
    }
}

fn optional(x: Option<f64>) -> Json {
    x.map_or(Json::Null, number)
}

fn object(members: Vec<(&str, Json)>) -> Json {
    Json::Object(
        members
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

fn interval_json(i: &Interval) -> Json {
    object(vec![
        ("estimate", number(i.estimate)),
        ("lower", number(i.lower)),
        ("upper", number(i.upper)),
    ])
}

fn proportion_json(p: &Proportion) -> Json {
    object(vec![
        ("k", count(p.k)),
        ("n", count(p.n)),
        ("estimate", optional(p.estimate)),
        ("lower", number(p.lower)),
        ("upper", number(p.upper)),
    ])
}

fn quantile_json(q: &Quantile) -> Json {
    object(vec![
        ("n", size(q.n)),
        ("estimate", optional(q.estimate)),
        ("lower", optional(q.lower)),
        ("upper", optional(q.upper)),
    ])
}

fn gate_json(g: &Gate) -> Json {
    object(vec![
        ("id", Json::string(g.id.as_str())),
        ("comparison", Json::string(g.comparison.as_str())),
        ("threshold", number(g.threshold)),
        ("estimate", optional(g.estimate)),
        ("lower", number(g.lower)),
        ("upper", number(g.upper)),
        ("verdict", Json::string(g.verdict.as_str())),
        ("note", g.note.as_deref().map_or(Json::Null, Json::string)),
    ])
}

impl Report {
    /// The overall verdict.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        overall(&self.gates)
    }

    /// The report as JSON, without the closing hash.
    #[must_use]
    pub fn body(&self) -> Json {
        let c = &self.contract;
        let fields = Json::Object(
            self.fields
                .iter()
                .map(|f| {
                    let t = &f.tally;
                    (
                        f.gate.name.clone(),
                        object(vec![
                            ("kind", Json::string(f.gate.kind.as_str())),
                            ("trials", count(t.trials)),
                            ("accepted", count(t.accepted)),
                            ("correct", count(t.correct)),
                            ("reviewed", count(t.reviewed)),
                            ("absent", count(t.absent)),
                            ("accepted_invalid", count(t.accepted_invalid)),
                            ("accepted_without_truth", count(t.accepted_without_truth)),
                            ("accuracy", proportion_json(&f.accuracy)),
                            ("coverage", proportion_json(&f.coverage)),
                        ]),
                    )
                })
                .collect(),
        );
        object(vec![
            ("schema", Json::string(SCHEMA_ID)),
            (
                "contract",
                object(vec![
                    ("id", Json::string(c.id.as_str())),
                    ("class", Json::string(c.class.as_str())),
                    ("status", Json::string(c.status.as_str())),
                    ("corpus", Json::string(c.corpus.as_str())),
                ]),
            ),
            (
                "inputs",
                object(vec![
                    (
                        "contract_sha256",
                        Json::string(self.digests.contract.as_str()),
                    ),
                    ("truth_sha256", Json::string(self.digests.truth.as_str())),
                    (
                        "predictions_sha256",
                        Json::string(self.digests.predictions.as_str()),
                    ),
                    (
                        "baseline_sha256",
                        self.digests
                            .baseline
                            .as_deref()
                            .map_or(Json::Null, Json::string),
                    ),
                ]),
            ),
            (
                "method",
                object(vec![
                    ("level", number(self.resampling.level)),
                    ("resamples", Json::count(self.resampling.resamples)),
                    ("seed", count(self.resampling.seed)),
                    ("ratios", Json::string(self.resampling.method.as_str())),
                    ("proportions", Json::string("clopper-pearson")),
                    ("quantiles", Json::string("order-statistics")),
                ]),
            ),
            (
                "documents",
                object(vec![
                    ("truth", size(self.counts.truth)),
                    ("predicted", size(self.counts.predicted)),
                    ("baseline", self.counts.baseline.map_or(Json::Null, size)),
                    ("with_reference", size(self.counts.with_reference)),
                ]),
            ),
            (
                "cer",
                self.cer.as_ref().map_or(Json::Null, |cer| {
                    object(vec![
                        ("documents", size(cer.documents)),
                        ("reference_chars", count(cer.reference_chars)),
                        ("edits", count(cer.edits)),
                        ("rate", interval_json(&cer.interval)),
                    ])
                }),
            ),
            ("fields", fields),
            (
                "latency_ms",
                self.latency.as_ref().map_or(Json::Null, |l| {
                    object(vec![
                        ("p50", quantile_json(&l.p50)),
                        ("p95", quantile_json(&l.p95)),
                    ])
                }),
            ),
            (
                "regression",
                self.regression.as_ref().map_or(Json::Null, |r| {
                    object(vec![
                        ("baseline_edits", count(r.base_edits)),
                        ("edits", count(r.new_edits)),
                        ("relative_change", interval_json(&r.change)),
                    ])
                }),
            ),
            (
                "gates",
                Json::Array(self.gates.iter().map(gate_json).collect()),
            ),
            ("verdict", Json::string(self.verdict().as_str())),
        ])
    }

    /// The report with its closing `report_sha256`, as a JSON value.
    ///
    /// # Errors
    ///
    /// Only if the body contains a number the canonical writer refuses, which `body` never produces.
    pub fn to_json(&self) -> Result<Json> {
        let mut body = self.body();
        let hash = canonical_sha256(&body)?;
        if let Json::Object(members) = &mut body {
            members.push(("report_sha256".to_owned(), Json::string(hash)));
        }
        Ok(body)
    }

    /// The canonical JSON text of the report, ending in the hash of everything before it.
    ///
    /// # Errors
    ///
    /// See [`Report::to_json`].
    pub fn to_canonical_string(&self) -> Result<String> {
        to_canonical_string(&self.to_json()?)
    }

    /// The hash that closes the report.
    ///
    /// # Errors
    ///
    /// See [`Report::to_json`].
    pub fn sha256(&self) -> Result<String> {
        canonical_sha256(&self.body())
    }
}

fn fixed(x: f64, digits: usize) -> String {
    if x.is_finite() {
        format!("{x:.digits$}")
    } else if x.is_nan() {
        "n/a".to_owned()
    } else if x > 0.0 {
        "+inf".to_owned()
    } else {
        "-inf".to_owned()
    }
}

fn span(lower: f64, upper: f64, digits: usize) -> String {
    format!("[{}, {}]", fixed(lower, digits), fixed(upper, digits))
}

fn optional_span(lower: Option<f64>, upper: Option<f64>, digits: usize) -> String {
    let end = |x: Option<f64>| x.map_or_else(|| "unbounded".to_owned(), |v| fixed(v, digits));
    format!("[{}, {}]", end(lower), end(upper))
}

impl Report {
    /// A summary for a terminal. Like the JSON form it contains nothing that varies between runs.
    #[must_use]
    pub fn render_text(&self) -> String {
        use std::fmt::Write as _;
        let c = &self.contract;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "wanjiru eval: {} ({}), corpus {}",
            c.id,
            c.status.as_str(),
            c.corpus
        );
        let _ = writeln!(
            out,
            "documents: {} of class {}, {} with output{}",
            self.counts.truth,
            c.class.as_str(),
            self.counts.predicted,
            self.counts
                .baseline
                .map_or_else(String::new, |b| format!(", baseline has output for {b}"))
        );
        let _ = writeln!(
            out,
            "intervals: {} %, {}, {} resamples, seed {}",
            fixed(100.0 * self.resampling.level, 1),
            self.resampling.method.as_str(),
            self.resampling.resamples,
            self.resampling.seed
        );
        out.push('\n');
        match &self.cer {
            Some(cer) => {
                let _ = writeln!(
                    out,
                    "character error rate: {} {} ({} edits / {} characters, {} documents)",
                    fixed(cer.interval.estimate, 4),
                    span(cer.interval.lower, cer.interval.upper, 4),
                    cer.edits,
                    cer.reference_chars,
                    cer.documents
                );
            }
            None => out.push_str("character error rate: not measured (fewer than two documents with reference text)\n"),
        }
        for f in &self.fields {
            let t = &f.tally;
            let rate = |p: &Proportion| {
                p.estimate.map_or_else(
                    || format!("n/a {}", span(p.lower, p.upper, 4)),
                    |e| format!("{} {}", fixed(e, 4), span(p.lower, p.upper, 4)),
                )
            };
            let _ = writeln!(
                out,
                "field {} ({}): accuracy {} ({}/{}), coverage {} ({}/{}); reviewed {}, absent {}, accepted invalid {}, accepted without truth {}",
                f.gate.name,
                f.gate.kind.as_str(),
                rate(&f.accuracy),
                t.correct,
                t.accepted,
                rate(&f.coverage),
                t.accepted,
                t.trials,
                t.reviewed,
                t.absent,
                t.accepted_invalid,
                t.accepted_without_truth
            );
        }
        if let Some(l) = &self.latency {
            let q = |q: &Quantile| {
                format!(
                    "{} {}",
                    q.estimate.map_or_else(|| "n/a".to_owned(), |e| fixed(e, 1)),
                    optional_span(q.lower, q.upper, 1)
                )
            };
            let _ = writeln!(
                out,
                "latency ms: p50 {}, p95 {} (n = {})",
                q(&l.p50),
                q(&l.p95),
                l.p50.n
            );
        }
        if let Some(r) = &self.regression {
            let _ = writeln!(
                out,
                "change in errors against the baseline: {} {} ({} edits against {})",
                fixed(r.change.estimate, 4),
                span(r.change.lower, r.change.upper, 4),
                r.new_edits,
                r.base_edits
            );
        }
        out.push_str("\ngates:\n");
        for g in &self.gates {
            let estimate = g.estimate.map_or_else(|| "n/a".to_owned(), |e| fixed(e, 4));
            let _ = write!(
                out,
                "  {:<13} {:<42} {} {:<9} {} {}",
                g.verdict.as_str(),
                g.id,
                g.comparison.as_str(),
                fixed(g.threshold, 4),
                estimate,
                span(g.lower, g.upper, 4)
            );
            if let Some(note) = &g.note {
                let _ = write!(out, "  ({note})");
            }
            out.push('\n');
        }
        let _ = writeln!(out, "\nverdict: {}", self.verdict().as_str());
        if let Ok(hash) = self.sha256() {
            let _ = writeln!(out, "report sha256: {hash}");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::Comparison;
    use wanjiru_docir::{DocClass, FieldKind, parse};

    pub(crate) fn sample() -> Report {
        let gate = FieldGate {
            name: "total_amount".to_owned(),
            kind: FieldKind::Amount,
            min: 0.995,
            coverage: 0.95,
        };
        let accuracy = crate::intervals::proportion(355, 356, 0.95);
        let coverage = crate::intervals::proportion(356, 360, 0.95);
        let mut gates = vec![
            Gate::measured(
                "field_exact_match.total_amount.min",
                Comparison::AtLeast,
                0.995,
                accuracy.estimate,
                accuracy.lower,
                accuracy.upper,
            ),
            Gate::measured(
                "field_exact_match.total_amount.coverage",
                Comparison::AtLeast,
                0.95,
                coverage.estimate,
                coverage.lower,
                coverage.upper,
            ),
            Gate::not_measured(
                "ece_char.max",
                Comparison::AtMost,
                0.02,
                "calibration is measured from Phase 6",
            ),
        ];
        gates.push(Gate::measured(
            "latency_ms.p95_max",
            Comparison::AtMost,
            2500.0,
            Some(2100.0),
            2000.0,
            f64::INFINITY,
        ));
        Report {
            contract: Contract {
                id: "invoice-fields-v1".to_owned(),
                class: DocClass::Invoice,
                status: crate::contract::Status::Draft,
                corpus: "golden/invoices-v0".to_owned(),
                fields: vec![gate.clone()],
                ece_char_max: Some(0.02),
                conformal: None,
                latency_p50_max: None,
                latency_p95_max: Some(2500.0),
                regression_budget_cer_rel: None,
            },
            resampling: Resampling::DEFAULT,
            digests: Digests {
                contract: "a".repeat(64),
                truth: "b".repeat(64),
                predictions: "c".repeat(64),
                baseline: None,
            },
            counts: Counts {
                truth: 360,
                predicted: 358,
                baseline: None,
                with_reference: 360,
            },
            cer: Some(CerSummary {
                documents: 360,
                reference_chars: 326_000,
                edits: 4010,
                interval: Interval {
                    estimate: 0.0123,
                    lower: 0.0101,
                    upper: 0.0150,
                },
            }),
            fields: vec![FieldSummary {
                gate,
                tally: Tally {
                    trials: 360,
                    accepted: 356,
                    correct: 355,
                    reviewed: 3,
                    absent: 1,
                    accepted_invalid: 0,
                    accepted_without_truth: 0,
                },
                accuracy,
                coverage,
            }],
            latency: None,
            regression: None,
            gates,
        }
    }

    #[test]
    fn the_json_form_ends_with_the_hash_of_the_rest() {
        let report = sample();
        let text = report.to_canonical_string().expect("canonical");
        let value = parse(&text).expect("json");
        let hash = value
            .get("report_sha256")
            .and_then(Json::as_str)
            .expect("hash")
            .to_owned();
        assert_eq!(hash, report.sha256().expect("hash"));
        // Remove the member again and hash the rest: the same value.
        let Json::Object(mut members) = value else {
            panic!("object")
        };
        members.retain(|(k, _)| k != "report_sha256");
        assert_eq!(
            canonical_sha256(&Json::Object(members)).expect("hash"),
            hash
        );
        assert_eq!(
            report.to_canonical_string().expect("again"),
            text,
            "deterministic"
        );
    }

    #[test]
    fn infinite_ends_become_null_and_the_schema_is_named() {
        let text = sample().to_canonical_string().expect("canonical");
        let value = parse(&text).expect("json");
        assert_eq!(value.get("schema").and_then(Json::as_str), Some(SCHEMA_ID));
        let gates = value.get("gates").and_then(Json::as_array).expect("gates");
        let latency = gates
            .iter()
            .find(|g| g.get("id").and_then(Json::as_str) == Some("latency_ms.p95_max"))
            .expect("gate");
        assert_eq!(latency.get("upper"), Some(&Json::Null));
        assert_eq!(
            latency.get("verdict").and_then(Json::as_str),
            Some("inconclusive")
        );
        let unmeasured = gates
            .iter()
            .find(|g| g.get("id").and_then(Json::as_str) == Some("ece_char.max"))
            .expect("gate");
        assert_eq!(
            unmeasured.get("verdict").and_then(Json::as_str),
            Some("not_measured")
        );
        assert_eq!(unmeasured.get("lower"), Some(&Json::Null));
        assert_eq!(
            value.get("verdict").and_then(Json::as_str),
            Some("inconclusive")
        );
        assert_eq!(value.get("regression"), Some(&Json::Null));
    }

    #[test]
    fn the_text_summary_has_no_timestamps_and_names_every_gate() {
        let report = sample();
        let text = report.render_text();
        for needle in [
            "invoice-fields-v1 (draft)",
            "character error rate: 0.0123 [0.0101, 0.0150]",
            "field total_amount (amount): accuracy 0.9972",
            "inconclusive  field_exact_match.total_amount.min",
            "not_measured  ece_char.max",
            "(calibration is measured from Phase 6)",
            "+inf]",
            "verdict: inconclusive",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in\n{text}");
        }
        assert!(text.contains(&report.sha256().expect("hash")));
        assert_eq!(text, report.render_text());
    }
}
