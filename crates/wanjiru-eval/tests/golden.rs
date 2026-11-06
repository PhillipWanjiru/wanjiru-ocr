//! Cross-checks against an independent implementation.
//!
//! The data in `tests/golden/` comes from `tools/oracles/make_eval_vectors.py`, which runs
//! `tools/oracles/eval_oracle.py`: a second implementation of the harness that shares no code with this
//! crate. Dates come from Python's `datetime`, Clopper-Pearson intervals from SciPy's beta quantiles,
//! order-statistic ranks from exact rational arithmetic, YAML from PyYAML, the edit distance from a
//! plain dynamic programme. Bootstrap intervals are compared bit for bit: the random generator and the
//! order of floating-point operations are specified, so a difference of one bit is a bug on one side.
//! Whole reports are compared member by member; only the intervals of proportions, which are found by
//! different numerical methods, are allowed a difference of 1e-9.

#![allow(
    clippy::float_cmp,
    reason = "bit-for-bit agreement is what these tests check"
)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use wanjiru_docir::{FieldKind, Json, parse, to_canonical_string};
use wanjiru_eval::intervals::{Method, Resampling, ratio_interval, relative_change};
use wanjiru_eval::plan::{PlanInput, minimum_detectable_difference, plan};
use wanjiru_eval::run::{Inputs, evaluate};
use wanjiru_eval::validators::validate;
use wanjiru_eval::yaml;
use wanjiru_math::stats::{quantile_rank_interval, quantile_sorted};
use wanjiru_types::codes;

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

fn read(path: &PathBuf) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn records(name: &str) -> Vec<Json> {
    read(&golden(name))
        .lines()
        .map(|line| parse(line).unwrap_or_else(|e| panic!("{name}: {e}: {line}")))
        .collect()
}

fn unhex(text: &str) -> String {
    if text == "-" {
        return String::new();
    }
    let bytes: Vec<u8> = (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex byte"))
        .collect();
    String::from_utf8(bytes).expect("UTF-8")
}

fn number(value: &Json, key: &str) -> f64 {
    value
        .get(key)
        .and_then(Json::as_f64)
        .unwrap_or_else(|| panic!("number `{key}` in {value:?}"))
}

/// A non-negative integer stored as a JSON number.
fn unsigned(value: f64) -> u64 {
    assert!(
        value >= 0.0 && value.fract().abs() < f64::EPSILON && value < 9.0e15,
        "{value} is not a small count"
    );
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked on the lines above"
    )]
    let count = value as u64;
    count
}

fn count_of(value: &Json, key: &str) -> u64 {
    unsigned(number(value, key))
}

fn small_count(value: &Json, key: &str) -> u32 {
    u32::try_from(count_of(value, key)).expect("fits in 32 bits")
}

fn text<'a>(value: &'a Json, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Json::as_str)
        .unwrap_or_else(|| panic!("string `{key}`"))
}

fn counts(value: &Json, key: &str) -> Vec<u64> {
    value
        .get(key)
        .and_then(Json::as_array)
        .unwrap_or_else(|| panic!("array `{key}`"))
        .iter()
        .map(|n| unsigned(n.as_f64().expect("number")))
        .collect()
}

/// A number that the generator wrote as a number, `"inf"`, `"-inf"` or `null`.
fn float_or_special(value: &Json) -> Option<f64> {
    match value {
        Json::Null => None,
        Json::String(s) if s == "inf" => Some(f64::INFINITY),
        Json::String(s) if s == "-inf" => Some(f64::NEG_INFINITY),
        other => Some(other.as_f64().expect("number")),
    }
}

#[test]
fn validators_agree_with_the_independent_implementations() {
    let (mut valid, mut invalid) = (0, 0);
    for line in read(&golden("validators.txt")).lines() {
        let mut parts = line.split(' ');
        let kind = FieldKind::parse(parts.next().expect("kind")).expect("known kind");
        let verdict = parts.next().expect("verdict");
        let value = unhex(parts.next().expect("value"));
        let ours = validate(kind, &value);
        assert_eq!(
            ours.is_ok(),
            verdict == "ok",
            "{kind:?} {value:?}: {ours:?}"
        );
        if verdict == "ok" {
            valid += 1;
        } else {
            invalid += 1;
        }
    }
    assert!(valid >= 1000 && invalid >= 1000, "{valid} {invalid}");
}

#[test]
fn bootstrap_intervals_agree_to_the_last_bit() {
    let (mut checked, mut refused) = (0, 0);
    for case in records("intervals.jsonl") {
        let method = Method::parse(text(&case, "method")).expect("method");
        let settings = Resampling {
            level: number(&case, "level"),
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a small integer"
            )]
            resamples: small_count(&case, "resamples"),
            seed: text(&case, "seed").parse().expect("64-bit seed"),
            method,
        };
        let result = match text(&case, "kind") {
            "ratio" => ratio_interval(&counts(&case, "y"), &counts(&case, "x"), &settings),
            "change" => relative_change(&counts(&case, "new"), &counts(&case, "base"), &settings),
            other => panic!("unknown kind {other}"),
        };
        if case.get("error").is_some() {
            let error = result.expect_err("the oracle refuses this case");
            assert_eq!(error.code(), codes::STATS_ARGUMENT, "{case:?}");
            refused += 1;
            continue;
        }
        let got = result.unwrap_or_else(|e| panic!("{e}: {case:?}"));
        let want = case.get("expected").expect("expected");
        for (label, ours) in [
            ("estimate", got.estimate),
            ("lower", got.lower),
            ("upper", got.upper),
        ] {
            let theirs = float_or_special(want.get(label).expect("member")).expect("a number");
            assert_eq!(
                ours.to_bits(),
                theirs.to_bits(),
                "{label}: {ours} vs {theirs} in {case:?}"
            );
        }
        checked += 1;
    }
    assert!(checked >= 250 && refused >= 4, "{checked} {refused}");
}

#[test]
fn clopper_pearson_agrees_with_scipy() {
    let mut checked = 0;
    for case in records("proportions.jsonl") {
        let (k, n) = (count_of(&case, "k"), count_of(&case, "n"));
        let got = wanjiru_eval::intervals::proportion(k, n, number(&case, "level"));
        let want = case
            .get("expected")
            .and_then(Json::as_array)
            .expect("triple");
        match &want[0] {
            Json::Null => assert_eq!((n, got.estimate), (0, None)),
            estimate => assert_eq!(got.estimate, estimate.as_f64(), "{case:?}"),
        }
        for (label, ours, theirs) in [
            ("lower", got.lower, &want[1]),
            ("upper", got.upper, &want[2]),
        ] {
            let theirs = theirs.as_f64().expect("number");
            assert!(
                (ours - theirs).abs() <= 1e-9,
                "{label}: {ours} vs {theirs} in {case:?}"
            );
        }
        checked += 1;
    }
    assert!(checked >= 500);
}

#[test]
fn order_statistic_ranks_agree_with_exact_arithmetic() {
    let mut bounded_both = 0;
    for case in records("ranks.jsonl") {
        let (n, q, level) = (
            count_of(&case, "n"),
            number(&case, "q"),
            number(&case, "level"),
        );
        let (lower, upper) = quantile_rank_interval(n, q, 1.0 - level);
        let want = case.get("expected").and_then(Json::as_array).expect("pair");
        let as_rank = |value: &Json| value.as_f64().map(unsigned);
        assert_eq!(
            (lower, upper),
            (as_rank(&want[0]), as_rank(&want[1])),
            "{case:?}"
        );
        if lower.is_some() && upper.is_some() {
            bounded_both += 1;
        }
    }
    assert!(bounded_both >= 40, "{bounded_both}");
}

#[test]
fn sample_quantiles_agree_to_the_last_bit() {
    for case in records("quantiles.jsonl") {
        let values: Vec<f64> = case
            .get("values")
            .and_then(Json::as_array)
            .expect("values")
            .iter()
            .map(|v| v.as_f64().expect("number"))
            .collect();
        let got = quantile_sorted(&values, number(&case, "q"));
        assert_eq!(
            got.to_bits(),
            number(&case, "expected").to_bits(),
            "{case:?}"
        );
    }
}

#[test]
fn plans_agree_with_scipy() {
    let mut checked = 0;
    for case in records("plan.jsonl") {
        let input = PlanInput {
            p: number(&case, "p"),
            margin: number(&case, "margin"),
            confidence: number(&case, "confidence"),
            fields_per_document: small_count(&case, "fields_per_document"),
            icc: number(&case, "icc"),
        };
        let got = plan(&input).unwrap_or_else(|e| panic!("{e}: {case:?}"));
        let want = case.get("expected").expect("expected");
        assert!(
            (got.z - number(want, "z")).abs() <= 1e-12 * got.z.abs(),
            "{case:?}"
        );
        assert_eq!(
            got.independent_fields,
            count_of(want, "independent_fields"),
            "{case:?}"
        );
        assert_eq!(got.fields, count_of(want, "fields"), "{case:?}");
        assert_eq!(got.documents, count_of(want, "documents"), "{case:?}");
        assert_eq!(
            got.design_effect.to_bits(),
            number(want, "design_effect").to_bits(),
            "{case:?}"
        );
        let difference = case.get("difference").expect("difference");
        let ours = minimum_detectable_difference(
            input.p,
            number(difference, "effective_n"),
            number(difference, "alpha"),
            number(difference, "power"),
        )
        .expect("difference");
        let theirs = number(difference, "expected");
        assert!(
            (ours - theirs).abs() <= 1e-10 * theirs,
            "{ours} vs {theirs} in {case:?}"
        );
        checked += 1;
    }
    assert!(checked >= 100);
}

#[test]
fn the_yaml_reader_agrees_with_pyyaml() {
    let mut checked = 0;
    for case in records("yaml.jsonl") {
        let source = text(&case, "yaml");
        let ours = yaml::parse(source).unwrap_or_else(|e| panic!("{e}\n{source}"));
        let theirs = case.get("json").expect("json");
        assert_eq!(
            to_canonical_string(&ours).expect("canonical"),
            to_canonical_string(theirs).expect("canonical"),
            "{source}"
        );
        checked += 1;
    }
    assert!(checked >= 150);
}

fn member_names(members: &[(String, Json)]) -> BTreeSet<&str> {
    members.iter().map(|(k, _)| k.as_str()).collect()
}

/// Compare the report of this crate with the oracle's, member by member.
fn compare(path: &str, got: &Json, want: &Json, tolerant: bool, problems: &mut Vec<String>) {
    match (got, want) {
        (Json::Number(a), Json::Number(b)) => {
            let equal = if tolerant {
                (a - b).abs() <= 1e-9 * 1.0_f64.max(a.abs()).max(b.abs())
            } else {
                a.to_bits() == b.to_bits() || a == b
            };
            if !equal {
                problems.push(format!("{path}: {a} against {b}"));
            }
        }
        (Json::Object(a), Json::Object(b)) => {
            let (ka, kb) = (member_names(a), member_names(b));
            if ka != kb {
                problems.push(format!(
                    "{path}: members differ: only here {:?}, only there {:?}",
                    ka.difference(&kb).collect::<Vec<_>>(),
                    kb.difference(&ka).collect::<Vec<_>>()
                ));
            }
            for (key, theirs) in b {
                if let Some(ours) = got.get(key) {
                    compare(
                        &format!("{path}/{key}"),
                        ours,
                        theirs,
                        tolerant || path == "/fields",
                        problems,
                    );
                }
            }
        }
        (Json::Array(a), Json::Array(b)) => {
            if a.len() != b.len() {
                problems.push(format!("{path}: {} elements against {}", a.len(), b.len()));
            }
            for (i, (ours, theirs)) in a.iter().zip(b).enumerate() {
                // The intervals of proportions come from different numerical methods.
                let proportion_gate =
                    path == "/gates" && text(theirs, "id").starts_with("field_exact_match.");
                compare(
                    &format!("{path}/{i}"),
                    ours,
                    theirs,
                    tolerant || proportion_gate,
                    problems,
                );
            }
        }
        (a, b) => {
            if a != b {
                problems.push(format!("{path}: {a:?} against {b:?}"));
            }
        }
    }
}

#[test]
fn whole_reports_agree_with_the_oracle() {
    let mut names: Vec<String> = fs::read_dir(golden("cases"))
        .expect("cases directory")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("UTF-8 name")
        })
        .collect();
    names.sort();
    assert!(names.len() >= 12, "{names:?}");
    let (mut gate_verdicts, mut overall) = (BTreeSet::new(), BTreeSet::new());
    for name in &names {
        let directory = golden("cases").join(name);
        let contract = read(&directory.join("contract.yaml"));
        let truth = read(&directory.join("truth.jsonl"));
        let predictions = read(&directory.join("predictions.jsonl"));
        let baseline = directory
            .join("baseline.jsonl")
            .exists()
            .then(|| read(&directory.join("baseline.jsonl")));
        let settings = parse(&read(&directory.join("settings.json"))).expect("settings");
        let resampling = Resampling {
            level: number(&settings, "level"),
            resamples: small_count(&settings, "resamples"),
            seed: count_of(&settings, "seed"),
            method: Method::parse(text(&settings, "method")).expect("method"),
        };
        let expected = parse(&read(&directory.join("expected.json"))).expect("expected report");
        let report = evaluate(
            &Inputs {
                contract: &contract,
                truth: &truth,
                predictions: &predictions,
                baseline: baseline.as_deref(),
            },
            &resampling,
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut problems = Vec::new();
        compare("", &report.body(), &expected, false, &mut problems);
        assert!(problems.is_empty(), "{name}:\n{}", problems.join("\n"));
        for gate in expected
            .get("gates")
            .and_then(Json::as_array)
            .expect("gates")
        {
            gate_verdicts.insert(text(gate, "verdict").to_owned());
        }
        overall.insert(text(&expected, "verdict").to_owned());
        // The text and JSON forms exist for every case and are deterministic.
        assert_eq!(
            report.to_canonical_string().expect("json"),
            report.to_canonical_string().expect("json")
        );
        assert!(
            report
                .render_text()
                .contains(&format!("verdict: {}", text(&expected, "verdict"))),
            "{name}"
        );
    }
    let want = |items: &[&str]| {
        items
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        gate_verdicts,
        want(&["fail", "inconclusive", "not_measured", "pass"])
    );
    assert_eq!(overall, want(&["fail", "inconclusive", "pass"]));
}

/// The files of the repository: the real contracts and the synthetic fixtures that ship with it.
fn repository(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn the_fixture_runs_agree_with_the_oracle() {
    let fixtures = "eval/fixtures/synthetic-v0";
    let contracts = [
        ("invoice-fields-v1", "eval/contracts/invoice-fields-v1.yaml"),
        ("receipt-fields-v1", "eval/contracts/receipt-fields-v1.yaml"),
        ("id-mrz-v1", "eval/contracts/id-mrz-v1.yaml"),
        (
            "demo-lenient-invoice",
            "eval/fixtures/synthetic-v0/demo-lenient-invoice.yaml",
        ),
    ];
    let truth = read(&repository(&format!("{fixtures}/truth.jsonl")));
    let predictions = read(&repository(&format!("{fixtures}/predictions.jsonl")));
    let baseline = read(&repository(&format!("{fixtures}/baseline.jsonl")));
    let mut verdicts = BTreeSet::new();
    for (name, contract_path) in contracts {
        let contract = read(&repository(contract_path));
        let expected = parse(&read(&golden("fixtures").join(format!("{name}.json"))))
            .expect("expected report");
        let report = evaluate(
            &Inputs {
                contract: &contract,
                truth: &truth,
                predictions: &predictions,
                baseline: Some(&baseline),
            },
            &Resampling::DEFAULT,
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut problems = Vec::new();
        compare("", &report.body(), &expected, false, &mut problems);
        assert!(problems.is_empty(), "{name}:\n{}", problems.join("\n"));
        verdicts.insert(text(&expected, "verdict").to_owned());
    }
    // The four runs are meant to show every overall verdict a person can meet except `not_measured`.
    assert_eq!(
        verdicts,
        ["fail", "inconclusive", "pass"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<BTreeSet<_>>()
    );
}
