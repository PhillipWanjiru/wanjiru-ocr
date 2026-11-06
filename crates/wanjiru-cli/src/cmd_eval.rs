//! `wanjiru eval`: judge an engine by a contract; plan a golden set; check a contract.

use std::fmt::Write as _;

use wanjiru_eval::contract::Contract;
use wanjiru_eval::gates::Verdict;
use wanjiru_eval::intervals::{Method, Resampling};
use wanjiru_eval::plan::{PlanInput, minimum_detectable_difference, plan};
use wanjiru_eval::run::{Inputs, evaluate};
use wanjiru_types::{Result, WnjError};

use crate::app::{Environment, Outcome};
use crate::args::{Spec, parse, usage};

const EVAL: Spec = Spec {
    options: &[
        "contract",
        "truth",
        "pred",
        "baseline",
        "level",
        "resamples",
        "seed",
        "method",
        "format",
        "out",
    ],
    switches: &["strict"],
};

const PLAN: Spec = Spec {
    options: &[
        "p",
        "margin",
        "confidence",
        "fields-per-doc",
        "icc",
        "documents",
        "alpha",
        "power",
    ],
    switches: &[],
};

const NONE: Spec = Spec {
    options: &[],
    switches: &[],
};

pub fn run(args: &[String], env: &Environment<'_>) -> Result<Outcome> {
    match args.split_first() {
        Some((first, rest)) if first == "plan" => plan_command(rest),
        Some((first, rest)) if first == "contract" => contract_command(rest, env),
        _ => judge(args, env),
    }
}

fn judge(args: &[String], env: &Environment<'_>) -> Result<Outcome> {
    let parsed = parse(args, &EVAL)?;
    if let Some(extra) = parsed.positionals.first() {
        return Err(usage(format!("unexpected argument `{extra}`")));
    }
    let format = parsed.option("format").unwrap_or("text");
    if !matches!(format, "text" | "json") {
        return Err(usage(format!(
            "--format is `text` or `json`, not `{format}`"
        )));
    }
    let method = match parsed.option("method") {
        None => Method::BootstrapT,
        Some(text) => Method::parse(text).ok_or_else(|| {
            usage(format!(
                "--method is `bootstrap-t` or `percentile`, not `{text}`"
            ))
        })?,
    };
    let resampling = Resampling {
        level: parsed.number("level", Resampling::DEFAULT.level)?,
        resamples: parsed.number("resamples", Resampling::DEFAULT.resamples)?,
        seed: parsed.number("seed", Resampling::DEFAULT.seed)?,
        method,
    };
    let contract = env.read_file(parsed.required("contract")?)?;
    let truth = env.read_file(parsed.required("truth")?)?;
    let predictions = env.read_file(parsed.required("pred")?)?;
    let baseline = parsed
        .option("baseline")
        .map(|path| env.read_file(path))
        .transpose()?;
    let report = evaluate(
        &Inputs {
            contract: &contract,
            truth: &truth,
            predictions: &predictions,
            baseline: baseline.as_deref(),
        },
        &resampling,
    )?;
    let json = report.to_canonical_string()? + "\n";
    if let Some(path) = parsed.option("out") {
        env.write_file(path, &json)?;
    }
    let code = match report.verdict() {
        Verdict::Pass => 0,
        Verdict::Fail => 1,
        Verdict::Inconclusive | Verdict::NotMeasured => {
            if parsed.switch("strict") {
                1
            } else {
                3
            }
        }
    };
    Ok(Outcome {
        code,
        stdout: if format == "json" {
            json
        } else {
            report.render_text()
        },
        stderr: String::new(),
    })
}

fn plan_command(args: &[String]) -> Result<Outcome> {
    let parsed = parse(args, &PLAN)?;
    if let Some(extra) = parsed.positionals.first() {
        return Err(usage(format!("unexpected argument `{extra}`")));
    }
    let defaults = PlanInput::DEFAULT;
    let input = PlanInput {
        p: parsed.number("p", defaults.p)?,
        margin: parsed.number("margin", defaults.margin)?,
        confidence: parsed.number("confidence", defaults.confidence)?,
        fields_per_document: parsed.number("fields-per-doc", defaults.fields_per_document)?,
        icc: parsed.number("icc", defaults.icc)?,
    };
    let result = plan(&input)?;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "assumptions: accuracy {} to within +-{} at confidence {}; {} fields per document, intraclass correlation {}",
        input.p, input.margin, input.confidence, input.fields_per_document, input.icc
    );
    let _ = writeln!(out, "normal quantile z: {:.4}", result.z);
    let _ = writeln!(
        out,
        "fields needed if independent: {}",
        result.independent_fields
    );
    let _ = writeln!(out, "design effect: {:.4}", result.design_effect);
    let _ = writeln!(out, "fields needed: {}", result.fields);
    let _ = writeln!(out, "documents needed: {}", result.documents);
    if let Some(text) = parsed.option("documents") {
        let documents: u64 = text
            .parse()
            .map_err(|_| usage(format!("--documents: `{text}` is not a valid number")))?;
        let alpha = parsed.number("alpha", 0.05)?;
        let power = parsed.number("power", 0.8)?;
        #[allow(
            clippy::cast_precision_loss,
            reason = "a corpus size is far below 2^53"
        )]
        let effective =
            documents as f64 * f64::from(input.fields_per_document) / result.design_effect;
        let difference = minimum_detectable_difference(input.p, effective, alpha, power)?;
        let _ = writeln!(
            out,
            "with {documents} documents ({effective:.0} effective fields) two engines at accuracy {} are told apart (alpha {alpha}, power {power}) when they differ by at least {difference:.4}",
            input.p
        );
    }
    Ok(Outcome::ok(out))
}

fn contract_command(args: &[String], env: &Environment<'_>) -> Result<Outcome> {
    let parsed = parse(args, &NONE)?;
    let [path] = parsed.positionals.as_slice() else {
        return Err(usage("`eval contract` takes exactly one file"));
    };
    let contract = Contract::from_yaml(&env.read_file(path)?).map_err(|e| {
        WnjError::new(e.code(), e.kind(), format!("{path}: {}", e.message()))
            .at(e.path().unwrap_or("").to_owned())
    })?;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "contract {} ({}): class {}, corpus {}",
        contract.id,
        contract.status.as_str(),
        contract.class.as_str(),
        contract.corpus
    );
    for f in &contract.fields {
        let _ = writeln!(
            out,
            "field {} ({}): accuracy >= {}, coverage >= {}",
            f.name,
            f.kind.as_str(),
            f.min,
            f.coverage
        );
    }
    if let Some(max) = contract.ece_char_max {
        let _ = writeln!(out, "ece_char <= {max} (measured from Phase 6)");
    }
    if let Some(c) = contract.conformal {
        let _ = writeln!(
            out,
            "conformal: alpha {}, coverage tolerance {} (measured from Phase 6)",
            c.alpha, c.coverage_tolerance
        );
    }
    for (name, value) in [
        ("p50", contract.latency_p50_max),
        ("p95", contract.latency_p95_max),
    ] {
        if let Some(ms) = value {
            let _ = writeln!(out, "latency {name} <= {ms} ms");
        }
    }
    if let Some(budget) = contract.regression_budget_cer_rel {
        let _ = writeln!(
            out,
            "regression: the character error rate may rise by at most {budget} (relative)"
        );
    }
    let _ = writeln!(out, "{} thresholds", contract.gate_count());
    Ok(Outcome::ok(out))
}

#[cfg(test)]
mod tests {
    use crate::app::testing::Files;

    const CONTRACT: &str = "contract: tiny\nclass: invoice\ncorpus: tiny\nmetrics:\n  field_exact_match:\n    total: {kind: amount, min: 0.5, coverage: 0.5}\n  latency_ms: {p50_max: 100}\ngates: {regression_budget_cer_rel: 0.1}\n";

    fn truth(n: usize) -> String {
        (0..n)
            .map(|i| format!(r#"{{"id":"d{i:03}","class":"invoice","text":"Total 10.00","fields":{{"total":"10.00"}}}}"#))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn predictions(n: usize, wrong: usize) -> String {
        (0..n)
            .map(|i| {
                let (text, value) = if i < wrong {
                    ("Total 1O.00", "10.01")
                } else {
                    ("Total 10.00", "10.00")
                };
                crate::testing_docs::document(
                    &format!("d{i:03}"),
                    text,
                    value,
                    20.0 + (i % 40) as f64,
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn files(n: usize, wrong: usize) -> Files {
        Files::with(&[
            ("c.yaml", CONTRACT),
            ("t.jsonl", &truth(n)),
            ("p.jsonl", &predictions(n, wrong)),
            ("b.jsonl", &predictions(n, n)),
        ])
    }

    #[test]
    fn exit_codes_follow_the_verdict() {
        let pass = files(200, 0).run("eval --contract c.yaml --truth t.jsonl --pred p.jsonl");
        assert_eq!(
            (pass.code, pass.stderr.as_str()),
            (0, ""),
            "{}",
            pass.stdout
        );
        assert!(pass.stdout.ends_with(&format!(
            "verdict: pass\nreport sha256: {}\n",
            &pass.stdout[pass.stdout.len() - 65..pass.stdout.len() - 1]
        )));
        let fail = files(200, 190).run("eval --contract c.yaml --truth t.jsonl --pred p.jsonl");
        assert_eq!(fail.code, 1, "{}", fail.stdout);
        assert!(fail.stdout.contains("verdict: fail"));
        let unclear = files(6, 3).run("eval --contract c.yaml --truth t.jsonl --pred p.jsonl");
        assert_eq!(unclear.code, 3, "{}", unclear.stdout);
        let strict =
            files(6, 3).run("eval --contract c.yaml --truth t.jsonl --pred p.jsonl --strict");
        assert_eq!(strict.code, 1);
        assert_eq!(
            strict.stdout, unclear.stdout,
            "--strict changes the exit code, not the report"
        );
    }

    #[test]
    fn json_output_is_canonical_and_out_writes_the_same_text() {
        let f = files(120, 5);
        let out = f.run("eval --contract c.yaml --truth t.jsonl --pred p.jsonl --baseline b.jsonl --format json --out report.json --seed 3 --resamples 500");
        assert!(out.code <= 3, "{}", out.stderr);
        assert_eq!(out.stdout, f.0.borrow()["report.json"]);
        let value = wanjiru_docir::parse(&out.stdout).expect("json");
        assert_eq!(
            value.get("schema").and_then(wanjiru_docir::Json::as_str),
            Some("wanjiru.eval-report/0")
        );
        assert_eq!(
            wanjiru_docir::to_canonical_string(&value).expect("canonical") + "\n",
            out.stdout
        );
        // The text form also writes the JSON file, and prints the text.
        let text = f.run("eval --contract c.yaml --truth t.jsonl --pred p.jsonl --baseline b.jsonl --out other.json --seed 3 --resamples 500");
        assert!(text.stdout.starts_with("wanjiru eval: tiny (draft)"));
        assert_eq!(f.0.borrow()["other.json"], out.stdout);
    }

    #[test]
    fn bad_input_is_exit_code_2_with_a_code_and_a_location() {
        let f = files(20, 0);
        for (command, code) in [
            ("eval --contract c.yaml --truth t.jsonl", "WNJ-E5001"),
            (
                "eval --contract c.yaml --truth t.jsonl --pred p.jsonl --format xml",
                "WNJ-E5001",
            ),
            (
                "eval --contract c.yaml --truth t.jsonl --pred p.jsonl --method bca",
                "WNJ-E5001",
            ),
            (
                "eval --contract c.yaml --truth t.jsonl --pred p.jsonl --level high",
                "WNJ-E5001",
            ),
            (
                "eval --contract c.yaml --truth t.jsonl --pred p.jsonl extra",
                "WNJ-E5001",
            ),
            (
                "eval --contract c.yaml --truth t.jsonl --pred p.jsonl --level 1",
                "WNJ-E4005",
            ),
            (
                "eval --contract c.yaml --truth t.jsonl --pred missing.jsonl",
                "WNJ-E5002",
            ),
        ] {
            let out = f.run(command);
            assert_eq!(out.code, 2, "{command}");
            assert!(
                out.stderr.starts_with(&format!("error[{code}]")),
                "{command}: {}",
                out.stderr
            );
            assert!(out.stdout.is_empty());
        }
    }

    #[test]
    fn the_plan_reproduces_the_phase_one_numbers() {
        let out = Files::default().run("eval plan");
        assert_eq!(out.code, 0);
        for needle in [
            "fields needed if independent: 1522",
            "design effect: 2.3500",
            "fields needed: 3575",
            "documents needed: 358",
        ] {
            assert!(out.stdout.contains(needle), "{needle} in {}", out.stdout);
        }
        let out = Files::default().run("eval plan --documents 360");
        assert!(
            out.stdout
                .contains("with 360 documents (1532 effective fields)"),
            "{}",
            out.stdout
        );
        assert!(
            Files::default()
                .run("eval plan --p 2")
                .stderr
                .starts_with("error[WNJ-E4005]")
        );
        assert!(
            Files::default()
                .run("eval plan --documents lots")
                .stderr
                .starts_with("error[WNJ-E5001]")
        );
    }

    #[test]
    fn a_contract_is_summarised_or_refused() {
        let f = Files::with(&[
            ("c.yaml", CONTRACT),
            ("bad.yaml", "contract: x\nclass: nope\n"),
        ]);
        let out = f.run("eval contract c.yaml");
        assert_eq!(out.code, 0);
        assert_eq!(
            out.stdout,
            "contract tiny (draft): class invoice, corpus tiny\n\
             field total (amount): accuracy >= 0.5, coverage >= 0.5\n\
             latency p50 <= 100 ms\n\
             regression: the character error rate may rise by at most 0.1 (relative)\n\
             4 thresholds\n"
        );
        let out = f.run("eval contract bad.yaml");
        assert_eq!(out.code, 2);
        assert!(
            out.stderr.contains("bad.yaml") && out.stderr.contains("WNJ-E4002"),
            "{}",
            out.stderr
        );
        assert_eq!(f.run("eval contract").code, 2);
    }
}
