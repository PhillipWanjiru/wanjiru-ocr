//! `wanjiru-types`: geometry in typed coordinate frames, units and the error taxonomy (L0 Foundation).
//!
//! Everything above this crate speaks these types. Coordinates carry their frame in the type, so a
//! pixel coordinate cannot be passed where a PDF-point coordinate is expected; transforms are `f64`
//! while stored coordinates are `f32` (DocIR polygons). See `CONTRACT.md` for the invariants.
//!
//! # Pixel-centre convention
//!
//! Pixel `(i, j)` covers `[i, i+1) x [j, j+1)` and its centre is `(i + 0.5, j + 0.5)`. A continuous
//! coordinate of `(0.0, 0.0)` is the top-left corner of the top-left pixel.

#![forbid(unsafe_code)]

pub mod error;
pub mod frames;
pub mod geometry;
pub mod units;

pub use error::{ErrorCode, ErrorKind, Layer, Result, WnjError, codes};
pub use frames::{Frame, Line, Normalised, Page, PdfPoints, Source};
pub use geometry::{Affine, Point, Polygon, Rect, Size};
pub use units::{Dpi, mm_to_points, pdf_to_source, pixels_to_points, points_to_pixels};
