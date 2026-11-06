//! The command as a person meets it: the real binary, its arguments, what it prints and how it exits.
//!
//! `src/app.rs` is tested in process, without a terminal. These tests start the program the way a
//! shell does and read the repository's own contracts and synthetic fixtures, so they would fail if a
//! contract stopped parsing, if the fixtures drifted from what the contracts expect, or if the text of
//! a report changed without anyone deciding that it should.
//!
//! The four runs of `eval/fixtures/synthetic-v0` are stored as transcripts in
//! `eval/fixtures/synthetic-v0/expected/`: the command, the exit code and the standard output, which
//! ends with the SHA-256 of the whole report, so a change in any number is a change in the transcript.
//! The numbers themselves are checked against an independent implementation by
//! `crates/wanjiru-eval/tests/golden.rs`; a transcript only pins how they are shown. When a change of
//! the text is intended, regenerate the transcripts and read the diff:
//!
//! ```text
//! WANJIRU_BLESS=1 cargo test -p wanjiru-cli --test transcripts
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use wanjiru_docir::{Json, canonical_sha256, parse, to_canonical_string};

const FIXTURES: &str = "eval/fixtures/synthetic-v0";

/// The root of the repository: the programs are started there, as the documentation tells people to.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn wanjiru(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wanjiru"))
        .args(args)
        .current_dir(root())
        .output()
        .expect("the wanjiru binary starts")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("standard output is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("standard error is UTF-8")
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("the program exits normally")
}

/// One of the four documented runs.
struct Run {
    name: &'static str,
    contract: &'static str,
    exit_code: i32,
}

const RUNS: [Run; 4] = [
    Run {
        name: "invoice-fields-v1",
        contract: "eval/contracts/invoice-fields-v1.yaml",
        exit_code: 3,
    },
    Run {
        name: "receipt-fields-v1",
        contract: "eval/contracts/receipt-fields-v1.yaml",
        exit_code: 1,
    },
    Run {
        name: "id-mrz-v1",
        contract: "eval/contracts/id-mrz-v1.yaml",
        exit_code: 1,
    },
    Run {
        name: "demo-lenient-invoice",
        contract: "eval/fixtures/synthetic-v0/demo-lenient-invoice.yaml",
        exit_code: 0,
    },
];

fn eval_arguments(contract: &'static str) -> Vec<String> {
    let files = |name: &str| format!("{FIXTURES}/{name}.jsonl");
    vec![
        "eval".to_owned(),
        "--contract".to_owned(),
        contract.to_owned(),
        "--truth".to_owned(),
        files("truth"),
        "--pred".to_owned(),
        files("predictions"),
        "--baseline".to_owned(),
        files("baseline"),
    ]
}

fn run_eval(extra: &[&str], contract: &'static str) -> Output {
    let mut arguments = eval_arguments(contract);
    arguments.extend(extra.iter().map(|s| (*s).to_owned()));
    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    wanjiru(&borrowed)
}

fn transcript(run: &Run, output: &Output) -> String {
    format!(
        "$ wanjiru {}\n# exit code {}\n\n{}",
        eval_arguments(run.contract).join(" "),
        code(output),
        stdout(output)
    )
}

#[test]
fn the_four_documented_runs_print_what_the_transcripts_say() {
    let bless = std::env::var_os("WANJIRU_BLESS").is_some();
    for run in &RUNS {
        let output = run_eval(&[], run.contract);
        assert_eq!(code(&output), run.exit_code, "{}", run.name);
        assert_eq!(
            stderr(&output),
            "",
            "{}: nothing on standard error",
            run.name
        );
        let got = transcript(run, &output);
        let path = root().join(format!("{FIXTURES}/expected/{}.txt", run.name));
        if bless {
            fs::create_dir_all(path.parent().expect("a directory")).expect("create expected/");
            fs::write(&path, &got).expect("write the transcript");
            continue;
        }
        let want = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (WANJIRU_BLESS=1 writes it)", path.display()));
        assert_eq!(
            got, want,
            "{}: the transcript changed; if that is intended, run with WANJIRU_BLESS=1 and read the diff",
            run.name
        );
    }
}

#[test]
fn the_four_runs_between_them_show_every_verdict_a_person_can_meet() {
    let mut verdicts: Vec<String> = RUNS
        .iter()
        .map(|run| {
            let text = stdout(&run_eval(&[], run.contract));
            text.lines()
                .find_map(|line| line.strip_prefix("verdict: "))
                .expect("a verdict line")
                .to_owned()
        })
        .collect();
    verdicts.sort();
    verdicts.dedup();
    assert_eq!(verdicts, ["fail", "inconclusive", "pass"]);
}

#[test]
fn strict_makes_an_inconclusive_run_fail_and_changes_nothing_else() {
    let plain = run_eval(&[], RUNS[0].contract);
    let strict = run_eval(&["--strict"], RUNS[0].contract);
    assert_eq!((code(&plain), code(&strict)), (3, 1));
    assert_eq!(stdout(&plain), stdout(&strict));
    // A run that passes or fails outright is not affected.
    assert_eq!(code(&run_eval(&["--strict"], RUNS[3].contract)), 0);
    assert_eq!(code(&run_eval(&["--strict"], RUNS[1].contract)), 1);
}

#[test]
fn the_json_report_is_canonical_and_carries_the_hash_the_text_report_prints() {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("transcripts");
    fs::create_dir_all(&directory).expect("a scratch directory");
    let out = directory.join("report.json");
    let out_text = out.to_str().expect("UTF-8 path");
    let json = run_eval(&["--format", "json", "--out", out_text], RUNS[0].contract);
    assert_eq!(code(&json), 3);
    let text = run_eval(&[], RUNS[0].contract);

    // Standard output and the file written by `--out` are the same canonical line.
    let printed = stdout(&json);
    assert_eq!(printed, fs::read_to_string(&out).expect("the report file"));
    assert!(printed.ends_with('\n') && !printed[..printed.len() - 1].contains('\n'));
    let report = parse(&printed).expect("the report is JSON");
    assert_eq!(
        to_canonical_string(&report).expect("canonical") + "\n",
        printed
    );

    // The report ends with the hash of everything else, and the text form prints the same hash.
    let hash = report
        .get("report_sha256")
        .and_then(Json::as_str)
        .expect("report_sha256")
        .to_owned();
    let body = Json::Object(
        report
            .as_object()
            .expect("an object")
            .iter()
            .filter(|(key, _)| key != "report_sha256")
            .cloned()
            .collect(),
    );
    assert_eq!(canonical_sha256(&body).expect("hash"), hash);
    assert!(
        stdout(&text).contains(&format!("report sha256: {hash}\n")),
        "the text report names the same hash"
    );
}

#[test]
fn the_same_inputs_give_the_same_bytes_and_a_different_seed_gives_different_intervals() {
    let first = stdout(&run_eval(&["--format", "json"], RUNS[0].contract));
    let again = stdout(&run_eval(&["--format", "json"], RUNS[0].contract));
    let other = stdout(&run_eval(
        &["--format", "json", "--seed", "2"],
        RUNS[0].contract,
    ));
    assert_eq!(first, again);
    assert_ne!(first, other);
}

#[test]
fn every_synthetic_document_is_valid_docir() {
    for name in ["predictions", "baseline"] {
        let file = format!("{FIXTURES}/{name}.jsonl");
        let output = wanjiru(&["docir", "check", &file]);
        assert_eq!(code(&output), 0, "{name}: {}", stderr(&output));
        let last = stdout(&output)
            .lines()
            .last()
            .expect("a summary line")
            .to_owned();
        assert_eq!(last, "300 documents valid", "{name}");
    }
}

#[test]
fn a_broken_document_is_named_by_file_line_and_code() {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("transcripts");
    fs::create_dir_all(&directory).expect("a scratch directory");
    let source = fs::read_to_string(root().join(format!("{FIXTURES}/predictions.jsonl")))
        .expect("the fixture");
    let mut lines: Vec<String> = source.lines().take(3).map(str::to_owned).collect();
    // The second document loses its schema member.
    lines[1] = lines[1].replacen("\"schema\":\"wanjiru.docir/0\",", "", 1);
    let file = directory.join("broken.jsonl");
    fs::write(&file, lines.join("\n") + "\n").expect("write");
    let output = wanjiru(&["docir", "check", file.to_str().expect("UTF-8 path")]);
    assert_eq!(code(&output), 2);
    let errors = stderr(&output);
    assert!(
        errors.starts_with("error[WNJ-E"),
        "a coded error on standard error: {errors}"
    );
    assert!(errors.contains("broken.jsonl:2"), "{errors}");
}

#[test]
fn canon_prints_the_form_that_hashes_to_what_check_reports() {
    let source = fs::read_to_string(root().join(format!("{FIXTURES}/predictions.jsonl")))
        .expect("the fixture");
    let first = source.lines().next().expect("a document");
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("transcripts");
    fs::create_dir_all(&directory).expect("a scratch directory");
    let file = directory.join("one.json");
    fs::write(&file, first).expect("write");
    let path = file.to_str().expect("UTF-8 path");
    let canon = stdout(&wanjiru(&["docir", "canon", path]));
    let hash = stdout(&wanjiru(&["docir", "canon", path, "--hash"]));
    let checked = stdout(&wanjiru(&["docir", "check", path]));
    assert_eq!(
        canonical_sha256(&parse(&canon).expect("canonical text is JSON")).expect("hash"),
        hash.trim()
    );
    assert!(checked.contains(hash.trim()), "{checked}");
}

#[test]
fn the_contracts_of_the_repository_are_accepted_and_a_bad_one_is_refused_with_its_location() {
    for run in &RUNS {
        let output = wanjiru(&["eval", "contract", run.contract]);
        assert_eq!(code(&output), 0, "{}: {}", run.name, stderr(&output));
        assert!(
            stdout(&output).starts_with(&format!("contract {} (", run.name)),
            "{}: {}",
            run.name,
            stdout(&output)
        );
    }
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("transcripts");
    fs::create_dir_all(&directory).expect("a scratch directory");
    let bad = directory.join("bad.yaml");
    fs::write(
        &bad,
        "contract: bad\nclass: invoice\nstatus: draft\ncorpus: golden/bad\nmetrics:\n  field_exact_match:\n    total_amount: {kind: amount, min: 1.5, coverage: 0.9}\ngates:\n  confidence_interval: bootstrap-by-document\n",
    )
    .expect("write");
    let output = wanjiru(&["eval", "contract", bad.to_str().expect("UTF-8 path")]);
    assert_eq!(code(&output), 2);
    assert_eq!(stdout(&output), "");
    let errors = stderr(&output);
    assert!(errors.starts_with("error[WNJ-E4002]"), "{errors}");
    assert!(errors.contains("total_amount"), "{errors}");
}

#[test]
fn problems_with_the_command_line_leave_standard_output_empty_and_exit_with_2() {
    let cases: [(&[&str], &str); 5] = [
        (&["frobnicate"], "error[WNJ-E5001]"),
        (&["eval"], "error[WNJ-E5001]"),
        (
            &[
                "eval",
                "--contract",
                "no-such-file.yaml",
                "--truth",
                "a",
                "--pred",
                "b",
            ],
            "error[WNJ-E5002]",
        ),
        (&["eval", "--format", "xml"], "error[WNJ-E5001]"),
        (&["docir", "check"], "error[WNJ-E5001]"),
    ];
    for (arguments, expected) in cases {
        let output = wanjiru(arguments);
        assert_eq!(code(&output), 2, "{arguments:?}");
        assert_eq!(stdout(&output), "", "{arguments:?}");
        assert!(
            stderr(&output).starts_with(expected),
            "{arguments:?}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn help_and_version_say_what_they_are_for() {
    let help = wanjiru(&["help"]);
    assert_eq!(code(&help), 0);
    assert!(stdout(&help).starts_with("wanjiru: the Wanjiru System command line\n"));
    assert_eq!(
        stdout(&wanjiru(&[])),
        stdout(&help),
        "no arguments show the help"
    );
    let version = wanjiru(&["version"]);
    assert_eq!(code(&version), 0);
    assert_eq!(
        stdout(&version),
        "wanjiru 0.1.0 (DocIR wanjiru.docir/0, evaluation report wanjiru.eval-report/0)\n"
    );
}

#[test]
fn the_golden_set_plan_prints_the_numbers_of_the_blueprint() {
    let output = wanjiru(&["eval", "plan"]);
    assert_eq!(code(&output), 0);
    let text = stdout(&output);
    // Chapter 33: 99 % accuracy to within half a point at 95 % confidence, ten fields per document,
    // intraclass correlation 0.15, needs 3,575 fields in 358 documents.
    assert!(text.contains("fields needed: 3575\n"), "{text}");
    assert!(text.contains("documents needed: 358\n"), "{text}");
}

/// `docs/errors.md` is the output of `wanjiru errors`, so the catalogue people read cannot drift from
/// the registry the program uses. `WANJIRU_BLESS=1` writes the file.
#[test]
fn the_documented_error_catalogue_is_what_the_command_prints() {
    let output = wanjiru(&["errors"]);
    assert_eq!(code(&output), 0);
    assert_eq!(stderr(&output), "");
    let got = stdout(&output);
    let path = root().join("docs/errors.md");
    if std::env::var_os("WANJIRU_BLESS").is_some() {
        fs::write(&path, &got).expect("write docs/errors.md");
        return;
    }
    let want = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (WANJIRU_BLESS=1 writes it)", path.display()));
    assert_eq!(
        got, want,
        "docs/errors.md is out of date; run with WANJIRU_BLESS=1 and read the diff"
    );
}

#[test]
fn one_error_code_can_be_asked_for_and_a_wrong_one_is_a_usage_error() {
    let output = wanjiru(&["errors", "WNJ-E1003"]);
    assert_eq!(code(&output), 0);
    assert_eq!(
        stdout(&output),
        "WNJ-E1003  DocIR schema violation  (L1 core)\nA JSON value does not satisfy the DocIR schema.\n"
    );
    for (argument, message) in [
        ("WNJ-E4999", "WNJ-E4999 is not in the registry"),
        ("WNJ-E9999", "`WNJ-E9999` is not an error code"),
    ] {
        let output = wanjiru(&["errors", argument]);
        assert_eq!(code(&output), 2, "{argument}");
        assert_eq!(stdout(&output), "", "{argument}");
        assert!(
            stderr(&output).starts_with(&format!("error[WNJ-E5001]: {message}")),
            "{}",
            stderr(&output)
        );
    }
}
