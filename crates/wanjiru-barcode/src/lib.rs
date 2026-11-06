//! `wanjiru-barcode`: barcode and QR decoding with cross-checks against printed text (L2 I/O and runtime).
//!
//! Contract only. Implemented in Phase 7. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

#![forbid(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 7;
