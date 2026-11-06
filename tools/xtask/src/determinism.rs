//! No platform mathematics in the crates whose output must be bit-identical everywhere (ADR-005).
//!
//! `f64::exp`, `ln`, `powf`, `sin` and their relatives call the platform's maths library, whose last
//! bit differs between systems and versions. The crates below promise the same bytes on every
//! machine, so they may use only operations that IEEE 754 defines exactly (`+ - * /`, `sqrt`,
//! `floor`, `ceil`, `abs`, comparisons) and their own implementations of the rest. Code under
//! `#[cfg(test)]` is exempt: a test may compare against the platform function on purpose.
//!
//! A deliberate use carries `// determinism: allow <reason>` on its line.

use crate::lexer::{Line, scan};
use crate::repo::{CrateInfo, Repository, SourceFile};
use crate::report::Violation;

const CHECK: &str = "determinism";

/// The crates that promise bit-identical results on every platform.
pub const DETERMINISTIC_CRATES: &[&str] = &["wanjiru-math", "wanjiru-docir", "wanjiru-eval"];

/// Methods and associated functions that reach the platform's maths library, or that the compiler may
/// lower to it.
const BANNED: &[&str] = &[
    "ln",
    "log",
    "log2",
    "log10",
    "ln_1p",
    "exp",
    "exp2",
    "exp_m1",
    "powf",
    "powi",
    "sin",
    "cos",
    "tan",
    "sin_cos",
    "asin",
    "acos",
    "atan",
    "atan2",
    "sinh",
    "cosh",
    "tanh",
    "asinh",
    "acosh",
    "atanh",
    "cbrt",
    "hypot",
    "to_degrees",
    "to_radians",
    "gamma",
    "ln_gamma",
    "erf",
    "erfc",
];

/// The lines of `lines` that are not inside an item marked `#[cfg(test)]`.
fn non_test_lines(lines: &[Line]) -> Vec<usize> {
    let mut keep = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let squeezed: String = lines[i]
            .code
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if squeezed.starts_with("#[cfg(test)]") {
            // Skip the attribute and the item it governs: up to the matching brace, or to a `;`.
            let mut depth = 0_i32;
            let mut opened = false;
            let mut j = i + 1;
            'item: while j < lines.len() {
                for c in lines[j].code.chars() {
                    match c {
                        '{' => {
                            depth += 1;
                            opened = true;
                        }
                        '}' => depth -= 1,
                        ';' if !opened => break 'item,
                        _ => {}
                    }
                    if opened && depth == 0 {
                        break 'item;
                    }
                }
                j += 1;
            }
            i = j + 1;
        } else {
            keep.push(i);
            i += 1;
        }
    }
    keep
}

/// The uses of platform mathematics in one source file.
pub fn check_source(file: &SourceFile) -> Vec<Violation> {
    let lines = scan(&file.text);
    let mut out = Vec::new();
    for index in non_test_lines(&lines) {
        let line = &lines[index];
        if line.comment.contains("determinism: allow") {
            continue;
        }
        let code = &line.code;
        for name in BANNED {
            let method = format!(".{name}(");
            let path = [format!("f64::{name}("), format!("f32::{name}(")];
            let hit = code.contains(&method)
                || path.iter().any(|p| code.contains(p.as_str()))
                // a turbofish or a line break between the dot and the name is not used in this repository
                || code.contains(&format!(".{name}::<"));
            if hit {
                out.push(Violation::new(
                    CHECK,
                    format!("{}:{}", file.path, index + 1),
                    format!("`{name}` reaches the platform's maths library, whose last bit differs between systems; use an own implementation (see wanjiru-math::special) or mark the line `// determinism: allow <reason>`"),
                ));
            }
        }
    }
    out
}

fn is_checked(krate: &CrateInfo, file: &SourceFile) -> bool {
    DETERMINISTIC_CRATES.contains(&krate.dir.as_str())
        && file.path.starts_with(&format!("crates/{}/src/", krate.dir))
}

/// Check the source of every deterministic crate.
pub fn check(repo: &Repository) -> Vec<Violation> {
    repo.crates
        .iter()
        .flat_map(|krate| {
            krate
                .sources
                .iter()
                .filter(|file| is_checked(krate, file))
                .flat_map(check_source)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::testing::krate;

    fn found(text: &str) -> Vec<String> {
        check_source(&SourceFile {
            path: "crates/wanjiru-math/src/x.rs".to_owned(),
            text: text.to_owned(),
        })
        .iter()
        .map(ToString::to_string)
        .collect()
    }

    #[test]
    fn platform_functions_are_reported_with_their_line() {
        let f = found(
            "fn a(x: f64) -> f64 {\n    x.exp() + f64::ln(x) + x.powi(2) + x.sin_cos().0\n}\n",
        );
        assert_eq!(f.len(), 4, "{f:#?}");
        assert!(f.iter().all(|m| m.contains("x.rs:2:")), "{f:#?}");
    }

    #[test]
    fn exact_operations_and_own_functions_are_fine() {
        assert_eq!(
            found(
                "fn a(x: f64) -> f64 { x.sqrt() + x.floor() + x.abs() + x.max(1.0) + exp(x) + special::exp(x) + x.fract() }\n"
            ),
            Vec::<String>::new()
        );
    }

    #[test]
    fn names_in_comments_strings_and_longer_identifiers_are_not_calls() {
        assert_eq!(
            found(
                "// x.exp() is not used\nlet s = \"x.ln()\";\nlet y = x.exp_approx();\nlet z = x.logistic();\n"
            ),
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_code_is_exempt_wherever_it_sits() {
        let source = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t(x: f64) -> f64 { x.exp() }\n}\nfn b(x: f64) -> f64 { x.ln() }\n#[cfg(test)]\nuse std::f64::consts::PI;\nfn c(x: f64) -> f64 { x.tanh() }\n";
        let f = found(source);
        assert_eq!(f.len(), 2, "{f:#?}");
        assert!(
            f[0].contains("x.rs:6:") && f[1].contains("x.rs:9:"),
            "{f:#?}"
        );
    }

    #[test]
    fn a_marked_line_is_allowed() {
        assert_eq!(
            found("let a = x.exp(); // determinism: allow this value only picks a buffer size\n"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn only_the_deterministic_crates_are_checked() {
        let bad = "fn a(x: f64) -> f64 { x.exp() }\n";
        let in_math = krate(
            "wanjiru-math",
            &[],
            None,
            &[("src/lib.rs", bad), ("tests/t.rs", bad)],
        );
        let in_types = krate("wanjiru-types", &[], None, &[("src/lib.rs", bad)]);
        let repo = Repository {
            crates: vec![in_math, in_types],
            ..Repository::default()
        };
        let f = check(&repo);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].location.starts_with("crates/wanjiru-math/src/lib.rs"));
    }
}
