//! The error taxonomy.
//!
//! Every error the engine can return carries a stable code `WNJ-E<layer><number>`, for example
//! `WNJ-E2014`: the digit after `E` is the layer of the crate that raises it, the number is unique
//! within the layer. Codes are never reused; a retired code stays retired. The registry of codes in
//! use is [`codes`], and `docs/errors.md` (from Phase 3) is generated from it.

use core::fmt;
use core::str::FromStr;

/// The layer of the crate that raises an error (see the layer table in the workspace documentation).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// Foundation: `wanjiru-types`, `wanjiru-math`.
    L0 = 0,
    /// Core: `wanjiru-docir`, `wanjiru-simd`, `wanjiru-imgproc`.
    L1 = 1,
    /// I/O and runtime.
    L2 = 2,
    /// Domain logic.
    L3 = 3,
    /// Orchestration.
    L4 = 4,
    /// Surfaces.
    L5 = 5,
}

impl Layer {
    /// The decimal digit used in error codes.
    #[must_use]
    pub const fn digit(self) -> u8 {
        self as u8
    }

    /// What the layer is called in the documentation: `foundation`, `core`, `I/O and runtime`,
    /// `domain logic`, `orchestration` or `surfaces`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::L0 => "foundation",
            Self::L1 => "core",
            Self::L2 => "I/O and runtime",
            Self::L3 => "domain logic",
            Self::L4 => "orchestration",
            Self::L5 => "surfaces",
        }
    }

    /// The layer for a digit `0..=5`.
    #[must_use]
    pub const fn from_digit(d: u8) -> Option<Self> {
        match d {
            0 => Some(Self::L0),
            1 => Some(Self::L1),
            2 => Some(Self::L2),
            3 => Some(Self::L3),
            4 => Some(Self::L4),
            5 => Some(Self::L5),
            _ => None,
        }
    }
}

/// A stable error code, rendered as `WNJ-E<layer><number:03>`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErrorCode {
    layer: Layer,
    number: u16,
}

impl ErrorCode {
    /// Largest number within a layer.
    pub const MAX_NUMBER: u16 = 999;

    /// Define a code in a constant. A number outside `1..=999` fails to compile when used in a `const`.
    #[must_use]
    pub const fn new(layer: Layer, number: u16) -> Self {
        assert!(
            number >= 1 && number <= Self::MAX_NUMBER,
            "error numbers are 1..=999"
        );
        Self { layer, number }
    }

    /// Checked constructor for parsed input.
    #[must_use]
    pub const fn try_new(layer: Layer, number: u16) -> Option<Self> {
        if number >= 1 && number <= Self::MAX_NUMBER {
            Some(Self { layer, number })
        } else {
            None
        }
    }

    /// The layer digit.
    #[must_use]
    pub const fn layer(self) -> Layer {
        self.layer
    }

    /// The number within the layer.
    #[must_use]
    pub const fn number(self) -> u16 {
        self.number
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WNJ-E{}{:03}", self.layer.digit(), self.number)
    }
}

impl FromStr for ErrorCode {
    type Err = ();

    fn from_str(s: &str) -> core::result::Result<Self, ()> {
        let rest = s.strip_prefix("WNJ-E").ok_or(())?;
        if rest.len() != 4 || !rest.bytes().all(|b| b.is_ascii_digit()) {
            return Err(());
        }
        let layer = Layer::from_digit(rest.as_bytes()[0] - b'0').ok_or(())?;
        let number: u16 = rest[1..].parse().map_err(|_| ())?;
        Self::try_new(layer, number).ok_or(())
    }
}

/// Coarse classification of an error, for callers that map errors to behaviour.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The caller supplied something invalid.
    InvalidInput,
    /// The input is valid but uses a feature this build does not support.
    Unsupported,
    /// A resource limit was reached.
    ResourceLimit,
    /// The data is damaged.
    Corrupt,
    /// An operating-system level failure.
    Io,
    /// The operation was cancelled.
    Cancelled,
    /// A bug: an invariant of the engine itself was violated.
    Internal,
}

/// An error with a stable code, a kind, a human-readable message and an optional location.
#[derive(Clone, Debug, PartialEq)]
pub struct WnjError {
    code: ErrorCode,
    kind: ErrorKind,
    message: String,
    path: Option<String>,
}

impl WnjError {
    /// Build an error.
    #[must_use]
    pub fn new(code: ErrorCode, kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            code,
            kind,
            message: message.into(),
            path: None,
        }
    }

    /// Attach a location, such as a JSON pointer (`/pages/0/lines/3`) or a file name.
    #[must_use]
    pub fn at(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// The stable code.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    /// The coarse kind.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The location, if one was attached.
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
}

impl fmt::Display for WnjError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)?;
        if let Some(p) = &self.path {
            write!(f, " (at {p})")?;
        }
        Ok(())
    }
}

impl std::error::Error for WnjError {}

/// Result alias used across the workspace.
pub type Result<T> = core::result::Result<T, WnjError>;

/// The registry of codes in use. Add new codes at the end of a layer's block; never renumber, never reuse.
pub mod codes {
    use super::{ErrorCode, Layer};

    /// Declares every code once: its constant, documented as written here, its short title and the
    /// registry [`ALL`], which lists them in the order below and repeats the documentation as the
    /// description, so that the constant, the list and `docs/errors.md` cannot drift apart.
    macro_rules! registry {
        ($($(#[doc = $doc:literal])+ $name:ident = ($layer:ident, $number:literal, $title:literal);)+) => {
            $(
                $(#[doc = $doc])+
                pub const $name: ErrorCode = ErrorCode::new(Layer::$layer, $number);
            )+

            /// Every code, in order: the code, a short title for lists, and the description.
            pub const ALL: &[(ErrorCode, &str, &str)] = &[
                $(($name, $title, concat!($($doc),+))),+
            ];
        };
    }

    registry! {
        /// A polygon with fewer than three vertices, or a non-finite coordinate.
        INVALID_GEOMETRY = (L0, 1, "invalid geometry");
        /// A transform that cannot be inverted.
        NOT_INVERTIBLE = (L0, 2, "transform not invertible");

        /// JSON text is not well formed.
        JSON_SYNTAX = (L1, 1, "malformed JSON");
        /// A JSON object repeats a key.
        JSON_DUPLICATE_KEY = (L1, 2, "duplicate JSON key");
        /// A JSON value does not satisfy the DocIR schema.
        SCHEMA_VIOLATION = (L1, 3, "DocIR schema violation");
        /// A transform chain is not connected, or a step is malformed.
        CHAIN_INVALID = (L1, 4, "invalid transform chain");
        /// A confidence outside `[0, 1]`, or not a number.
        CONFIDENCE_INVALID = (L1, 5, "invalid confidence");
        /// A DocIR document declares a schema version this build does not read.
        SCHEMA_VERSION = (L1, 6, "unsupported DocIR schema version");
        /// A JSON number that cannot be represented (NaN, infinity) was passed to the canonical writer.
        JSON_NUMBER = (L1, 7, "JSON number not representable");

        /// A quality contract could not be parsed.
        CONTRACT_PARSE = (L4, 1, "contract parse error");
        /// A quality contract has a key or value the harness does not know.
        CONTRACT_INVALID = (L4, 2, "contract invalid");
        /// Two documents in one corpus share an identifier.
        CORPUS_DUPLICATE_ID = (L4, 3, "duplicate document id");
        /// The corpus contains no document of the contract's class.
        CORPUS_EMPTY = (L4, 4, "no documents of the contract's class");
        /// Invalid arguments to a statistical routine (empty sample, probability outside `[0, 1]`).
        STATS_ARGUMENT = (L4, 5, "invalid statistical argument");
        /// A record of a ground-truth or baseline file is malformed, or a truth value breaks the syntax of its kind.
        CORPUS_RECORD_INVALID = (L4, 6, "invalid corpus record");
        /// A prediction names a document that is not in the ground truth.
        CORPUS_UNKNOWN_ID = (L4, 7, "prediction for an unknown document");

        /// Command-line usage error.
        USAGE = (L5, 1, "usage error");
        /// A file or directory could not be read.
        CANNOT_READ = (L5, 2, "cannot read input");
        /// A file could not be written.
        CANNOT_WRITE = (L5, 3, "cannot write output");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_parse_round_trip() {
        let c = ErrorCode::new(Layer::L2, 14);
        assert_eq!(c.to_string(), "WNJ-E2014");
        assert_eq!("WNJ-E2014".parse::<ErrorCode>(), Ok(c));
        for bad in [
            "WNJ-E2000",
            "WNJ-E6001",
            "WNJ-E201",
            "wnj-e2014",
            "WNJ-E20x4",
            "WNJ-E20140",
            "",
        ] {
            assert!(bad.parse::<ErrorCode>().is_err(), "{bad}");
        }
    }

    #[test]
    fn registry_has_unique_codes_with_layer_matching_their_block() {
        let mut seen = std::collections::BTreeSet::new();
        for (code, title, description) in codes::ALL {
            assert!(seen.insert(*code), "duplicate {code}");
            assert!(!title.is_empty());
            assert!(description.trim().ends_with('.'), "{code}: {description:?}");
        }
    }

    #[test]
    fn error_display_includes_code_message_and_path() {
        let e = WnjError::new(
            codes::JSON_SYNTAX,
            ErrorKind::InvalidInput,
            "unexpected end",
        )
        .at("line 3");
        assert_eq!(e.to_string(), "WNJ-E1001: unexpected end (at line 3)");
        assert_eq!(e.kind(), ErrorKind::InvalidInput);
    }

    #[test]
    fn try_new_enforces_range() {
        assert!(ErrorCode::try_new(Layer::L0, 0).is_none());
        assert!(ErrorCode::try_new(Layer::L0, 1000).is_none());
        assert!(ErrorCode::try_new(Layer::L0, 999).is_some());
    }
}
