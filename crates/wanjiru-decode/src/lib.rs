//! `wanjiru-decode`: CTC decoding, beam search, language models, MBR, lattices and automata (L3 Domain logic).
//!
//! Contract only. CTC and beam search in Phase 3; language model, MBR, lattices and automata in Phase 5. The contract is in `CONTRACT.md`; nothing in this crate is stable yet.

#![forbid(unsafe_code)]

/// The phase of the build plan in which this crate first receives working code.
pub const FIRST_WORKING_PHASE: u8 = 3;
