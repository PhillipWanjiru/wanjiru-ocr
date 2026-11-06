//! Minimal command-line parsing: positionals, `--name value` or `--name=value` options, and
//! `--switch` flags. No abbreviations, no repeated options, no silent ignoring of unknown flags.

use std::collections::{BTreeMap, BTreeSet};

use wanjiru_types::{ErrorKind, Result, WnjError, codes};

/// The options and switches a command knows.
#[derive(Copy, Clone)]
pub struct Spec {
    /// Names (without the dashes) of options that take a value.
    pub options: &'static [&'static str],
    /// Names of switches that do not.
    pub switches: &'static [&'static str],
}

/// A parsed command line.
#[derive(Debug, Default)]
pub struct Parsed {
    /// Arguments that are not options, in order.
    pub positionals: Vec<String>,
    options: BTreeMap<String, String>,
    switches: BTreeSet<String>,
}

pub fn usage(message: impl Into<String>) -> WnjError {
    WnjError::new(codes::USAGE, ErrorKind::InvalidInput, message)
}

impl Parsed {
    /// The value of an option, if it was given.
    pub fn option(&self, name: &str) -> Option<&str> {
        self.options.get(name).map(String::as_str)
    }

    /// Whether a switch was given.
    pub fn switch(&self, name: &str) -> bool {
        self.switches.contains(name)
    }

    /// The value of a required option.
    pub fn required(&self, name: &str) -> Result<&str> {
        self.option(name)
            .ok_or_else(|| usage(format!("missing required option --{name}")))
    }

    /// A numeric option with a default.
    pub fn number<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T> {
        match self.option(name) {
            None => Ok(default),
            Some(text) => text
                .parse()
                .map_err(|_| usage(format!("--{name}: `{text}` is not a valid number"))),
        }
    }
}

/// Parse `args` (without the program and command names) against `spec`.
pub fn parse(args: &[String], spec: &Spec) -> Result<Parsed> {
    let mut parsed = Parsed::default();
    let mut tokens = args.iter();
    while let Some(token) = tokens.next() {
        if token == "--" {
            parsed.positionals.extend(tokens.by_ref().cloned());
            break;
        }
        let Some(flag) = token.strip_prefix("--") else {
            if token.len() > 1 && token.starts_with('-') {
                return Err(usage(format!("unknown option `{token}`")));
            }
            parsed.positionals.push(token.clone());
            continue;
        };
        let (name, inline) = match flag.split_once('=') {
            Some((name, value)) => (name, Some(value.to_owned())),
            None => (flag, None),
        };
        if spec.switches.contains(&name) {
            if inline.is_some() {
                return Err(usage(format!("--{name} does not take a value")));
            }
            if !parsed.switches.insert(name.to_owned()) {
                return Err(usage(format!("--{name} given twice")));
            }
        } else if spec.options.contains(&name) {
            let value = match inline {
                Some(value) => value,
                None => tokens
                    .next()
                    .cloned()
                    .ok_or_else(|| usage(format!("--{name} needs a value")))?,
            };
            if parsed.options.insert(name.to_owned(), value).is_some() {
                return Err(usage(format!("--{name} given twice")));
            }
        } else {
            return Err(usage(format!("unknown option `--{name}`")));
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: Spec = Spec {
        options: &["seed", "out"],
        switches: &["strict"],
    };

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn options_switches_and_positionals() {
        let p = parse(&words("a --seed 7 b --out=x.json --strict c"), &SPEC).expect("valid");
        assert_eq!(p.positionals, ["a", "b", "c"]);
        assert_eq!(p.option("seed"), Some("7"));
        assert_eq!(p.option("out"), Some("x.json"));
        assert!(p.switch("strict"));
        assert!(!p.switch("other"));
        assert_eq!(p.number::<u64>("seed", 1).expect("number"), 7);
        assert_eq!(p.number::<u32>("resamples", 2000).expect("default"), 2000);
    }

    #[test]
    fn a_double_dash_ends_options() {
        let p = parse(&words("--strict -- --seed -x"), &SPEC).expect("valid");
        assert_eq!(p.positionals, ["--seed", "-x"]);
        assert!(p.switch("strict"));
    }

    #[test]
    fn mistakes_are_usage_errors() {
        for bad in [
            "--bogus",
            "-x",
            "--seed",
            "--seed 1 --seed 2",
            "--strict --strict",
            "--strict=yes",
            "--seed=1 --seed 2",
        ] {
            let e = parse(&words(bad), &SPEC).expect_err(bad);
            assert_eq!(e.code(), codes::USAGE, "{bad}");
        }
        let p = parse(&words("--seed abc"), &SPEC).expect("parses");
        assert!(p.number::<u64>("seed", 1).is_err());
        assert!(p.required("out").is_err());
    }

    #[test]
    fn a_lone_dash_is_a_positional() {
        let p = parse(&words("-"), &SPEC).expect("valid");
        assert_eq!(p.positionals, ["-"]);
    }
}
