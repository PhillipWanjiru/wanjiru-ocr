//! Points, sizes, rectangles, polygons and affine maps in typed frames.
//!
//! Stored coordinates are `f32` (DocIR polygons); transforms and all intermediate arithmetic are
//! `f64`. Operations are total: NaN in gives NaN out, and an uninvertible map returns `None`.

use core::marker::PhantomData;

use crate::error::{ErrorKind, Result, WnjError, codes};
use crate::frames::Frame;

/// A point in frame `F`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Point<F: Frame> {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
    frame: PhantomData<F>,
}

impl<F: Frame> Point<F> {
    /// Build a point from its coordinates.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            frame: PhantomData,
        }
    }

    /// True if both coordinates are finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Squared Euclidean distance, computed in `f64`.
    #[must_use]
    pub fn distance_sq(&self, other: &Self) -> f64 {
        let dx = f64::from(self.x) - f64::from(other.x);
        let dy = f64::from(self.y) - f64::from(other.y);
        dx * dx + dy * dy
    }

    /// Euclidean distance, computed in `f64` with a correctly rounded square root.
    #[must_use]
    pub fn distance(&self, other: &Self) -> f64 {
        self.distance_sq(other).sqrt()
    }
}

/// A width and a height in frame `F`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Size<F: Frame> {
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
    frame: PhantomData<F>,
}

impl<F: Frame> Size<F> {
    /// Build a size.
    #[must_use]
    pub const fn new(w: f32, h: f32) -> Self {
        Self {
            w,
            h,
            frame: PhantomData,
        }
    }

    /// Area in `f64`.
    #[must_use]
    pub fn area(&self) -> f64 {
        f64::from(self.w) * f64::from(self.h)
    }
}

/// An axis-aligned rectangle in frame `F`, half-open: it contains `x0 <= x < x1` and `y0 <= y < y1`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rect<F: Frame> {
    /// Left edge.
    pub x0: f32,
    /// Top edge.
    pub y0: f32,
    /// Right edge (exclusive).
    pub x1: f32,
    /// Bottom edge (exclusive).
    pub y1: f32,
    frame: PhantomData<F>,
}

impl<F: Frame> Rect<F> {
    /// Build a rectangle from two corners in any order.
    #[must_use]
    pub fn new(xa: f32, ya: f32, xb: f32, yb: f32) -> Self {
        Self {
            x0: xa.min(xb),
            y0: ya.min(yb),
            x1: xa.max(xb),
            y1: ya.max(yb),
            frame: PhantomData,
        }
    }

    /// Width.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    /// Height.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }

    /// Area in `f64`.
    #[must_use]
    pub fn area(&self) -> f64 {
        (f64::from(self.x1) - f64::from(self.x0)) * (f64::from(self.y1) - f64::from(self.y0))
    }

    /// True if all four edges are finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.x0.is_finite() && self.y0.is_finite() && self.x1.is_finite() && self.y1.is_finite()
    }

    /// Half-open containment test.
    #[must_use]
    pub fn contains(&self, p: Point<F>) -> bool {
        p.x >= self.x0 && p.x < self.x1 && p.y >= self.y0 && p.y < self.y1
    }

    /// Intersection, or `None` if the rectangles do not overlap in a region of positive area.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let x0 = self.x0.max(other.x0);
        let y0 = self.y0.max(other.y0);
        let x1 = self.x1.min(other.x1);
        let y1 = self.y1.min(other.y1);
        (x0 < x1 && y0 < y1).then_some(Self {
            x0,
            y0,
            x1,
            y1,
            frame: PhantomData,
        })
    }

    /// Smallest rectangle containing both.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
            frame: PhantomData,
        }
    }

    /// Intersection over union, in `[0, 1]`; zero when either rectangle is empty.
    #[must_use]
    pub fn iou(&self, other: &Self) -> f64 {
        let inter = self.intersection(other).map_or(0.0, |r| r.area());
        let union = self.area() + other.area() - inter;
        if union > 0.0 { inter / union } else { 0.0 }
    }
}

/// A closed polygon with at least three finite vertices in frame `F`.
#[derive(Clone, Debug, PartialEq)]
pub struct Polygon<F: Frame> {
    points: Vec<Point<F>>,
}

impl<F: Frame> Polygon<F> {
    /// Build a polygon; fails with `WNJ-E0001` on fewer than three vertices or a non-finite coordinate.
    pub fn new(points: Vec<Point<F>>) -> Result<Self> {
        if points.len() < 3 {
            return Err(WnjError::new(
                codes::INVALID_GEOMETRY,
                ErrorKind::InvalidInput,
                format!("a polygon needs at least 3 vertices, got {}", points.len()),
            ));
        }
        if let Some(i) = points.iter().position(|p| !p.is_finite()) {
            return Err(WnjError::new(
                codes::INVALID_GEOMETRY,
                ErrorKind::InvalidInput,
                format!("polygon vertex {i} is not finite"),
            ));
        }
        Ok(Self { points })
    }

    /// The vertices, in order.
    #[must_use]
    pub fn points(&self) -> &[Point<F>] {
        &self.points
    }

    /// Signed area by the shoelace formula, summed in vertex order in `f64`. Positive when the
    /// vertices run counter-clockwise in a y-up frame, which is clockwise on screen in our y-down frames.
    #[must_use]
    pub fn signed_area(&self) -> f64 {
        let n = self.points.len();
        let mut acc = 0.0_f64;
        for i in 0..n {
            let p = &self.points[i];
            let q = &self.points[(i + 1) % n];
            acc += f64::from(p.x) * f64::from(q.y) - f64::from(q.x) * f64::from(p.y);
        }
        acc * 0.5
    }

    /// Absolute area.
    #[must_use]
    pub fn area(&self) -> f64 {
        self.signed_area().abs()
    }

    /// Smallest axis-aligned rectangle containing all vertices.
    #[must_use]
    pub fn bounding_rect(&self) -> Rect<F> {
        let mut r = Rect::new(
            self.points[0].x,
            self.points[0].y,
            self.points[0].x,
            self.points[0].y,
        );
        for p in &self.points[1..] {
            r.x0 = r.x0.min(p.x);
            r.y0 = r.y0.min(p.y);
            r.x1 = r.x1.max(p.x);
            r.y1 = r.y1.max(p.y);
        }
        r
    }

    /// Even-odd containment test by ray casting in `f64`.
    #[must_use]
    pub fn contains(&self, p: Point<F>) -> bool {
        let (px, py) = (f64::from(p.x), f64::from(p.y));
        let n = self.points.len();
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = (f64::from(self.points[i].x), f64::from(self.points[i].y));
            let (xj, yj) = (f64::from(self.points[j].x), f64::from(self.points[j].y));
            if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Map every vertex through `m`.
    #[must_use]
    pub fn transform<B: Frame>(&self, m: &Affine<F, B>) -> Polygon<B> {
        Polygon {
            points: self.points.iter().map(|p| m.apply(*p)).collect(),
        }
    }
}

/// An affine map from frame `A` to frame `B`, stored as `[xx, yx, xy, yy, tx, ty]` (the SVG
/// `matrix(a b c d e f)` order):
///
/// ```text
/// x' = xx * x + xy * y + tx
/// y' = yx * x + yy * y + ty
/// ```
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Affine<A: Frame, B: Frame> {
    m: [f64; 6],
    frames: PhantomData<(A, B)>,
}

impl<A: Frame, B: Frame> Affine<A, B> {
    /// Build from the six matrix elements `[xx, yx, xy, yy, tx, ty]`.
    #[must_use]
    pub const fn from_matrix(m: [f64; 6]) -> Self {
        Self {
            m,
            frames: PhantomData,
        }
    }

    /// The six matrix elements `[xx, yx, xy, yy, tx, ty]`.
    #[must_use]
    pub const fn matrix(&self) -> [f64; 6] {
        self.m
    }

    /// The numerically identity map, tagged as a map from `A` to `B` (a pure re-labelling of frames).
    #[must_use]
    pub const fn identity() -> Self {
        Self::from_matrix([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    }

    /// Translation by `(tx, ty)`.
    #[must_use]
    pub const fn translation(tx: f64, ty: f64) -> Self {
        Self::from_matrix([1.0, 0.0, 0.0, 1.0, tx, ty])
    }

    /// Scaling by `(sx, sy)` about the origin.
    #[must_use]
    pub const fn scale(sx: f64, sy: f64) -> Self {
        Self::from_matrix([sx, 0.0, 0.0, sy, 0.0, 0.0])
    }

    /// Rotation by `theta` radians about the origin (counter-clockwise in y-up, clockwise on screen).
    /// Uses the platform's `sin` and `cos`; use [`Affine::from_matrix`] where bit-exactness matters.
    #[must_use]
    pub fn rotation(theta: f64) -> Self {
        let (s, c) = theta.sin_cos();
        Self::from_matrix([c, s, -s, c, 0.0, 0.0])
    }

    /// Map a point given in `f64`, without rounding to `f32`.
    #[must_use]
    pub fn apply_f64(&self, x: f64, y: f64) -> (f64, f64) {
        let m = &self.m;
        (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    }

    /// Map a stored point: computed in `f64`, rounded once to `f32`.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the single rounding to f32 is the storage contract"
    )]
    pub fn apply(&self, p: Point<A>) -> Point<B> {
        let (x, y) = self.apply_f64(f64::from(p.x), f64::from(p.y));
        Point::new(x as f32, y as f32)
    }

    /// Compose: first `self` (`A` to `B`), then `next` (`B` to `C`).
    #[must_use]
    pub fn then<C: Frame>(&self, next: &Affine<B, C>) -> Affine<A, C> {
        let a = &self.m;
        let b = &next.m;
        Affine::from_matrix([
            b[0] * a[0] + b[2] * a[1],
            b[1] * a[0] + b[3] * a[1],
            b[0] * a[2] + b[2] * a[3],
            b[1] * a[2] + b[3] * a[3],
            b[0] * a[4] + b[2] * a[5] + b[4],
            b[1] * a[4] + b[3] * a[5] + b[5],
        ])
    }

    /// Determinant of the linear part.
    #[must_use]
    pub fn determinant(&self) -> f64 {
        self.m[0] * self.m[3] - self.m[1] * self.m[2]
    }

    /// Ratio of the largest to the smallest singular value of the linear part (1 for a similarity,
    /// infinity for a singular or non-finite map).
    #[must_use]
    pub fn condition_number(&self) -> f64 {
        let m = &self.m;
        let frob = m[0] * m[0] + m[1] * m[1] + m[2] * m[2] + m[3] * m[3];
        let det = self.determinant();
        if !frob.is_finite() || !det.is_finite() || det == 0.0 {
            return f64::INFINITY;
        }
        let disc = (frob * frob - 4.0 * det * det).max(0.0).sqrt();
        let s_max = ((frob + disc) * 0.5).sqrt();
        let s_min = ((frob - disc) * 0.5).max(0.0).sqrt();
        if s_min == 0.0 {
            // Cancellation in `frob - disc` for a nearly singular map: recover s_min from det = s_max * s_min.
            return s_max / (det.abs() / s_max);
        }
        s_max / s_min
    }

    /// The inverse map, or `None` if the matrix is singular or not finite.
    #[must_use]
    pub fn inverse(&self) -> Option<Affine<B, A>> {
        let det = self.determinant();
        if !det.is_finite() || det == 0.0 || !self.m.iter().all(|v| v.is_finite()) {
            return None;
        }
        let m = &self.m;
        let inv = [
            m[3] / det,
            -m[1] / det,
            -m[2] / det,
            m[0] / det,
            (m[2] * m[5] - m[3] * m[4]) / det,
            (m[1] * m[4] - m[0] * m[5]) / det,
        ];
        inv.iter()
            .all(|v| v.is_finite())
            .then(|| Affine::from_matrix(inv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frames::{Page, PdfPoints, Source};

    /// SplitMix64, local to the tests because this crate may not depend on `wanjiru-math`.
    struct Sm(u64);
    impl Sm {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        #[allow(clippy::cast_precision_loss)]
        fn unit(&mut self) -> f64 {
            (self.next() >> 11) as f64 / (1_u64 << 53) as f64
        }
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.unit()
        }
    }

    fn random_affine(rng: &mut Sm, max_cond: f64) -> Affine<Source, Page> {
        let mean = rng.range(0.25, 4.0);
        let ratio = rng.range(1.0, max_cond);
        let (s1, s2) = (mean * ratio.sqrt(), mean / ratio.sqrt());
        let r1 = Affine::<Source, Source>::rotation(rng.range(0.0, std::f64::consts::TAU));
        let d = Affine::<Source, Source>::scale(s1, s2);
        let r2 = Affine::<Source, Source>::rotation(rng.range(0.0, std::f64::consts::TAU));
        let t = Affine::<Source, Page>::translation(
            rng.range(-5000.0, 5000.0),
            rng.range(-5000.0, 5000.0),
        );
        r1.then(&d).then(&r2).then(&t)
    }

    #[test]
    fn translation_and_scale_compose_in_order() {
        let s = Affine::<Source, Page>::scale(2.0, 3.0);
        let t = Affine::<Page, Page>::translation(10.0, 20.0);
        let st = s.then(&t);
        assert_eq!(st.apply_f64(1.0, 1.0), (12.0, 23.0));
        let ts = Affine::<Source, Source>::translation(10.0, 20.0)
            .then(&Affine::<Source, Page>::scale(2.0, 3.0));
        assert_eq!(ts.apply_f64(1.0, 1.0), (22.0, 63.0));
    }

    #[test]
    fn pixel_centre_scale_map() {
        // In continuous coordinates (pixel i covers [i, i+1)) the map from a destination raster to its
        // source for a scale factor s is a plain scaling. In index space, where pixel i sits at the
        // integer i, it becomes x_src = (x_dst + 0.5) / s - 0.5: conjugation by a half-pixel shift.
        let s = 4.0;
        let dst_to_src = Affine::<Page, Source>::scale(1.0 / s, 1.0 / s);
        let to_index = Affine::<Source, Source>::translation(-0.5, -0.5);
        for i in 0..64_u32 {
            let centre = f64::from(i) + 0.5;
            let (x, y) = dst_to_src.apply_f64(centre, centre);
            let expected_index = (f64::from(i) + 0.5) / s - 0.5;
            let (xi, yi) = to_index.apply_f64(x, y);
            assert!((xi - expected_index).abs() < 1e-12 && (yi - expected_index).abs() < 1e-12);
        }
        // The centre of destination pixel 5 is at 5.5; it lies in source pixel 1, at source coordinate 1.375.
        let (x, _) = dst_to_src.apply_f64(5.5, 5.5);
        assert!((x - 1.375).abs() < 1e-12);
    }

    #[test]
    fn inverse_of_singular_is_none() {
        assert!(Affine::<Source, Page>::scale(0.0, 1.0).inverse().is_none());
        assert!(
            Affine::<Source, Page>::from_matrix([f64::NAN, 0.0, 0.0, 1.0, 0.0, 0.0])
                .inverse()
                .is_none()
        );
        assert!(
            Affine::<Source, Page>::from_matrix([1e-200, 0.0, 0.0, 1e-200, 0.0, 0.0])
                .inverse()
                .is_none()
        );
    }

    #[test]
    fn condition_number_known_values() {
        assert!((Affine::<Source, Page>::rotation(0.7).condition_number() - 1.0).abs() < 1e-12);
        assert!((Affine::<Source, Page>::scale(10.0, 2.0).condition_number() - 5.0).abs() < 1e-12);
        assert!(
            Affine::<Source, Page>::scale(1.0, 0.0)
                .condition_number()
                .is_infinite()
        );
    }

    #[test]
    fn round_trip_f64_is_exact_to_a_nanopixel() {
        let mut rng = Sm(1);
        let mut worst = 0.0_f64;
        for _ in 0..100_000 {
            let forward = random_affine(&mut rng, 50.0);
            let backward = forward.inverse().expect("well conditioned");
            let start = (rng.range(0.0, 8000.0), rng.range(0.0, 8000.0));
            let mapped = forward.apply_f64(start.0, start.1);
            let back = backward.apply_f64(mapped.0, mapped.1);
            worst = worst.max((start.0 - back.0).hypot(start.1 - back.1));
        }
        assert!(worst < 1e-6, "worst f64 round trip displacement {worst}");
    }

    #[test]
    fn round_trip_f32_storage_stays_below_five_hundredths_of_a_pixel() {
        let mut rng = Sm(2);
        let mut worst = 0.0_f64;
        for _ in 0..100_000 {
            let m = random_affine(&mut rng, 50.0);
            let inv = m.inverse().expect("well conditioned");
            #[allow(clippy::cast_possible_truncation)]
            let p =
                Point::<Source>::new(rng.range(0.0, 8000.0) as f32, rng.range(0.0, 8000.0) as f32);
            let back = inv.apply(m.apply(p));
            worst = worst.max(p.distance(&back));
        }
        assert!(worst < 0.05, "worst f32 round trip displacement {worst}");
    }

    #[test]
    fn composition_is_associative_to_rounding() {
        let mut rng = Sm(3);
        for _ in 0..1000 {
            let a = random_affine(&mut rng, 10.0);
            let b = Affine::<Page, Page>::from_matrix(random_affine(&mut rng, 10.0).matrix());
            let c = Affine::<Page, Page>::from_matrix(random_affine(&mut rng, 10.0).matrix());
            let left = a.then(&b).then(&c).matrix();
            let right = a.then(&b.then(&c)).matrix();
            for (l, r) in left.iter().zip(right.iter()) {
                assert!((l - r).abs() <= 1e-9 * (1.0 + l.abs()), "{l} vs {r}");
            }
        }
    }

    #[test]
    fn pdf_page_flip_maps_corners() {
        let m = crate::units::pdf_to_source(792.0, crate::units::Dpi::new(144.0).expect("dpi"));
        // Bottom-left of a US-letter page (0, 0) is the bottom-left corner of a 1224 px tall raster.
        let (x, y) = m.apply_f64(0.0, 0.0);
        assert!((x - 0.0).abs() < 1e-12 && (y - 1584.0).abs() < 1e-12);
        let (x, y) = m.apply_f64(612.0, 792.0);
        assert!((x - 1224.0).abs() < 1e-12 && (y - 0.0).abs() < 1e-12);
        let _: Affine<PdfPoints, Source> = m;
    }

    #[test]
    fn rect_algebra() {
        let a = Rect::<Source>::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::<Source>::new(5.0, 5.0, 15.0, 15.0);
        assert_eq!(a.intersection(&b), Some(Rect::new(5.0, 5.0, 10.0, 10.0)));
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert!((a.iou(&b) - 25.0 / 175.0).abs() < 1e-12);
        assert!(a.contains(Point::new(0.0, 0.0)) && !a.contains(Point::new(10.0, 5.0)));
        assert_eq!(
            Rect::<Source>::new(3.0, 3.0, 1.0, 1.0),
            Rect::new(1.0, 1.0, 3.0, 3.0)
        );
        let far = Rect::<Source>::new(20.0, 20.0, 30.0, 30.0);
        assert!(a.intersection(&far).is_none());
        assert!(a.iou(&far).abs() < f64::EPSILON);
    }

    #[test]
    fn polygon_area_and_containment() {
        let square = Polygon::<Source>::new(vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
            Point::new(0.0, 4.0),
        ])
        .expect("square");
        assert!((square.area() - 16.0).abs() < 1e-12);
        assert!(square.contains(Point::new(2.0, 2.0)) && !square.contains(Point::new(5.0, 2.0)));
        let moved = square.transform(&Affine::<Source, Page>::translation(10.0, 0.0));
        assert_eq!(
            moved.bounding_rect(),
            Rect::<Page>::new(10.0, 0.0, 14.0, 4.0)
        );
    }

    #[test]
    fn polygon_rejects_degenerate_input() {
        let two = Polygon::<Source>::new(vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)]);
        assert!(two.is_err());
        let nan = Polygon::<Source>::new(vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, f32::NAN),
            Point::new(2.0, 0.0),
        ]);
        assert_eq!(
            nan.expect_err("nan").code(),
            crate::error::codes::INVALID_GEOMETRY
        );
    }
}
