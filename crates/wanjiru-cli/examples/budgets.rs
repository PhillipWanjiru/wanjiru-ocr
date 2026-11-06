//! Measures the budgets that Phase 1 can measure and prints them as a Markdown table.
//!
//! ```text
//! cargo build --release -p wanjiru-cli
//! cargo run --release -p wanjiru-cli --example budgets
//! ```
//!
//! Each figure is the median of several repetitions after a warm-up, with the fastest and the slowest
//! beside it. Times depend on the machine, and the machine this was first run on is not the reference
//! machine that Phase 10 will fix, so the numbers in `docs/budgets.md` are a first reading and a
//! regression guide, not a gate.
//!
//! What is measured, by entry:
//!
//! - `B-types-01`: `Affine::then` and `Affine::apply`.
//! - `B-math-01` to `B-math-04`: Myers distance on 5,000 x 5,000 symbols, Clopper-Pearson at n = 10^5,
//!   SHA-256 throughput, `exp` throughput.
//! - `B-docir-01`: strict parse, schema validation and canonical form of a document of about 1 MB.
//! - `B-eval-01`: `evaluate` over 360 invoices with a baseline and the default 2,000 resamples.
//! - `B-cli-01`: wall time of `wanjiru version`, from spawning the process to its exit.

#![forbid(unsafe_code)]
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "this program's job is to print what it measured"
)]

use std::fmt::Write as _;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use wanjiru_docir::{parse, to_canonical_string, validate};
use wanjiru_eval::intervals::Resampling;
use wanjiru_eval::run::{Inputs, evaluate};
use wanjiru_math::digest::sha256;
use wanjiru_math::edit_distance::levenshtein;
use wanjiru_math::rng::Rng;
use wanjiru_math::special::exp;
use wanjiru_math::stats::clopper_pearson;
use wanjiru_types::{Affine, Line, Page, Point, Source};

type Outcome<T> = Result<T, Box<dyn std::error::Error>>;

/// Seconds per repetition, sorted.
struct Samples(Vec<f64>);

impl Samples {
    fn median(&self) -> f64 {
        let n = self.0.len();
        if n % 2 == 1 {
            self.0[n / 2]
        } else {
            0.5 * (self.0[n / 2 - 1] + self.0[n / 2])
        }
    }

    fn min(&self) -> f64 {
        self.0[0]
    }

    fn max(&self) -> f64 {
        self.0[self.0.len() - 1]
    }

    /// The sample percentile by the nearest rank.
    fn percentile(&self, percent: usize) -> f64 {
        let rank = (self.0.len() * percent)
            .div_ceil(100)
            .clamp(1, self.0.len());
        self.0[rank - 1]
    }
}

/// Run `work` `warm_up` times unmeasured, then `repetitions` times measured.
fn measure(warm_up: usize, repetitions: usize, mut work: impl FnMut()) -> Samples {
    for _ in 0..warm_up {
        work();
    }
    let mut seconds = Vec::with_capacity(repetitions);
    for _ in 0..repetitions {
        let start = Instant::now();
        work();
        seconds.push(start.elapsed().as_secs_f64());
    }
    seconds.sort_by(f64::total_cmp);
    Samples(seconds)
}

/// A duration with a unit that keeps three or four significant digits.
fn duration(seconds: f64) -> String {
    if seconds < 1e-6 {
        format!("{:.1} ns", seconds * 1e9)
    } else if seconds < 1e-3 {
        format!("{:.2} us", seconds * 1e6)
    } else if seconds < 1.0 {
        format!("{:.2} ms", seconds * 1e3)
    } else {
        format!("{seconds:.3} s")
    }
}

struct Row {
    id: &'static str,
    what: String,
    median: String,
    spread: String,
    note: String,
}

fn row(
    id: &'static str,
    what: impl Into<String>,
    samples: &Samples,
    scale: f64,
    note: impl Into<String>,
) -> Row {
    Row {
        id,
        what: what.into(),
        median: duration(samples.median() / scale),
        spread: format!(
            "{} to {}",
            duration(samples.min() / scale),
            duration(samples.max() / scale)
        ),
        note: note.into(),
    }
}

fn random_symbols(rng: &mut Rng, n: usize, alphabet: u64) -> Vec<u8> {
    (0..n)
        .map(|_| u8::try_from(rng.below(alphabet)).unwrap_or(0))
        .collect()
}

fn b_types(rows: &mut Vec<Row>) {
    let a: Affine<Source, Page> = Affine::from_matrix([0.866, 0.5, -0.5, 0.866, 12.0, -7.5]);
    let b: Affine<Page, Line> = Affine::from_matrix([1.1, 0.0, 0.0, 0.9, -3.0, 4.0]);
    let inner = 1_000_000_u32;
    let compose = measure(2, 15, || {
        let mut sum = 0.0;
        for i in 0..inner {
            let a = Affine::<Source, Page>::from_matrix([
                0.866,
                0.5,
                -0.5,
                0.866,
                f64::from(i & 7),
                -7.5,
            ]);
            let c = black_box(&a).then(black_box(&b));
            sum += c.matrix()[4];
        }
        black_box(sum);
    });
    rows.push(row(
        "B-types-01",
        "`Affine::then`, per call",
        &compose,
        f64::from(inner),
        "one million calls per repetition",
    ));
    let apply = measure(2, 15, || {
        let mut p = Point::<Source>::new(10.0, 20.0);
        for _ in 0..inner {
            let q = black_box(&a).apply(black_box(p));
            p = Point::new(q.x * 0.5 + 1.0, q.y * 0.5 + 1.0);
        }
        black_box(p);
    });
    rows.push(row(
        "B-types-01",
        "`Affine::apply`, per call",
        &apply,
        f64::from(inner),
        "one million calls per repetition; includes the two multiplications that keep the point bounded",
    ));
}

fn b_math(rows: &mut Vec<Row>) {
    let mut rng = Rng::seed_from_u64(0x00C0_FFEE);
    let a = random_symbols(&mut rng, 5_000, 26);
    let b = random_symbols(&mut rng, 5_000, 26);
    let distance = measure(3, 41, || {
        black_box(levenshtein(black_box(&a), black_box(&b)));
    });
    rows.push(row(
        "B-math-01",
        "Myers distance, 5,000 x 5,000 symbols over 26 letters",
        &distance,
        1.0,
        format!("distance {}", levenshtein(&a, &b)),
    ));

    let interval = measure(3, 41, || {
        black_box(clopper_pearson(black_box(50_000), black_box(100_000), 0.05));
    });
    rows.push(row(
        "B-math-02",
        "Clopper-Pearson interval, k = 50,000 of n = 10^5, 95 %",
        &interval,
        1.0,
        "the middle of the range, where the search is longest",
    ));

    let bytes = 16 * 1024 * 1024_usize;
    let data: Vec<u8> = (0..bytes)
        .map(|i| u8::try_from(i % 251).unwrap_or(0))
        .collect();
    let digest = measure(1, 9, || {
        black_box(sha256(black_box(&data)));
    });
    let rate = bytes as f64 / digest.median() / 1e6;
    rows.push(row(
        "B-math-03",
        "SHA-256 over 16 MiB",
        &digest,
        1.0,
        format!("{rate:.0} MB/s at the median"),
    ));

    let n = 1_000_000_u32;
    let exponential = measure(2, 15, || {
        let mut sum = 0.0;
        for i in 0..n {
            sum += exp(f64::from(i) * 6.0e-5 - 30.0);
        }
        black_box(sum);
    });
    let rate = f64::from(n) / exponential.median() / 1e6;
    rows.push(row(
        "B-math-04",
        "`exp`, one million arguments in [-30, 30]",
        &exponential,
        1.0,
        format!("{rate:.1} million per second at the median"),
    ));
}

/// A document of at least `target` bytes: one page, many lines of four words each.
fn big_document(target: usize) -> String {
    let zero = "0".repeat(64);
    let one = "1".repeat(64);
    let head = format!(
        r#"{{"schema":"wanjiru.docir/0","id":"budget-1mb","class":"invoice","source":{{"sha256":"{zero}","media_type":"image/png"}},"pages":[{{"index":0,"size":{{"w":2480,"h":3508}},"chain":{{"from":"source","steps":[]}},"lines":["#
    );
    let tail = format!(
        r#"]}}],"fields":[],"run":{{"engine":"budget 0","config_sha256":"{one}","timing_ms":{{"total":1}}}}}}"#
    );
    let mut text = head;
    let mut n = 0_usize;
    while text.len() + tail.len() < target {
        if n > 0 {
            text.push(',');
        }
        let y = (n % 300) * 11;
        let (y2, y3) = (y + 10, y + 5);
        let mut words = String::new();
        for (k, w) in ["Total", "amount", "due", "1234.56"].iter().enumerate() {
            let x = k * 600;
            let x2 = x + 560;
            if k > 0 {
                words.push(',');
            }
            let _ = write!(
                words,
                r#"{{"text":"{w}","polygon":[[{x},{y}],[{x2},{y}],[{x2},{y2}],[{x},{y2}]],"confidence":{{"value":0.95,"calibrated":false}}}}"#
            );
        }
        let _ = write!(
            text,
            r#"{{"id":"l{n}","text":"Total amount due 1234.56","polygon":[[0,{y}],[2480,{y}],[2480,{y2}],[0,{y2}],[0,{y3}]],"confidence":{{"value":0.93,"calibrated":false}},"words":[{words}]}}"#
        );
        n += 1;
    }
    text.push_str(&tail);
    text
}

fn b_docir(rows: &mut Vec<Row>) -> Outcome<()> {
    let text = big_document(1_000_000);
    let json = parse(&text)?;
    validate(&json)?;
    let canonical = to_canonical_string(&json)?;
    let whole = || -> Outcome<usize> {
        let json = parse(black_box(&text))?;
        validate(&json)?;
        Ok(to_canonical_string(&json)?.len())
    };
    let total = measure(2, 21, || {
        let _ = black_box(whole());
    });
    rows.push(row(
        "B-docir-01",
        "parse, validate and canonicalise one document",
        &total,
        1.0,
        format!(
            "{} bytes in, {} bytes out; target below 20 ms",
            text.len(),
            canonical.len()
        ),
    ));
    let parsing = measure(2, 21, || {
        let _ = black_box(parse(black_box(&text)));
    });
    rows.push(row(
        "B-docir-01",
        "of which: strict parse",
        &parsing,
        1.0,
        "alone",
    ));
    let validation = measure(2, 21, || {
        let _ = black_box(validate(black_box(&json)));
    });
    rows.push(row(
        "B-docir-01",
        "of which: schema validation",
        &validation,
        1.0,
        "alone, on the parsed value",
    ));
    let writing = measure(2, 21, || {
        let _ = black_box(to_canonical_string(black_box(&json)));
    });
    rows.push(row(
        "B-docir-01",
        "of which: canonical form",
        &writing,
        1.0,
        "alone, on the parsed value",
    ));
    Ok(())
}

fn repository_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// The invoice lines of a JSON Lines text, `copies` times, each copy with its own identifiers.
fn replicate_invoices(text: &str, copies: usize) -> String {
    let invoices: Vec<&str> = text
        .lines()
        .filter(|line| line.contains(r#""class":"invoice""#))
        .collect();
    let mut out = String::new();
    for k in 0..copies {
        for line in &invoices {
            out.push_str(&line.replacen(r#""id":"inv-"#, &format!(r#""id":"inv-c{k}-"#), 1));
            out.push('\n');
        }
    }
    out
}

fn b_eval(rows: &mut Vec<Row>) -> Outcome<()> {
    let read = |relative: &str| std::fs::read_to_string(repository_file(relative));
    let contract = read("eval/contracts/invoice-fields-v1.yaml")?;
    let truth = replicate_invoices(&read("eval/fixtures/synthetic-v0/truth.jsonl")?, 3);
    let predictions = replicate_invoices(&read("eval/fixtures/synthetic-v0/predictions.jsonl")?, 3);
    let baseline = replicate_invoices(&read("eval/fixtures/synthetic-v0/baseline.jsonl")?, 3);
    let documents = truth.lines().count();
    let inputs = Inputs {
        contract: &contract,
        truth: &truth,
        predictions: &predictions,
        baseline: Some(&baseline),
    };
    let resampling = Resampling::DEFAULT;
    let report = evaluate(&inputs, &resampling)?;
    let samples = measure(1, 9, || {
        let _ = black_box(evaluate(black_box(&inputs), &resampling));
    });
    rows.push(row(
        "B-eval-01",
        format!("`evaluate`, {documents} invoices with a baseline, {} resamples", resampling.resamples),
        &samples,
        1.0,
        format!("120 fixture invoices three times over: timing only, the verdict ({}) is not evidence; target below 2 s", report.verdict().as_str()),
    ));
    Ok(())
}

/// `target/<profile>/wanjiru`, found from this program's own path (`target/<profile>/examples/budgets`).
fn cli_binary() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    let binary = me
        .parent()?
        .parent()?
        .join(format!("wanjiru{}", std::env::consts::EXE_SUFFIX));
    binary.is_file().then_some(binary)
}

fn b_cli(rows: &mut Vec<Row>) {
    let Some(binary) = cli_binary() else {
        rows.push(Row {
            id: "B-cli-01",
            what: "`wanjiru version` start-up".to_owned(),
            median: "not measured".to_owned(),
            spread: String::new(),
            note: "build the binary first: `cargo build --release -p wanjiru-cli`".to_owned(),
        });
        return;
    };
    let samples = measure(5, 201, || {
        let status = Command::new(&binary)
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = black_box(status);
    });
    rows.push(Row {
        id: "B-cli-01",
        what: "`wanjiru version`, process start to exit".to_owned(),
        median: duration(samples.median()),
        spread: format!("{} to {}", duration(samples.min()), duration(samples.max())),
        note: format!("p95 {}; 201 runs", duration(samples.percentile(95))),
    });
}

fn machine() -> String {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = cpuinfo
        .lines()
        .find(|line| line.starts_with("model name"))
        .and_then(|line| line.split(':').nth(1))
        .map_or("unknown processor", str::trim);
    let cpus = std::thread::available_parallelism().map_or(1, usize::from);
    format!(
        "{model}; {cpus} logical CPUs; {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn main() -> Outcome<()> {
    let mut rows = Vec::new();
    b_types(&mut rows);
    b_math(&mut rows);
    b_docir(&mut rows)?;
    b_eval(&mut rows)?;
    b_cli(&mut rows);

    println!("Machine: {}", machine());
    println!(
        "Build: {}",
        if cfg!(debug_assertions) {
            "debug (do not use these numbers)"
        } else {
            "release"
        }
    );
    println!();
    println!("| Entry | What | Median | Fastest to slowest | Note |");
    println!("|---|---|---|---|---|");
    for r in &rows {
        println!(
            "| {} | {} | {} | {} | {} |",
            r.id, r.what, r.median, r.spread, r.note
        );
    }
    Ok(())
}
