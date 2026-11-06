//! What a check reports.

use std::fmt;

/// One thing a check found wrong, with where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The check that found it: `layers`, `contracts`, `unsafe` or `licences`.
    pub check: &'static str,
    /// A file, a crate or a package.
    pub location: String,
    /// What is wrong and, when it is not obvious, how to put it right.
    pub message: String,
}

impl Violation {
    /// A violation of `check` at `location`.
    pub fn new(
        check: &'static str,
        location: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            check,
            location: location.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.check, self.location, self.message)
    }
}
