//! No input, however mangled, may make the JSON reader, the validator or the canonical writer panic.
//!
//! DocIR files come from other programs and, one day, from other people. A malformed document has to be an error
//! with a code and a position, never a crash. This test takes real documents (the 300 predictions of the
//! synthetic fixtures), mangles them with a seeded random stream, and asserts that the reader, the schema
//! validator and the canonical writer all return. It is deterministic and runs on every `cargo test`; it does not
//! replace coverage-guided fuzzing, which arrives with the image and PDF parsers (`fuzz/`, Phase 2).

use std::fs;
use std::path::Path;

use wanjiru_docir::{parse, parse_bytes, to_canonical_string, validate};
use wanjiru_math::rng::Rng;

const ALPHABET: [char; 36] = [
    '[',
    ']',
    '{',
    '}',
    ':',
    ',',
    '"',
    '\\',
    '/',
    '-',
    '+',
    '.',
    'e',
    'E',
    '0',
    '1',
    '9',
    't',
    'n',
    'u',
    ' ',
    '\t',
    '\n',
    '\r',
    'é',
    '€',
    '😀',
    '\u{0}',
    '\u{1f}',
    '\u{7f}',
    '\u{85}',
    '\u{2028}',
    '\u{feff}',
    '\u{ffff}',
    '\u{10ffff}',
    'x',
];

fn documents() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../eval/fixtures/synthetic-v0/predictions.jsonl");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines().step_by(7).map(str::to_owned).collect()
}

fn mangle(rng: &mut Rng, text: &str) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    for _ in 0..=rng.below_usize(3) {
        let length = chars.len();
        match rng.below_usize(5) {
            0 if length > 0 => {
                chars.remove(rng.below_usize(length));
            }
            1 => {
                let at = rng.below_usize(length + 1);
                chars.insert(at, ALPHABET[rng.below_usize(ALPHABET.len())]);
            }
            2 if length > 0 => {
                let from = rng.below_usize(length);
                let to = (from + 1 + rng.below_usize(40)).min(length);
                chars.drain(from..to);
            }
            3 if length > 0 => chars.truncate(rng.below_usize(length)),
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
fn the_reader_the_validator_and_the_writer_return_on_mangled_documents() {
    let documents = documents();
    assert!(documents.len() >= 40, "{} documents", documents.len());
    let mut rng = Rng::seed_from_u64(20_261_107);
    let (mut parsed, mut validated, mut refused) = (0_u32, 0_u32, 0_u32);
    for document in &documents {
        for _ in 0..500 {
            let mangled = mangle(&mut rng, document);
            match parse(&mangled) {
                Ok(value) => {
                    parsed += 1;
                    if validate(&value).is_ok() {
                        validated += 1;
                    }
                    // Whatever parsed can be written canonically, or is refused with a code (a number that
                    // cannot be represented, say).
                    let _ = to_canonical_string(&value);
                }
                Err(_) => refused += 1,
            }
        }
    }
    assert!(
        refused > 1_000,
        "only {refused} mangled documents were refused"
    );
    assert!(parsed > 100, "only {parsed} mangled documents still parsed");
    assert!(
        validated > 0,
        "no mangled document was still valid: the mangler is too strong to test the writer"
    );
}

#[test]
fn the_byte_reader_returns_on_arbitrary_bytes() {
    let documents = documents();
    let mut rng = Rng::seed_from_u64(20_261_108);
    for document in documents.iter().take(20) {
        let original = document.as_bytes();
        for _ in 0..500 {
            let mut bytes = original.to_vec();
            for _ in 0..=rng.below_usize(4) {
                if bytes.is_empty() {
                    break;
                }
                let at = rng.below_usize(bytes.len());
                match rng.below_usize(3) {
                    0 => bytes[at] = u8::try_from(rng.below_usize(256)).unwrap_or(0),
                    1 => {
                        bytes.remove(at);
                    }
                    _ => bytes.insert(at, u8::try_from(rng.below_usize(256)).unwrap_or(0)),
                }
            }
            // invalid UTF-8, overlong forms, lone surrogates and stray continuation bytes included
            let _ = parse_bytes(&bytes);
        }
    }
}
