//! Coordinate frames.
//!
//! A frame is a zero-sized tag. Points, rectangles, polygons and transforms carry it as a type
//! parameter, so mixing frames is a compile error rather than a runtime check.

use core::fmt::Debug;

/// A coordinate frame.
pub trait Frame: Copy + Clone + Debug + Default + PartialEq + Eq + 'static {
    /// Stable lower-case name, used in DocIR provenance chains.
    const NAME: &'static str;
}

macro_rules! frame {
    ($(#[$meta:meta])* $name:ident, $label:literal) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
        pub struct $name;
        impl Frame for $name {
            const NAME: &'static str = $label;
        }
    };
}

frame!(
    /// Pixels of the original decoded image, origin top-left, y down. DocIR geometry lives here.
    Source,
    "source"
);
frame!(
    /// Pixels of the conditioned page image (after crop, deskew, scale), origin top-left, y down.
    Page,
    "page"
);
frame!(
    /// Pixels of a rectified text-line strip, origin at the strip's top-left, y down.
    Line,
    "line"
);
frame!(
    /// PDF user space in points (1/72 inch), origin bottom-left, y up.
    PdfPoints,
    "pdf"
);
frame!(
    /// Unit square over a page: `(0, 0)` top-left, `(1, 1)` bottom-right.
    Normalised,
    "normalised"
);

/// Resolve a frame name from a DocIR chain to a static name, if it is one of ours.
#[must_use]
pub fn known_frame_name(name: &str) -> Option<&'static str> {
    [
        Source::NAME,
        Page::NAME,
        Line::NAME,
        PdfPoints::NAME,
        Normalised::NAME,
    ]
    .into_iter()
    .find(|n| *n == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_stable_and_unique() {
        let names = [
            Source::NAME,
            Page::NAME,
            Line::NAME,
            PdfPoints::NAME,
            Normalised::NAME,
        ];
        assert_eq!(names, ["source", "page", "line", "pdf", "normalised"]);
        for (i, a) in names.iter().enumerate() {
            for b in &names[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn frames_are_zero_sized() {
        assert_eq!(size_of::<Source>(), 0);
        assert_eq!(size_of::<PdfPoints>(), 0);
    }

    #[test]
    fn lookup_by_name() {
        assert_eq!(known_frame_name("page"), Some("page"));
        assert_eq!(known_frame_name("nope"), None);
    }
}
