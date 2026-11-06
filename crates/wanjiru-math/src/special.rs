//! Special functions built from basic IEEE operations only (see the determinism rule in the crate
//! documentation): an exponential, and the normal density, distribution and quantile function.

/// High part of `ln 2` (the fdlibm constant `0x3fe62e42fee00000`), with 21 trailing zero bits so that
/// `k * LN2_HI` is exact for `|k| < 2^21`.
const LN2_HI: f64 = f64::from_bits(0x3FE6_2E42_FEE0_0000);
/// `ln 2 - LN2_HI` (the fdlibm constant `0x3dea39ef35793c76`).
const LN2_LO: f64 = f64::from_bits(0x3DEA_39EF_3579_3C76);
/// `1 / ln 2`.
const INV_LN2: f64 = std::f64::consts::LOG2_E;
/// `ln(f64::MAX)`: above this `exp` overflows.
const EXP_OVERFLOW: f64 = 709.782_712_893_384;
/// Below this `exp` is smaller than the smallest subnormal and returns zero.
const EXP_UNDERFLOW: f64 = -745.133_219_101_941_1;
/// `1 / sqrt(2 pi)`.
const FRAC_1_SQRT_2PI: f64 = 0.398_942_280_401_432_7;

/// Taylor coefficients `1 / n!` for `n = 1..=13`.
const EXP_COEFFICIENTS: [f64; 13] = [
    1.0,
    0.5,
    0.166_666_666_666_666_66,
    0.041_666_666_666_666_664,
    0.008_333_333_333_333_333,
    0.001_388_888_888_888_889,
    0.000_198_412_698_412_698_4,
    2.480_158_730_158_73e-5,
    2.755_731_922_398_589_3e-6,
    2.755_731_922_398_589e-7,
    2.505_210_838_544_172e-8,
    2.087_675_698_786_81e-9,
    1.605_904_383_682_161_3e-10,
];

/// `2^k` as a float, exact for `-1022 <= k <= 1023`.
fn pow2(k: i32) -> f64 {
    #[allow(
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        reason = "k + 1023 is in 1..=2046"
    )]
    f64::from_bits(((k + 1023) as u64) << 52)
}

/// The exponential function, accurate to about 1.5 ulp over the whole range, using only `+ - * /`,
/// `round` and bit operations. The argument is reduced as `x = k ln 2 + r` with `|r| <= ln 2 / 2`,
/// `exp(r)` is a degree-13 Taylor polynomial evaluated by Horner's rule, and the result is scaled by `2^k`.
#[must_use]
pub fn exp(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x > EXP_OVERFLOW {
        return f64::INFINITY;
    }
    if x < EXP_UNDERFLOW {
        return 0.0;
    }
    let kf = (x * INV_LN2).round();
    let r = (x - kf * LN2_HI) - kf * LN2_LO;
    let mut p = EXP_COEFFICIENTS[12];
    for c in EXP_COEFFICIENTS[..12].iter().rev() {
        p = p * r + c;
    }
    p = p * r + 1.0;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "|k| <= 1075 by the range checks above"
    )]
    let k = kf as i32;
    let (k1, k2) = (k / 2, k - k / 2);
    p * pow2(k1) * pow2(k2)
}

/// The standard normal density, with relative error about 3e-16 over the whole range where it is
/// representable. `x * x` is not computed directly: `|x|` is split (Dekker) into a 26-bit head and a
/// tail so that the large part of the exponent is exact, which keeps the relative error from growing
/// with `x^2`.
#[must_use]
pub fn normal_pdf(x: f64) -> f64 {
    let ax = x.abs();
    let c = 134_217_729.0 * ax; // 2^27 + 1
    let hi = c - (c - ax);
    let lo = ax - hi;
    let e_hi = exp(-0.5 * hi * hi);
    let e_lo = exp(-(hi * lo + 0.5 * lo * lo));
    e_hi * e_lo * FRAC_1_SQRT_2PI
}

/// The lower tail `Phi(-a)` for `a >= 2` by Laplace's continued fraction
/// `phi(a) / (a + 1/(a + 2/(a + 3/(a + ...))))`, evaluated backwards from a fixed depth of 250 terms:
/// relative error about 1e-15.
fn lower_tail(a: f64) -> f64 {
    let mut t = a;
    let mut n = 250.0_f64;
    while n >= 1.0 {
        t = a + n / t;
        n -= 1.0;
    }
    normal_pdf(a) / t
}

/// The standard normal distribution function.
///
/// For `|x| < 2`, `Phi(x) = 1/2 + phi(x) * sum_n x^(2n+1) / (1 * 3 * 5 * ... * (2n+1))`; every term is
/// positive, so there is no cancellation (absolute error below 3e-16). For `|x| >= 2` the tail comes
/// from a continued fraction, so tiny probabilities keep their relative accuracy (about 1e-15) and
/// the upper half is `1 - tail`. Beyond `|x| = 38.5` the result is exactly 0 or 1.
#[must_use]
pub fn normal_cdf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x >= 38.5 {
        return 1.0;
    }
    if x < -38.5 {
        return 0.0;
    }
    if x <= -2.0 {
        return lower_tail(-x);
    }
    if x >= 2.0 {
        return 1.0 - lower_tail(x);
    }
    let ax = x.abs();
    let x2 = ax * ax;
    let mut term = ax;
    let mut sum = ax;
    let mut n = 1.0_f64;
    while n < 500.0 {
        term *= x2 / (2.0 * n + 1.0);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
        n += 1.0;
    }
    let upper = 0.5 + normal_pdf(ax) * sum;
    if x >= 0.0 { upper } else { 1.0 - upper }
}

/// The standard normal quantile function (inverse distribution function) by bisection on
/// [`normal_cdf`], through the lower tail so that both tails keep relative accuracy: relative error
/// below 1e-11 for `p` in `[1e-300, 1 - 1e-16]`; infinite at 0 and 1.
#[must_use]
pub fn normal_quantile(p: f64) -> f64 {
    if p.is_nan() || !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    // `p` is in [0, 1] here, so these two comparisons are exactly `p == 0` and `p == 1`.
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    if p > 0.5 {
        // 1 - p is exact for p in [0.5, 1] (Sterbenz), and Phi^-1(p) = -Phi^-1(1 - p).
        return -normal_quantile(1.0 - p);
    }
    let (mut lo, mut hi) = (-38.6_f64, 0.0_f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if normal_cdf(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_special_values() {
        assert!((exp(0.0) - 1.0).abs() < f64::EPSILON);
        assert!(exp(f64::NAN).is_nan());
        assert!(exp(710.0).is_infinite());
        assert!(exp(-746.0).abs() < f64::MIN_POSITIVE);
        assert!(exp(f64::NEG_INFINITY).abs() < f64::MIN_POSITIVE);
        assert!(exp(f64::INFINITY).is_infinite());
        assert!((exp(1.0) - std::f64::consts::E).abs() <= 2.0 * f64::EPSILON * std::f64::consts::E);
        assert!((exp(-1.0) - (-1.0_f64).exp()).abs() < 1e-16);
    }

    #[test]
    fn exp_is_close_to_the_platform_exp_everywhere() {
        // The platform value is itself within an ulp; the sharper check against a 50-digit reference
        // is in tests/golden.rs.
        let mut x = -700.0;
        while x < 700.0 {
            let (a, b) = (exp(x), x.exp());
            assert!(
                ((a - b) / b).abs() < 4.0 * f64::EPSILON,
                "x = {x}: {a} vs {b}"
            );
            x += 0.37;
        }
    }

    #[test]
    fn normal_cdf_reference_points() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-16);
        assert!((normal_cdf(1.959_963_984_540_054) - 0.975).abs() < 1e-14);
        assert!((normal_cdf(-1.959_963_984_540_054) - 0.025).abs() < 1e-14);
        assert!((normal_cdf(1.0) - 0.841_344_746_068_542_9).abs() < 1e-15);
        assert!((normal_cdf(3.0) - 0.998_650_101_968_369_9).abs() < 1e-15);
        // Lower tail keeps relative accuracy: Phi(-6) = 9.865876450376946e-10.
        assert!((normal_cdf(-6.0) / 9.865_876_450_376_946e-10 - 1.0).abs() < 1e-13);
        assert!(normal_cdf(f64::NAN).is_nan());
        // 1 - Phi(-9) = 1 - 1.1e-19 rounds to exactly 1; beyond -38.5 the result is exactly 0.
        assert_eq!(normal_cdf(9.0).to_bits(), 1.0_f64.to_bits());
        assert_eq!(normal_cdf(-40.0).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn normal_quantile_inverts_the_cdf() {
        assert!((normal_quantile(0.975) - 1.959_963_984_540_054).abs() < 1e-13);
        assert!((normal_quantile(0.8) - 0.841_621_233_572_914_3).abs() < 1e-13);
        assert!(normal_quantile(0.5).abs() < 1e-14);
        assert!(normal_quantile(0.0).is_infinite() && normal_quantile(0.0) < 0.0);
        assert!(normal_quantile(1.0).is_infinite() && normal_quantile(1.0) > 0.0);
        assert!(normal_quantile(1.5).is_nan());
        for i in 1..1000 {
            let p = f64::from(i) / 1000.0;
            assert!(
                (normal_cdf(normal_quantile(p)) - p).abs() < 1e-14,
                "p = {p}"
            );
        }
        let z = normal_quantile(1e-10);
        assert!((normal_cdf(z) / 1e-10 - 1.0).abs() < 1e-10, "z = {z}");
    }
}
