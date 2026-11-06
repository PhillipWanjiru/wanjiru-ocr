//! `wanjiru errors`: the catalogue of error codes, as Markdown. `docs/errors.md` is this output, and a
//! test of this crate fails when the file and the command disagree.

use std::fmt::Write as _;
use std::str::FromStr as _;

use wanjiru_types::{ErrorCode, Result, codes};

use crate::app::Outcome;
use crate::args::usage;

const INTRODUCTION: &str = "\
# Error codes

Every failure the Wanjiru System reports carries a stable code: `WNJ-E`, the digit of the layer that
reports it, and three digits. A code is never renumbered and never reused; a retired code stays
retired. Messages may change between versions, codes may not.

This file is the output of `wanjiru errors`, and a test fails when the two differ. To change it, change
the registry in `crates/wanjiru-types/src/error.rs` and write the file again:
`cargo run -p wanjiru-cli -- errors > docs/errors.md`.

";

/// A table cell: the text with the one character that would end the cell escaped.
fn cell(text: &str) -> String {
    text.trim().replace('|', "\\|")
}

/// The whole catalogue as Markdown.
pub fn catalogue() -> String {
    let mut out = String::from(INTRODUCTION);
    out.push_str("| Code | Layer | Title | When it is reported |\n|---|---|---|---|\n");
    for (code, title, description) in codes::ALL {
        let layer = code.layer();
        let _ = writeln!(
            out,
            "| `{code}` | L{} {} | {} | {} |",
            layer.digit(),
            layer.name(),
            cell(title),
            cell(description)
        );
    }
    out
}

/// One code, as it is shown by `wanjiru errors CODE`.
fn describe(text: &str) -> Result<Outcome> {
    let code = ErrorCode::from_str(text).map_err(|()| {
        usage(format!(
            "`{text}` is not an error code: expected `WNJ-E` and four digits, such as WNJ-E4002"
        ))
    })?;
    let Some((_, title, description)) = codes::ALL.iter().find(|(known, _, _)| *known == code)
    else {
        return Err(usage(format!(
            "{code} is not in the registry; `wanjiru errors` lists every code"
        )));
    };
    let layer = code.layer();
    Ok(Outcome::ok(format!(
        "{code}  {title}  (L{} {})\n{}\n",
        layer.digit(),
        layer.name(),
        description.trim()
    )))
}

pub fn run(args: &[String]) -> Result<Outcome> {
    match args {
        [] => Ok(Outcome::ok(catalogue())),
        [code] => describe(code),
        _ => Err(usage(
            "`errors` takes at most one argument, a code such as WNJ-E4002",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_catalogue_lists_every_code_once_with_its_description() {
        let text = catalogue();
        for (code, title, description) in codes::ALL {
            let line = text
                .lines()
                .find(|line| line.starts_with(&format!("| `{code}` |")))
                .unwrap_or_else(|| panic!("{code} is missing"));
            assert!(line.contains(title), "{line}");
            assert!(line.contains(description.trim()), "{line}");
            assert_eq!(text.matches(&format!("`{code}`")).count(), 1, "{code}");
        }
        assert!(text.ends_with("|\n"));
    }

    #[test]
    fn one_code_is_described_in_a_sentence() {
        let out = run(&args(&["WNJ-E4002"])).expect("a known code");
        assert_eq!(out.code, 0);
        assert_eq!(
            out.stdout,
            "WNJ-E4002  contract invalid  (L4 orchestration)\n\
             A quality contract has a key or value the harness does not know.\n"
        );
    }

    #[test]
    fn unknown_and_malformed_codes_are_usage_errors() {
        for bad in ["WNJ-E4999", "wnj-e4002", "E4002", "WNJ-E40", "", "x y"] {
            let error = run(&args(&[bad])).expect_err(bad);
            assert_eq!(error.code(), codes::USAGE, "{bad}");
        }
        assert!(run(&args(&["WNJ-E4002", "WNJ-E4001"])).is_err());
    }

    #[test]
    fn a_pipe_in_a_description_cannot_break_the_table() {
        assert_eq!(cell(" a | b "), "a \\| b");
    }
}
