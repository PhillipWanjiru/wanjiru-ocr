//! Field-level scoring: for each gated field, how often the engine accepts it and how often an
//! accepted value is exactly right.
//!
//! The unit is the document. For a field `f` a *trial* is a truth document that carries `f`. A trial
//! is *accepted* if the engine's document has a field of that name whose decision is `accept`; it is
//! *correct* if, in addition, the kind and the value are exactly those of the truth. A field the
//! engine sent to review, or did not produce, or a document it did not produce at all, is not
//! accepted; none of them is dropped from the trials. Two rates follow:
//!
//! * selective accuracy = correct / accepted: of the values the engine stands behind, how many are
//!   right (the quantity a business user cares about, since accepted values are not looked at);
//! * coverage = accepted / trials: how often the engine stands behind a value at all.
//!
//! Accepted values that break the syntax of their kind (`accepted_invalid`) and accepted fields in
//! documents whose truth lacks the field (`accepted_without_truth`) are counted and reported; they do
//! not enter the rates. The second count is only as good as the labelling: a field the annotators did
//! not mark counts as absent from the document, which is a limitation of truth files, not of the
//! engine. Currencies are not scored in v0.

use wanjiru_docir::{Decision, Document};
use wanjiru_types::{ErrorKind, Result, WnjError, codes};

use crate::contract::FieldGate;
use crate::corpus::Case;
use crate::validators::validate;

/// Counts for one gated field over a corpus.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Truth documents that carry the field.
    pub trials: u64,
    /// Trials in which the engine accepted the field.
    pub accepted: u64,
    /// Accepted trials whose kind and value equal the truth.
    pub correct: u64,
    /// Trials in which the engine sent the field to review.
    pub reviewed: u64,
    /// Trials without a document or without the field in the engine's output.
    pub absent: u64,
    /// Accepted values, in any document, that break the syntax of the kind.
    pub accepted_invalid: u64,
    /// Accepted values in documents whose truth lacks the field.
    pub accepted_without_truth: u64,
}

fn find<'a>(document: Option<&'a Document>, name: &str) -> Option<&'a wanjiru_docir::Field> {
    document.and_then(|d| d.fields.iter().find(|f| f.name == name))
}

/// Count the outcomes of `gate`'s field over `cases`.
///
/// # Errors
///
/// `WNJ-E4006` if a truth value breaks the syntax of the field's kind: a wrong label silently lowers
/// every score, so the corpus is refused instead.
pub fn tally(gate: &FieldGate, cases: &[Case<'_>]) -> Result<Tally> {
    let mut t = Tally::default();
    for case in cases {
        let truth = case.truth.fields.get(&gate.name);
        if let Some(value) = truth {
            if let Err(reason) = validate(gate.kind, value) {
                return Err(WnjError::new(
                    codes::CORPUS_RECORD_INVALID,
                    ErrorKind::InvalidInput,
                    format!(
                        "the truth value of field `{}` is not a valid {}: {reason}",
                        gate.name,
                        gate.kind.as_str()
                    ),
                )
                .at(format!("document {}", case.truth.id)));
            }
            t.trials += 1;
        }
        let predicted = find(case.prediction, &gate.name);
        let accepted = predicted.filter(|f| f.decision == Decision::Accept);
        if let Some(field) = accepted {
            if validate(gate.kind, &field.value).is_err() {
                t.accepted_invalid += 1;
            }
        }
        match (truth, predicted, accepted) {
            (Some(_), None, _) => t.absent += 1,
            (Some(_), Some(_), None) => t.reviewed += 1,
            (Some(expected), Some(_), Some(field)) => {
                t.accepted += 1;
                if field.kind == gate.kind && field.value == *expected {
                    t.correct += 1;
                }
            }
            (None, _, Some(_)) => t.accepted_without_truth += 1,
            (None, _, None) => {}
        }
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Contract;
    use crate::corpus::{Corpus, testing};
    use wanjiru_docir::{DocClass, FieldKind};

    fn gate(name: &str, kind: FieldKind) -> FieldGate {
        FieldGate {
            name: name.to_owned(),
            kind,
            min: 0.9,
            coverage: 0.9,
        }
    }

    fn corpus(truths: &[(&str, &[(&str, &str)])], predictions: &[String]) -> Corpus {
        let truth: Vec<String> = truths
            .iter()
            .map(|(id, fields)| testing::truth(id, "invoice", None, fields))
            .collect();
        Corpus::parse(&truth.join("\n"), &predictions.join("\n"), None).expect("valid corpus")
    }

    fn doc(id: &str, fields: &[testing::FieldSpec<'_>]) -> String {
        testing::document(id, "invoice", &["x"], fields, None)
    }

    #[test]
    fn every_outcome_lands_in_exactly_one_bucket() {
        let truths: &[(&str, &[(&str, &str)])] = &[
            ("d1", &[("total", "10.00")]), // accepted, correct
            ("d2", &[("total", "10.00")]), // accepted, wrong value
            ("d3", &[("total", "10.00")]), // sent to review
            ("d4", &[("total", "10.00")]), // field missing in the output
            ("d5", &[("total", "10.00")]), // no output document at all
            ("d6", &[]),                   // accepted although the truth lacks it
            ("d7", &[("total", "10.00")]), // accepted, right value, wrong kind
            ("d8", &[]),                   // nothing in truth, nothing predicted
        ];
        let predictions = [
            doc("d1", &[("total", "amount", "10.00", true)]),
            doc("d2", &[("total", "amount", "10.01", true)]),
            doc("d3", &[("total", "amount", "10.00", false)]),
            doc("d4", &[("other", "text", "z", true)]),
            doc("d6", &[("total", "amount", "5.00", true)]),
            doc("d7", &[("total", "text", "10.00", true)]),
            doc("d8", &[]),
        ];
        let corpus = corpus(truths, &predictions);
        let cases = corpus.cases(DocClass::Invoice).expect("cases");
        let t = tally(&gate("total", FieldKind::Amount), &cases).expect("tally");
        assert_eq!(
            t,
            Tally {
                trials: 6,
                accepted: 3,
                correct: 1,
                reviewed: 1,
                absent: 2,
                accepted_invalid: 0,
                accepted_without_truth: 1,
            }
        );
        // The buckets of the trials add up.
        assert_eq!(t.accepted + t.reviewed + t.absent, t.trials);
    }

    #[test]
    fn accepted_values_that_break_the_syntax_are_counted() {
        let truths: &[(&str, &[(&str, &str)])] =
            &[("d1", &[("total", "10.00")]), ("d2", &[("total", "11.00")])];
        let predictions = [
            doc("d1", &[("total", "amount", "10,00", true)]),
            doc("d2", &[("total", "amount", "11.00", true)]),
        ];
        let corpus = corpus(truths, &predictions);
        let cases = corpus.cases(DocClass::Invoice).expect("cases");
        let t = tally(&gate("total", FieldKind::Amount), &cases).expect("tally");
        assert_eq!((t.accepted, t.correct, t.accepted_invalid), (2, 1, 1));
    }

    #[test]
    fn a_malformed_truth_value_refuses_the_corpus() {
        let truths: &[(&str, &[(&str, &str)])] = &[("d1", &[("iban", "DE89370400440532013001")])];
        let corpus = corpus(truths, &[]);
        let cases = corpus.cases(DocClass::Invoice).expect("cases");
        let e = tally(&gate("iban", FieldKind::Iban), &cases).expect_err("bad label");
        assert_eq!(e.code(), codes::CORPUS_RECORD_INVALID);
        assert_eq!(e.path(), Some("document d1"));
        assert!(e.message().contains("mod 97"), "{e}");
    }

    #[test]
    fn contracts_and_tallies_agree_on_names() {
        let c = Contract::from_yaml(
            "contract: t\nclass: invoice\ncorpus: c\nmetrics:\n  field_exact_match:\n    date: {min: 0.9, coverage: 0.9}\ngates: {}\n",
        )
        .expect("contract");
        let truths: &[(&str, &[(&str, &str)])] = &[("d1", &[("date", "2026-02-29")])];
        let corpus = corpus(truths, &[]);
        let cases = corpus.cases(c.class).expect("cases");
        assert!(
            tally(&c.fields[0], &cases).is_err(),
            "2026 is not a leap year"
        );
    }
}
