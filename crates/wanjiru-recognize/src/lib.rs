//! `wanjiru-recognize`: line rectification, the recogniser network wrapper and hypothesis generation (L3 Domain logic).
//!
//! Contract only. Baseline in Phase 3; character map and own models in Phase 4. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

#![forbid(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 3;
