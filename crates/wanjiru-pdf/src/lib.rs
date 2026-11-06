//! `wanjiru-pdf`: PDF page classification, rasterisation and text-layer probes (L2 I/O and runtime).
//!
//! Contract only. Rasterisation in Phase 2; text-layer extraction and PDF/A support in Phase 8. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

// Unsafe zone (see CONTRACT.md, "Unsafe policy"): unsafe code is denied crate-wide and allowed, with a
// SAFETY comment on every block, only in the private modules that need it.
#![deny(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 2;
