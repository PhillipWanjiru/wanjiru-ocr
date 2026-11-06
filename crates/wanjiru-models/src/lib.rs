//! `wanjiru-models`: model packages, manifests, registry and signatures (L2 I/O and runtime).
//!
//! Contract only. Registry in Phase 4; signing in Phase 8. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

#![forbid(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 3;
