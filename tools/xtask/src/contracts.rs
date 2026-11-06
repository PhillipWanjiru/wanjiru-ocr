//! Every crate has a `CONTRACT.md`, and it agrees with the crate.
//!
//! The contract is the written promise of a crate: what it is for, what it offers, what always holds,
//! what it is checked against, what it may cost, how it treats `unsafe`, and which crates it may use.
//! This check keeps the promise and the manifest from drifting apart.

use std::collections::BTreeSet;

use crate::layers::{LAYERS, is_unsafe_zone, layer_of};
use crate::repo::{CrateInfo, Repository};
use crate::report::Violation;

const CHECK: &str = "contracts";

/// The sections every contract has, in this order. Other sections (for example "Open questions") may follow.
pub const SECTIONS: &[&str] = &[
    "Purpose",
    "Public API",
    "Invariants",
    "Oracles",
    "Budget",
    "Unsafe policy",
    "Allowed dependencies",
];

/// The files a contract names in backticks: repository paths such as `docs/budgets.md`, and, for a crate
/// that is implemented, paths inside the crate such as `tests/golden/`. Patterns and placeholders
/// (`*`, `<`, `{`) are not paths.
fn referenced_paths(text: &str, dir: &str, implemented: bool) -> Vec<String> {
    const ROOTED: &[&str] = &["docs/", "tools/", "eval/", "data/", "design/", "crates/"];
    const RELATIVE: &[&str] = &["tests/", "src/", "schema/", "benches/", "examples/"];
    let mut out = Vec::new();
    let mut parts = text.split('`');
    parts.next();
    while let Some(span) = parts.next() {
        parts.next();
        if span.is_empty()
            || span.contains(|c: char| {
                c.is_whitespace() || matches!(c, '*' | '<' | '{' | '(' | ':' | '…' | ',')
            })
        {
            continue;
        }
        if ROOTED.iter().any(|p| span.starts_with(p)) {
            out.push(span.to_owned());
        } else if implemented && RELATIVE.iter().any(|p| span.starts_with(p)) {
            out.push(format!("crates/{dir}/{span}"));
        }
    }
    out
}

/// A parsed contract.
#[derive(Debug, Default)]
struct Contract {
    title: String,
    rows: Vec<(String, String)>,
    sections: Vec<(String, Vec<String>)>,
}

fn parse(text: &str) -> Contract {
    let mut contract = Contract::default();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("# ") {
            if contract.title.is_empty() {
                title.trim().clone_into(&mut contract.title);
            }
        } else if let Some(name) = line.strip_prefix("## ") {
            contract.sections.push((name.trim().to_owned(), Vec::new()));
        } else if let Some((_, body)) = contract.sections.last_mut() {
            body.push(line.to_owned());
        } else if line.starts_with('|') {
            let cells: Vec<&str> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            if let [key, value] = cells.as_slice() {
                if !key.is_empty() && !key.chars().all(|c| c == '-' || c == ':') {
                    contract.rows.push(((*key).to_owned(), (*value).to_owned()));
                }
            }
        }
    }
    contract
}

impl Contract {
    fn row(&self, key: &str) -> Option<&str> {
        self.rows
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    fn section(&self, name: &str) -> Option<&[String]> {
        self.sections
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, body)| body.as_slice())
    }
}

fn is_item(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("- ")
        || line
            .split_once(". ")
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// The names in a line such as ``- Workspace: `a`, `b` `` or `- External: none`.
fn listed(body: &[String], label: &str) -> Option<Result<BTreeSet<String>, String>> {
    let prefix = format!("- {label}:");
    let line = body.iter().find(|l| l.trim_start().starts_with(&prefix))?;
    let rest = line.trim_start()[prefix.len()..].trim();
    if rest.eq_ignore_ascii_case("none") {
        return Some(Ok(BTreeSet::new()));
    }
    let mut names = BTreeSet::new();
    let mut parts = rest.split('`');
    parts.next();
    while let Some(name) = parts.next() {
        names.insert(name.to_owned());
        parts.next();
    }
    if names.is_empty() {
        Some(Err(format!(
            "`- {label}:` must list crates in backticks, or say `none`"
        )))
    } else {
        Some(Ok(names))
    }
}

fn check_crate(krate: &CrateInfo, paths: &BTreeSet<String>, out: &mut Vec<Violation>) {
    let location = format!("crates/{}/CONTRACT.md", krate.dir);
    let mut bad = |message: String| out.push(Violation::new(CHECK, location.clone(), message));
    let Some(text) = &krate.contract else {
        bad("missing: every crate has a CONTRACT.md".to_owned());
        return;
    };
    let contract = parse(text);

    if contract.title != format!("{}: contract", krate.dir) {
        bad(format!(
            "the title is `# {}`, expected `# {}: contract`",
            contract.title, krate.dir
        ));
    }

    // The header table.
    match (contract.row("Layer"), layer_of(&krate.dir)) {
        (None, _) => bad("the header table has no `Layer` row".to_owned()),
        (Some(row), Some(layer)) => {
            let written = row
                .strip_prefix('L')
                .and_then(|r| r.chars().next())
                .and_then(|c| c.to_digit(10));
            if written != Some(u32::from(layer)) {
                bad(format!(
                    "the `Layer` row says `{row}` but the layer table says L{layer}"
                ));
            }
        }
        (Some(_), None) => {}
    }
    match contract.row("Unsafe zone") {
        None => bad("the header table has no `Unsafe zone` row".to_owned()),
        Some(value) => {
            let zone = is_unsafe_zone(&krate.dir);
            if value != if zone { "yes" } else { "no" } {
                bad(format!(
                    "the `Unsafe zone` row says `{value}` but this crate {} an unsafe zone in the layer table",
                    if zone { "is" } else { "is not" }
                ));
            }
        }
    }
    match contract.row("Status") {
        None => bad("the header table has no `Status` row".to_owned()),
        Some(status)
            if !(status.starts_with("Contract only") || status.starts_with("Implemented")) =>
        {
            bad(format!(
                "the `Status` row must start with `Contract only` or `Implemented`, not `{status}`"
            ));
        }
        Some(_) => {}
    }

    // The sections, in order, with something in them.
    let present: Vec<&str> = contract.sections.iter().map(|(n, _)| n.as_str()).collect();
    let mut last = None;
    for required in SECTIONS {
        match present.iter().position(|p| p == required) {
            None => bad(format!("missing the section `## {required}`")),
            Some(index) => {
                if last.is_some_and(|l| index < l) {
                    bad(format!(
                        "the section `## {required}` is out of order; the order is {}",
                        SECTIONS.join(", ")
                    ));
                }
                last = Some(index);
            }
        }
    }
    let empty = |body: &[String]| body.iter().all(|l| l.trim().is_empty());
    for name in ["Purpose", "Public API", "Budget"] {
        if contract.section(name).is_some_and(empty) {
            bad(format!("the section `## {name}` is empty"));
        }
    }
    for name in ["Invariants", "Oracles"] {
        if contract
            .section(name)
            .is_some_and(|body| !body.iter().any(|l| is_item(l)))
        {
            bad(format!(
                "the section `## {name}` needs at least one list item"
            ));
        }
    }

    // The unsafe policy matches the zone.
    if let Some(body) = contract.section("Unsafe policy") {
        let text = body.join("\n");
        if is_unsafe_zone(&krate.dir) {
            if !text.contains("SAFETY") {
                bad(
                    "an unsafe zone's policy must require a `SAFETY` comment on every unsafe block"
                        .to_owned(),
                );
            }
        } else if !text.contains("#![forbid(unsafe_code)]") {
            bad("a crate outside the unsafe zones must say `#![forbid(unsafe_code)]` in its Unsafe policy".to_owned());
        }
    }

    // The files it names exist.
    let implemented = contract
        .row("Status")
        .is_some_and(|s| s.starts_with("Implemented"));
    for path in referenced_paths(text, &krate.dir, implemented) {
        let trimmed = path.trim_end_matches('/');
        let exists =
            paths.contains(trimmed) || paths.iter().any(|p| p.starts_with(&format!("{trimmed}/")));
        if !exists {
            bad(format!(
                "names `{path}`, which does not exist; create it or correct the contract"
            ));
        }
    }

    // The dependency list is the manifest.
    let Some(body) = contract.section("Allowed dependencies") else {
        return;
    };
    let Ok(manifest) = &krate.manifest else {
        return;
    };
    let internal = |name: &str| LAYERS.iter().any(|(n, _)| *n == name);
    let used_workspace: BTreeSet<String> = manifest
        .dependencies
        .iter()
        .filter(|d| internal(&d.name))
        .map(|d| d.name.clone())
        .collect();
    let used_external: BTreeSet<String> = manifest
        .dependencies
        .iter()
        .filter(|d| !internal(&d.name))
        .map(|d| d.name.clone())
        .collect();
    for (label, used) in [("Workspace", used_workspace), ("External", used_external)] {
        match listed(body, label) {
            None => bad(format!(
                "`## Allowed dependencies` has no `- {label}:` line"
            )),
            Some(Err(message)) => bad(message),
            Some(Ok(allowed)) => {
                for name in used.difference(&allowed) {
                    bad(format!(
                        "Cargo.toml uses `{name}`, which the contract does not list under `{label}`; add it with its reason, or remove the dependency"
                    ));
                }
                for name in allowed.difference(&used) {
                    bad(format!(
                        "the contract lists `{name}` under `{label}` but Cargo.toml does not use it; remove it from the contract, or add the dependency"
                    ));
                }
            }
        }
    }
}

/// Check every crate's contract.
pub fn check(repo: &Repository) -> Vec<Violation> {
    let mut out = Vec::new();
    for krate in &repo.crates {
        check_crate(krate, &repo.paths, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::testing::krate;

    /// A well-formed contract for `dir` with the given allowed dependencies.
    fn contract(dir: &str, layer: u8, zone: bool, workspace: &str, external: &str) -> String {
        let unsafe_policy = if zone {
            "Unsafe code only in private modules; every block has a `// SAFETY:` comment."
        } else {
            "`#![forbid(unsafe_code)]`."
        };
        format!(
            "# {dir}: contract\n\n| | |\n|---|---|\n| Layer | L{layer} Something (rule) |\n| Status | Contract only. Implemented in Phase 2. |\n| Unsafe zone | {} |\n\nIntro.\n\n## Purpose\n\nDoes a thing.\n\n## Public API\n\nPlanned.\n\n## Invariants\n\n1. It holds.\n\n## Oracles\n\n- Another implementation.\n\n## Budget\n\n- See docs/budgets.md.\n\n## Unsafe policy\n\n{unsafe_policy}\n\n## Allowed dependencies\n\n- Workspace: {workspace}\n- External: {external}\n\n## Open questions\n\n- None.\n",
            if zone { "yes" } else { "no" }
        )
    }

    fn found(krate: &CrateInfo) -> Vec<String> {
        found_with(krate, &[])
    }

    fn found_with(krate: &CrateInfo, paths: &[&str]) -> Vec<String> {
        let paths: BTreeSet<String> = paths.iter().map(|p| (*p).to_owned()).collect();
        let mut out = Vec::new();
        check_crate(krate, &paths, &mut out);
        out.iter().map(|v| v.message.clone()).collect()
    }

    #[test]
    fn a_well_formed_contract_that_matches_its_manifest_passes() {
        let text = contract(
            "wanjiru-docir",
            1,
            false,
            "`wanjiru-types`, `wanjiru-math`",
            "none",
        );
        let k = krate(
            "wanjiru-docir",
            &["wanjiru-types", "wanjiru-math"],
            Some(&text),
            &[],
        );
        assert_eq!(found(&k), Vec::<String>::new());
        let zone = contract("wanjiru-simd", 1, true, "`wanjiru-types`", "none");
        let k = krate("wanjiru-simd", &["wanjiru-types"], Some(&zone), &[]);
        assert_eq!(found(&k), Vec::<String>::new());
    }

    #[test]
    fn a_missing_contract_is_reported() {
        let k = krate("wanjiru-docir", &[], None, &[]);
        assert_eq!(found(&k), ["missing: every crate has a CONTRACT.md"]);
    }

    #[test]
    fn the_dependency_lists_must_equal_the_manifest_in_both_directions() {
        let text = contract(
            "wanjiru-docir",
            1,
            false,
            "`wanjiru-types`, `wanjiru-eval`",
            "none",
        );
        let k = krate(
            "wanjiru-docir",
            &["wanjiru-types", "wanjiru-math", "serde"],
            Some(&text),
            &[],
        );
        let messages = found(&k).join("\n");
        assert!(
            messages.contains(
                "uses `wanjiru-math`, which the contract does not list under `Workspace`"
            ),
            "{messages}"
        );
        assert!(
            messages
                .contains("lists `wanjiru-eval` under `Workspace` but Cargo.toml does not use it"),
            "{messages}"
        );
        assert!(
            messages.contains("uses `serde`, which the contract does not list under `External`"),
            "{messages}"
        );
    }

    #[test]
    fn dev_dependencies_count() {
        let text = contract("wanjiru-docir", 1, false, "`wanjiru-types`", "none");
        let k = krate(
            "wanjiru-docir",
            &["wanjiru-types", "wanjiru-math:dev"],
            Some(&text),
            &[],
        );
        assert!(found(&k).join("\n").contains("uses `wanjiru-math`"));
    }

    #[test]
    fn the_header_must_agree_with_the_layer_table() {
        let text = contract("wanjiru-docir", 3, true, "none", "none");
        let k = krate("wanjiru-docir", &[], Some(&text), &[]);
        let messages = found(&k).join("\n");
        assert!(
            messages.contains("says `L3 Something (rule)` but the layer table says L1"),
            "{messages}"
        );
        assert!(
            messages.contains("`Unsafe zone` row says `yes` but this crate is not an unsafe zone"),
            "{messages}"
        );
        // the sample's policy text is the zone's, which a crate outside the zones must not have
        assert!(
            messages.contains("must say `#![forbid(unsafe_code)]`"),
            "{messages}"
        );
    }

    #[test]
    fn the_unsafe_policy_must_match_the_zone() {
        let zone_without_safety = contract("wanjiru-simd", 1, true, "none", "none")
            .replace("every block has a `// SAFETY:` comment", "be careful");
        let k = krate("wanjiru-simd", &[], Some(&zone_without_safety), &[]);
        assert!(
            found(&k)
                .join("\n")
                .contains("must require a `SAFETY` comment")
        );
        let plain_without_forbid = contract("wanjiru-docir", 1, false, "none", "none")
            .replace("`#![forbid(unsafe_code)]`.", "No unsafe.");
        let k = krate("wanjiru-docir", &[], Some(&plain_without_forbid), &[]);
        assert!(
            found(&k)
                .join("\n")
                .contains("must say `#![forbid(unsafe_code)]`")
        );
    }

    #[test]
    fn sections_must_exist_in_order_and_not_be_empty() {
        let base = contract("wanjiru-docir", 1, false, "none", "none");
        let missing = base.replace("## Oracles\n\n- Another implementation.\n\n", "");
        assert!(
            found(&krate("wanjiru-docir", &[], Some(&missing), &[]))
                .join("\n")
                .contains("missing the section `## Oracles`")
        );
        let swapped = base
            .replace("## Invariants", "## TMP")
            .replace("## Oracles", "## Invariants")
            .replace("## TMP", "## Oracles");
        assert!(
            found(&krate("wanjiru-docir", &[], Some(&swapped), &[]))
                .join("\n")
                .contains("out of order")
        );
        let empty = base.replace("1. It holds.", "");
        assert!(
            found(&krate("wanjiru-docir", &[], Some(&empty), &[]))
                .join("\n")
                .contains("`## Invariants` needs at least one list item")
        );
        let empty_purpose = base.replace("Does a thing.", "");
        assert!(
            found(&krate("wanjiru-docir", &[], Some(&empty_purpose), &[]))
                .join("\n")
                .contains("`## Purpose` is empty")
        );
    }

    #[test]
    fn title_and_status_are_checked() {
        let base = contract("wanjiru-docir", 1, false, "none", "none");
        let wrong_title = base.replace("# wanjiru-docir: contract", "# something else");
        assert!(
            found(&krate("wanjiru-docir", &[], Some(&wrong_title), &[]))
                .join("\n")
                .contains("expected `# wanjiru-docir: contract`")
        );
        let wrong_status = base.replace("Contract only. Implemented in Phase 2.", "Soon");
        assert!(
            found(&krate("wanjiru-docir", &[], Some(&wrong_status), &[]))
                .join("\n")
                .contains("must start with `Contract only` or `Implemented`")
        );
    }

    #[test]
    fn dependency_lines_must_be_readable() {
        let base = contract("wanjiru-docir", 1, false, "wanjiru-types", "none");
        let messages = found(&krate(
            "wanjiru-docir",
            &["wanjiru-types"],
            Some(&base),
            &[],
        ))
        .join("\n");
        assert!(
            messages.contains("must list crates in backticks, or say `none`"),
            "{messages}"
        );
        let without = base.replace("- External: none\n", "");
        assert!(
            found(&krate("wanjiru-docir", &[], Some(&without), &[]))
                .join("\n")
                .contains("has no `- External:` line")
        );
    }

    #[test]
    fn named_files_must_exist() {
        let base = contract("wanjiru-docir", 1, false, "none", "none")
            .replace("Does a thing.", "See `docs/budgets.md`, `docs/adr/007.md`, `tests/golden/`, `tests/oracles`, `docs/<x>.md`, `src/*.rs` and `tools/oracles/make.py`.")
            .replace("Contract only. Implemented in Phase 2.", "Implemented in Phase 1.");
        let k = krate("wanjiru-docir", &[], Some(&base), &[]);
        let paths = [
            "docs",
            "docs/budgets.md",
            "crates",
            "crates/wanjiru-docir",
            "crates/wanjiru-docir/tests",
            "crates/wanjiru-docir/tests/golden",
            "crates/wanjiru-docir/tests/golden/a.txt",
        ];
        let messages = found_with(&k, &paths);
        assert_eq!(
            messages,
            [
                "names `docs/adr/007.md`, which does not exist; create it or correct the contract",
                "names `crates/wanjiru-docir/tests/oracles`, which does not exist; create it or correct the contract",
                "names `tools/oracles/make.py`, which does not exist; create it or correct the contract",
            ],
            "{messages:#?}"
        );
        // a contract-only crate may name files of its own that do not exist yet, but not files of the repository
        let planned = base.replace(
            "Implemented in Phase 1.",
            "Contract only. Implemented in Phase 2.",
        );
        let k = krate("wanjiru-docir", &[], Some(&planned), &[]);
        assert_eq!(found_with(&k, &paths).len(), 2);
    }

    #[test]
    fn the_parser_reads_rows_and_sections() {
        let c = parse(&contract("x", 2, false, "`a`", "none"));
        assert_eq!(c.title, "x: contract");
        assert_eq!(c.row("Layer"), Some("L2 Something (rule)"));
        assert_eq!(c.sections.len(), 8);
        assert_eq!(
            listed(
                c.section("Allowed dependencies").expect("section"),
                "Workspace"
            ),
            Some(Ok(BTreeSet::from(["a".to_owned()])))
        );
    }
}
