//! One evaluation: contract, truth, predictions and optionally a baseline in; a [`Report`] out.

use wanjiru_math::digest::sha256_hex;
use wanjiru_types::Result;

use crate::cer;
use crate::contract::Contract;
use crate::corpus::{Case, Corpus};
use crate::fields::tally;
use crate::gates::{Comparison, Gate};
use crate::intervals::{Resampling, proportion, quantile, ratio_interval, relative_change};
use crate::report::{
    CerSummary, Counts, Digests, FieldSummary, LatencySummary, Regression, Report,
};

/// The texts of the files an evaluation reads.
#[derive(Copy, Clone, Debug)]
pub struct Inputs<'a> {
    /// The quality contract (YAML).
    pub contract: &'a str,
    /// The ground truth (JSON Lines).
    pub truth: &'a str,
    /// The engine's documents (DocIR, JSON Lines).
    pub predictions: &'a str,
    /// A baseline run's documents, to measure regressions against.
    pub baseline: Option<&'a str>,
}

/// The paired comparison with a baseline uses its own random stream, so that adding `--baseline` does
/// not change any other interval: the seed is combined by exclusive or with this constant (the `SplitMix64` increment).
const PAIRED_STREAM: u64 = 0x9E37_79B9_7F4A_7C15;

/// Judge `inputs` by their contract.
///
/// # Errors
///
/// Anything that makes the inputs unusable: `WNJ-E4001`/`E4002` for the contract, `WNJ-E4003`,
/// `E4004`, `E4006`, `E4007` and the DocIR codes for the corpus files, `WNJ-E4005` for settings or
/// samples a statistic cannot be computed from. A corpus that merely fails the contract is not an
/// error; it is a report whose verdict is `fail`.
pub fn evaluate(inputs: &Inputs<'_>, resampling: &Resampling) -> Result<Report> {
    resampling.validate()?;
    let contract = Contract::from_yaml(inputs.contract)?;
    let corpus = Corpus::parse(inputs.truth, inputs.predictions, inputs.baseline)?;
    let cases = corpus.cases(contract.class)?;

    let with_reference: Vec<(&Case<'_>, &str)> = cases
        .iter()
        .filter_map(|case| case.truth.text.as_deref().map(|text| (case, text)))
        .collect();
    let counts = Counts {
        truth: cases.len(),
        predicted: cases.iter().filter(|c| c.prediction.is_some()).count(),
        baseline: corpus
            .has_baseline()
            .then(|| cases.iter().filter(|c| c.baseline.is_some()).count()),
        with_reference: with_reference.len(),
    };

    // Character error rate, and the paired comparison with the baseline.
    let scores: Vec<cer::Errors> = with_reference
        .iter()
        .map(|(case, text)| cer::score(text, case.prediction))
        .collect();
    let reference_lengths: Vec<u64> = scores.iter().map(|s| s.reference_len).collect();
    let edits: Vec<u64> = scores.iter().map(|s| s.edits).collect();
    let measurable = with_reference.len() >= 2 && reference_lengths.iter().sum::<u64>() > 0;
    let cer_summary = if measurable {
        Some(CerSummary {
            documents: with_reference.len(),
            reference_chars: reference_lengths.iter().sum(),
            edits: edits.iter().sum(),
            interval: ratio_interval(&edits, &reference_lengths, resampling)?,
        })
    } else {
        None
    };
    let regression = if corpus.has_baseline() && measurable {
        let base_edits: Vec<u64> = with_reference
            .iter()
            .map(|(case, text)| cer::score(text, case.baseline).edits)
            .collect();
        let paired = Resampling {
            seed: resampling.seed ^ PAIRED_STREAM,
            ..*resampling
        };
        Some(Regression {
            base_edits: base_edits.iter().sum(),
            new_edits: edits.iter().sum(),
            change: relative_change(&edits, &base_edits, &paired)?,
        })
    } else {
        None
    };

    // Fields.
    let mut fields = Vec::with_capacity(contract.fields.len());
    for gate in &contract.fields {
        let counted = tally(gate, &cases)?;
        fields.push(FieldSummary {
            gate: gate.clone(),
            tally: counted,
            accuracy: proportion(counted.correct, counted.accepted, resampling.level),
            coverage: proportion(counted.accepted, counted.trials, resampling.level),
        });
    }

    // Latency.
    let mut milliseconds: Vec<f64> = cases
        .iter()
        .filter_map(|case| case.prediction)
        .filter_map(|document| document.run.timing_ms.get("total").copied())
        .filter(|ms| ms.is_finite() && *ms >= 0.0)
        .collect();
    milliseconds.sort_by(f64::total_cmp);
    let latency = (!milliseconds.is_empty()).then(|| LatencySummary {
        p50: quantile(&milliseconds, 0.5, resampling.level),
        p95: quantile(&milliseconds, 0.95, resampling.level),
    });

    let gates = build_gates(
        &contract,
        &fields,
        latency.as_ref(),
        regression.as_ref(),
        corpus.has_baseline(),
    );
    Ok(Report {
        contract,
        resampling: *resampling,
        digests: Digests {
            contract: sha256_hex(inputs.contract.as_bytes()),
            truth: sha256_hex(inputs.truth.as_bytes()),
            predictions: sha256_hex(inputs.predictions.as_bytes()),
            baseline: inputs.baseline.map(|text| sha256_hex(text.as_bytes())),
        },
        counts,
        cer: cer_summary,
        fields,
        latency,
        regression,
        gates,
    })
}

fn build_gates(
    contract: &Contract,
    fields: &[FieldSummary],
    latency: Option<&LatencySummary>,
    regression: Option<&Regression>,
    has_baseline: bool,
) -> Vec<Gate> {
    let mut gates = Vec::new();
    for f in fields {
        let name = &f.gate.name;
        let accuracy = Gate::measured(
            format!("field_exact_match.{name}.min"),
            Comparison::AtLeast,
            f.gate.min,
            f.accuracy.estimate,
            f.accuracy.lower,
            f.accuracy.upper,
        );
        gates.push(if f.tally.trials == 0 {
            accuracy.with_note("no truth document carries this field")
        } else if f.tally.accepted == 0 {
            accuracy.with_note("the engine accepted no value")
        } else {
            accuracy
        });
        let coverage = Gate::measured(
            format!("field_exact_match.{name}.coverage"),
            Comparison::AtLeast,
            f.gate.coverage,
            f.coverage.estimate,
            f.coverage.lower,
            f.coverage.upper,
        );
        gates.push(if f.tally.trials == 0 {
            coverage.with_note("no truth document carries this field")
        } else {
            coverage
        });
    }
    if let Some(max) = contract.ece_char_max {
        gates.push(Gate::not_measured(
            "ece_char.max",
            Comparison::AtMost,
            max,
            "calibration of character confidences is measured from Phase 6",
        ));
    }
    if let Some(conformal) = contract.conformal {
        gates.push(Gate::not_measured(
            "conformal.coverage",
            Comparison::AtLeast,
            1.0 - conformal.alpha - conformal.coverage_tolerance,
            "conformal coverage is measured from Phase 6",
        ));
    }
    for (id, max, pick) in [
        ("latency_ms.p50_max", contract.latency_p50_max, true),
        ("latency_ms.p95_max", contract.latency_p95_max, false),
    ] {
        let Some(max) = max else { continue };
        gates.push(match latency {
            Some(l) => {
                let q = if pick { &l.p50 } else { &l.p95 };
                Gate::measured(
                    id,
                    Comparison::AtMost,
                    max,
                    q.estimate,
                    q.lower.unwrap_or(f64::NEG_INFINITY),
                    q.upper.unwrap_or(f64::INFINITY),
                )
            }
            None => Gate::not_measured(
                id,
                Comparison::AtMost,
                max,
                "no output carries run.timing_ms.total",
            ),
        });
    }
    if let Some(budget) = contract.regression_budget_cer_rel {
        let id = "gates.regression_budget_cer_rel";
        gates.push(match regression {
            Some(r) => Gate::measured(
                id,
                Comparison::AtMost,
                budget,
                Some(r.change.estimate),
                r.change.lower,
                r.change.upper,
            ),
            None if !has_baseline => Gate::not_measured(
                id,
                Comparison::AtMost,
                budget,
                "no baseline run was supplied",
            ),
            None => Gate::not_measured(
                id,
                Comparison::AtMost,
                budget,
                "the character error rate could not be measured: it needs two documents with reference text",
            ),
        });
    }
    gates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::testing::{document, truth};
    use crate::gates::Verdict;
    use wanjiru_types::codes;

    const CONTRACT: &str = "\
contract: tiny-v1
class: invoice
corpus: tests/tiny
metrics:
  field_exact_match:
    total: {kind: amount, min: 0.5, coverage: 0.5}
  ece_char: {max: 0.05}
  latency_ms: {p50_max: 100, p95_max: 1000}
gates:
  regression_budget_cer_rel: 0.1
";

    /// `n` invoices of the text `Total 10.00`: the engine reads the first `wrong` of them wrongly.
    fn corpus(n: usize, wrong: usize) -> (String, String, String) {
        let mut truths = Vec::new();
        let mut predictions = Vec::new();
        let mut baseline = Vec::new();
        for i in 0..n {
            let id = format!("inv-{i:03}");
            truths.push(truth(
                &id,
                "invoice",
                Some("Total 10.00"),
                &[("total", "10.00")],
            ));
            let (text, value) = if i < wrong {
                ("Total 1O.00", "10.01")
            } else {
                ("Total 10.00", "10.00")
            };
            predictions.push(document(
                &id,
                "invoice",
                &[text],
                &[("total", "amount", value, true)],
                Some(20.0 + (i % 50) as f64),
            ));
            baseline.push(document(
                &id,
                "invoice",
                &["Total 10.0O"],
                &[("total", "amount", "10.00", true)],
                Some(30.0),
            ));
        }
        (
            truths.join("\n"),
            predictions.join("\n"),
            baseline.join("\n"),
        )
    }

    #[test]
    fn a_good_engine_on_a_large_enough_corpus_passes() {
        let (t, p, b) = corpus(400, 0);
        let inputs = Inputs {
            contract: CONTRACT,
            truth: &t,
            predictions: &p,
            baseline: Some(&b),
        };
        let report = evaluate(&inputs, &Resampling::DEFAULT).expect("report");
        let by_id = |id: &str| {
            report
                .gates
                .iter()
                .find(|g| g.id == id)
                .unwrap_or_else(|| panic!("{id}"))
        };
        assert_eq!(by_id("field_exact_match.total.min").verdict, Verdict::Pass);
        assert_eq!(
            by_id("field_exact_match.total.coverage").verdict,
            Verdict::Pass
        );
        assert_eq!(by_id("latency_ms.p50_max").verdict, Verdict::Pass);
        assert_eq!(by_id("ece_char.max").verdict, Verdict::NotMeasured);
        // The baseline misreads one character in every document; the engine none: a large improvement.
        let regression = by_id("gates.regression_budget_cer_rel");
        assert_eq!(regression.verdict, Verdict::Pass);
        assert!((regression.estimate.expect("estimate") + 1.0).abs() < 1e-12);
        assert_eq!(report.verdict(), Verdict::Pass);
        let cer = report.cer.expect("cer");
        assert_eq!(
            (cer.documents, cer.edits, cer.reference_chars),
            (400, 0, 400 * 11)
        );
        assert_eq!(
            report.counts,
            Counts {
                truth: 400,
                predicted: 400,
                baseline: Some(400),
                with_reference: 400
            }
        );
    }

    #[test]
    fn a_bad_engine_fails_and_a_small_corpus_is_inconclusive() {
        let (t, p, _) = corpus(400, 300);
        let inputs = Inputs {
            contract: CONTRACT,
            truth: &t,
            predictions: &p,
            baseline: None,
        };
        let report = evaluate(&inputs, &Resampling::DEFAULT).expect("report");
        let accuracy = report
            .gates
            .iter()
            .find(|g| g.id == "field_exact_match.total.min")
            .expect("gate");
        assert_eq!(
            accuracy.verdict,
            Verdict::Fail,
            "accuracy is 25 % against a minimum of 50 %"
        );
        assert_eq!(report.verdict(), Verdict::Fail);
        let regression = report
            .gates
            .iter()
            .find(|g| g.id == "gates.regression_budget_cer_rel")
            .expect("gate");
        assert_eq!(regression.verdict, Verdict::NotMeasured);
        assert_eq!(
            regression.note.as_deref(),
            Some("no baseline run was supplied")
        );

        let (t, p, _) = corpus(6, 3);
        let inputs = Inputs {
            contract: CONTRACT,
            truth: &t,
            predictions: &p,
            baseline: None,
        };
        let report = evaluate(&inputs, &Resampling::DEFAULT).expect("report");
        let accuracy = report
            .gates
            .iter()
            .find(|g| g.id == "field_exact_match.total.min")
            .expect("gate");
        assert_eq!(
            accuracy.verdict,
            Verdict::Inconclusive,
            "3 of 6 cannot be told from 50 %"
        );
        assert_eq!(report.verdict(), Verdict::Inconclusive);
    }

    #[test]
    fn missing_outputs_count_as_failures_not_as_absences() {
        let (t, p, _) = corpus(100, 0);
        // The engine produced nothing for half of the documents.
        let kept: Vec<&str> = p.lines().take(50).collect();
        let inputs = Inputs {
            contract: CONTRACT,
            truth: &t,
            predictions: &kept.join("\n"),
            baseline: None,
        };
        let report = evaluate(&inputs, &Resampling::DEFAULT).expect("report");
        assert_eq!(report.counts.predicted, 50);
        let f = &report.fields[0];
        assert_eq!(
            (f.tally.trials, f.tally.accepted, f.tally.absent),
            (100, 50, 50)
        );
        assert_eq!(f.coverage.estimate, Some(0.5));
        let cer = report.cer.expect("cer");
        assert_eq!((cer.edits, cer.reference_chars), (50 * 11, 100 * 11));
        // Latency is only known for the documents that exist.
        assert_eq!(report.latency.expect("latency").p50.n, 50);
    }

    #[test]
    fn the_baseline_does_not_change_any_other_number() {
        let (t, p, b) = corpus(120, 10);
        let without = evaluate(
            &Inputs {
                contract: CONTRACT,
                truth: &t,
                predictions: &p,
                baseline: None,
            },
            &Resampling::DEFAULT,
        )
        .expect("report");
        let with = evaluate(
            &Inputs {
                contract: CONTRACT,
                truth: &t,
                predictions: &p,
                baseline: Some(&b),
            },
            &Resampling::DEFAULT,
        )
        .expect("report");
        assert_eq!(without.cer, with.cer);
        assert_eq!(without.fields, with.fields);
        assert_eq!(without.latency, with.latency);
        assert!(without.regression.is_none() && with.regression.is_some());
    }

    #[test]
    fn reports_are_reproducible_and_depend_on_the_seed() {
        let (t, p, b) = corpus(80, 5);
        let inputs = Inputs {
            contract: CONTRACT,
            truth: &t,
            predictions: &p,
            baseline: Some(&b),
        };
        let a = evaluate(&inputs, &Resampling::DEFAULT)
            .expect("report")
            .to_canonical_string()
            .expect("text");
        let again = evaluate(&inputs, &Resampling::DEFAULT)
            .expect("report")
            .to_canonical_string()
            .expect("text");
        assert_eq!(a, again);
        let other = evaluate(
            &inputs,
            &Resampling {
                seed: 7,
                ..Resampling::DEFAULT
            },
        )
        .expect("report")
        .to_canonical_string()
        .expect("text");
        assert_ne!(a, other);
    }

    #[test]
    fn input_problems_are_errors_not_verdicts() {
        let (t, p, _) = corpus(10, 0);
        let bad_contract = CONTRACT.replace("class: invoice", "class: receipt");
        let e = evaluate(
            &Inputs {
                contract: &bad_contract,
                truth: &t,
                predictions: &p,
                baseline: None,
            },
            &Resampling::DEFAULT,
        )
        .expect_err("no receipts");
        assert_eq!(e.code(), codes::CORPUS_EMPTY);
        let e = evaluate(
            &Inputs {
                contract: "contract: [",
                truth: &t,
                predictions: &p,
                baseline: None,
            },
            &Resampling::DEFAULT,
        )
        .expect_err("syntax");
        assert_eq!(e.code(), codes::CONTRACT_PARSE);
        let e = evaluate(
            &Inputs {
                contract: CONTRACT,
                truth: &t,
                predictions: &p,
                baseline: None,
            },
            &Resampling {
                resamples: 5,
                ..Resampling::DEFAULT
            },
        )
        .expect_err("settings");
        assert_eq!(e.code(), codes::STATS_ARGUMENT);
    }

    #[test]
    fn documents_without_reference_text_still_count_for_fields() {
        let t = [
            truth("a", "invoice", None, &[("total", "1.00")]),
            truth("b", "invoice", None, &[("total", "2.00")]),
        ]
        .join("\n");
        let p = [
            document(
                "a",
                "invoice",
                &["x"],
                &[("total", "amount", "1.00", true)],
                None,
            ),
            document(
                "b",
                "invoice",
                &["x"],
                &[("total", "amount", "2.00", true)],
                None,
            ),
        ]
        .join("\n");
        let contract = "contract: c\nclass: invoice\ncorpus: c\nmetrics:\n  field_exact_match:\n    total: {kind: amount, min: 0.5, coverage: 0.5}\n  latency_ms: {p50_max: 10}\ngates: {regression_budget_cer_rel: 0.1}\n";
        let report = evaluate(
            &Inputs {
                contract,
                truth: &t,
                predictions: &p,
                baseline: None,
            },
            &Resampling::DEFAULT,
        )
        .expect("report");
        assert!(report.cer.is_none());
        assert_eq!(report.fields[0].tally.correct, 2);
        let latency = report
            .gates
            .iter()
            .find(|g| g.id == "latency_ms.p50_max")
            .expect("gate");
        assert_eq!(latency.verdict, Verdict::NotMeasured);
    }
}
