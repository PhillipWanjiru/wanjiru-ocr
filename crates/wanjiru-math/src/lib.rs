//! `wanjiru-math`: deterministic arithmetic for the Wanjiru System (L0 Foundation).
//!
//! Own implementations of every numeric routine whose result must not depend on the platform:
//! edit distance, descriptive statistics and exact interval methods, compensated and fixed-tree
//! summation, a seeded random generator, SHA-256, and an exponential function built from basic IEEE
//! operations only (determinism tiers 1 and 2, ADR-005).
//!
//! # Determinism rule
//!
//! Only `+ - * /`, `sqrt`, `floor`/`trunc` and bit operations are used. No `libm` function
//! (`exp`, `ln`, `powf`, `sin`, ...) is called on any path of this crate, because those differ in the
//! last bit between platforms. Rust never fuses `a * b + c` into a fused multiply-add on its own, so
//! evaluation order is the order written.

#![forbid(unsafe_code)]

pub mod digest;
pub mod edit_distance;
pub mod rng;
pub mod special;
pub mod stats;
pub mod summation;
