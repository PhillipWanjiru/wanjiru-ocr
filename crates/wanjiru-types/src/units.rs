//! Units and conversions. Conversions are explicit functions, never implicit casts.

use crate::frames::{PdfPoints, Source};
use crate::geometry::Affine;

/// PDF points per inch.
pub const POINTS_PER_INCH: f64 = 72.0;
/// Millimetres per inch.
pub const MM_PER_INCH: f64 = 25.4;

/// Dots per inch: finite and strictly positive.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Dpi(f64);

impl Dpi {
    /// Validate a resolution; `None` for zero, negative, infinite or NaN values.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && value > 0.0).then_some(Self(value))
    }

    /// The value in dots per inch.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Convert a length in PDF points to pixels at `dpi`.
#[must_use]
pub fn points_to_pixels(points: f64, dpi: Dpi) -> f64 {
    points * dpi.get() / POINTS_PER_INCH
}

/// Convert a length in pixels at `dpi` to PDF points.
#[must_use]
pub fn pixels_to_points(pixels: f64, dpi: Dpi) -> f64 {
    pixels * POINTS_PER_INCH / dpi.get()
}

/// Convert millimetres to PDF points.
#[must_use]
pub fn mm_to_points(mm: f64) -> f64 {
    mm * POINTS_PER_INCH / MM_PER_INCH
}

/// The map from PDF user space (points, y up) to source pixels (y down) for a page of height
/// `page_height_pt` rasterised at `dpi`, under the pixel-centre convention: the page's top-left
/// corner is the top-left corner of pixel `(0, 0)`.
#[must_use]
pub fn pdf_to_source(page_height_pt: f64, dpi: Dpi) -> Affine<PdfPoints, Source> {
    let s = dpi.get() / POINTS_PER_INCH;
    Affine::from_matrix([s, 0.0, 0.0, -s, 0.0, page_height_pt * s])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_validation() {
        assert!(Dpi::new(300.0).is_some());
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(Dpi::new(bad).is_none());
        }
    }

    #[test]
    fn conversions_round_trip() {
        let dpi = Dpi::new(300.0).expect("dpi");
        assert!((points_to_pixels(72.0, dpi) - 300.0).abs() < 1e-12);
        assert!((pixels_to_points(points_to_pixels(123.456, dpi), dpi) - 123.456).abs() < 1e-12);
        assert!((mm_to_points(25.4) - 72.0).abs() < 1e-12);
    }
}
