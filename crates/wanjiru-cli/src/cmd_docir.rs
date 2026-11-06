//! `wanjiru docir`: validate DocIR documents and print their canonical form.

use std::fmt::Write as _;
use std::path::Path;

use wanjiru_docir::Document;
use wanjiru_types::{ErrorKind, Result, WnjError};

use crate::app::{Environment, Outcome};
use crate::args::{Spec, parse, usage};

const CHECK: Spec = Spec {
    options: &[],
    switches: &[],
};

const CANON: Spec = Spec {
    options: &[],
    switches: &["hash"],
};

pub fn run(args: &[String], env: &Environment<'_>) -> Result<Outcome> {
    match args.split_first() {
        Some((first, rest)) if first == "check" => check(rest, env),
        Some((first, rest)) if first == "canon" => canon(rest, env),
        Some((other, _)) => Err(usage(format!(
            "unknown docir command `{other}`; try `check` or `canon`"
        ))),
        None => Err(usage("`docir` needs a command: `check` or `canon`")),
    }
}

fn located(error: &WnjError, place: &str) -> WnjError {
    let path = match error.path() {
        Some(inner) => format!("{place} {inner}"),
        None => place.to_owned(),
    };
    WnjError::new(error.code(), ErrorKind::InvalidInput, error.message()).at(path)
}

fn check(args: &[String], env: &Environment<'_>) -> Result<Outcome> {
    let parsed = parse(args, &CHECK)?;
    if parsed.positionals.is_empty() {
        return Err(usage("`docir check` needs at least one file"));
    }
    let mut out = String::new();
    let mut count = 0_usize;
    for file in &parsed.positionals {
        let text = env.read_file(file)?;
        if Path::new(file)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))
        {
            for (index, line) in text.split('\n').enumerate() {
                let line = line.strip_suffix('\r').unwrap_or(line);
                if line.trim().is_empty() {
                    continue;
                }
                let place = format!("{file}:{}", index + 1);
                let document = Document::from_json_str(line).map_err(|e| located(&e, &place))?;
                let _ = writeln!(
                    out,
                    "ok {place} {} {}",
                    document.id,
                    document.canonical_sha256()?
                );
                count += 1;
            }
        } else {
            let document = Document::from_json_str(&text).map_err(|e| located(&e, file))?;
            let _ = writeln!(
                out,
                "ok {file} {} {}",
                document.id,
                document.canonical_sha256()?
            );
            count += 1;
        }
    }
    let _ = writeln!(out, "{count} documents valid");
    Ok(Outcome::ok(out))
}

fn canon(args: &[String], env: &Environment<'_>) -> Result<Outcome> {
    let parsed = parse(args, &CANON)?;
    let [file] = parsed.positionals.as_slice() else {
        return Err(usage("`docir canon` takes exactly one file"));
    };
    let document = Document::from_json_str(&env.read_file(file)?).map_err(|e| located(&e, file))?;
    let text = if parsed.switch("hash") {
        document.canonical_sha256()?
    } else {
        document.to_canonical_string()?
    };
    Ok(Outcome::ok(text + "\n"))
}

#[cfg(test)]
mod tests {
    use crate::app::testing::Files;

    fn document(id: &str) -> String {
        crate::testing_docs::document(id, "Total 10.00", "10.00", 12.5)
    }

    #[test]
    fn check_validates_single_documents_and_json_lines() {
        let one = document("a");
        let two = format!("{}\n\n{}\r\n", document("b"), document("c"));
        let f = Files::with(&[("one.json", &one), ("two.jsonl", &two)]);
        let out = f.run("docir check one.json two.jsonl");
        assert_eq!(out.code, 0, "{}", out.stderr);
        let lines: Vec<&str> = out.stdout.lines().collect();
        assert_eq!(lines.len(), 4);
        assert!(
            lines[0].starts_with("ok one.json a ") && lines[0].len() == "ok one.json a ".len() + 64
        );
        assert!(
            lines[1].starts_with("ok two.jsonl:1 b ") && lines[2].starts_with("ok two.jsonl:3 c ")
        );
        assert_eq!(lines[3], "3 documents valid");
    }

    #[test]
    fn check_names_the_place_of_the_first_problem() {
        let broken = document("x").replace("\"index\":0", "\"index\":2");
        let f = Files::with(&[
            ("ok.json", &document("a")),
            ("bad.jsonl", &format!("{}\n{broken}\n", document("b"))),
        ]);
        let out = f.run("docir check ok.json bad.jsonl");
        assert_eq!(out.code, 2);
        assert!(out.stdout.is_empty());
        assert!(out.stderr.starts_with("error[WNJ-E1003]"), "{}", out.stderr);
        assert!(
            out.stderr.contains("(bad.jsonl:2 /pages/0/index)"),
            "{}",
            out.stderr
        );
        assert_eq!(f.run("docir check").code, 2);
    }

    #[test]
    fn canon_prints_the_canonical_text_or_its_hash() {
        let doc = document("a");
        let f = Files::with(&[("d.json", &doc)]);
        let text = f.run("docir canon d.json");
        let hash = f.run("docir canon d.json --hash");
        assert_eq!((text.code, hash.code), (0, 0));
        let canonical = text.stdout.trim_end();
        assert_eq!(
            wanjiru_docir::to_canonical_string(&wanjiru_docir::parse(&doc).expect("json"))
                .expect("canonical"),
            canonical
        );
        assert_eq!(
            wanjiru_math::digest::sha256_hex(canonical.as_bytes()),
            hash.stdout.trim_end()
        );
        assert_eq!(f.run("docir canon").code, 2);
        assert_eq!(f.run("docir canon d.json d.json").code, 2);
    }
}
