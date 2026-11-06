//! `wanjiru-layout`: regions, reading order, tables and key-value pairs (L3 Domain logic).
//!
//! Contract only. Implemented in Phase 7. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

#![forbid(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 7;
