//! Descriptive statistics and exact (distribution-free or binomial) interval methods.

use crate::summation::neumaier_sum;

/// Arithmetic mean by compensated summation; NaN for an empty slice.
#[must_use]
pub fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return f64::NAN;
    }
    neumaier_sum(xs) / xs.len() as f64
}

/// Sample variance (divisor `n - 1`) by Welford's one-pass algorithm; NaN for fewer than two values.
#[must_use]
pub fn variance(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return f64::NAN;
    }
    let mut count = 0.0_f64;
    let mut mean = 0.0_f64;
    let mut m2 = 0.0_f64;
    for &x in xs {
        count += 1.0;
        let delta = x - mean;
        mean += delta / count;
        m2 += delta * (x - mean);
    }
    m2 / (count - 1.0)
}

/// Sample standard deviation.
#[must_use]
pub fn std_dev(xs: &[f64]) -> f64 {
    variance(xs).sqrt()
}

/// Quantile `p` of an ascending-sorted slice by linear interpolation between order statistics
/// (Hyndman and Fan type 7, the default of R and NumPy): `h = (n - 1) p`, `x[lo] + (h - lo)(x[lo+1] - x[lo])`.
/// NaN for an empty slice; `p` is clamped to `[0, 1]`.
#[must_use]
pub fn quantile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() || p.is_nan() {
        return f64::NAN;
    }
    let n = sorted.len();
    let h = (n - 1) as f64 * p.clamp(0.0, 1.0);
    let lo_f = h.floor();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "0 <= h <= n - 1"
    )]
    let lo = lo_f as usize;
    let hi = (lo + 1).min(n - 1);
    sorted[lo] + (h - lo_f) * (sorted[hi] - sorted[lo])
}

/// Weights of the binomial probability mass function relative to its mode, split at `k`:
/// returns `(sum of w_j for j <= k, sum of w_j for j > k)`. Requires `0 < p < 1`.
///
/// `w_{j+1} / w_j = (n - j) / (j + 1) * p / (1 - p)`; every term is at most 1, there is no
/// logarithm and no subtraction, so both tails keep full relative precision. Walking away from the
/// mode, the loop must reach the far side of `k` even when the terms in between are negligible, so
/// that a tiny tail is summed rather than lost; it stops only inside the region it is walking towards
/// (`j > k` going up, `j <= k` going down), once a term is below 1e-20 of that region's sum, or when
/// the weight underflows to zero.
fn binomial_split(k: u64, n: u64, p: f64) -> (f64, f64) {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "0 <= (n + 1) p < n + 1"
    )]
    let mode = ((((n as f64) + 1.0) * p).floor() as u64).min(n);
    let ratio = p / (1.0 - p);
    let (mut le, mut gt) = (0.0_f64, 0.0_f64);
    if mode <= k {
        le += 1.0;
    } else {
        gt += 1.0;
    }
    // Upwards from the mode.
    let mut weight = 1.0_f64;
    let mut index = mode;
    while index < n {
        weight *= ((n - index) as f64) / ((index + 1) as f64) * ratio;
        index += 1;
        if index <= k {
            le += weight;
        } else {
            gt += weight;
            if weight < 1e-20 * gt {
                break;
            }
        }
        if weight == 0.0 {
            break;
        }
    }
    // Downwards from the mode.
    let mut weight = 1.0_f64;
    let mut index = mode;
    while index > 0 {
        weight *= (index as f64) / ((n - index + 1) as f64) / ratio;
        index -= 1;
        if index <= k {
            le += weight;
            if weight < 1e-20 * le {
                break;
            }
        } else {
            gt += weight;
        }
        if weight == 0.0 {
            break;
        }
    }
    (le, gt)
}

/// `P(X <= k)` for `X ~ Binomial(n, p)`.
#[must_use]
pub fn binomial_cdf(k: u64, n: u64, p: f64) -> f64 {
    if k >= n || p <= 0.0 {
        return 1.0;
    }
    if p >= 1.0 {
        return 0.0;
    }
    let (le, gt) = binomial_split(k, n, p);
    le / (le + gt)
}

/// `P(X > k)` for `X ~ Binomial(n, p)` (the survival function), with full relative precision in the tail.
#[must_use]
pub fn binomial_sf(k: u64, n: u64, p: f64) -> f64 {
    if k >= n || p <= 0.0 {
        return 0.0;
    }
    if p >= 1.0 {
        return 1.0;
    }
    let (le, gt) = binomial_split(k, n, p);
    gt / (le + gt)
}

/// Root of an increasing function on `(0, 1)` by bisection to the last representable bracket.
fn bisect_increasing(f: impl Fn(f64) -> f64) -> f64 {
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if f(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The exact two-sided Clopper-Pearson interval for a binomial proportion with `k` successes in
/// `n` trials at confidence `1 - alpha`: guaranteed coverage of at least `1 - alpha`.
/// `None` if `n == 0`, `k > n` or `alpha` is outside `(0, 1)`.
#[must_use]
pub fn clopper_pearson(k: u64, n: u64, alpha: f64) -> Option<(f64, f64)> {
    if n == 0 || k > n || !(alpha > 0.0 && alpha < 1.0) {
        return None;
    }
    let tail = alpha / 2.0;
    let lower = if k == 0 {
        0.0
    } else {
        // Smallest p with P(X >= k | p) = alpha / 2; P(X >= k) = P(X > k - 1) increases with p.
        bisect_increasing(|p| binomial_sf(k - 1, n, p) - tail)
    };
    let upper = if k == n {
        1.0
    } else {
        // Largest p with P(X <= k | p) = alpha / 2; P(X <= k) decreases with p.
        bisect_increasing(|p| tail - binomial_cdf(k, n, p))
    };
    Some((lower, upper))
}

/// Distribution-free confidence interval for the `q`-quantile of a continuous distribution from
/// the order statistics of `n` independent observations: the 1-based ranks `(l, u)` such that
/// `X_(l) <= quantile_q <= X_(u)` with probability at least `1 - alpha`. A side is `None` when no
/// rank is far enough out (the interval is unbounded on that side).
///
/// With `B ~ Binomial(n, q)` the number of observations below the quantile,
/// `P(X_(l) > xi_q) = P(B < l)` and `P(X_(u) < xi_q) = P(B >= u)`; each is kept at or below `alpha / 2`.
#[must_use]
pub fn quantile_rank_interval(n: u64, q: f64, alpha: f64) -> (Option<u64>, Option<u64>) {
    if n == 0 || !(q > 0.0 && q < 1.0) || !(alpha > 0.0 && alpha < 1.0) {
        return (None, None);
    }
    let tail = alpha / 2.0;
    // Largest l in 1..=n with P(B <= l - 1) <= tail (the probability grows with l).
    let lower = if binomial_cdf(0, n, q) > tail {
        None
    } else {
        let (mut lo, mut hi) = (1_u64, n);
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if binomial_cdf(mid - 1, n, q) <= tail {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        Some(lo)
    };
    // Smallest u in 1..=n with P(B >= u) = P(B > u - 1) <= tail (the probability shrinks with u).
    let upper = if binomial_sf(n - 1, n, q) > tail {
        None
    } else {
        let (mut lo, mut hi) = (1_u64, n);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if binomial_sf(mid - 1, n, q) <= tail {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        Some(lo)
    };
    (lower, upper)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn mean_variance_and_quantiles() {
        let xs = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        assert!((mean(&xs) - 5.0).abs() < 1e-15);
        assert!((variance(&xs) - 32.0 / 7.0).abs() < 1e-15);
        assert!((std_dev(&xs) - (32.0_f64 / 7.0).sqrt()).abs() < 1e-15);
        assert!(mean(&[]).is_nan() && variance(&[1.0]).is_nan());
        let sorted = [1.0, 2.0, 3.0, 4.0];
        assert!((quantile_sorted(&sorted, 0.0) - 1.0).abs() < f64::EPSILON);
        assert!((quantile_sorted(&sorted, 1.0) - 4.0).abs() < f64::EPSILON);
        assert!((quantile_sorted(&sorted, 0.5) - 2.5).abs() < f64::EPSILON);
        assert!((quantile_sorted(&sorted, 0.25) - 1.75).abs() < f64::EPSILON);
        assert!(quantile_sorted(&[], 0.5).is_nan());
        assert!((quantile_sorted(&[7.0], 0.9) - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn binomial_probabilities_match_closed_forms() {
        // Binomial(10, 0.5): P(X <= 3) = 176 / 1024.
        assert!((binomial_cdf(3, 10, 0.5) - 176.0 / 1024.0).abs() < 1e-14);
        assert!((binomial_sf(3, 10, 0.5) - 848.0 / 1024.0).abs() < 1e-14);
        // Edges.
        assert!((binomial_cdf(0, 5, 0.0) - 1.0).abs() < f64::EPSILON);
        assert!(binomial_cdf(4, 5, 1.0).abs() < f64::EPSILON);
        assert!((binomial_cdf(5, 5, 0.3) - 1.0).abs() < f64::EPSILON);
        // Extreme tail keeps relative precision: P(X >= 100) for Binomial(100, 0.5) = 2^-100.
        let tail = binomial_sf(99, 100, 0.5);
        assert!((tail / 2.0_f64.powi(-100) - 1.0).abs() < 1e-12, "{tail}");
    }

    #[test]
    fn cdf_and_sf_sum_to_one() {
        let mut rng = Rng::seed_from_u64(21);
        for _ in 0..500 {
            let n = 1 + rng.below(5000);
            let k = rng.below(n);
            let p = 0.001 + 0.998 * rng.next_f64();
            let total = binomial_cdf(k, n, p) + binomial_sf(k, n, p);
            assert!((total - 1.0).abs() < 1e-12, "n={n} k={k} p={p}: {total}");
        }
    }

    #[test]
    fn clopper_pearson_known_values() {
        // k = 0: upper bound is 1 - (alpha/2)^(1/n).
        let (lo, hi) = clopper_pearson(0, 10, 0.05).expect("valid");
        assert_eq!(lo.to_bits(), 0.0_f64.to_bits());
        assert!((hi - (1.0 - 0.025_f64.powf(0.1))).abs() < 1e-12, "{hi}");
        // k = n: lower bound is (alpha/2)^(1/n).
        let (lo, hi) = clopper_pearson(10, 10, 0.05).expect("valid");
        assert_eq!(hi.to_bits(), 1.0_f64.to_bits());
        assert!((lo - 0.025_f64.powf(0.1)).abs() < 1e-12, "{lo}");
        // 3 successes in 10 trials, 95%: the Beta quantiles of SciPy, [0.0667395112, 0.6524528501].
        let (lo, hi) = clopper_pearson(3, 10, 0.05).expect("valid");
        assert!(
            (lo - 0.066_739_511_177_734_47).abs() < 1e-12
                && (hi - 0.652_452_850_059_997_3).abs() < 1e-12,
            "{lo} {hi}"
        );
        assert!(clopper_pearson(0, 0, 0.05).is_none());
        assert!(clopper_pearson(11, 10, 0.05).is_none());
        assert!(clopper_pearson(5, 10, 0.0).is_none());
    }

    #[test]
    fn clopper_pearson_covers_at_least_the_nominal_level() {
        // Exact coverage at a few (n, p): the probability that the interval contains p is >= 95%.
        for &(n, p) in &[(20_u64, 0.3_f64), (50, 0.9), (100, 0.99), (200, 0.5)] {
            let mut coverage = 0.0;
            for k in 0..=n {
                let (lo, hi) = clopper_pearson(k, n, 0.05).expect("valid");
                if lo <= p && p <= hi {
                    // P(X = k) = cdf(k) - cdf(k - 1).
                    let pk = binomial_cdf(k, n, p)
                        - if k == 0 {
                            0.0
                        } else {
                            binomial_cdf(k - 1, n, p)
                        };
                    coverage += pk;
                }
            }
            assert!(coverage >= 0.95 - 1e-9, "n={n} p={p}: coverage {coverage}");
        }
    }

    #[test]
    fn quantile_ranks_for_a_hundred_observations() {
        // The 95th percentile from n = 100 is bracketed by [X_(90), X_(100)] (P(B <= 90) = 0.0282 is
        // just above 2.5%, P(B <= 89) = 0.0115 below it); the median from n = 100 by [X_(40), X_(61)],
        // the classical table entry. Re-checked against SciPy in tests/golden.rs.
        assert_eq!(
            quantile_rank_interval(100, 0.95, 0.05),
            (Some(90), Some(100))
        );
        assert_eq!(quantile_rank_interval(100, 0.5, 0.05), (Some(40), Some(61)));
        // With 20 observations the 95th percentile cannot be bounded above.
        assert_eq!(quantile_rank_interval(20, 0.95, 0.05).1, None);
        assert_eq!(quantile_rank_interval(0, 0.5, 0.05), (None, None));
    }

    #[test]
    fn quantile_rank_interval_covers_in_simulation() {
        let mut rng = Rng::seed_from_u64(31);
        let (n, q, alpha) = (60_u64, 0.9_f64, 0.05_f64);
        let (l, u) = quantile_rank_interval(n, q, alpha);
        let (l, u) = (l.expect("lower"), u.expect("upper"));
        let mut hits = 0_u32;
        let trials = 20_000_u32;
        for _ in 0..trials {
            // Uniform(0, 1) observations: the true q-quantile is q.
            let mut xs: Vec<f64> = (0..n).map(|_| rng.next_f64()).collect();
            xs.sort_by(f64::total_cmp);
            let lo = xs[usize::try_from(l - 1).expect("small")];
            let hi = xs[usize::try_from(u - 1).expect("small")];
            hits += u32::from(lo <= q && q <= hi);
        }
        let coverage = f64::from(hits) / f64::from(trials);
        // Exact coverage is at least 95%; the simulation error is about 0.2 percentage points.
        assert!(coverage > 0.945, "coverage {coverage}");
    }
}
