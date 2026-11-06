//! `wanjiru-detect`: text detection: pre- and post-processing around the detector network (L3 Domain logic).
//!
//! Contract only. Baseline in Phase 3; own model in Phase 7; sparse inference in Phase 10. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

#![forbid(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 3;
