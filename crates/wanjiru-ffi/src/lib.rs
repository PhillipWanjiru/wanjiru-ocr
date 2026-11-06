//! `wanjiru-ffi`: the C ABI: implementation of include/wanjiru.h (L5 Surfaces).
//!
//! Contract only. Implemented in Phase 8. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

// Unsafe zone (see CONTRACT.md, "Unsafe policy"): unsafe code is denied crate-wide and allowed, with a
// SAFETY comment on every block, only in the private modules that need it.
#![deny(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 8;
