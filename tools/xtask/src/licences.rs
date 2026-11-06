//! Licences of the dependency graph, checked offline.
//!
//! `cargo deny` does the same and more in CI; this check reads the output of `cargo metadata` and the
//! allow-list of `deny.toml`, so that it also runs where cargo-deny is not installed and before a
//! dependency is added to the workspace.

use std::collections::BTreeSet;

use crate::json::Json;
use crate::report::Violation;
use crate::toml::{self, Value};

const CHECK: &str = "licences";

/// The licences `deny.toml` allows.
pub fn read_allow_list(deny_toml: &str) -> Result<BTreeSet<String>, String> {
    let document = toml::parse(deny_toml)?;
    let list = document
        .get(&["licenses", "allow"])
        .and_then(Value::as_array)
        .ok_or("deny.toml has no [licenses] allow list")?;
    Ok(list
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect())
}

#[derive(Debug, PartialEq)]
enum Token {
    Open,
    Close,
    And,
    Or,
    With,
    Id(String),
}

fn tokenize(expression: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut Vec<Token>| {
        if word.is_empty() {
            return;
        }
        let token = match word.to_ascii_uppercase().as_str() {
            "AND" => Token::And,
            "OR" => Token::Or,
            "WITH" => Token::With,
            _ => Token::Id(std::mem::take(word)),
        };
        word.clear();
        out.push(token);
    };
    for c in expression.chars() {
        match c {
            '(' | ')' | '/' => {
                flush(&mut word, &mut out);
                out.push(match c {
                    '(' => Token::Open,
                    ')' => Token::Close,
                    _ => Token::Or, // the legacy `MIT/Apache-2.0` of older crates
                });
            }
            c if c.is_whitespace() => flush(&mut word, &mut out),
            c if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | ':') => word.push(c),
            c => {
                return Err(format!(
                    "unexpected `{c}` in the licence expression `{expression}`"
                ));
            }
        }
    }
    flush(&mut word, &mut out);
    Ok(out)
}

struct Evaluator<'a> {
    tokens: Vec<Token>,
    pos: usize,
    allow: &'a BTreeSet<String>,
}

impl Evaluator<'_> {
    fn or(&mut self) -> Result<bool, String> {
        let mut result = self.and()?;
        while self.tokens.get(self.pos) == Some(&Token::Or) {
            self.pos += 1;
            let next = self.and()?;
            result |= next;
        }
        Ok(result)
    }

    fn and(&mut self) -> Result<bool, String> {
        let mut result = self.factor()?;
        while self.tokens.get(self.pos) == Some(&Token::And) {
            self.pos += 1;
            let next = self.factor()?;
            result &= next;
        }
        Ok(result)
    }

    fn factor(&mut self) -> Result<bool, String> {
        match self.tokens.get(self.pos) {
            Some(Token::Open) => {
                self.pos += 1;
                let inner = self.or()?;
                if self.tokens.get(self.pos) != Some(&Token::Close) {
                    return Err("a parenthesis is not closed".to_owned());
                }
                self.pos += 1;
                Ok(inner)
            }
            Some(Token::Id(id)) => {
                self.pos += 1;
                let base = id.trim_end_matches('+');
                let allowed = self.allow.contains(base);
                // `X WITH exception`: the licence before WITH decides.
                if self.tokens.get(self.pos) == Some(&Token::With) {
                    self.pos += 1;
                    if !matches!(self.tokens.get(self.pos), Some(Token::Id(_))) {
                        return Err("WITH needs an exception name".to_owned());
                    }
                    self.pos += 1;
                }
                Ok(allowed)
            }
            other => Err(format!("a licence name is expected, found {other:?}")),
        }
    }
}

/// Whether an SPDX expression can be satisfied with the allowed licences alone.
pub fn satisfied(expression: &str, allow: &BTreeSet<String>) -> Result<bool, String> {
    let tokens = tokenize(expression)?;
    let mut evaluator = Evaluator {
        tokens,
        pos: 0,
        allow,
    };
    let result = evaluator.or()?;
    if evaluator.pos != evaluator.tokens.len() {
        return Err(format!(
            "the licence expression `{expression}` has text after its end"
        ));
    }
    Ok(result)
}

const CRATES_IO: [&str; 2] = [
    "registry+https://github.com/rust-lang/crates.io-index",
    "sparse+https://index.crates.io/",
];

/// Check every package of a `cargo metadata` output.
pub fn check(
    metadata: &Json,
    allow: &BTreeSet<String>,
    workspace_licence: &str,
) -> Result<Vec<Violation>, String> {
    let packages = metadata
        .get("packages")
        .and_then(Json::as_array)
        .ok_or("the metadata has no `packages`")?;
    let mut out = Vec::new();
    for package in packages {
        let name = package.get("name").and_then(Json::as_str).unwrap_or("?");
        let version = package.get("version").and_then(Json::as_str).unwrap_or("?");
        let location = format!("{name} {version}");
        let licence = package.get("license").and_then(Json::as_str);
        match package.get("source") {
            None | Some(Json::Null) => {
                // a crate of this repository, or a path dependency
                if licence != Some(workspace_licence) {
                    out.push(Violation::new(
                        CHECK,
                        location,
                        format!(
                            "a crate of this repository must be licensed `{workspace_licence}`; it declares {}",
                            licence.map_or_else(|| "no licence".to_owned(), |l| format!("`{l}`"))
                        ),
                    ));
                }
            }
            Some(Json::String(source)) => {
                if !CRATES_IO.contains(&source.as_str()) {
                    out.push(Violation::new(
                        CHECK,
                        location.clone(),
                        format!(
                            "comes from `{source}`; only crates.io is allowed (deny.toml [sources])"
                        ),
                    ));
                }
                match licence {
                    None => out.push(Violation::new(
                        CHECK,
                        location,
                        "declares no SPDX licence; read its licence file, decide, and record the decision in data/registry/licences.yaml before allowing it",
                    )),
                    Some(expression) => match satisfied(expression, allow) {
                        Ok(true) => {}
                        Ok(false) => out.push(Violation::new(CHECK, location, format!("licence `{expression}` cannot be satisfied with the allow-list of deny.toml"))),
                        Err(e) => out.push(Violation::new(CHECK, location, e)),
                    },
                }
            }
            Some(_) => out.push(Violation::new(
                CHECK,
                location,
                "the metadata has a `source` that is not a string",
            )),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    fn allow(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn expressions_are_evaluated_with_and_or_with_and_parentheses() {
        let a = allow(&["MIT", "Apache-2.0", "Unicode-3.0"]);
        for (expression, want) in [
            ("MIT", true),
            ("GPL-3.0-only", false),
            ("MIT OR GPL-3.0-only", true),
            ("MIT AND GPL-3.0-only", false),
            ("(MIT OR Apache-2.0) AND Unicode-3.0", true),
            ("(GPL-2.0-only OR GPL-3.0-only) AND MIT", false),
            ("MIT/Apache-2.0", true),
            ("GPL-3.0-only/LGPL-2.1-only", false),
            ("Apache-2.0 WITH LLVM-exception", true),
            ("GPL-2.0-only WITH Classpath-exception-2.0", false),
            ("mit or apache-2.0", false), // identifiers are case-sensitive, keywords are not
            ("MIT or Apache-2.0", true),
            ("Apache-2.0+", true),
            ("MIT AND (Apache-2.0 OR GPL-3.0-only)", true),
        ] {
            assert_eq!(satisfied(expression, &a), Ok(want), "{expression}");
        }
    }

    #[test]
    fn garbled_expressions_are_errors() {
        let a = allow(&["MIT"]);
        for expression in [
            "",
            "MIT OR",
            "(MIT",
            "MIT)",
            "MIT AND AND MIT",
            "MIT WITH",
            "MIT; DROP",
            "MIT MIT",
        ] {
            assert!(satisfied(expression, &a).is_err(), "{expression:?}");
        }
    }

    #[test]
    fn the_allow_list_is_read_from_deny_toml() {
        let list = read_allow_list("[licenses]\nallow = [\n  \"MIT\", # a\n  \"Apache-2.0\",\n]\n")
            .expect("list");
        assert_eq!(list, allow(&["MIT", "Apache-2.0"]));
        assert!(read_allow_list("[bans]\n").is_err());
        let real = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deny.toml"),
        )
        .expect("deny.toml");
        let real = read_allow_list(&real).expect("the real deny.toml");
        assert!(
            real.contains("MIT")
                && real.contains("Apache-2.0")
                && !real.iter().any(|l| l.contains("GPL"))
        );
    }

    fn metadata(packages: &str) -> Json {
        json::parse(&format!(
            "{{\"packages\": [{packages}], \"workspace_members\": []}}"
        ))
        .expect("json")
    }

    #[test]
    fn workspace_crates_must_carry_the_workspace_licence() {
        let m = metadata(
            r#"{"name":"wanjiru-types","version":"0.1.0","license":"Apache-2.0","source":null},
               {"name":"wanjiru-math","version":"0.1.0","license":"MIT","source":null},
               {"name":"wanjiru-eval","version":"0.1.0","license":null,"source":null}"#,
        );
        let found = check(&m, &allow(&["MIT"]), "Apache-2.0").expect("check");
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].message.contains("declares `MIT`"), "{found:?}");
        assert!(
            found[1].message.contains("declares no licence"),
            "{found:?}"
        );
    }

    #[test]
    fn external_packages_are_checked_against_the_allow_list_and_the_source() {
        let m = metadata(
            r#"{"name":"fine","version":"1.0.0","license":"MIT OR Apache-2.0","source":"registry+https://github.com/rust-lang/crates.io-index"},
               {"name":"sparse","version":"1.0.0","license":"MIT","source":"sparse+https://index.crates.io/"},
               {"name":"copyleft","version":"2.0.0","license":"GPL-3.0-only","source":"registry+https://github.com/rust-lang/crates.io-index"},
               {"name":"nolicence","version":"0.1.0","license":null,"source":"registry+https://github.com/rust-lang/crates.io-index"},
               {"name":"gitdep","version":"0.1.0","license":"MIT","source":"git+https://example.com/x#abc"},
               {"name":"odd","version":"0.1.0","license":"MIT; rm","source":"sparse+https://index.crates.io/"}"#,
        );
        let found: Vec<String> = check(&m, &allow(&["MIT", "Apache-2.0"]), "Apache-2.0")
            .expect("check")
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(found.len(), 4, "{found:#?}");
        assert!(
            found
                .iter()
                .any(|f| f.contains("copyleft 2.0.0") && f.contains("cannot be satisfied"))
        );
        assert!(
            found
                .iter()
                .any(|f| f.contains("nolicence") && f.contains("licences.yaml"))
        );
        assert!(
            found
                .iter()
                .any(|f| f.contains("gitdep") && f.contains("only crates.io"))
        );
        assert!(
            found
                .iter()
                .any(|f| f.contains("odd") && f.contains("unexpected `;`"))
        );
    }

    #[test]
    fn metadata_without_packages_is_an_error() {
        assert!(check(&Json::Null, &allow(&[]), "MIT").is_err());
    }
}
