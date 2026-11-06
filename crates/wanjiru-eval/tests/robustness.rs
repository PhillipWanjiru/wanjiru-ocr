//! No input, however mangled, may make a reader panic, hang or overflow the stack.
//!
//! The readers of this crate parse files that a person chooses: quality contracts, truth files, prediction
//! files. A malformed file has to be an error with a code and a line, never a crash. This test takes the real
//! files of the repository and mangles them with a seeded random stream (deletions, insertions of characters
//! that mean something to a reader, truncation, duplication of a stretch) and asserts one thing: the readers
//! return. It is the cheap cousin of a fuzzer: deterministic, run on every `cargo test`, and not a
//! substitute for coverage-guided fuzzing, which arrives with the parsers that need it (`fuzz/`, Phase 2).
//!
//! It exists because one crafted contract once overflowed the stack of the YAML reader
//! (`docs/lab/2026-10-29.md`, A1), and every earlier test had used input that a person would write.

use std::fs;
use std::path::{Path, PathBuf};

use wanjiru_eval::run::{Inputs, evaluate};
use wanjiru_eval::{intervals::Resampling, yaml};
use wanjiru_math::rng::Rng;

/// Characters that mean something to a reader, and some that are awkward for text handling.
const ALPHABET: [char; 40] = [
    '[',
    ']',
    '{',
    '}',
    ':',
    ',',
    '#',
    '-',
    '?',
    '&',
    '*',
    '!',
    '|',
    '>',
    '\'',
    '"',
    '%',
    '@',
    '`',
    '\t',
    '\n',
    '\r',
    ' ',
    '\\',
    '0',
    '9',
    '.',
    'e',
    '+',
    'a',
    'é',
    '€',
    '😀',
    '\u{0}',
    '\u{85}',
    '\u{2028}',
    '\u{feff}',
    '\u{7ff}',
    '\u{ffff}',
    '\u{10ffff}',
];

fn repository(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read(path: &str) -> String {
    let path = repository(path);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Between one and four random changes to `text`, made on characters so that the result is valid UTF-8.
fn mangle(rng: &mut Rng, text: &str) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    for _ in 0..=rng.below_usize(4) {
        let length = chars.len();
        match rng.below_usize(6) {
            0 if length > 0 => {
                chars.remove(rng.below_usize(length));
            }
            1 => {
                let at = rng.below_usize(length + 1);
                chars.insert(at, ALPHABET[rng.below_usize(ALPHABET.len())]);
            }
            2 if length > 0 => {
                let from = rng.below_usize(length);
                let to = (from + 1 + rng.below_usize(60)).min(length);
                chars.drain(from..to);
            }
            3 if length > 0 => chars.truncate(rng.below_usize(length)),
            4 if length > 0 => {
                let from = rng.below_usize(length);
                let to = (from + 1 + rng.below_usize(80)).min(length);
                let piece: Vec<char> = chars[from..to].to_vec();
                let at = rng.below_usize(length + 1);
                chars.splice(at..at, piece);
            }
            _ if length > 0 => {
                let at = rng.below_usize(length);
                chars[at] = ALPHABET[rng.below_usize(ALPHABET.len())];
            }
            _ => {}
        }
    }
    chars.into_iter().collect()
}

#[test]
fn the_yaml_reader_returns_on_mangled_files() {
    let files = [
        "eval/contracts/invoice-fields-v1.yaml",
        "eval/contracts/receipt-fields-v1.yaml",
        "eval/contracts/id-mrz-v1.yaml",
        "eval/fixtures/synthetic-v0/demo-lenient-invoice.yaml",
        "data/registry/licences.yaml",
        "data/registry/datasets.yaml",
    ];
    let mut rng = Rng::seed_from_u64(20_261_105);
    let (mut accepted, mut refused) = (0_u32, 0_u32);
    for file in files {
        let original = read(file);
        for _ in 0..4_000 {
            match yaml::parse(&mangle(&mut rng, &original)) {
                Ok(_) => accepted += 1,
                Err(_) => refused += 1,
            }
        }
    }
    // A mangler that only ever produced garbage, or only ever left the text intact, would prove little.
    assert!(
        accepted > 100,
        "only {accepted} mangled files were still valid"
    );
    assert!(refused > 100, "only {refused} mangled files were refused");
}

#[test]
fn evaluation_returns_on_mangled_inputs() {
    let first_lines = |name: &str, n: usize| -> String {
        let text = read(&format!("eval/fixtures/synthetic-v0/{name}"));
        let mut kept = String::new();
        for line in text.lines().take(n) {
            kept.push_str(line);
            kept.push('\n');
        }
        kept
    };
    let contract = read("eval/contracts/invoice-fields-v1.yaml");
    let truth = first_lines("truth.jsonl", 14);
    let predictions = first_lines("predictions.jsonl", 14);
    let baseline = first_lines("baseline.jsonl", 14);
    // 100 is the fewest resamples the harness accepts; fewer is refused before any input is read, which
    // would make this test pass without testing anything.
    let settings = Resampling {
        resamples: 100,
        ..Resampling::DEFAULT
    };
    let mut rng = Rng::seed_from_u64(20_261_106);
    let (mut reports, mut errors) = (0_u32, 0_u32);
    for round in 0..1_500_u32 {
        let (mut c, mut t, mut p, mut b) = (
            contract.clone(),
            truth.clone(),
            predictions.clone(),
            baseline.clone(),
        );
        match round % 4 {
            0 => c = mangle(&mut rng, &c),
            1 => t = mangle(&mut rng, &t),
            2 => p = mangle(&mut rng, &p),
            _ => b = mangle(&mut rng, &b),
        }
        // The result, report or error, does not matter. Returning does.
        let outcome = evaluate(
            &Inputs {
                contract: &c,
                truth: &t,
                predictions: &p,
                baseline: Some(&b),
            },
            &settings,
        );
        if outcome.is_ok() {
            reports += 1;
        } else {
            errors += 1;
        }
    }
    // The mangling has to reach the evaluation itself, and has to break things too.
    assert!(
        reports > 100,
        "only {reports} mangled runs produced a report"
    );
    assert!(errors > 100, "only {errors} mangled runs were refused");
}
