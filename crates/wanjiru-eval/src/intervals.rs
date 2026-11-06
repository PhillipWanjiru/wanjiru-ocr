//! Confidence intervals for the quantities the gates are made of.
//!
//! Three kinds of quantity occur, and each gets the interval that suits it:
//!
//! * **Proportions** (selective accuracy, coverage): the exact Clopper-Pearson interval. A field name
//!   is unique within a document, so every document is one trial per field and the trials are
//!   independent across documents; no design effect is needed. Pooled gates over several fields would
//!   need one, and v0 has none.
//! * **Ratios of sums over documents** (character error rate, the relative change against a
//!   baseline): resampling whole documents. Characters of one document are not independent (a
//!   blurred page ruins them together), so the document is the unit that is drawn. The default is the
//!   bootstrap-t interval, which keeps its coverage on the skewed per-document error distributions
//!   that real corpora have; the percentile interval is available for comparison. The coverage of
//!   both is measured by `tests/coverage.rs`.
//! * **Quantiles** (latency): distribution-free order-statistic intervals, no resampling.
//!
//! Counts are integers and sums of counts are exact; floating point enters only in the final ratio.
//! The random generator is [`wanjiru_math::rng::Rng`], specified by its algorithm, so an interval is
//! reproducible from `(data, level, resamples, seed)` on any machine and in any language that
//! implements xoshiro256** (`tools/oracles/eval_oracle.py` does).

use wanjiru_math::rng::Rng;
use wanjiru_math::stats::{clopper_pearson, quantile_rank_interval, quantile_sorted};
use wanjiru_types::{ErrorKind, Result, WnjError, codes};

/// How the resampled replicates become an interval.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Method {
    /// Studentised bootstrap: the quantiles of `(R* - R) / se*` are applied to the original standard
    /// error. Second-order accurate.
    BootstrapT,
    /// The quantiles of the resampled ratios themselves.
    Percentile,
}

impl Method {
    /// The spelling used on the command line and in reports.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BootstrapT => "bootstrap-t",
            Self::Percentile => "percentile",
        }
    }

    /// Read the command-line spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "bootstrap-t" | "t" => Some(Self::BootstrapT),
            "percentile" => Some(Self::Percentile),
            _ => None,
        }
    }
}

/// Settings of a resampling interval.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Resampling {
    /// Confidence level, in `(0, 1)`.
    pub level: f64,
    /// Number of bootstrap resamples.
    pub resamples: u32,
    /// Seed of the random generator.
    pub seed: u64,
    /// The interval method.
    pub method: Method,
}

impl Resampling {
    /// The default of `wanjiru eval`: 95 %, 2000 resamples, seed 1, bootstrap-t.
    pub const DEFAULT: Self = Self {
        level: 0.95,
        resamples: 2000,
        seed: 1,
        method: Method::BootstrapT,
    };

    /// The miscoverage `1 - level`.
    #[must_use]
    pub fn alpha(&self) -> f64 {
        1.0 - self.level
    }

    /// Check that the settings are usable.
    ///
    /// # Errors
    ///
    /// `WNJ-E4005` for a level outside `(0, 1)` or a number of resamples outside 100 to 1000000.
    pub fn validate(&self) -> Result<()> {
        if !(self.level > 0.0 && self.level < 1.0) {
            return Err(argument(format!(
                "the confidence level is in (0, 1), found {}",
                self.level
            )));
        }
        if !(100..=1_000_000).contains(&self.resamples) {
            return Err(argument(format!(
                "the number of resamples is between 100 and 1000000, found {}",
                self.resamples
            )));
        }
        Ok(())
    }
}

fn argument(message: String) -> WnjError {
    WnjError::new(codes::STATS_ARGUMENT, ErrorKind::InvalidInput, message)
}

/// A point estimate with a two-sided interval. The ends may be infinite where the data do not bound
/// the quantity.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Interval {
    /// The estimate from the whole sample.
    pub estimate: f64,
    /// Lower end.
    pub lower: f64,
    /// Upper end.
    pub upper: f64,
}

#[allow(clippy::cast_precision_loss, reason = "counts stay far below 2^53")]
const fn float(count: u64) -> f64 {
    count as f64
}

#[allow(clippy::cast_precision_loss, reason = "sizes stay far below 2^53")]
const fn float_usize(count: usize) -> f64 {
    count as f64
}

/// The linearised standard error of a ratio of sums `R = sum(y) / sum(x)` over `n` clusters:
/// `sqrt(n / (n - 1) * sum((y_i - R x_i)^2)) / sum(x)`, with the sum accumulated in the order given.
fn linearised_se(
    clusters: impl Iterator<Item = (u64, u64)>,
    n: usize,
    ratio: f64,
    sum_x: u64,
) -> f64 {
    let mut residual_squares = 0.0_f64;
    for (y, x) in clusters {
        let d = float(y) - ratio * float(x);
        residual_squares += d * d;
    }
    let n = float_usize(n);
    ((n / (n - 1.0)) * residual_squares).sqrt() / float(sum_x)
}

/// An interval for `R = sum(y) / sum(x)`, resampling the `n` pairs `(y_i, x_i)` with replacement.
///
/// With [`Method::BootstrapT`], `se` is the linearised standard error of the whole sample and the
/// interval is `[R - q(1 - a/2) se, R - q(a/2) se]` for the quantiles `q` of `t* = (R* - R) / se*`
/// over the resamples whose `se*` is positive. If `se` is zero the interval is the point `R`. The
/// lower end is not allowed below zero, which a ratio of counts cannot be.
///
/// # Errors
///
/// `WNJ-E4005` for fewer than two pairs, mismatched lengths, `sum(x) = 0`, or unusable settings.
pub fn ratio_interval(y: &[u64], x: &[u64], settings: &Resampling) -> Result<Interval> {
    settings.validate()?;
    let n = y.len();
    if n != x.len() {
        return Err(argument(format!(
            "{n} numerators but {} denominators",
            x.len()
        )));
    }
    if n < 2 {
        return Err(argument(format!(
            "an interval needs at least two documents, found {n}"
        )));
    }
    let sum_x: u64 = x.iter().sum();
    let sum_y: u64 = y.iter().sum();
    if sum_x == 0 {
        return Err(argument("the denominators sum to zero".to_owned()));
    }
    let ratio = float(sum_y) / float(sum_x);
    let se = linearised_se(y.iter().copied().zip(x.iter().copied()), n, ratio, sum_x);
    if settings.method == Method::BootstrapT && se == 0.0 {
        return Ok(Interval {
            estimate: ratio,
            lower: ratio,
            upper: ratio,
        });
    }

    let alpha = settings.alpha();
    let mut rng = Rng::seed_from_u64(settings.seed);
    let mut drawn = vec![0_usize; n];
    let mut values = Vec::with_capacity(settings.resamples as usize);
    for _ in 0..settings.resamples {
        let (mut resampled_y, mut resampled_x) = (0_u64, 0_u64);
        for slot in &mut drawn {
            let i = rng.below_usize(n);
            *slot = i;
            resampled_y += y[i];
            resampled_x += x[i];
        }
        if resampled_x == 0 {
            continue;
        }
        let resampled_ratio = float(resampled_y) / float(resampled_x);
        match settings.method {
            Method::Percentile => values.push(resampled_ratio),
            Method::BootstrapT => {
                let resampled_se = linearised_se(
                    drawn.iter().map(|&i| (y[i], x[i])),
                    n,
                    resampled_ratio,
                    resampled_x,
                );
                if resampled_se > 0.0 && resampled_se.is_finite() {
                    values.push((resampled_ratio - ratio) / resampled_se);
                }
            }
        }
    }
    if values.len() < 20 {
        return Err(argument(format!(
            "only {} of {} resamples were usable; the sample is too small or too uniform for an interval",
            values.len(),
            settings.resamples
        )));
    }
    values.sort_by(f64::total_cmp);
    let (low_q, high_q) = (
        quantile_sorted(&values, alpha / 2.0),
        quantile_sorted(&values, 1.0 - alpha / 2.0),
    );
    let (lower, upper) = match settings.method {
        Method::Percentile => (low_q, high_q),
        Method::BootstrapT => (ratio - high_q * se, ratio - low_q * se),
    };
    Ok(Interval {
        estimate: ratio,
        lower: lower.max(0.0),
        upper,
    })
}

/// The relative change `sum(new) / sum(base) - 1` of an error count against a baseline, over the same
/// documents, with an interval from resampling documents (pairs stay together).
///
/// A baseline without errors has no relative scale: if the new count is zero as well the change is
/// reported as 0 with the interval `[-1, +inf)` (nothing is known), otherwise it is `+inf` (any
/// error is an infinite relative increase).
///
/// # Errors
///
/// `WNJ-E4005` for fewer than two documents, mismatched lengths or unusable settings.
pub fn relative_change(new: &[u64], base: &[u64], settings: &Resampling) -> Result<Interval> {
    let (sum_new, sum_base): (u64, u64) = (new.iter().sum(), base.iter().sum());
    if sum_base == 0 {
        settings.validate()?;
        if new.len() != base.len() || new.len() < 2 {
            return Err(argument(format!(
                "an interval needs at least two documents with a pair each, found {} and {}",
                new.len(),
                base.len()
            )));
        }
        return Ok(if sum_new == 0 {
            Interval {
                estimate: 0.0,
                lower: -1.0,
                upper: f64::INFINITY,
            }
        } else {
            Interval {
                estimate: f64::INFINITY,
                lower: f64::INFINITY,
                upper: f64::INFINITY,
            }
        });
    }
    let ratio = ratio_interval(new, base, settings)?;
    Ok(Interval {
        estimate: ratio.estimate - 1.0,
        lower: ratio.lower - 1.0,
        upper: ratio.upper - 1.0,
    })
}

/// A binomial proportion with its exact interval.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Proportion {
    /// Successes.
    pub k: u64,
    /// Trials.
    pub n: u64,
    /// `k / n`, or `None` without trials.
    pub estimate: Option<f64>,
    /// Lower end (0 without trials).
    pub lower: f64,
    /// Upper end (1 without trials).
    pub upper: f64,
}

/// The Clopper-Pearson interval at confidence `level`. Without trials nothing is known: `[0, 1]`.
#[must_use]
pub fn proportion(k: u64, n: u64, level: f64) -> Proportion {
    match clopper_pearson(k, n, 1.0 - level) {
        Some((lower, upper)) => Proportion {
            k,
            n,
            estimate: Some(float(k) / float(n)),
            lower,
            upper,
        },
        None => Proportion {
            k,
            n,
            estimate: None,
            lower: 0.0,
            upper: 1.0,
        },
    }
}

/// A quantile of a sample with a distribution-free interval; an end is `None` if the sample is too
/// small to bound it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Quantile {
    /// Sample size.
    pub n: usize,
    /// The sample quantile (Hyndman-Fan type 7), `None` for an empty sample.
    pub estimate: Option<f64>,
    /// Lower end of the interval.
    pub lower: Option<f64>,
    /// Upper end of the interval.
    pub upper: Option<f64>,
}

/// The `q`-quantile of `sorted` (ascending) and its order-statistic interval at confidence `level`.
#[must_use]
pub fn quantile(sorted: &[f64], q: f64, level: f64) -> Quantile {
    let n = sorted.len();
    if n == 0 {
        return Quantile {
            n,
            estimate: None,
            lower: None,
            upper: None,
        };
    }
    let (lower_rank, upper_rank) = quantile_rank_interval(n as u64, q, 1.0 - level);
    let at = |rank: Option<u64>| {
        rank.and_then(|r| usize::try_from(r).ok())
            .map(|r| sorted[r - 1])
    };
    Quantile {
        n,
        estimate: Some(quantile_sorted(sorted, q)),
        lower: at(lower_rank),
        upper: at(upper_rank),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(method: Method) -> Resampling {
        Resampling {
            method,
            ..Resampling::DEFAULT
        }
    }

    /// 60 documents of 200 characters with a spread of error counts.
    fn sample() -> (Vec<u64>, Vec<u64>) {
        let x = vec![200_u64; 60];
        let y: Vec<u64> = (0..60_u64)
            .map(|i| [0, 1, 2, 3, 9, 0, 4, 2, 0, 15][(i % 10) as usize])
            .collect();
        (y, x)
    }

    #[test]
    fn proportions_use_the_exact_interval() {
        let p = proportion(0, 10, 0.95);
        assert_eq!(p.estimate, Some(0.0));
        assert!(p.lower.abs() < f64::EPSILON);
        assert!((p.upper - 0.308_5).abs() < 1e-4, "{}", p.upper);
        let p = proportion(5, 10, 0.95);
        assert!(
            (p.lower - 0.187_09).abs() < 1e-4 && (p.upper - 0.812_91).abs() < 1e-4,
            "{p:?}"
        );
        let p = proportion(0, 0, 0.95);
        assert_eq!((p.estimate, p.lower, p.upper), (None, 0.0, 1.0));
    }

    #[test]
    fn a_ratio_is_estimated_exactly_and_bracketed() {
        let (y, x) = sample();
        for method in [Method::BootstrapT, Method::Percentile] {
            let i = ratio_interval(&y, &x, &settings(method)).expect("interval");
            // 36 errors per ten documents of 200 characters: 216 errors in 12000 characters.
            assert!((i.estimate - 216.0 / 12000.0).abs() < 1e-15, "{i:?}");
            assert!(
                i.lower < i.estimate && i.estimate < i.upper,
                "{method:?} {i:?}"
            );
            assert!(i.lower >= 0.0);
        }
    }

    #[test]
    fn results_depend_on_the_seed_and_on_nothing_else() {
        let (errors, lengths) = sample();
        let run = |seed| {
            let settings = Resampling {
                seed,
                ..Resampling::DEFAULT
            };
            ratio_interval(&errors, &lengths, &settings).expect("interval")
        };
        let (first, again, other) = (run(1), run(1), run(2));
        assert_eq!(first, again);
        assert_ne!(first, other);
        assert!((first.lower - other.lower).abs() < 0.01);
        assert!((first.upper - other.upper).abs() < 0.01);
    }

    #[test]
    fn a_perfect_sample_has_a_point_interval_and_no_division_by_zero() {
        let x = vec![100_u64; 10];
        let y = vec![0_u64; 10];
        for method in [Method::BootstrapT, Method::Percentile] {
            let i = ratio_interval(&y, &x, &settings(method)).expect("interval");
            assert_eq!((i.estimate, i.lower, i.upper), (0.0, 0.0, 0.0));
        }
    }

    #[test]
    fn unusable_arguments_are_reported() {
        let (y, x) = sample();
        let cases: [(&[u64], &[u64], Resampling); 6] = [
            (&y[..1], &x[..1], Resampling::DEFAULT),
            (&y, &x[..5], Resampling::DEFAULT),
            (&[0, 0], &[0, 0], Resampling::DEFAULT),
            (
                &y,
                &x,
                Resampling {
                    level: 1.0,
                    ..Resampling::DEFAULT
                },
            ),
            (
                &y,
                &x,
                Resampling {
                    level: 0.0,
                    ..Resampling::DEFAULT
                },
            ),
            (
                &y,
                &x,
                Resampling {
                    resamples: 10,
                    ..Resampling::DEFAULT
                },
            ),
        ];
        for (y, x, s) in cases {
            let e = ratio_interval(y, x, &s).expect_err("invalid");
            assert_eq!(e.code(), codes::STATS_ARGUMENT, "{e}");
        }
    }

    #[test]
    fn the_relative_change_is_a_ratio_minus_one() {
        let (new, base): (Vec<u64>, Vec<u64>) = (0..80_u64)
            .map(|i| {
                let b = [0, 1, 2, 3, 9, 0, 4, 2, 0, 15][(i % 10) as usize];
                (b + u64::from(i % 4 == 0), b)
            })
            .unzip();
        let i = relative_change(&new, &base, &Resampling::DEFAULT).expect("interval");
        let expected = 20.0 / (36.0 * 8.0); // 20 extra errors on a base of 288
        assert!((i.estimate - expected).abs() < 1e-15, "{i:?}");
        assert!(i.lower < i.estimate && i.estimate < i.upper, "{i:?}");
        // Identical outputs: no change, no uncertainty.
        let same = relative_change(&base, &base, &Resampling::DEFAULT).expect("interval");
        assert_eq!((same.estimate, same.lower, same.upper), (0.0, 0.0, 0.0));
    }

    #[test]
    fn a_baseline_without_errors_has_no_relative_scale() {
        let zero = vec![0_u64; 20];
        let some: Vec<u64> = (0..20).map(|i| u64::from(i == 3)).collect();
        let i = relative_change(&zero, &zero, &Resampling::DEFAULT).expect("interval");
        assert_eq!((i.estimate, i.lower, i.upper), (0.0, -1.0, f64::INFINITY));
        let i = relative_change(&some, &zero, &Resampling::DEFAULT).expect("interval");
        assert_eq!(
            (i.estimate, i.lower, i.upper),
            (f64::INFINITY, f64::INFINITY, f64::INFINITY)
        );
    }

    #[test]
    fn latency_quantiles_come_with_order_statistics() {
        let sorted: Vec<f64> = (1..=200).map(f64::from).collect();
        let q = quantile(&sorted, 0.5, 0.95);
        assert_eq!(q.n, 200);
        assert!((q.estimate.expect("estimate") - 100.5).abs() < 1e-12);
        let (lo, hi) = (q.lower.expect("lower"), q.upper.expect("upper"));
        assert!(
            lo < 100.5 && 100.5 < hi && (lo - 86.0).abs() <= 2.0 && (hi - 115.0).abs() <= 2.0,
            "{lo} {hi}"
        );
        // 20 observations cannot bound the 95th percentile from above.
        let few: Vec<f64> = (1..=20).map(f64::from).collect();
        let q = quantile(&few, 0.95, 0.95);
        assert!(q.upper.is_none() && q.lower.is_some(), "{q:?}");
        let empty = quantile(&[], 0.5, 0.95);
        assert_eq!(
            (empty.n, empty.estimate, empty.lower, empty.upper),
            (0, None, None, None)
        );
    }

    #[test]
    fn method_names_round_trip() {
        for m in [Method::BootstrapT, Method::Percentile] {
            assert_eq!(Method::parse(m.as_str()), Some(m));
        }
        assert_eq!(Method::parse("t"), Some(Method::BootstrapT));
        assert_eq!(Method::parse("bca"), None);
    }
}
