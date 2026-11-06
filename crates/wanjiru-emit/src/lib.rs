//! `wanjiru-emit`: output formats: JSON, text, `hOCR`, ALTO, PAGE, PDF/A, CSV, Markdown (L4 Orchestration).
//!
//! Contract only. JSON, text and hOCR in Phase 3; ALTO, PAGE, CSV and Markdown in Phase 7; PDF/A in Phase 8. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

#![forbid(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 3;
