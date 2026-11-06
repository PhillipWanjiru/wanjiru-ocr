//! The command dispatcher: arguments in, [`Outcome`] out. File access goes through [`Environment`]
//! so that every command can be tested without touching the disk.

use wanjiru_types::{WnjError, codes};

use crate::{cmd_docir, cmd_errors, cmd_eval};

/// How commands read and write files.
pub struct Environment<'a> {
    /// Read a whole file as UTF-8 text.
    pub read: &'a dyn Fn(&str) -> std::io::Result<String>,
    /// Write a whole file.
    pub write: &'a dyn Fn(&str, &str) -> std::io::Result<()>,
}

impl Environment<'_> {
    /// Read a file, with a coded error naming it.
    pub fn read_file(&self, path: &str) -> Result<String, WnjError> {
        (self.read)(path).map_err(|e| {
            WnjError::new(
                codes::CANNOT_READ,
                wanjiru_types::ErrorKind::Io,
                format!("cannot read {path}: {e}"),
            )
        })
    }

    /// Write a file, with a coded error naming it.
    pub fn write_file(&self, path: &str, text: &str) -> Result<(), WnjError> {
        (self.write)(path, text).map_err(|e| {
            WnjError::new(
                codes::CANNOT_WRITE,
                wanjiru_types::ErrorKind::Io,
                format!("cannot write {path}: {e}"),
            )
        })
    }
}

/// What a command produced.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Exit code: 0 pass, 1 fail, 2 usage or input error, 3 inconclusive.
    pub code: u8,
    /// Text for standard output.
    pub stdout: String,
    /// Text for standard error.
    pub stderr: String,
}

impl Outcome {
    /// Success with output.
    pub fn ok(stdout: String) -> Self {
        Self {
            code: 0,
            stdout,
            stderr: String::new(),
        }
    }

    /// An error: exit code 2 and one line on standard error.
    pub fn error(error: &WnjError) -> Self {
        let location = error
            .path()
            .map_or_else(String::new, |path| format!(" ({path})"));
        let hint = if error.code() == codes::USAGE {
            "\nrun `wanjiru help` for the commands"
        } else {
            ""
        };
        Self {
            code: 2,
            stdout: String::new(),
            stderr: format!(
                "error[{}]: {}{location}{hint}\n",
                error.code(),
                error.message()
            ),
        }
    }
}

pub const HELP: &str = "\
wanjiru: the Wanjiru System command line

usage:
  wanjiru eval --contract FILE --truth FILE --pred FILE [--baseline FILE]
               [--level 0.95] [--resamples 2000] [--seed 1] [--method bootstrap-t|percentile]
               [--format text|json] [--out FILE] [--strict]
      Judge an engine's output by a quality contract. Exit code 0: pass, 1: fail,
      3: inconclusive (1 with --strict), 2: the inputs are unusable.
  wanjiru eval plan [--p 0.99] [--margin 0.005] [--confidence 0.95] [--fields-per-doc 10]
               [--icc 0.15] [--documents N] [--alpha 0.05] [--power 0.8]
      How many documents a golden set needs; with --documents, what difference it can detect.
  wanjiru eval contract FILE
      Check a contract and print what it demands.
  wanjiru docir check FILE...
      Validate DocIR documents (one JSON document, or JSON Lines for names ending in .jsonl).
  wanjiru docir canon FILE [--hash]
      Print the canonical form of a DocIR document, or its SHA-256.
  wanjiru errors [CODE]
      The catalogue of error codes (Markdown), or the description of one code.
  wanjiru version
  wanjiru help
";

fn version() -> String {
    format!(
        "wanjiru {} (DocIR {}, evaluation report {})\n",
        env!("CARGO_PKG_VERSION"),
        wanjiru_docir::SCHEMA_ID,
        wanjiru_eval::report::SCHEMA_ID
    )
}

fn dispatch(args: &[String], env: &Environment<'_>) -> Result<Outcome, WnjError> {
    let Some((command, rest)) = args.split_first() else {
        return Ok(Outcome::ok(HELP.to_owned()));
    };
    match command.as_str() {
        "help" | "--help" | "-h" => Ok(Outcome::ok(HELP.to_owned())),
        "version" | "--version" | "-V" => Ok(Outcome::ok(version())),
        "eval" => cmd_eval::run(rest, env),
        "docir" => cmd_docir::run(rest, env),
        "errors" => cmd_errors::run(rest),
        other => Err(crate::args::usage(format!("unknown command `{other}`"))),
    }
}

/// Run the command line `args` (without the program name).
pub fn run(args: &[String], env: &Environment<'_>) -> Outcome {
    dispatch(args, env).unwrap_or_else(|error| Outcome::error(&error))
}

#[cfg(test)]
pub(crate) mod testing {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::{Environment, Outcome, run};

    /// An in-memory file system.
    #[derive(Default)]
    pub struct Files(pub RefCell<BTreeMap<String, String>>);

    impl Files {
        pub fn with(files: &[(&str, &str)]) -> Self {
            Self(RefCell::new(
                files
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            ))
        }

        pub fn run(&self, command: &str) -> Outcome {
            let args: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
            let read = |path: &str| {
                self.0.borrow().get(path).cloned().ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::NotFound, "no such file")
                })
            };
            let write = |path: &str, text: &str| {
                self.0.borrow_mut().insert(path.to_owned(), text.to_owned());
                Ok(())
            };
            run(
                &args,
                &Environment {
                    read: &read,
                    write: &write,
                },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Files;
    use super::*;

    #[test]
    fn help_and_version() {
        let files = Files::default();
        for command in ["", "help", "--help", "-h"] {
            let out = files.run(command);
            assert_eq!((out.code, out.stdout.as_str()), (0, HELP), "{command:?}");
        }
        let out = files.run("version");
        assert_eq!(out.code, 0);
        assert!(
            out.stdout.starts_with(
                "wanjiru 0.1.0 (DocIR wanjiru.docir/0, evaluation report wanjiru.eval-report/0)"
            ),
            "{}",
            out.stdout
        );
    }

    #[test]
    fn unknown_commands_and_options_are_usage_errors() {
        let files = Files::default();
        for command in [
            "frobnicate",
            "eval --bogus",
            "docir",
            "docir explode f",
            "eval plan --p",
        ] {
            let out = files.run(command);
            assert_eq!(out.code, 2, "{command}");
            assert!(out.stdout.is_empty(), "{command}");
            assert!(
                out.stderr.starts_with("error[WNJ-E5001]"),
                "{command}: {}",
                out.stderr
            );
            assert!(
                out.stderr
                    .ends_with("run `wanjiru help` for the commands\n"),
                "{command}"
            );
        }
    }

    #[test]
    fn missing_files_are_reported_with_their_names() {
        let files = Files::default();
        let out = files.run("docir check nowhere.json");
        assert_eq!(out.code, 2);
        assert!(
            out.stderr
                .starts_with("error[WNJ-E5002]: cannot read nowhere.json"),
            "{}",
            out.stderr
        );
    }
}
