//! The evaluation harness of Wanjiru System: it turns "is the engine good enough?" into a number, an
//! interval and a verdict that anyone can recompute.
//!
//! The harness reads text (a quality contract, a ground-truth file, the engine's DocIR documents and,
//! optionally, a baseline run) and returns a [`report::Report`]. It does no file I/O: layer L4 has
//! none, and the command-line front end (`wanjiru eval`) reads the files. Everything it does is
//! deterministic: the same texts, settings and version give the same bytes, on any machine.
//!
//! ```
//! use wanjiru_eval::run::{Inputs, evaluate};
//! use wanjiru_eval::intervals::Resampling;
//!
//! let contract = "contract: demo\nclass: invoice\ncorpus: demo\nmetrics:\n  latency_ms: {p50_max: 1000}\ngates: {}\n";
//! let truth = r#"{"id":"a","class":"invoice"}"#;
//! // No engine output at all: the one truth document is a failure, and nothing can be said about latency.
//! let report = evaluate(
//!     &Inputs { contract, truth, predictions: "", baseline: None },
//!     &Resampling::DEFAULT,
//! )
//! .unwrap();
//! assert_eq!(report.verdict().as_str(), "inconclusive");
//! ```
//!
//! # What is measured
//!
//! * [`fields`]: per gated field, *selective accuracy* (right among accepted) and *coverage*
//!   (accepted among documents that have the field), each with an exact Clopper-Pearson interval.
//! * [`cer`] and [`intervals`]: the character error rate over documents, with a studentised bootstrap
//!   interval that resamples whole documents, and the relative change against a baseline run.
//! * Latency quantiles with distribution-free order-statistic intervals.
//! * [`gates`]: each threshold of the contract becomes a gate whose verdict is `pass`, `fail` or
//!   `inconclusive`, depending on where the interval lies; the overall verdict is the worst measured
//!   one.
//!
//! [`plan`] answers the question before the measurement: how many documents does a golden set need.
//!
//! # What keeps the numbers honest
//!
//! Missing output is a failure, never an absence. Truth values are validated against the syntax of
//! their kind ([`validators`]), independently of the parsers of the engine. The random generator is
//! specified by its algorithm ([`wanjiru_math::rng`]). The coverage of the intervals is measured by
//! simulation (`tests/coverage.rs`), and `tools/oracles/eval_oracle.py` recomputes whole reports in
//! Python from the same files; `tests/golden.rs` compares them byte for byte.

#![forbid(unsafe_code)]

pub mod cer;
pub mod contract;
pub mod corpus;
pub mod fields;
pub mod gates;
pub mod intervals;
pub mod plan;
pub mod report;
pub mod run;
pub mod validators;
pub mod yaml;

pub use contract::{Contract, FieldGate, Status};
pub use gates::{Comparison, Gate, Verdict};
pub use intervals::{Interval, Method, Resampling};
pub use report::Report;
pub use run::{Inputs, evaluate};
