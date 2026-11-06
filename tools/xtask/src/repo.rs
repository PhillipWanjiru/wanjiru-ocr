//! The repository as the checks see it: manifests, contracts and sources, read once.
//!
//! Every check works on a [`Repository`] value, not on the file system, so each can be tested with a
//! few lines of in-memory text, including deliberately broken ones.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::toml::{self, Document, Table, Value};

/// How a dependency is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `[dependencies]`
    Normal,
    /// `[dev-dependencies]`
    Dev,
    /// `[build-dependencies]`
    Build,
}

impl Kind {
    /// The name of the manifest section.
    pub fn section(self) -> &'static str {
        match self {
            Self::Normal => "dependencies",
            Self::Dev => "dev-dependencies",
            Self::Build => "build-dependencies",
        }
    }
}

/// One entry of a dependency table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// The package name (the `package = "..."` rename if there is one).
    pub name: String,
    /// Normal, development or build.
    pub kind: Kind,
    /// Declared as `{ workspace = true }`: version and source come from the workspace manifest.
    pub inherited: bool,
}

/// What the checks need from a crate's `Cargo.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// `[package] name`.
    pub name: String,
    /// All dependencies of all kinds, target-specific ones included.
    pub dependencies: Vec<Dependency>,
    /// `[lints] workspace = true`.
    pub inherits_lints: bool,
    /// `license`: the string, or `None` when absent or inherited (`license.workspace = true`).
    pub license: Option<String>,
}

fn dependencies_of(table: &Table, kind: Kind, out: &mut Vec<Dependency>) {
    for (key, value) in table {
        let (name, inherited) = match value {
            Value::Table(t) => (
                t.get("package")
                    .and_then(Value::as_str)
                    .unwrap_or(key)
                    .to_owned(),
                t.get("workspace").and_then(Value::as_bool) == Some(true),
            ),
            _ => (key.clone(), false),
        };
        out.push(Dependency {
            name,
            kind,
            inherited,
        });
    }
}

impl Manifest {
    /// Read the parts of a manifest the checks use.
    pub fn parse(text: &str) -> Result<Self, String> {
        let document = toml::parse(text)?;
        let name = document
            .get(&["package", "name"])
            .and_then(Value::as_str)
            .ok_or("the manifest has no [package] name")?
            .to_owned();
        let mut dependencies = Vec::new();
        for kind in [Kind::Normal, Kind::Dev, Kind::Build] {
            if let Some(table) = document.table(&[kind.section()]) {
                dependencies_of(table, kind, &mut dependencies);
            }
            if let Some(targets) = document.table(&["target"]) {
                for target in targets.values().filter_map(Value::as_table) {
                    if let Some(table) = target.get(kind.section()).and_then(Value::as_table) {
                        dependencies_of(table, kind, &mut dependencies);
                    }
                }
            }
        }
        dependencies.sort_by(|a, b| {
            (a.name.as_str(), a.kind.section()).cmp(&(b.name.as_str(), b.kind.section()))
        });
        Ok(Self {
            name,
            dependencies,
            inherits_lints: document
                .get(&["lints", "workspace"])
                .and_then(Value::as_bool)
                == Some(true),
            license: document
                .get(&["package", "license"])
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }
}

/// A Rust source file of a crate.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Path from the repository root with `/` separators, for messages.
    pub path: String,
    /// The text.
    pub text: String,
}

/// One crate under `crates/`.
#[derive(Debug, Clone)]
pub struct CrateInfo {
    /// The directory name.
    pub dir: String,
    /// The manifest, or why it could not be read.
    pub manifest: Result<Manifest, String>,
    /// The text of `CONTRACT.md`, if the file exists.
    pub contract: Option<String>,
    /// Every `.rs` file under the crate directory, in path order.
    pub sources: Vec<SourceFile>,
}

/// One line of `[workspace.dependencies]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDependency {
    /// The key.
    pub name: String,
    /// The `path`, for crates of this repository.
    pub path: Option<String>,
}

/// Everything the checks read.
#[derive(Debug, Clone, Default)]
pub struct Repository {
    /// `[workspace] members` of the root manifest.
    pub members: Vec<String>,
    /// `[workspace.dependencies]` of the root manifest.
    pub workspace_dependencies: Vec<WorkspaceDependency>,
    /// The crates, sorted by directory name.
    pub crates: Vec<CrateInfo>,
    /// Every file and directory of the repository except `target` and `.git`, as paths from the root with
    /// `/` separators; directories end without a slash.
    pub paths: BTreeSet<String>,
}

impl Repository {
    /// The root manifest's view.
    pub fn parse_root(&mut self, text: &str) -> Result<(), String> {
        let document: Document = toml::parse(text)?;
        self.members = document
            .get(&["workspace", "members"])
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        self.workspace_dependencies = document
            .table(&["workspace", "dependencies"])
            .map(|table| {
                table
                    .iter()
                    .map(|(name, value)| WorkspaceDependency {
                        name: name.clone(),
                        path: value
                            .as_table()
                            .and_then(|t| t.get("path"))
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(())
    }

    /// Read the repository at `root`.
    pub fn load(root: &Path) -> Result<Self, String> {
        let mut repo = Self::default();
        let manifest = read(&root.join("Cargo.toml"))?;
        repo.parse_root(&manifest)
            .map_err(|e| format!("Cargo.toml: {e}"))?;
        collect_paths(root, root, &mut repo.paths)?;
        let crates_dir = root.join("crates");
        let mut dirs: Vec<String> = fs::read_dir(&crates_dir)
            .map_err(|e| format!("{}: {e}", crates_dir.display()))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        dirs.sort();
        for dir in dirs {
            let base = crates_dir.join(&dir);
            let manifest = read(&base.join("Cargo.toml"))
                .and_then(|text| Manifest::parse(&text).map_err(|e| format!("Cargo.toml: {e}")));
            let contract = read(&base.join("CONTRACT.md")).ok();
            let mut sources = Vec::new();
            collect_sources(root, &base, &mut sources)?;
            repo.crates.push(CrateInfo {
                dir,
                manifest,
                contract,
                sources,
            });
        }
        Ok(repo)
    }
}

fn collect_paths(root: &Path, dir: &Path, out: &mut BTreeSet<String>) -> Result<(), String> {
    for entry in fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path
            .file_name()
            .is_some_and(|name| name == "target" || name == ".git")
        {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        out.insert(relative);
        if path.is_dir() {
            collect_paths(root, &path, out)?;
        }
    }
    Ok(())
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn collect_sources(root: &Path, dir: &Path, out: &mut Vec<SourceFile>) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect_sources(root, &path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push(SourceFile {
                path: relative,
                text: read(&path)?,
            });
        }
    }
    Ok(())
}

/// Builders for tests: a repository of a few small crates, without touching the disk.
#[cfg(test)]
pub mod testing {
    use std::fmt::Write as _;

    use super::*;

    /// A crate with the given manifest dependencies (`name` or `name:dev`) and sources.
    pub fn krate(
        dir: &str,
        dependencies: &[&str],
        contract: Option<&str>,
        sources: &[(&str, &str)],
    ) -> CrateInfo {
        let mut manifest = format!(
            "[package]\nname = \"{dir}\"\nversion.workspace = true\n\n[lints]\nworkspace = true\n\n[dependencies]\n"
        );
        let mut dev = String::from("\n[dev-dependencies]\n");
        for dependency in dependencies {
            match dependency.split_once(':') {
                Some((name, "dev")) => {
                    let _ = writeln!(dev, "{name} = {{ workspace = true }}");
                }
                _ => {
                    let _ = writeln!(manifest, "{dependency} = {{ workspace = true }}");
                }
            }
        }
        manifest.push_str(&dev);
        CrateInfo {
            dir: dir.to_owned(),
            manifest: Manifest::parse(&manifest),
            contract: contract.map(str::to_owned),
            sources: sources
                .iter()
                .map(|(path, text)| SourceFile {
                    path: format!("crates/{dir}/{path}"),
                    text: (*text).to_owned(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_dependencies_of_every_kind_and_target() {
        let m = Manifest::parse(
            r#"
[package]
name = "demo"
license = "MIT"

[dependencies]
a = { workspace = true }
b = "1"
c = { package = "real-c", version = "2" }

[dev-dependencies]
d = { workspace = true }

[build-dependencies.e]
version = "1"

[target.'cfg(unix)'.dependencies]
libc = "0.2"
"#,
        )
        .expect("a manifest");
        let listed: Vec<(&str, &str, bool)> = m
            .dependencies
            .iter()
            .map(|d| (d.name.as_str(), d.kind.section(), d.inherited))
            .collect();
        assert_eq!(
            listed,
            [
                ("a", "dependencies", true),
                ("b", "dependencies", false),
                ("d", "dev-dependencies", true),
                ("e", "build-dependencies", false),
                ("libc", "dependencies", false),
                ("real-c", "dependencies", false),
            ]
        );
        assert!(!m.inherits_lints);
        assert_eq!(m.license.as_deref(), Some("MIT"));
    }

    #[test]
    fn a_manifest_without_a_name_is_an_error() {
        assert!(Manifest::parse("[dependencies]\n").is_err());
    }

    #[test]
    fn the_root_manifest_lists_members_and_workspace_dependencies() {
        let mut repo = Repository::default();
        repo.parse_root(
            "[workspace]\nmembers = [\"crates/*\", \"tools/xtask\"]\n[workspace.dependencies]\nwanjiru-types = { path = \"crates/wanjiru-types\", version = \"0.1.0\" }\nserde = \"1\"\n",
        )
        .expect("root");
        assert_eq!(repo.members, ["crates/*", "tools/xtask"]);
        assert_eq!(
            repo.workspace_dependencies,
            [
                WorkspaceDependency {
                    name: "serde".to_owned(),
                    path: None
                },
                WorkspaceDependency {
                    name: "wanjiru-types".to_owned(),
                    path: Some("crates/wanjiru-types".to_owned())
                },
            ]
        );
    }
}
