//! Gates: a threshold, an estimate with its interval, and a three-valued verdict.
//!
//! A threshold test on an estimate is a statement about a sample; the gate asks what the interval
//! allows one to conclude about the engine:
//!
//! | comparison | `Pass`            | `Fail`            | otherwise      |
//! |------------|-------------------|-------------------|----------------|
//! | `>= t`     | lower end `>= t`  | upper end `< t`   | `Inconclusive` |
//! | `<= t`     | upper end `<= t`  | lower end `> t`   | `Inconclusive` |
//!
//! `Inconclusive` is a result, not a failure to compute: the corpus is too small (or the engine too
//! close to the threshold) to tell. The overall verdict is `Fail` if any gate fails, else
//! `Inconclusive` if any gate is inconclusive (or nothing was measured), else `Pass`. A merge gate
//! that treats `Inconclusive` as a pass would reward a small test set, so `wanjiru eval --strict`
//! turns it into a failing exit code. A threshold on a quantity that is not measured yet (`ece_char`
//! and `conformal`, until Phase 6) is reported as `NotMeasured` and takes no part in the verdict.
//! The intervals are two-sided at the stated level, so each one-sided conclusion holds at the level
//! `1 - (1 - level) / 2`.

/// The outcome of a gate.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The interval lies on the good side of the threshold.
    Pass,
    /// The interval lies on the bad side of the threshold.
    Fail,
    /// The interval contains the threshold.
    Inconclusive,
    /// The quantity is not measured by this version.
    NotMeasured,
}

impl Verdict {
    /// The spelling used in reports.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Inconclusive => "inconclusive",
            Self::NotMeasured => "not_measured",
        }
    }
}

/// Which side of the threshold is good.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Comparison {
    /// The quantity must be at least the threshold (accuracy, coverage).
    AtLeast,
    /// The quantity must be at most the threshold (latency, relative regression).
    AtMost,
}

impl Comparison {
    /// The operator as written in reports.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AtLeast => ">=",
            Self::AtMost => "<=",
        }
    }
}

/// Decide a gate from the interval `[lower, upper]` of the measured quantity. Unbounded ends are
/// infinite; a NaN end decides nothing.
#[must_use]
pub fn decide(comparison: Comparison, threshold: f64, lower: f64, upper: f64) -> Verdict {
    if lower.is_nan() || upper.is_nan() {
        return Verdict::Inconclusive;
    }
    match comparison {
        Comparison::AtLeast if lower >= threshold => Verdict::Pass,
        Comparison::AtLeast if upper < threshold => Verdict::Fail,
        Comparison::AtMost if upper <= threshold => Verdict::Pass,
        Comparison::AtMost if lower > threshold => Verdict::Fail,
        _ => Verdict::Inconclusive,
    }
}

/// One evaluated gate.
#[derive(Clone, Debug, PartialEq)]
pub struct Gate {
    /// Stable identifier, such as `field_exact_match.total_amount.min`.
    pub id: String,
    /// Which side is good.
    pub comparison: Comparison,
    /// The threshold from the contract.
    pub threshold: f64,
    /// The estimate, if any was possible.
    pub estimate: Option<f64>,
    /// Lower end of the interval (`-inf` if unbounded).
    pub lower: f64,
    /// Upper end of the interval (`+inf` if unbounded).
    pub upper: f64,
    /// The verdict.
    pub verdict: Verdict,
    /// Why a gate is not measured, or what limits it.
    pub note: Option<String>,
}

impl Gate {
    /// A gate decided from an interval.
    #[must_use]
    pub fn measured(
        id: impl Into<String>,
        comparison: Comparison,
        threshold: f64,
        estimate: Option<f64>,
        lower: f64,
        upper: f64,
    ) -> Self {
        Self {
            id: id.into(),
            comparison,
            threshold,
            estimate,
            lower,
            upper,
            verdict: decide(comparison, threshold, lower, upper),
            note: None,
        }
    }

    /// A gate that could not be measured, with the reason.
    #[must_use]
    pub fn not_measured(
        id: impl Into<String>,
        comparison: Comparison,
        threshold: f64,
        why: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            comparison,
            threshold,
            estimate: None,
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
            verdict: Verdict::NotMeasured,
            note: Some(why.into()),
        }
    }

    /// Attach a note.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// The verdict of a set of gates.
#[must_use]
pub fn overall(gates: &[Gate]) -> Verdict {
    let mut measured = 0_usize;
    let mut inconclusive = false;
    for gate in gates {
        match gate.verdict {
            Verdict::Fail => return Verdict::Fail,
            Verdict::Inconclusive => {
                measured += 1;
                inconclusive = true;
            }
            Verdict::Pass => measured += 1,
            Verdict::NotMeasured => {}
        }
    }
    if measured == 0 || inconclusive {
        Verdict::Inconclusive
    } else {
        Verdict::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Comparison::{AtLeast, AtMost};
    use Verdict::{Fail, Inconclusive, NotMeasured, Pass};

    const INF: f64 = f64::INFINITY;

    #[test]
    fn at_least_gates() {
        assert_eq!(decide(AtLeast, 0.9, 0.91, 0.99), Pass);
        assert_eq!(
            decide(AtLeast, 0.9, 0.9, 0.99),
            Pass,
            "the end may touch the threshold"
        );
        assert_eq!(decide(AtLeast, 0.9, 0.80, 0.89), Fail);
        assert_eq!(
            decide(AtLeast, 0.9, 0.80, 0.9),
            Inconclusive,
            "the upper end touches: not below"
        );
        assert_eq!(decide(AtLeast, 0.9, 0.85, 0.95), Inconclusive);
        assert_eq!(decide(AtLeast, 0.9, 0.0, 1.0), Inconclusive);
        assert_eq!(decide(AtLeast, 0.9, 0.95, INF), Pass);
        assert_eq!(decide(AtLeast, 0.9, f64::NEG_INFINITY, 0.5), Fail);
    }

    #[test]
    fn at_most_gates() {
        assert_eq!(decide(AtMost, 2500.0, 100.0, 2400.0), Pass);
        assert_eq!(decide(AtMost, 2500.0, 100.0, 2500.0), Pass);
        assert_eq!(decide(AtMost, 2500.0, 2600.0, 3000.0), Fail);
        assert_eq!(decide(AtMost, 2500.0, 2500.0, 3000.0), Inconclusive);
        assert_eq!(
            decide(AtMost, 2500.0, 100.0, INF),
            Inconclusive,
            "an unbounded upper end cannot pass"
        );
        assert_eq!(decide(AtMost, 0.02, f64::NEG_INFINITY, INF), Inconclusive);
        assert_eq!(
            decide(AtMost, 0.02, INF, INF),
            Fail,
            "an infinite regression exceeds any budget"
        );
    }

    #[test]
    fn nan_decides_nothing() {
        assert_eq!(decide(AtLeast, 0.5, f64::NAN, 1.0), Inconclusive);
        assert_eq!(decide(AtMost, 0.5, 0.0, f64::NAN), Inconclusive);
    }

    fn gate(verdict: Verdict) -> Gate {
        let mut g = Gate::measured("g", AtLeast, 0.5, Some(0.5), 0.4, 0.6);
        g.verdict = verdict;
        g
    }

    #[test]
    fn the_overall_verdict_is_the_worst_measured_one() {
        assert_eq!(overall(&[gate(Pass), gate(Pass)]), Pass);
        assert_eq!(overall(&[gate(Pass), gate(Inconclusive)]), Inconclusive);
        assert_eq!(overall(&[gate(Inconclusive), gate(Fail), gate(Pass)]), Fail);
        assert_eq!(
            overall(&[gate(Pass), gate(NotMeasured)]),
            Pass,
            "unmeasured gates are excluded"
        );
        assert_eq!(
            overall(&[gate(NotMeasured)]),
            Inconclusive,
            "nothing measured is not a pass"
        );
        assert_eq!(overall(&[]), Inconclusive);
    }

    #[test]
    fn constructors_fill_the_verdict() {
        let g = Gate::measured("x", AtLeast, 0.9, Some(0.95), 0.92, 0.97).with_note("hello");
        assert_eq!((g.verdict, g.note.as_deref()), (Pass, Some("hello")));
        let n = Gate::not_measured("y", AtMost, 0.02, "Phase 6");
        assert_eq!((n.verdict, n.estimate), (NotMeasured, None));
        assert_eq!(n.note.as_deref(), Some("Phase 6"));
    }
}
