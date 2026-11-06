//! The repository's own data files: the registries parse, say what the policy needs them to say, and
//! agree with what the build enforces. The research register and the UX documents also agree about which
//! hypotheses exist.
//!
//! A registry nobody can trust is worse than none: a row that is missing a licence, a duplicate
//! identifier, or an allow-list that disagrees with `deny.toml` would each let something through that the
//! rules say must not pass. These tests are cheap, and they run wherever `cargo test` does.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use wanjiru_docir::Json;
use wanjiru_eval::yaml;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load(name: &str) -> Json {
    let path = root().join("data/registry").join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    yaml::parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The entries of one section of a registry: each is keyed by its identifier, so that a duplicate
/// identifier is a parse error and cannot be written by accident.
fn entries<'a>(document: &'a Json, section: &str) -> &'a [(String, Json)] {
    document
        .get(section)
        .and_then(Json::as_object)
        .unwrap_or_else(|| panic!("no section named `{section}`"))
}

fn field<'a>(row: &'a Json, key: &str) -> &'a str {
    row.get(key)
        .and_then(Json::as_str)
        .unwrap_or_else(|| panic!("a row has no text field `{key}`: {row:?}"))
}

/// The registries: file, schema identifier, name of the section of entries, fields every entry must have.
const REGISTRIES: [(&str, &str, &str, &[&str]); 4] = [
    (
        "licences.yaml",
        "wanjiru.registry.licences/0",
        "licences",
        &["policy", "note"],
    ),
    (
        "datasets.yaml",
        "wanjiru.registry.datasets/0",
        "datasets",
        &[
            "title",
            "kind",
            "licence",
            "personal_data",
            "consent",
            "status",
            "use",
        ],
    ),
    (
        "models.yaml",
        "wanjiru.registry.models/0",
        "models",
        &["licence", "status"],
    ),
    (
        "fonts.yaml",
        "wanjiru.registry.fonts/0",
        "fonts",
        &["licence", "status"],
    ),
];

#[test]
fn every_registry_parses_names_its_schema_and_has_complete_entries() {
    for (file, schema, section, required) in REGISTRIES {
        let document = load(file);
        assert_eq!(field(&document, "schema"), schema, "{file}");
        for (id, entry) in entries(&document, section) {
            for key in required {
                assert!(
                    !field(entry, key).trim().is_empty(),
                    "{file}: `{id}` has an empty `{key}`"
                );
            }
            if section != "licences" {
                assert!(
                    entry.get("verified").and_then(Json::as_bool).is_some(),
                    "{file}: `{id}` needs `verified: true` or `false`"
                );
            }
        }
    }
}

#[test]
fn a_duplicate_identifier_cannot_be_written() {
    // Entries are keyed by identifier and the reader refuses duplicate keys, so uniqueness is a
    // property of the format and not something each registry has to check.
    let text = "licences:\n  MIT:\n    policy: allowed\n  MIT:\n    policy: denied\n";
    assert!(yaml::parse(text).is_err());
}

#[test]
fn a_dataset_that_claims_to_be_verified_names_real_terms_and_files_that_exist() {
    let document = load("datasets.yaml");
    for (id, row) in entries(&document, "datasets") {
        for key in ["path", "datasheet"] {
            let path = root().join(field(row, key));
            assert!(
                path.exists(),
                "{id}: `{key}` names {}, which does not exist",
                path.display()
            );
        }
        let verified = row.get("verified").and_then(Json::as_bool) == Some(true);
        let licence = field(row, "licence");
        if verified {
            assert!(
                !licence.starts_with("to be") && licence != "unknown",
                "{id} is marked verified but its licence is `{licence}`"
            );
        }
        // Personal data never enters the repository: a row that says "yes" must also say where the
        // documents are not.
        if field(row, "personal_data") != "none" {
            assert!(
                field(row, "personal_data_note").contains("never the documents"),
                "{id}: a dataset with personal data must say that the repository holds manifests only"
            );
        }
    }
}

#[test]
fn the_candidates_of_the_licence_register_are_all_unverified_until_decided() {
    // Phase 1 adopts no outside component. A candidate that is marked verified must have been decided
    // by an ADR, and this test is where that change is noticed and the rule is rewritten.
    let document = load("licences.yaml");
    for (id, row) in entries(&document, "candidates") {
        for key in ["used_by", "purpose", "licence_claimed", "status"] {
            assert!(!field(row, key).trim().is_empty(), "{id}: empty `{key}`");
        }
        assert_eq!(
            row.get("verified").and_then(Json::as_bool),
            Some(false),
            "{id}: verified is false until an ADR adopts the component"
        );
    }
}

/// The identifiers between `allow = [` and `]` of `[licenses]` in `deny.toml`.
fn deny_allow_list() -> BTreeSet<String> {
    let text = fs::read_to_string(root().join("deny.toml")).expect("deny.toml");
    let section = text
        .split("[licenses]")
        .nth(1)
        .expect("a [licenses] section");
    let list = section
        .split("allow = [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .expect("an allow list");
    list.lines()
        .filter_map(|line| {
            let line = line.split('#').next().unwrap_or("").trim();
            line.trim_end_matches(',')
                .strip_prefix('"')?
                .strip_suffix('"')
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn the_register_allows_exactly_the_licences_the_build_allows() {
    let document = load("licences.yaml");
    let registered: BTreeSet<String> = entries(&document, "licences")
        .iter()
        .filter(|(_, row)| matches!(field(row, "policy"), "allowed" | "review"))
        .map(|(id, _)| id.clone())
        .collect();
    assert_eq!(
        registered,
        deny_allow_list(),
        "data/registry/licences.yaml and deny.toml disagree"
    );
    for (id, row) in entries(&document, "licences") {
        assert!(
            matches!(field(row, "policy"), "allowed" | "review" | "denied"),
            "{id}: policy must be allowed, review or denied"
        );
    }
}

#[test]
fn the_project_licence_is_allowed_by_its_own_policy() {
    let manifest = fs::read_to_string(root().join("Cargo.toml")).expect("Cargo.toml");
    let licence = manifest
        .lines()
        .find_map(|line| line.strip_prefix("license = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("a license in [workspace.package]");
    assert!(deny_allow_list().contains(licence), "{licence}");
}

/// True for an identifier such as `HP-A1`, `HJ-3` or `HY-6`: a kind, a dash, then capitals and digits.
fn is_hypothesis_id(word: &str) -> bool {
    word.len() >= 4
        && matches!(word.get(..3), Some("HP-" | "HJ-" | "HY-"))
        && word[3..]
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// Every hypothesis identifier in `text`.
fn hypothesis_ids(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|word| is_hypothesis_id(word))
        .map(str::to_owned)
        .collect()
}

fn insights() -> String {
    let path = root().join("design/research/insights.md");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_register_of_hypotheses_and_the_ux_documents_name_the_same_identifiers() {
    let directory = root().join("design/ux");
    let mut in_documents = BTreeSet::new();
    for entry in fs::read_dir(&directory).unwrap_or_else(|e| panic!("{}: {e}", directory.display()))
    {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_some_and(|extension| extension == "md") {
            let text =
                fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            in_documents.extend(hypothesis_ids(&text));
        }
    }
    assert!(!in_documents.is_empty(), "design/ux names no hypotheses");
    let in_register = hypothesis_ids(&insights());
    let missing: Vec<_> = in_documents.difference(&in_register).collect();
    let extra: Vec<_> = in_register.difference(&in_documents).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "in design/ux but not in the register: {missing:?}; in the register but not in design/ux: {extra:?}"
    );
}

#[test]
fn every_row_of_the_register_of_hypotheses_has_a_known_state() {
    let mut counted = 0;
    for line in insights().lines() {
        let cells: Vec<&str> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() == 5 && is_hypothesis_id(cells[0]) {
            counted += 1;
            assert!(
                matches!(cells[4], "untested" | "supported" | "refuted" | "changed"),
                "{}: the state `{}` is not one of untested, supported, refuted, changed",
                cells[0],
                cells[4]
            );
        }
    }
    assert!(counted > 0, "the register has no rows");
}
