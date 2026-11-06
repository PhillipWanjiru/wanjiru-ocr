//! `wanjiru-runtime`: the `ModelRuntime` trait and its back ends (ONNX Runtime first) (L2 I/O and runtime).
//!
//! Contract only. Trait and ONNX back end in Phase 3; native back end later (ADR-003). The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

// Unsafe zone (see CONTRACT.md, "Unsafe policy"): unsafe code is denied crate-wide and allowed, with a
// SAFETY comment on every block, only in the private modules that need it.
#![deny(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 3;
