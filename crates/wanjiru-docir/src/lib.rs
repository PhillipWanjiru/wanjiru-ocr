//! `wanjiru-docir`: DocIR, the single result type of the Wanjiru System engine (L1 Core).
//!
//! DocIR holds what the engine read from a document: pages with lines and words, typed fields that
//! point back at the lines and regions that justify them, and a record of the run. Geometry is stored
//! in source pixels together with invertible transform chains, so a result can always be drawn on the
//! customer's original image.
//!
//! The crate owns four things:
//!
//! - the model ([`Document`] and its parts) and its JSON form, read strictly and written canonically;
//! - the JSON Schema of that form (`schema/docir-v0.schema.json`) and a validator for the subset of
//!   JSON Schema it uses ([`schema`]);
//! - a strict JSON reader ([`json`]) and the canonical serialisation of RFC 8785 ([`canonical`]),
//!   which make a hash of a result mean the same thing everywhere;
//! - transform chains ([`transform`]): composable, invertible maps between named coordinate frames.
//!
//! Schema version 0 is a draft: it may change until Gate G1.

#![forbid(unsafe_code)]

pub mod canonical;
pub mod confidence;
mod convert;
pub mod json;
pub mod model;
pub mod schema;
pub mod transform;

pub use canonical::{canonical_sha256, format_number, to_canonical_string};
pub use confidence::Confidence;
pub use json::{Json, MAX_DEPTH, parse, parse_bytes};
pub use model::{
    Decision, DocClass, Document, Evidence, Field, FieldKind, Line, Page, Run, SCHEMA_ID,
    SourceInfo, Word,
};
pub use schema::{Violation, validate};
pub use transform::{Chain, Homography, Step, Transform};
