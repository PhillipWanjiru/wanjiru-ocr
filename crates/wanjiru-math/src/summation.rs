//! Summation whose result does not depend on the platform, the thread count or the optimiser.
//!
//! [`neumaier_sum`] is compensated left-to-right summation. [`pairwise_sum`] and
//! [`dot_fixed_tree`] add in a binary tree whose shape depends only on the length of the input, so
//! splitting the work over threads along that tree cannot change a single bit of the result.

/// Leaf size of the fixed summation tree: runs of this many terms are added left to right.
pub const TREE_LEAF: usize = 8;

/// Compensated (Neumaier) sum, left to right. The error is independent of the number of terms to
/// first order. Non-finite inputs fall back to the plain sum, which propagates infinities and NaN.
#[must_use]
pub fn neumaier_sum(xs: &[f64]) -> f64 {
    let mut sum = 0.0_f64;
    let mut compensation = 0.0_f64;
    for &x in xs {
        let t = sum + x;
        if sum.abs() >= x.abs() {
            compensation += (sum - t) + x;
        } else {
            compensation += (x - t) + sum;
        }
        sum = t;
    }
    let total = sum + compensation;
    if total.is_finite() { total } else { sum }
}

/// Sum by a binary tree of fixed shape: the slice is split at `len / 2` until at most [`TREE_LEAF`]
/// terms remain, which are added left to right.
#[must_use]
pub fn pairwise_sum(xs: &[f64]) -> f64 {
    if xs.len() <= TREE_LEAF {
        let mut acc = 0.0;
        for &x in xs {
            acc += x;
        }
        return acc;
    }
    let mid = xs.len() / 2;
    pairwise_sum(&xs[..mid]) + pairwise_sum(&xs[mid..])
}

/// Dot product by the same fixed tree as [`pairwise_sum`], with each product rounded once (no fused
/// multiply-add). If the slices differ in length, the extra elements of the longer one are ignored.
#[must_use]
pub fn dot_fixed_tree(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    dot_range(&a[..n], &b[..n])
}

fn dot_range(a: &[f64], b: &[f64]) -> f64 {
    if a.len() <= TREE_LEAF {
        let mut acc = 0.0;
        for (x, y) in a.iter().zip(b) {
            acc += x * y;
        }
        return acc;
    }
    let mid = a.len() / 2;
    dot_range(&a[..mid], &b[..mid]) + dot_range(&a[mid..], &b[mid..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    /// Exact sum by Shewchuk's expansion arithmetic (the algorithm behind Python's `math.fsum`):
    /// the result is the correctly rounded sum of the exact real numbers.
    fn exact_sum(xs: &[f64]) -> f64 {
        let mut partials: Vec<f64> = Vec::new();
        for &x in xs {
            let mut x = x;
            let mut kept = Vec::with_capacity(partials.len() + 1);
            for &y in &partials {
                let (hi, lo) = if x.abs() < y.abs() { (y, x) } else { (x, y) };
                let s = hi + lo;
                let err = lo - (s - hi);
                if err != 0.0 {
                    kept.push(err);
                }
                x = s;
            }
            kept.push(x);
            partials = kept;
        }
        // Sum the partials from the largest down, as fsum does, to round correctly.
        let mut total = 0.0;
        for &p in partials.iter().rev() {
            total += p;
        }
        total
    }

    #[test]
    fn neumaier_recovers_what_the_naive_sum_loses() {
        let xs = [1e16, 1.0, -1e16];
        let naive: f64 = xs.iter().sum();
        assert!(
            naive.abs() < f64::EPSILON,
            "naive sum loses the 1.0: {naive}"
        );
        assert!((neumaier_sum(&xs) - 1.0).abs() < f64::EPSILON);
        let many = vec![0.1_f64; 1_000_000];
        assert!((neumaier_sum(&many) - 100_000.0).abs() < 1e-9);
    }

    #[test]
    fn neumaier_is_within_a_few_ulp_of_the_exact_sum_on_hard_inputs() {
        let mut rng = Rng::seed_from_u64(11);
        for case in 0..2_000 {
            let n = 1 + usize::try_from(rng.below(400)).expect("small");
            let xs: Vec<f64> = (0..n)
                .map(|_| {
                    // Wide dynamic range and mixed signs: cancellation is severe.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                    let exponent = (rng.below(60) as i32) - 30;
                    let mantissa = rng.next_f64() * 2.0 - 1.0;
                    mantissa * 2.0_f64.powi(exponent)
                })
                .collect();
            let exact = exact_sum(&xs);
            let got = neumaier_sum(&xs);
            let magnitude: f64 = xs.iter().map(|v| v.abs()).sum();
            // Compensated summation: error below a few ulp of the result plus a term of order
            // n * eps^2 * sum |x|.
            let bound =
                4.0 * f64::EPSILON * exact.abs() + 1e3 * f64::EPSILON * f64::EPSILON * magnitude;
            assert!(
                (got - exact).abs() <= bound,
                "case {case}: got {got}, exact {exact}"
            );
        }
    }

    #[test]
    fn non_finite_inputs_propagate() {
        assert!(neumaier_sum(&[1.0, f64::INFINITY, 2.0]).is_infinite());
        assert!(neumaier_sum(&[1.0, f64::NAN]).is_nan());
        assert!(neumaier_sum(&[f64::INFINITY, f64::NEG_INFINITY]).is_nan());
        assert!(neumaier_sum(&[]).abs() < f64::MIN_POSITIVE);
    }

    #[test]
    fn tree_sums_have_a_shape_that_depends_only_on_length() {
        let mut rng = Rng::seed_from_u64(3);
        let xs: Vec<f64> = (0..1003).map(|_| rng.next_f64() - 0.5).collect();
        // Same input, same bits, every time.
        assert_eq!(
            pairwise_sum(&xs).to_bits(),
            pairwise_sum(&xs.clone()).to_bits()
        );
        // The tree splits at len / 2, so a manual evaluation of the top levels agrees bit for bit.
        let mid = xs.len() / 2;
        assert_eq!(
            pairwise_sum(&xs).to_bits(),
            (pairwise_sum(&xs[..mid]) + pairwise_sum(&xs[mid..])).to_bits()
        );
        let exact = exact_sum(&xs);
        assert!((pairwise_sum(&xs) - exact).abs() < 1e-12);
    }

    #[test]
    fn dot_product_matches_a_reference_and_ignores_extra_elements() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let b = [10.0, 9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0, 99.0];
        assert!((dot_fixed_tree(&a, &b) - 220.0).abs() < f64::EPSILON);
        assert!(dot_fixed_tree(&[], &[]).abs() < f64::MIN_POSITIVE);
        let mut rng = Rng::seed_from_u64(4);
        let x: Vec<f64> = (0..777).map(|_| rng.next_f64()).collect();
        let y: Vec<f64> = (0..777).map(|_| rng.next_f64()).collect();
        let products: Vec<f64> = x.iter().zip(&y).map(|(p, q)| p * q).collect();
        assert_eq!(
            dot_fixed_tree(&x, &y).to_bits(),
            pairwise_sum(&products).to_bits()
        );
    }
}
