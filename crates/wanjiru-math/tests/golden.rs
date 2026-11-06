//! Cross-checks against independent references.
//!
//! The vectors in `tests/golden/` come from `tools/oracles/make_math_vectors.py`: `hashlib`, 60-digit
//! `decimal` arithmetic, SciPy, `math.fsum` and a pure-Python dynamic programme. Nothing here shares
//! code with the crate under test.

use wanjiru_math::digest::sha256_hex;
use wanjiru_math::edit_distance::{levenshtein, levenshtein_dp};
use wanjiru_math::rng::Rng;
use wanjiru_math::special::{exp, normal_cdf, normal_quantile};
use wanjiru_math::stats::{binomial_cdf, binomial_sf, clopper_pearson, quantile_rank_interval};
use wanjiru_math::summation::neumaier_sum;

fn float(bits: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(bits, 16).expect("16 hex digits"))
}

fn unhex(s: &str) -> Vec<u8> {
    if s == "-" {
        return Vec::new();
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex byte"))
        .collect()
}

fn records(text: &str) -> impl Iterator<Item = Vec<&str>> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split_whitespace().collect())
}

#[test]
fn sha256_agrees_with_hashlib() {
    let mut n = 0;
    for r in records(include_str!("golden/sha256.txt")) {
        assert_eq!(sha256_hex(&unhex(r[0])), r[1], "input {}", r[0]);
        n += 1;
    }
    assert!(n >= 140);
}

#[test]
fn exp_is_within_two_ulp_of_the_correctly_rounded_value() {
    let mut worst = 0_u64;
    let mut n = 0;
    for r in records(include_str!("golden/exp.txt")) {
        let (x, want) = (float(r[0]), float(r[1]));
        let got = exp(x);
        let ulps = got.to_bits().abs_diff(want.to_bits());
        worst = worst.max(ulps);
        assert!(
            ulps <= 2,
            "exp({x:e}) = {got:e}, want {want:e} ({ulps} ulp)"
        );
        n += 1;
    }
    assert!(n >= 1900, "records read: {n}");
    println!("exp: worst error {worst} ulp over {n} arguments");
}

#[test]
fn normal_distribution_agrees_with_scipy() {
    let (mut cdf_n, mut inv_n) = (0, 0);
    for r in records(include_str!("golden/normal.txt")) {
        let (arg, want) = (float(r[1]), float(r[2]));
        match r[0] {
            "cdf" => {
                let got = normal_cdf(arg);
                if arg <= 0.0 && want > 1e-300 {
                    assert!(
                        ((got - want) / want).abs() < 3e-14,
                        "cdf({arg}) = {got:e}, want {want:e}"
                    );
                } else {
                    assert!(
                        (got - want).abs() < 1e-15,
                        "cdf({arg}) = {got:e}, want {want:e}"
                    );
                }
                cdf_n += 1;
            }
            "inv" => {
                let got = normal_quantile(arg);
                assert!(
                    ((got - want) / want.abs().max(1e-3)).abs() < 1e-11,
                    "inv({arg:e}) = {got}, want {want}"
                );
                inv_n += 1;
            }
            other => panic!("unknown record kind {other}"),
        }
    }
    assert!(cdf_n >= 2000 && inv_n >= 1000, "{cdf_n} {inv_n}");
}

fn close_relative(got: f64, want: f64, tolerance: f64) -> bool {
    if want < 1e-290 {
        got < 1e-280
    } else {
        ((got - want) / want).abs() < tolerance
    }
}

#[test]
fn binomial_cdf_and_sf_agree_with_scipy() {
    for r in records(include_str!("golden/binomial.txt")) {
        let (k, n): (u64, u64) = (r[0].parse().expect("k"), r[1].parse().expect("n"));
        let (p, cdf, sf) = (float(r[2]), float(r[3]), float(r[4]));
        assert!(
            close_relative(binomial_cdf(k, n, p), cdf, 1e-9),
            "cdf k={k} n={n} p={p}: {} vs {cdf}",
            binomial_cdf(k, n, p)
        );
        assert!(
            close_relative(binomial_sf(k, n, p), sf, 1e-9),
            "sf k={k} n={n} p={p}: {} vs {sf}",
            binomial_sf(k, n, p)
        );
    }
}

#[test]
fn clopper_pearson_agrees_with_beta_quantiles() {
    for r in records(include_str!("golden/clopper_pearson.txt")) {
        let (k, n): (u64, u64) = (r[0].parse().expect("k"), r[1].parse().expect("n"));
        let (alpha, lo, hi) = (float(r[2]), float(r[3]), float(r[4]));
        let (got_lo, got_hi) = clopper_pearson(k, n, alpha).expect("valid");
        assert!(
            (got_lo - lo).abs() < 1e-9,
            "lower k={k} n={n} alpha={alpha}: {got_lo} vs {lo}"
        );
        assert!(
            (got_hi - hi).abs() < 1e-9,
            "upper k={k} n={n} alpha={alpha}: {got_hi} vs {hi}"
        );
    }
}

#[test]
fn quantile_ranks_agree_with_scipy() {
    let parse = |s: &str| {
        if s == "-" {
            None
        } else {
            Some(s.parse::<u64>().expect("rank"))
        }
    };
    for r in records(include_str!("golden/quantile_ranks.txt")) {
        let n: u64 = r[0].parse().expect("n");
        let (q, alpha) = (float(r[1]), float(r[2]));
        assert_eq!(
            quantile_rank_interval(n, q, alpha),
            (parse(r[3]), parse(r[4])),
            "n={n} q={q} alpha={alpha}"
        );
    }
}

#[test]
fn edit_distance_agrees_with_the_python_dynamic_programme() {
    let mut n = 0;
    for r in records(include_str!("golden/edit_distance.txt")) {
        let a: Vec<char> = String::from_utf8(unhex(r[0]))
            .expect("utf-8")
            .chars()
            .collect();
        let b: Vec<char> = String::from_utf8(unhex(r[1]))
            .expect("utf-8")
            .chars()
            .collect();
        let want: usize = r[2].parse().expect("distance");
        assert_eq!(levenshtein(&a, &b), want, "myers: {a:?} {b:?}");
        assert_eq!(levenshtein_dp(&a, &b), want, "dp: {a:?} {b:?}");
        n += 1;
    }
    assert!(n >= 450);
}

#[test]
fn random_generator_matches_the_python_port() {
    for r in records(include_str!("golden/rng.txt")) {
        let seed: u64 = r[1].parse().expect("seed");
        let mut rng = Rng::seed_from_u64(seed);
        match r[0] {
            "u64" => {
                for v in &r[2..] {
                    assert_eq!(
                        rng.next_u64(),
                        u64::from_str_radix(v, 16).expect("hex"),
                        "seed {seed}"
                    );
                }
            }
            "f64" => {
                for v in &r[2..] {
                    assert_eq!(
                        rng.next_f64().to_bits(),
                        u64::from_str_radix(v, 16).expect("hex"),
                        "seed {seed}"
                    );
                }
            }
            "below" => {
                let bound: u64 = r[2].parse().expect("bound");
                for v in &r[3..] {
                    assert_eq!(
                        rng.below(bound),
                        v.parse::<u64>().expect("value"),
                        "seed {seed} bound {bound}"
                    );
                }
            }
            other => panic!("unknown record kind {other}"),
        }
    }
}

#[test]
fn compensated_sum_is_close_to_the_exact_sum() {
    for r in records(include_str!("golden/summation.txt")) {
        let eq = r.iter().position(|t| *t == "=").expect("separator");
        let xs: Vec<f64> = r[..eq].iter().map(|b| float(b)).collect();
        let exact = float(r[eq + 1]);
        let magnitude: f64 = xs.iter().map(|v| v.abs()).sum();
        let bound =
            4.0 * f64::EPSILON * exact.abs() + 1e3 * f64::EPSILON * f64::EPSILON * magnitude;
        let got = neumaier_sum(&xs);
        assert!(
            (got - exact).abs() <= bound,
            "got {got:e}, exact {exact:e}, bound {bound:e}"
        );
    }
}
