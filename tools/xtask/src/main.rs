//! `cargo xtask`: the checks that keep the repository's architecture honest.
//!
//! ```text
//! cargo xtask check             all of the checks below
//! cargo xtask check-layers      crates depend only on lower layers; workspace manifests agree
//! cargo xtask check-contracts   every crate has a CONTRACT.md that agrees with its manifest
//! cargo xtask check-unsafe      `unsafe` only in the zones, each use explained
//! cargo xtask check-determinism no platform maths in the crates that promise identical bytes
//! cargo xtask licences          licences of the dependency graph against deny.toml
//! ```
//!
//! Options: `--root DIR` checks another copy of the repository; `--metadata FILE` (licences) reads a
//! saved `cargo metadata` output instead of running cargo. Exit code 0: nothing found; 1: violations;
//! 2: the repository could not be read or the arguments were wrong.

#![forbid(unsafe_code)]
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "this program's job is to print what it found"
)]

mod contracts;
mod determinism;
mod json;
mod layers;
mod lexer;
mod licences;
mod repo;
mod report;
mod toml;
mod unsafe_code;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use repo::Repository;
use report::Violation;

const USAGE: &str = "usage: cargo xtask <check | check-layers | check-contracts | check-unsafe | check-determinism | licences> [--root DIR] [--metadata FILE]";

struct Options {
    command: String,
    root: PathBuf,
    metadata: Option<PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut command = None;
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut metadata = None;
    let mut iter = args.iter();
    while let Some(argument) = iter.next() {
        match argument.as_str() {
            "--root" => root = PathBuf::from(iter.next().ok_or("--root needs a directory")?),
            "--metadata" => {
                metadata = Some(PathBuf::from(iter.next().ok_or("--metadata needs a file")?));
            }
            "help" | "--help" | "-h" => return Err(USAGE.to_owned()),
            other if other.starts_with("--") => {
                return Err(format!("unknown option `{other}`\n{USAGE}"));
            }
            other if command.is_none() => command = Some(other.to_owned()),
            other => return Err(format!("unexpected argument `{other}`\n{USAGE}")),
        }
    }
    Ok(Options {
        command: command.ok_or_else(|| USAGE.to_owned())?,
        root,
        metadata,
    })
}

/// The output of `cargo metadata` for the workspace at `root`, or the saved file.
fn metadata(options: &Options) -> Result<json::Json, String> {
    let text = if let Some(path) = &options.metadata {
        std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?
    } else {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .args([
                "metadata",
                "--format-version",
                "1",
                "--offline",
                "--manifest-path",
            ])
            .arg(options.root.join("Cargo.toml"))
            .output()
            .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        String::from_utf8(output.stdout)
            .map_err(|e| format!("cargo metadata printed invalid UTF-8: {e}"))?
    };
    json::parse(&text)
}

fn check_licences(options: &Options) -> Result<Vec<Violation>, String> {
    let deny = std::fs::read_to_string(options.root.join("deny.toml"))
        .map_err(|e| format!("deny.toml: {e}"))?;
    let allow = licences::read_allow_list(&deny)?;
    licences::check(&metadata(options)?, &allow, "Apache-2.0")
}

fn load(root: &Path) -> Result<Repository, String> {
    Repository::load(root)
}

fn run(options: &Options) -> Result<(Vec<Violation>, Vec<String>), String> {
    let mut violations = Vec::new();
    let mut notes = Vec::new();
    let command = options.command.as_str();
    if !matches!(
        command,
        "check"
            | "check-layers"
            | "check-contracts"
            | "check-unsafe"
            | "check-determinism"
            | "licences"
    ) {
        return Err(format!("unknown command `{command}`\n{USAGE}"));
    }
    if command != "licences" {
        let repository = load(&options.root)?;
        if matches!(command, "check" | "check-layers") {
            violations.extend(layers::check(&repository));
            notes.push(format!(
                "layers: {} crates in six layers",
                repository.crates.len()
            ));
            for exception in layers::SAME_LAYER_EXCEPTIONS {
                notes.push(format!(
                    "allowed same-layer dependency {} -> {} ({})",
                    exception.from, exception.to, exception.reason
                ));
            }
        }
        if matches!(command, "check" | "check-contracts") {
            violations.extend(contracts::check(&repository));
            notes.push(format!("contracts: {} crates", repository.crates.len()));
        }
        if matches!(command, "check" | "check-unsafe") {
            violations.extend(unsafe_code::check(&repository));
            let files: usize = repository.crates.iter().map(|c| c.sources.len()).sum();
            notes.push(format!(
                "unsafe: {files} source files in {} crates, {} unsafe zones",
                repository.crates.len(),
                layers::UNSAFE_ZONES.len()
            ));
        }
        if matches!(command, "check" | "check-determinism") {
            violations.extend(determinism::check(&repository));
            notes.push(format!(
                "determinism: {}",
                determinism::DETERMINISTIC_CRATES.join(", ")
            ));
        }
    }
    if matches!(command, "check" | "licences") {
        violations.extend(check_licences(options)?);
        notes.push("licences: dependency graph against deny.toml".to_owned());
    }
    Ok((violations, notes))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    match run(&options) {
        Ok((violations, notes)) => {
            for violation in &violations {
                println!("{violation}");
            }
            if violations.is_empty() {
                println!("xtask {}: ok", options.command);
                for note in &notes {
                    println!("  {note}");
                }
                ExitCode::SUCCESS
            } else {
                println!(
                    "xtask {}: {} violation(s)",
                    options.command,
                    violations.len()
                );
                ExitCode::from(1)
            }
        }
        Err(message) => {
            eprintln!("xtask {}: {message}", options.command);
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn arguments_are_parsed() {
        let o = parse_args(&args(&[
            "check",
            "--root",
            "/tmp/x",
            "--metadata",
            "m.json",
        ]))
        .expect("options");
        assert_eq!(
            (o.command.as_str(), o.root, o.metadata),
            (
                "check",
                PathBuf::from("/tmp/x"),
                Some(PathBuf::from("m.json"))
            )
        );
        for bad in [
            &[][..],
            &["check", "--nope"],
            &["check", "extra"],
            &["--root"],
            &["help"],
        ] {
            assert!(parse_args(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn an_unknown_command_is_an_error() {
        let options = parse_args(&args(&["frobnicate"])).expect("options");
        assert!(
            run(&options)
                .expect_err("unknown")
                .contains("unknown command")
        );
    }

    /// The repository this program lives in obeys its own rules. This is the check that CI runs, and
    /// the reason a layering mistake in any crate fails `cargo test --workspace` as well.
    #[test]
    fn this_repository_passes_its_own_architecture_checks() {
        let options = Options {
            command: "check-layers".to_owned(),
            root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
            metadata: None,
        };
        let repository = load(&options.root).expect("repository");
        let mut found: Vec<String> = Vec::new();
        found.extend(layers::check(&repository).iter().map(ToString::to_string));
        found.extend(
            contracts::check(&repository)
                .iter()
                .map(ToString::to_string),
        );
        found.extend(
            unsafe_code::check(&repository)
                .iter()
                .map(ToString::to_string),
        );
        found.extend(
            determinism::check(&repository)
                .iter()
                .map(ToString::to_string),
        );
        assert!(found.is_empty(), "{}", found.join("\n"));
        assert_eq!(repository.crates.len(), 25);
    }
}
