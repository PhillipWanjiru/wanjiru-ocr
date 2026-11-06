//! The `unsafe` policy (ADR-007).
//!
//! Outside the unsafe zones a crate forbids `unsafe` at its root and uses none. Inside a zone every
//! `unsafe` block, impl and attribute has a `// SAFETY:` comment directly above it that says why the
//! obligation holds, and every `unsafe fn` and `unsafe trait` has a `# Safety` section in its
//! documentation that says what the caller must guarantee. The compiler checks that `unsafe` is
//! allowed where it is used; this check keeps the explanations from being skipped.

use crate::layers::is_unsafe_zone;
use crate::lexer::{Line, scan};
use crate::repo::{CrateInfo, Repository, SourceFile};
use crate::report::Violation;

const CHECK: &str = "unsafe";

/// What follows an `unsafe` keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Use {
    /// `unsafe { ... }`
    Block,
    /// `unsafe impl`
    Impl,
    /// `unsafe extern "C" { ... }` and `#[unsafe(no_mangle)]`
    Declaration,
    /// `unsafe fn name` and `unsafe trait`
    Item,
    /// `unsafe fn(...)` and `unsafe extern "C" fn(...)`: a type, which makes no promise by itself.
    PointerType,
    /// Anything else.
    Other,
}

struct Token {
    line: usize,
    text: String,
}

fn tokens(lines: &[Line]) -> Vec<Token> {
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let mut word = String::new();
        let flush = |word: &mut String, out: &mut Vec<Token>| {
            if !word.is_empty() {
                out.push(Token {
                    line: index,
                    text: std::mem::take(word),
                });
            }
        };
        for c in line.code.chars() {
            if c.is_alphanumeric() || c == '_' {
                word.push(c);
            } else {
                flush(&mut word, &mut out);
                if !c.is_whitespace() {
                    out.push(Token {
                        line: index,
                        text: c.to_string(),
                    });
                }
            }
        }
        flush(&mut word, &mut out);
    }
    out
}

fn classify(tokens: &[Token], at: usize) -> Use {
    let text = |i: usize| tokens.get(i).map_or("", |t| t.text.as_str());
    let mut i = at + 1;
    match text(i) {
        "{" => Use::Block,
        "impl" => Use::Impl,
        "trait" => Use::Item,
        "(" => Use::Declaration,
        "fn" => {
            if text(i + 1) == "(" {
                Use::PointerType
            } else {
                Use::Item
            }
        }
        "extern" => {
            i += 1;
            while text(i) == "\"" {
                i += 1;
            }
            match text(i) {
                "fn" if text(i + 1) == "(" => Use::PointerType,
                "fn" => Use::Item,
                "{" => Use::Declaration,
                _ => Use::Other,
            }
        }
        _ => Use::Other,
    }
}

/// The comment text directly above `line`: comment-only lines, and attribute lines when `through_attributes`.
fn comment_above(lines: &[Line], line: usize, through_attributes: bool) -> String {
    let mut text = String::new();
    let mut i = line;
    while i > 0 {
        i -= 1;
        let above = &lines[i];
        if above.is_comment_only() || (through_attributes && above.is_attribute()) {
            text.push_str(&above.comment);
            text.push('\n');
        } else {
            break;
        }
    }
    text
}

fn has_safety_comment(lines: &[Line], line: usize) -> bool {
    lines[line].comment.contains("SAFETY:") || comment_above(lines, line, false).contains("SAFETY:")
}

fn has_safety_doc(lines: &[Line], line: usize) -> bool {
    comment_above(lines, line, true).contains("# Safety")
}

/// Whether `code` contains `word` as a whole identifier, at the positions returned.
fn find_word(code: &str, word: &str) -> Vec<usize> {
    let bytes = code.as_bytes();
    let is_identifier = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    code.match_indices(word)
        .map(|(i, _)| i)
        .filter(|&i| {
            (i == 0 || !is_identifier(bytes[i - 1]))
                && bytes.get(i + word.len()).is_none_or(|&b| !is_identifier(b))
        })
        .collect()
}

/// The uses of `unsafe` in one file that break the policy of a crate that is (or is not) a zone.
pub fn check_source(file: &SourceFile, zone: bool) -> Vec<Violation> {
    let lines = scan(&file.text);
    let toks = tokens(&lines);
    let mut out = Vec::new();
    for (index, token) in toks.iter().enumerate() {
        if token.text != "unsafe" {
            continue;
        }
        let location = format!("{}:{}", file.path, token.line + 1);
        if !zone {
            out.push(Violation::new(
                CHECK,
                location,
                "`unsafe` outside an unsafe zone; this crate forbids unsafe code (the zones are listed in tools/xtask/src/layers.rs and ADR-007)",
            ));
            continue;
        }
        match classify(&toks, index) {
            Use::PointerType => {}
            Use::Block | Use::Impl | Use::Declaration => {
                if !has_safety_comment(&lines, token.line) {
                    out.push(Violation::new(CHECK, location, "this `unsafe` has no `// SAFETY:` comment directly above it that says why it is sound"));
                }
            }
            Use::Item => {
                if !has_safety_doc(&lines, token.line) {
                    out.push(Violation::new(CHECK, location, "this `unsafe fn` or `unsafe trait` has no `# Safety` section in its documentation saying what the caller must guarantee"));
                }
            }
            Use::Other => out.push(Violation::new(CHECK, location, "`unsafe` in a position this check does not understand; use a block, fn, impl or trait")),
        }
    }
    if !zone {
        // Allowing the lint is as much a breach as using the keyword.
        for (number, line) in lines.iter().enumerate() {
            let squeezed: String = line.code.chars().filter(|c| !c.is_whitespace()).collect();
            if (squeezed.contains("allow(") || squeezed.contains("expect("))
                && !find_word(&squeezed, "unsafe_code").is_empty()
            {
                out.push(Violation::new(
                    CHECK,
                    format!("{}:{}", file.path, number + 1),
                    "`allow(unsafe_code)` outside an unsafe zone",
                ));
            }
        }
    }
    out
}

/// The files that start a crate: `src/lib.rs`, `src/main.rs` and `src/bin/*.rs`.
fn is_root(krate: &CrateInfo, file: &SourceFile) -> bool {
    let base = format!("crates/{}/src/", krate.dir);
    file.path.strip_prefix(&base).is_some_and(|rest| {
        rest == "lib.rs"
            || rest == "main.rs"
            || (rest.starts_with("bin/") && rest.matches('/').count() == 1)
    })
}

fn has_forbid(file: &SourceFile) -> bool {
    scan(&file.text).iter().any(|line| {
        let squeezed: String = line.code.chars().filter(|c| !c.is_whitespace()).collect();
        squeezed.contains("#![forbid(unsafe_code)]")
    })
}

/// Check every crate.
pub fn check(repo: &Repository) -> Vec<Violation> {
    let mut out = Vec::new();
    for krate in &repo.crates {
        let zone = is_unsafe_zone(&krate.dir);
        for file in &krate.sources {
            out.extend(check_source(file, zone));
            if !zone && is_root(krate, file) && !has_forbid(file) {
                out.push(Violation::new(CHECK, file.path.clone(), "the root of a crate outside the unsafe zones must say `#![forbid(unsafe_code)]`"));
            }
        }
        if !zone && !krate.sources.iter().any(|f| is_root(krate, f)) {
            out.push(Violation::new(
                CHECK,
                format!("crates/{}", krate.dir),
                "no src/lib.rs or src/main.rs found",
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::testing::krate;

    fn file(text: &str) -> SourceFile {
        SourceFile {
            path: "crates/x/src/lib.rs".to_owned(),
            text: text.to_owned(),
        }
    }

    fn zone(text: &str) -> Vec<String> {
        check_source(&file(text), true)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn plain(text: &str) -> Vec<String> {
        check_source(&file(text), false)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn a_documented_block_in_a_zone_passes() {
        let ok = "fn f(p: *const u8) -> u8 {\n    // SAFETY: the caller of `f` passes a pointer to a live byte.\n    unsafe { *p }\n}\n";
        assert_eq!(zone(ok), Vec::<String>::new());
        let same_line = "let a = unsafe { *p }; // SAFETY: p is valid\n";
        assert_eq!(zone(same_line), Vec::<String>::new());
        let above_the_statement = "// SAFETY: p is valid.\nlet a = unsafe { *p };\n";
        assert_eq!(zone(above_the_statement), Vec::<String>::new());
    }

    #[test]
    fn an_undocumented_block_is_reported_with_its_line() {
        let found = zone("fn f() {\n    let a = 1;\n    unsafe { g() }\n}\n");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].starts_with("unsafe: crates/x/src/lib.rs:3:"),
            "{found:?}"
        );
        assert!(found[0].contains("SAFETY"), "{found:?}");
    }

    #[test]
    fn a_comment_separated_by_a_blank_line_does_not_count() {
        assert_eq!(zone("// SAFETY: fine\n\nunsafe { g() }\n").len(), 1);
        assert_eq!(
            zone("// SAFETY: fine\nlet y = 1;\nunsafe { g() }\n").len(),
            1
        );
    }

    #[test]
    fn unsafe_fn_and_trait_need_a_safety_section_in_their_docs() {
        let ok = "/// Reads one byte.\n///\n/// # Safety\n///\n/// `p` must point to a live byte.\n#[inline]\npub unsafe fn read(p: *const u8) -> u8 { 0 }\n";
        assert_eq!(zone(ok), Vec::<String>::new());
        let missing = "/// Reads one byte.\npub unsafe fn read(p: *const u8) -> u8 { 0 }\n";
        assert!(zone(missing)[0].contains("# Safety"));
        let a_trait = "/// A promise.\npub unsafe trait Zeroable {}\n";
        assert!(zone(a_trait)[0].contains("# Safety"));
        // a SAFETY comment is for obligations met, not for obligations imposed
        assert_eq!(zone("// SAFETY: fine\npub unsafe fn read() {}\n").len(), 1);
    }

    #[test]
    fn impl_and_extern_blocks_and_unsafe_attributes_need_a_safety_comment() {
        assert_eq!(zone("unsafe impl Send for T {}\n").len(), 1);
        assert_eq!(
            zone("// SAFETY: T has no thread affinity.\nunsafe impl Send for T {}\n").len(),
            0
        );
        assert_eq!(zone("unsafe extern \"C\" {\n    fn f();\n}\n").len(), 1);
        assert_eq!(zone("// SAFETY: the symbol is declared in include/wanjiru.h.\nunsafe extern \"C\" {\n    fn f();\n}\n").len(), 0);
        assert_eq!(
            zone("#[unsafe(no_mangle)]\npub extern \"C\" fn f() {}\n").len(),
            1
        );
        assert_eq!(
            zone(
                "// SAFETY: the name is unique.\n#[unsafe(no_mangle)]\npub extern \"C\" fn f() {}\n"
            )
            .len(),
            0
        );
    }

    #[test]
    fn function_pointer_types_are_not_promises() {
        let source = "type Callback = unsafe extern \"C\" fn(*mut u8);\ntype Plain = unsafe fn(u8) -> u8;\nstruct S { f: unsafe fn() }\n";
        assert_eq!(zone(source), Vec::<String>::new());
    }

    #[test]
    fn the_word_in_comments_strings_and_longer_identifiers_is_not_the_keyword() {
        let source = "// unsafe { } is not allowed here\n/* unsafe */\nlet s = \"unsafe { }\";\nlet unsafe_code = 1;\nlet t = r#\"unsafe\"#;\n#![forbid(unsafe_code)]\n";
        assert_eq!(plain(source), Vec::<String>::new());
    }

    #[test]
    fn outside_a_zone_any_unsafe_is_reported_even_when_documented() {
        let found = plain("// SAFETY: sure\nunsafe { g() }\n");
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("outside an unsafe zone"));
    }

    #[test]
    fn outside_a_zone_allowing_the_lint_is_reported() {
        assert_eq!(plain("#![allow(unsafe_code)]\n").len(), 1);
        assert_eq!(plain("#[allow( unsafe_code )]\nmod m {}\n").len(), 1);
        assert_eq!(
            plain("#[expect(unsafe_code, reason = \"x\")]\nmod m {}\n").len(),
            1
        );
        assert_eq!(
            plain("#![forbid(unsafe_code)]\n#![deny(unsafe_code)]\n").len(),
            0
        );
        // in a zone the lint may be allowed, per module
        assert_eq!(zone("#![allow(unsafe_code)]\n"), Vec::<String>::new());
    }

    #[test]
    fn the_root_of_a_plain_crate_must_forbid_unsafe_code() {
        let with = krate(
            "wanjiru-docir",
            &[],
            None,
            &[
                ("src/lib.rs", "#![forbid(unsafe_code)]\npub fn f() {}\n"),
                ("src/other.rs", "pub fn g() {}\n"),
            ],
        );
        let without = krate(
            "wanjiru-docir",
            &[],
            None,
            &[("src/lib.rs", "pub fn f() {}\n")],
        );
        let in_comment = krate(
            "wanjiru-docir",
            &[],
            None,
            &[("src/lib.rs", "// #![forbid(unsafe_code)]\n")],
        );
        let repo = |c| Repository {
            crates: vec![c],
            ..Repository::default()
        };
        assert_eq!(check(&repo(with)), Vec::new());
        assert_eq!(check(&repo(without)).len(), 1);
        assert_eq!(check(&repo(in_comment)).len(), 1);
    }

    #[test]
    fn binaries_and_a_missing_root_are_handled() {
        let bin = krate(
            "wanjiru-cli",
            &[],
            None,
            &[
                ("src/main.rs", "#![forbid(unsafe_code)]\nfn main() {}\n"),
                ("src/bin/extra.rs", "fn main() {}\n"),
            ],
        );
        let repo = Repository {
            crates: vec![bin],
            ..Repository::default()
        };
        let found = check(&repo);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].location.ends_with("src/bin/extra.rs"));
        let none = krate("wanjiru-docir", &[], None, &[]);
        assert!(
            check(&Repository {
                crates: vec![none],
                ..Repository::default()
            })[0]
                .message
                .contains("no src/lib.rs")
        );
    }

    #[test]
    fn zone_crates_need_no_forbid_and_are_checked_for_their_unsafe_code() {
        let simd = krate(
            "wanjiru-simd",
            &[],
            None,
            &[
                ("src/lib.rs", "#![deny(unsafe_code)]\nmod k;\n"),
                (
                    "src/k.rs",
                    "#![allow(unsafe_code)]\nfn f() { unsafe { g() } }\n",
                ),
            ],
        );
        let found = check(&Repository {
            crates: vec![simd],
            ..Repository::default()
        });
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].location.ends_with("src/k.rs:2"), "{found:?}");
    }
}
