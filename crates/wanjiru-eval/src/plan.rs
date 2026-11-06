//! Sample-size planning: how many documents a golden set needs before a threshold can be decided.
//!
//! To show that a proportion `p` (say 0.99) is known to within `+-margin` at confidence `c` from `n`
//! independent trials, the normal approximation asks for `n = z^2 p (1 - p) / margin^2`, `z` being the
//! normal quantile at `1 - (1 - c) / 2`. The fields of one document are not independent trials:
//! they share a scan, a layout, a printer. With `m` fields per document and an intraclass correlation
//! `rho` the variance of the proportion grows by the design effect `1 + (m - 1) rho`, so
//! `n * deff` fields are needed, in `ceil(n * deff / m)` documents. The same arithmetic run
//! backwards gives the smallest difference between two engines that a corpus can detect.
//!
//! The planning values are assumptions to be replaced by measurements: `rho` is estimated from the
//! first pilot corpus (the `fields` of a truth file by document), and the plan is recomputed.

use wanjiru_math::special::normal_quantile;
use wanjiru_types::{ErrorKind, Result, WnjError, codes};

/// Inputs of the planner.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PlanInput {
    /// Expected proportion, for example 0.99.
    pub p: f64,
    /// Half-width of the interval one wants, for example 0.005.
    pub margin: f64,
    /// Confidence level, for example 0.95.
    pub confidence: f64,
    /// Fields judged per document.
    pub fields_per_document: u32,
    /// Intraclass correlation of the fields of a document.
    pub icc: f64,
}

impl PlanInput {
    /// The assumptions of the Phase 1 plan: 99 % accuracy to within half a point at 95 %, ten fields
    /// per document, intraclass correlation 0.15.
    pub const DEFAULT: Self = Self {
        p: 0.99,
        margin: 0.005,
        confidence: 0.95,
        fields_per_document: 10,
        icc: 0.15,
    };
}

/// The result of planning.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Plan {
    /// The normal quantile used.
    pub z: f64,
    /// Fields needed if they were independent.
    pub independent_fields: u64,
    /// The design effect `1 + (m - 1) rho`.
    pub design_effect: f64,
    /// Fields needed given the design effect.
    pub fields: u64,
    /// Documents needed.
    pub documents: u64,
}

fn argument(message: String) -> WnjError {
    WnjError::new(codes::STATS_ARGUMENT, ErrorKind::InvalidInput, message)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value was checked to be finite and below 2^53"
)]
fn ceil_count(x: f64) -> Result<u64> {
    if x.is_finite() && (0.0..9.0e15).contains(&x) {
        Ok(x.ceil() as u64)
    } else {
        Err(argument(format!(
            "the plan needs {x} trials, which is not a usable number"
        )))
    }
}

/// Plan the size of a corpus.
///
/// # Errors
///
/// `WNJ-E4005` if `p` or `confidence` is outside `(0, 1)`, `margin` is not positive, `icc` is outside
/// `[0, 1]`, or there are no fields per document.
pub fn plan(input: &PlanInput) -> Result<Plan> {
    let PlanInput {
        p,
        margin,
        confidence,
        fields_per_document,
        icc,
    } = *input;
    if !(p > 0.0 && p < 1.0) {
        return Err(argument(format!("p is in (0, 1), found {p}")));
    }
    if !(margin > 0.0 && margin < 1.0) {
        return Err(argument(format!("the margin is in (0, 1), found {margin}")));
    }
    if !(confidence > 0.0 && confidence < 1.0) {
        return Err(argument(format!(
            "the confidence is in (0, 1), found {confidence}"
        )));
    }
    if !(0.0..=1.0).contains(&icc) {
        return Err(argument(format!(
            "the intraclass correlation is in [0, 1], found {icc}"
        )));
    }
    if fields_per_document == 0 {
        return Err(argument("a document has at least one field".to_owned()));
    }
    let z = normal_quantile(1.0 - (1.0 - confidence) / 2.0);
    let raw = z * z * p * (1.0 - p) / (margin * margin);
    let design_effect = 1.0 + (f64::from(fields_per_document) - 1.0) * icc;
    let fields = ceil_count(raw * design_effect)?;
    Ok(Plan {
        z,
        independent_fields: ceil_count(raw)?,
        design_effect,
        fields,
        documents: fields.div_ceil(u64::from(fields_per_document)),
    })
}

/// The smallest difference between the accuracies of two engines (each measured on `effective_n`
/// independent trials around accuracy `p`) that a two-sided test at `alpha` detects with probability
/// `power`: `(z_{1 - alpha/2} + z_{power}) sqrt(2 p (1 - p) / effective_n)`.
///
/// # Errors
///
/// `WNJ-E4005` for `p`, `alpha` or `power` outside `(0, 1)` or a non-positive `effective_n`.
pub fn minimum_detectable_difference(
    p: f64,
    effective_n: f64,
    alpha: f64,
    power: f64,
) -> Result<f64> {
    if !(p > 0.0 && p < 1.0 && alpha > 0.0 && alpha < 1.0 && power > 0.0 && power < 1.0) {
        return Err(argument(format!(
            "p, alpha and power are in (0, 1), found {p}, {alpha}, {power}"
        )));
    }
    if !(effective_n > 0.0 && effective_n.is_finite()) {
        return Err(argument(format!(
            "the effective sample size is positive, found {effective_n}"
        )));
    }
    let z = normal_quantile(1.0 - alpha / 2.0) + normal_quantile(power);
    Ok(z * (2.0 * p * (1.0 - p) / effective_n).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_plan_needs_358_documents() {
        let plan = plan(&PlanInput::DEFAULT).expect("plan");
        assert!((plan.z - 1.959_963_984_540_054).abs() < 1e-12, "{}", plan.z);
        // 1.96^2 * 0.99 * 0.01 / 0.005^2 = 1521.2: the Phase 1 blueprint says 1521, the ceiling is 1522.
        assert_eq!(plan.independent_fields, 1522);
        assert!((plan.design_effect - 2.35).abs() < 1e-12);
        assert_eq!(plan.fields, 3575);
        assert_eq!(plan.documents, 358);
    }

    #[test]
    fn independent_fields_need_no_design_effect() {
        let input = PlanInput {
            icc: 0.0,
            ..PlanInput::DEFAULT
        };
        let plan = plan(&input).expect("plan");
        assert_eq!(
            (plan.fields, plan.independent_fields, plan.documents),
            (1522, 1522, 153)
        );
        let one = PlanInput {
            fields_per_document: 1,
            ..PlanInput::DEFAULT
        };
        assert_eq!(super::plan(&one).expect("plan").fields, 1522);
    }

    #[test]
    fn a_tighter_margin_costs_quadratically() {
        let wide = plan(&PlanInput {
            margin: 0.01,
            icc: 0.0,
            ..PlanInput::DEFAULT
        })
        .expect("plan");
        let narrow = plan(&PlanInput {
            margin: 0.005,
            icc: 0.0,
            ..PlanInput::DEFAULT
        })
        .expect("plan");
        let ratio = narrow.independent_fields as f64 / wide.independent_fields as f64;
        assert!((ratio - 4.0).abs() < 0.01, "{ratio}");
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        let d = PlanInput::DEFAULT;
        for bad in [
            PlanInput { p: 0.0, ..d },
            PlanInput { p: 1.0, ..d },
            PlanInput { margin: 0.0, ..d },
            PlanInput {
                confidence: 1.0,
                ..d
            },
            PlanInput { icc: 1.5, ..d },
            PlanInput { icc: f64::NAN, ..d },
            PlanInput {
                fields_per_document: 0,
                ..d
            },
        ] {
            assert_eq!(
                plan(&bad).expect_err("invalid").code(),
                codes::STATS_ARGUMENT,
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_detectable_difference_shrinks_with_the_sample() {
        let small = minimum_detectable_difference(0.95, 500.0, 0.05, 0.8).expect("mdd");
        let large = minimum_detectable_difference(0.95, 2000.0, 0.05, 0.8).expect("mdd");
        assert!((small / large - 2.0).abs() < 1e-12);
        // (1.96 + 0.8416) * sqrt(2 * 0.95 * 0.05 / 500) = 0.0386
        assert!((small - 0.038_6).abs() < 1e-4, "{small}");
        assert!(minimum_detectable_difference(0.95, 0.0, 0.05, 0.8).is_err());
        assert!(minimum_detectable_difference(1.0, 10.0, 0.05, 0.8).is_err());
    }
}
