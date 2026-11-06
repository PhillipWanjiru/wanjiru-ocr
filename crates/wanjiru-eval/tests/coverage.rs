//! How often do the intervals of `wanjiru-eval` contain the truth?
//!
//! A confidence interval is a promise about repetition: of many corpora drawn from the same
//! population, 95 % of the 95 % intervals should contain the population's character error rate. This
//! test keeps the promise honest by simulation. The population is built to look like real recognition
//! errors, which are not spread evenly: most documents are read almost perfectly and a few (a crumpled
//! receipt, a fax) are read badly. Document lengths are log-normal; a document is "hard" with
//! probability 0.1 and then has a per-character error probability of 0.06, otherwise 0.002; the
//! population rate is therefore exactly 0.0078. Errors within a document are Bernoulli draws.
//!
//! The default test runs the settings that gate G1 will use (a golden set of about 360 documents). The
//! survey, which also covers 50 and 100 documents, is `#[ignore]`d and run nightly:
//! `cargo test -p wanjiru-eval --test coverage -- --ignored --nocapture`.
//!
//! Measured by the survey (4,000 corpora per cell, 1,000 resamples, seed 7):
//!
//! | documents | bootstrap-t | percentile |
//! |-----------|-------------|------------|
//! | 50        | 0.957       | 0.883      |
//! | 100       | 0.971       | 0.917      |
//! | 200       | 0.961       | 0.927      |
//! | 360       | 0.953       | 0.932      |
//! | 1000      | 0.950       | 0.943      |
//!
//! The percentile interval under-covers at every size, which is why the studentised interval is the
//! default. In this population bootstrap-t is conservative up to 200 documents and close to nominal
//! from 360 on. A heavier-tailed population (per-document error probability Beta(0.5, 24.5), simulated
//! in Python, 1,500 corpora per cell) behaves the other way at 100 documents: 0.934 for bootstrap-t,
//! 0.907 for the percentile interval, and 0.946 and 0.940 at 360. So how an interval at about 100
//! documents behaves depends on how heavy the tail of the errors is, which no one knows before the
//! first real corpus is measured; at 360 documents bootstrap-t is within a point of nominal in both.
//! That is why the golden-set plan (`wanjiru eval plan`) asks for about 360 documents per class, and
//! why a short corpus gets the verdict `inconclusive` instead of a verdict from a point estimate.

use wanjiru_eval::intervals::{Method, Resampling, ratio_interval};
use wanjiru_math::rng::Rng;
use wanjiru_math::special::normal_quantile;

const POPULATION_RATE: f64 = 0.9 * 0.002 + 0.1 * 0.06;

/// One corpus of `n` documents: `(errors, characters)` per document.
fn draw_corpus(rng: &mut Rng, n: usize) -> (Vec<u64>, Vec<u64>) {
    let mut errors = Vec::with_capacity(n);
    let mut lengths = Vec::with_capacity(n);
    for _ in 0..n {
        let z = normal_quantile(rng.next_f64().max(1e-12));
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the exponential of a modest normal variate is a modest positive number"
        )]
        let length = ((250.0_f64.ln() + 0.7 * z).exp() as u64).max(20);
        let q = if rng.next_f64() < 0.1 { 0.06 } else { 0.002 };
        let wrong = (0..length).filter(|_| rng.next_f64() < q).count() as u64;
        errors.push(wrong);
        lengths.push(length);
    }
    (errors, lengths)
}

/// The share of `experiments` corpora whose interval contains the population rate.
fn coverage(n: usize, experiments: u32, resamples: u32, method: Method, seed: u64) -> f64 {
    let mut rng = Rng::seed_from_u64(seed);
    let mut covered = 0_u32;
    for experiment in 0..experiments {
        let (errors, lengths) = draw_corpus(&mut rng, n);
        let settings = Resampling {
            level: 0.95,
            resamples,
            seed: u64::from(experiment) + 1,
            method,
        };
        let interval = ratio_interval(&errors, &lengths, &settings).expect("interval");
        if interval.lower <= POPULATION_RATE && POPULATION_RATE <= interval.upper {
            covered += 1;
        }
    }
    f64::from(covered) / f64::from(experiments)
}

#[test]
fn the_default_interval_keeps_its_promise_at_360_documents() {
    let t = coverage(360, 1500, 600, Method::BootstrapT, 2026);
    println!("bootstrap-t, n = 360: coverage {t:.4}");
    assert!(
        (0.935..=0.965).contains(&t),
        "a 95 % interval covered the population in {:.1} % of 1500 corpora",
        100.0 * t
    );
}

#[test]
#[ignore = "slow survey for the nightly run; prints the table of docs/lab"]
fn survey() {
    println!("| documents | bootstrap-t | percentile |");
    println!("|-----------|-------------|------------|");
    for n in [50_usize, 100, 200, 360, 1000] {
        let t = coverage(n, 4000, 1000, Method::BootstrapT, 7);
        let p = coverage(n, 4000, 1000, Method::Percentile, 7);
        println!("| {n} | {t:.4} | {p:.4} |");
    }
}
