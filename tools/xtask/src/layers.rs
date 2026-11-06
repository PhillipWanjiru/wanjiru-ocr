//! The layer rule: a crate depends only on crates of a lower layer.
//!
//! The table below is the single place where the layer of a crate is declared; every `CONTRACT.md`
//! repeats it and `contracts.rs` checks that the two agree.

use std::collections::{BTreeMap, BTreeSet};

use crate::repo::{Kind, Repository};
use crate::report::Violation;

const CHECK: &str = "layers";

/// The layer of every crate: 0 foundation, 1 core, 2 I/O and runtime, 3 domain logic, 4 orchestration,
/// 5 surfaces.
pub const LAYERS: &[(&str, u8)] = &[
    ("wanjiru-types", 0),
    ("wanjiru-math", 0),
    ("wanjiru-docir", 1),
    ("wanjiru-simd", 1),
    ("wanjiru-imgproc", 1),
    ("wanjiru-codecs", 2),
    ("wanjiru-pdf", 2),
    ("wanjiru-barcode", 2),
    ("wanjiru-runtime", 2),
    ("wanjiru-models", 2),
    ("wanjiru-detect", 3),
    ("wanjiru-recognize", 3),
    ("wanjiru-decode", 3),
    ("wanjiru-fields", 3),
    ("wanjiru-layout", 3),
    ("wanjiru-calibrate", 3),
    ("wanjiru-engine", 4),
    ("wanjiru-emit", 4),
    ("wanjiru-eval", 4),
    ("wanjiru", 5),
    ("wanjiru-ffi", 5),
    ("wanjiru-cli", 5),
    ("wanjiru-worker", 5),
    ("wanjiru-server", 5),
    ("wanjiru-py", 5),
];

/// The crates in which `unsafe` is allowed, each block with a SAFETY comment (ADR-007).
pub const UNSAFE_ZONES: &[&str] = &[
    "wanjiru-simd",
    "wanjiru-runtime",
    "wanjiru-codecs",
    "wanjiru-pdf",
    "wanjiru-ffi",
    "wanjiru-worker",
];

/// A dependency between two crates of the same layer that the rule would otherwise forbid.
pub struct Exception {
    /// The depending crate.
    pub from: &'static str,
    /// The crate depended on.
    pub to: &'static str,
    /// Why the exception is reasonable.
    pub reason: &'static str,
}

/// The same-layer exceptions. Each is proposed in ADR-011 (status: Proposed) and is allowed, not required.
pub const SAME_LAYER_EXCEPTIONS: &[Exception] = &[
    Exception {
        from: "wanjiru-imgproc",
        to: "wanjiru-simd",
        reason: "resampling and binarisation are the natural callers of the SIMD kernels",
    },
    Exception {
        from: "wanjiru-ffi",
        to: "wanjiru",
        reason: "a surface is a thin layer over the facade",
    },
    Exception {
        from: "wanjiru-cli",
        to: "wanjiru",
        reason: "a surface is a thin layer over the facade",
    },
    Exception {
        from: "wanjiru-worker",
        to: "wanjiru",
        reason: "a surface is a thin layer over the facade",
    },
    Exception {
        from: "wanjiru-server",
        to: "wanjiru",
        reason: "a surface is a thin layer over the facade",
    },
    Exception {
        from: "wanjiru-py",
        to: "wanjiru",
        reason: "a surface is a thin layer over the facade",
    },
];

/// The layer of a crate of this repository.
pub fn layer_of(name: &str) -> Option<u8> {
    LAYERS
        .iter()
        .find(|(crate_name, _)| *crate_name == name)
        .map(|(_, layer)| *layer)
}

/// Whether `name` is declared an unsafe zone.
pub fn is_unsafe_zone(name: &str) -> bool {
    UNSAFE_ZONES.contains(&name)
}

fn excepted(from: &str, to: &str) -> bool {
    SAME_LAYER_EXCEPTIONS
        .iter()
        .any(|e| e.from == from && e.to == to)
}

/// Check the layer rule and the consistency of the workspace manifests with the crate directories.
pub fn check(repo: &Repository) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for krate in &repo.crates {
        let location = format!("crates/{}", krate.dir);
        let manifest = match &krate.manifest {
            Ok(manifest) => manifest,
            Err(e) => {
                out.push(Violation::new(
                    CHECK,
                    location,
                    format!("the manifest cannot be read: {e}"),
                ));
                continue;
            }
        };
        if manifest.name != krate.dir {
            out.push(Violation::new(
                CHECK,
                format!("{location}/Cargo.toml"),
                format!(
                    "the package is named `{}` but its directory is `{}`",
                    manifest.name, krate.dir
                ),
            ));
        }
        if !manifest.inherits_lints {
            out.push(Violation::new(
                CHECK,
                format!("{location}/Cargo.toml"),
                "missing `[lints] workspace = true`: the workspace lints (unsafe_code, unwrap_used, ...) would not apply",
            ));
        }
        let Some(layer) = layer_of(&krate.dir) else {
            out.push(Violation::new(
                CHECK,
                location,
                "the crate is not in the layer table of tools/xtask/src/layers.rs; add it with its layer",
            ));
            continue;
        };
        for dependency in &manifest.dependencies {
            if !dependency.inherited {
                out.push(Violation::new(
                    CHECK,
                    format!("{location}/Cargo.toml"),
                    format!(
                        "`{}` ({}) is not declared as `{{ workspace = true }}`; declare it once in [workspace.dependencies] with the reason it is needed",
                        dependency.name,
                        dependency.kind.section()
                    ),
                ));
            }
            let Some(target) = layer_of(&dependency.name) else {
                if layer == 0 {
                    out.push(Violation::new(
                        CHECK,
                        format!("{location}/Cargo.toml"),
                        format!(
                            "`{}` is an L0 crate and may use the standard library only, not `{}`",
                            krate.dir, dependency.name
                        ),
                    ));
                }
                continue;
            };
            graph
                .entry(krate.dir.clone())
                .or_default()
                .insert(dependency.name.clone());
            if target < layer || excepted(&krate.dir, &dependency.name) {
                continue;
            }
            let how = match dependency.kind {
                Kind::Normal => "depends on",
                Kind::Dev => "has a dev-dependency on",
                Kind::Build => "has a build-dependency on",
            };
            let relation = if target == layer {
                "of the same layer"
            } else {
                "of a higher layer"
            };
            out.push(Violation::new(
                CHECK,
                format!("{location}/Cargo.toml"),
                format!(
                    "{} (L{layer}) {how} {} (L{target}), a crate {relation}; a crate may use only lower layers (exceptions: see SAME_LAYER_EXCEPTIONS, ADR-011)",
                    krate.dir, dependency.name
                ),
            ));
        }
    }

    // Every crate of the table exists, and the workspace manifest knows every crate.
    for (name, _) in LAYERS {
        if !repo.crates.iter().any(|c| c.dir == *name) {
            out.push(Violation::new(
                CHECK,
                format!("crates/{name}"),
                "the layer table lists this crate but the directory does not exist",
            ));
        }
    }
    for krate in &repo.crates {
        match repo
            .workspace_dependencies
            .iter()
            .find(|d| d.name == krate.dir)
        {
            None => out.push(Violation::new(
                CHECK,
                "Cargo.toml",
                format!("[workspace.dependencies] has no entry for `{}`", krate.dir),
            )),
            Some(entry) if entry.path.as_deref() != Some(&format!("crates/{}", krate.dir)) => out
                .push(Violation::new(
                    CHECK,
                    "Cargo.toml",
                    format!(
                        "[workspace.dependencies] `{}` has path {:?}, expected \"crates/{}\"",
                        krate.dir, entry.path, krate.dir
                    ),
                )),
            Some(_) => {}
        }
    }
    for entry in &repo.workspace_dependencies {
        if entry.path.is_some() && !repo.crates.iter().any(|c| c.dir == entry.name) {
            out.push(Violation::new(
                CHECK,
                "Cargo.toml",
                format!(
                    "[workspace.dependencies] `{}` names a crate that does not exist",
                    entry.name
                ),
            ));
        }
    }
    if !repo.members.iter().any(|m| m == "crates/*") {
        out.push(Violation::new(
            CHECK,
            "Cargo.toml",
            "[workspace] members must include \"crates/*\" so that no crate escapes the checks",
        ));
    }

    if let Some(cycle) = find_cycle(&graph) {
        out.push(Violation::new(
            CHECK,
            "Cargo.toml",
            format!("dependency cycle: {}", cycle.join(" -> ")),
        ));
    }
    out
}

fn find_cycle(graph: &BTreeMap<String, BTreeSet<String>>) -> Option<Vec<String>> {
    fn visit(
        node: &str,
        graph: &BTreeMap<String, BTreeSet<String>>,
        done: &mut BTreeSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        if let Some(start) = path.iter().position(|p| p == node) {
            let mut cycle = path[start..].to_vec();
            cycle.push(node.to_owned());
            return Some(cycle);
        }
        if done.contains(node) {
            return None;
        }
        path.push(node.to_owned());
        for next in graph.get(node).into_iter().flatten() {
            if let Some(cycle) = visit(next, graph, done, path) {
                return Some(cycle);
            }
        }
        path.pop();
        done.insert(node.to_owned());
        None
    }
    let mut done = BTreeSet::new();
    for node in graph.keys() {
        if let Some(cycle) = visit(node, graph, &mut done, &mut Vec::new()) {
            return Some(cycle);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::testing::krate;
    use crate::repo::{Repository, WorkspaceDependency};

    /// A repository with exactly the crates of the layer table, each with the given extra dependencies.
    fn repository(extra: &[(&str, &[&str])]) -> Repository {
        let mut repo = Repository {
            members: vec!["crates/*".to_owned(), "tools/xtask".to_owned()],
            ..Repository::default()
        };
        for (name, _) in LAYERS {
            let dependencies = extra
                .iter()
                .find(|(n, _)| n == name)
                .map_or(&[][..], |(_, d)| d);
            repo.crates.push(krate(name, dependencies, None, &[]));
            repo.workspace_dependencies.push(WorkspaceDependency {
                name: (*name).to_owned(),
                path: Some(format!("crates/{name}")),
            });
        }
        repo
    }

    fn messages(repo: &Repository) -> Vec<String> {
        check(repo).iter().map(ToString::to_string).collect()
    }

    #[test]
    fn the_table_covers_25_crates_in_six_layers() {
        assert_eq!(LAYERS.len(), 25);
        let layers: BTreeSet<u8> = LAYERS.iter().map(|(_, l)| *l).collect();
        assert_eq!(layers.into_iter().collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5]);
        let names: BTreeSet<&str> = LAYERS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), 25, "no crate is listed twice");
        assert!(UNSAFE_ZONES.iter().all(|z| names.contains(z)));
        assert!(
            SAME_LAYER_EXCEPTIONS
                .iter()
                .all(|e| names.contains(e.from) && names.contains(e.to))
        );
        // every exception really is between crates of one layer
        assert!(
            SAME_LAYER_EXCEPTIONS
                .iter()
                .all(|e| layer_of(e.from) == layer_of(e.to))
        );
    }

    #[test]
    fn a_clean_repository_has_no_violations() {
        let repo = repository(&[
            ("wanjiru-docir", &["wanjiru-types", "wanjiru-math"]),
            (
                "wanjiru-eval",
                &["wanjiru-types", "wanjiru-math", "wanjiru-docir"],
            ),
            (
                "wanjiru-cli",
                &["wanjiru-eval", "wanjiru-docir", "wanjiru-math:dev"],
            ),
        ]);
        assert_eq!(messages(&repo), Vec::<String>::new());
    }

    #[test]
    fn an_upward_dependency_is_reported_with_both_layers() {
        let repo = repository(&[("wanjiru-docir", &["wanjiru-eval"])]);
        let found = messages(&repo);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("wanjiru-docir (L1) depends on wanjiru-eval (L4)"),
            "{found:?}"
        );
        assert!(found[0].contains("higher layer"), "{found:?}");
    }

    #[test]
    fn a_dev_dependency_upward_is_reported_too() {
        let repo = repository(&[("wanjiru-math", &["wanjiru-eval:dev"])]);
        let found = messages(&repo).join("\n");
        assert!(
            found.contains("has a dev-dependency on wanjiru-eval"),
            "{found}"
        );
    }

    #[test]
    fn a_sideways_dependency_is_reported_unless_it_is_a_listed_exception() {
        let sideways = repository(&[("wanjiru-docir", &["wanjiru-simd"])]);
        assert!(messages(&sideways).join("\n").contains("same layer"));
        let excepted = repository(&[
            ("wanjiru-imgproc", &["wanjiru-simd"]),
            ("wanjiru-cli", &["wanjiru"]),
        ]);
        assert_eq!(messages(&excepted), Vec::<String>::new());
        // the exception is directional
        let reversed = repository(&[("wanjiru-simd", &["wanjiru-imgproc"])]);
        assert!(!messages(&reversed).is_empty());
    }

    #[test]
    fn l0_crates_use_the_standard_library_only() {
        let repo = repository(&[("wanjiru-math", &["serde"])]);
        let found = messages(&repo).join("\n");
        assert!(found.contains("standard library only"), "{found}");
    }

    #[test]
    fn dependencies_must_be_declared_in_the_workspace() {
        let mut repo = repository(&[]);
        repo.crates[2] = krate("wanjiru-docir", &[], None, &[]);
        repo.crates[2].manifest = crate::repo::Manifest::parse(
            "[package]\nname = \"wanjiru-docir\"\n[lints]\nworkspace = true\n[dependencies]\nwanjiru-types = { path = \"../wanjiru-types\" }\n",
        );
        let found = messages(&repo).join("\n");
        assert!(
            found.contains("not declared as `{ workspace = true }`"),
            "{found}"
        );
    }

    #[test]
    fn a_missing_lints_table_a_wrong_name_and_an_unknown_crate_are_reported() {
        let mut repo = repository(&[]);
        repo.crates[0].manifest = crate::repo::Manifest::parse("[package]\nname = \"oops\"\n");
        repo.crates.push(krate("wanjiru-extra", &[], None, &[]));
        let found = messages(&repo).join("\n");
        assert!(
            found.contains("is named `oops` but its directory is `wanjiru-types`"),
            "{found}"
        );
        assert!(
            found.contains("missing `[lints] workspace = true`"),
            "{found}"
        );
        assert!(
            found.contains("`wanjiru-extra`") && found.contains("not in the layer table"),
            "{found}"
        );
        assert!(
            found.contains("has no entry for `wanjiru-extra`"),
            "{found}"
        );
    }

    #[test]
    fn a_crate_that_is_missing_and_a_stray_workspace_entry_are_reported() {
        let mut repo = repository(&[]);
        repo.crates.retain(|c| c.dir != "wanjiru-py");
        repo.workspace_dependencies.push(WorkspaceDependency {
            name: "wanjiru-ghost".to_owned(),
            path: Some("crates/wanjiru-ghost".to_owned()),
        });
        let found = messages(&repo).join("\n");
        assert!(
            found.contains("crates/wanjiru-py: the layer table lists this crate"),
            "{found}"
        );
        assert!(
            found.contains("`wanjiru-ghost` names a crate that does not exist"),
            "{found}"
        );
    }

    #[test]
    fn a_wrong_workspace_path_is_reported() {
        let mut repo = repository(&[]);
        repo.workspace_dependencies[0].path = Some("crates/elsewhere".to_owned());
        assert!(
            messages(&repo)
                .join("\n")
                .contains("expected \"crates/wanjiru-types\"")
        );
    }

    #[test]
    fn members_must_include_the_crates_glob() {
        let mut repo = repository(&[]);
        repo.members = vec!["tools/xtask".to_owned()];
        assert!(messages(&repo).join("\n").contains("\"crates/*\""));
    }

    #[test]
    fn a_cycle_through_an_exception_is_found() {
        // imgproc -> simd is allowed; simd -> imgproc is not, but a cycle must be named even so.
        let mut graph = BTreeMap::new();
        graph.insert("a".to_owned(), BTreeSet::from(["b".to_owned()]));
        graph.insert("b".to_owned(), BTreeSet::from(["c".to_owned()]));
        graph.insert("c".to_owned(), BTreeSet::from(["a".to_owned()]));
        assert_eq!(
            find_cycle(&graph),
            Some(vec![
                "a".to_owned(),
                "b".to_owned(),
                "c".to_owned(),
                "a".to_owned()
            ])
        );
        graph.remove("c");
        assert_eq!(find_cycle(&graph), None);
    }
}
