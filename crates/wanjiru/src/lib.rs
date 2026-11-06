//! `wanjiru`: the facade: the stable public Rust API of the Wanjiru System (L5 Surfaces).
//!
//! Contract only. Version 0 in Phase 3; version 1 (frozen surface) in Phase 8. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

#![forbid(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 3;
