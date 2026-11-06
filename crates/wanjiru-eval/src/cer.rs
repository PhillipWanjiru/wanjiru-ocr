//! Character error rate: edit distance over Unicode scalar values, summed over documents.
//!
//! The reference of a document is the `text` of its truth record: its lines joined by `\n`. The
//! hypothesis is the recognised text of all lines of all pages, in reading order, joined the same
//! way. No normalisation is applied (the corpus is expected to be in NFC): a character that is
//! recognised as a look-alike is an error, which is what a person who searches the text would say.
//! A document the engine produced no output for has an empty hypothesis, so every reference
//! character counts as an error; it is never dropped from the score.

use wanjiru_docir::Document;
use wanjiru_math::edit_distance::levenshtein_chars;

/// The recognised text of a document: all lines of all pages, joined by `\n`.
#[must_use]
pub fn hypothesis_text(document: &Document) -> String {
    let lines: Vec<&str> = document
        .pages
        .iter()
        .flat_map(|page| page.lines.iter().map(|line| line.text.as_str()))
        .collect();
    lines.join("\n")
}

/// Errors of one document.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Errors {
    /// Length of the reference in Unicode scalar values.
    pub reference_len: u64,
    /// Levenshtein distance between reference and hypothesis.
    pub edits: u64,
}

/// Score `document` (or its absence) against `reference`.
#[must_use]
pub fn score(reference: &str, document: Option<&Document>) -> Errors {
    let reference_len = reference.chars().count() as u64;
    let edits = match document {
        Some(document) => levenshtein_chars(reference, &hypothesis_text(document)) as u64,
        None => reference_len,
    };
    Errors {
        reference_len,
        edits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::testing::document;

    fn parsed(lines: &[&str]) -> Document {
        Document::from_json_str(&document("d", "invoice", lines, &[], None)).expect("valid")
    }

    #[test]
    fn lines_are_joined_by_newlines() {
        assert_eq!(hypothesis_text(&parsed(&["a", "b c", ""])), "a\nb c\n");
        assert_eq!(hypothesis_text(&parsed(&[])), "");
    }

    #[test]
    fn identical_text_has_no_errors_and_a_missing_document_has_all() {
        let d = parsed(&["Total 12,50 €", "Danke"]);
        assert_eq!(
            score("Total 12,50 €\nDanke", Some(&d)),
            Errors {
                reference_len: 19,
                edits: 0
            }
        );
        assert_eq!(
            score("Total 12,50 €\nDanke", None),
            Errors {
                reference_len: 19,
                edits: 19
            }
        );
    }

    #[test]
    fn distance_counts_scalar_values_not_bytes() {
        let d = parsed(&["Größe"]);
        assert_eq!(score("Größe", Some(&d)).edits, 0);
        // `o` against `ö` is one substitution (two on UTF-8 bytes).
        assert_eq!(
            score("Große", Some(&d)),
            Errors {
                reference_len: 5,
                edits: 1
            }
        );
        // `o`/`ö`, `s`/`ß` and one surplus `s`.
        assert_eq!(
            score("Grosse", Some(&d)),
            Errors {
                reference_len: 6,
                edits: 3
            }
        );
    }

    #[test]
    fn an_empty_reference_scores_the_insertions() {
        let d = parsed(&["abc"]);
        assert_eq!(
            score("", Some(&d)),
            Errors {
                reference_len: 0,
                edits: 3
            }
        );
    }
}
