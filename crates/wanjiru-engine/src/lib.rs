//! `wanjiru-engine`: the pipeline: stages, budgets, cancellation and the cascade (L4 Orchestration).
//!
//! Contract only. Pipeline in Phase 3; cascade and compute-follows-ink in Phase 10. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

#![forbid(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 3;
