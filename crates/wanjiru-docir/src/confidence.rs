//! Confidence values.
//!
//! A confidence is a number in `[0, 1]` plus a flag that says whether it has been calibrated, that is,
//! whether "0.9" has been checked to mean "right about 90% of the time" on held-out data (Phase 6).
//! Before calibration the number is only a score; consumers that threshold on it must be able to tell
//! the difference, so the flag travels with the value.

use wanjiru_types::{ErrorKind, Result, WnjError, codes};

/// A confidence in `[0, 1]`, never NaN, with its calibration status.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Confidence {
    value: f64,
    calibrated: bool,
}

impl Confidence {
    /// Build a confidence; fails with `WNJ-E1005` if `value` is NaN or outside `[0, 1]`.
    pub fn new(value: f64, calibrated: bool) -> Result<Self> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self { value, calibrated })
        } else {
            Err(WnjError::new(
                codes::CONFIDENCE_INVALID,
                ErrorKind::InvalidInput,
                format!("a confidence is a number in [0, 1], got {value}"),
            ))
        }
    }

    /// An uncalibrated score.
    pub fn score(value: f64) -> Result<Self> {
        Self::new(value, false)
    }

    /// A calibrated probability.
    pub fn probability(value: f64) -> Result<Self> {
        Self::new(value, true)
    }

    /// The number, in `[0, 1]`.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.value
    }

    /// Whether the number has been calibrated.
    #[must_use]
    pub const fn is_calibrated(self) -> bool {
        self.calibrated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_closed_unit_interval() {
        for v in [0.0, 1.0, 0.5, f64::MIN_POSITIVE, 1.0 - f64::EPSILON] {
            let c = Confidence::score(v).expect("in range");
            assert!((c.value() - v).abs() < f64::EPSILON);
            assert!(!c.is_calibrated());
        }
        assert!(
            Confidence::probability(0.9)
                .expect("in range")
                .is_calibrated()
        );
    }

    #[test]
    fn rejects_everything_else() {
        for v in [
            -0.0001,
            1.0001,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1e300,
        ] {
            let e = Confidence::new(v, false).expect_err("out of range");
            assert_eq!(e.code().to_string(), "WNJ-E1005");
        }
    }
}
