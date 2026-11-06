//! The `wanjiru` command.
//!
//! Everything it prints goes through [`app::run`], which returns text and an exit code and never
//! touches the terminal itself; this file only connects it to the file system and the process.

#![forbid(unsafe_code)]

mod app;
mod args;
mod cmd_docir;
mod cmd_errors;
mod cmd_eval;
#[cfg(test)]
mod testing_docs;

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = Vec::new();
    for argument in std::env::args_os().skip(1) {
        let Ok(text) = argument.into_string() else {
            let _ = writeln!(
                std::io::stderr(),
                "error[WNJ-E5001]: an argument is not valid UTF-8"
            );
            return ExitCode::from(2);
        };
        args.push(text);
    }
    let read = |path: &str| std::fs::read_to_string(path);
    let write = |path: &str, text: &str| std::fs::write(path, text);
    let outcome = app::run(
        &args,
        &app::Environment {
            read: &read,
            write: &write,
        },
    );
    // A closed pipe (`wanjiru eval ... | head`) is not an error worth a message.
    let _ = std::io::stdout().write_all(outcome.stdout.as_bytes());
    let _ = std::io::stderr().write_all(outcome.stderr.as_bytes());
    ExitCode::from(outcome.code)
}
