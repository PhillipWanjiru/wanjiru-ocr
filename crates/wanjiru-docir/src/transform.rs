//! Transform chains: how the source image was turned into the frames the models saw.
//!
//! Every geometry in DocIR is stored in source pixels, so that a box on screen is a box on the
//! customer's original. Deskew, crop, rescale and perspective correction are recorded as a [`Chain`]
//! of steps, each from one named frame to the next. A chain can be applied to a point, composed with
//! another chain and inverted; the inverse of a chain is again a chain.
//!
//! Arithmetic is in `f64`. Round trips are exact to about `1e-6` px for coordinates up to `10^4` px
//! and chains whose condition number stays below `10^3`; the `f32` coordinates stored in a document
//! add a representation error of about `6e-8` times the coordinate.

use core::marker::PhantomData;

use wanjiru_types::frames::known_frame_name;
use wanjiru_types::{Affine, ErrorKind, Frame, Result, Source, WnjError, codes};

/// A projective map between frames `A` and `B`, as a row-major 3x3 matrix acting on `(x, y, 1)`:
/// `[x', y', w'] = M [x, y, 1]` and the image of the point is `(x'/w', y'/w')`. Only defined up to a
/// scale factor; two matrices that differ by one describe the same map.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Homography<A: Frame, B: Frame> {
    m: [f64; 9],
    frames: PhantomData<(A, B)>,
}

fn invert_3x3(matrix: &[f64; 9]) -> Option<[f64; 9]> {
    let [m00, m01, m02, m10, m11, m12, m20, m21, m22] = *matrix;
    // The adjugate: the transposed matrix of cofactors.
    let adjugate = [
        m11 * m22 - m12 * m21,
        m02 * m21 - m01 * m22,
        m01 * m12 - m02 * m11,
        m12 * m20 - m10 * m22,
        m00 * m22 - m02 * m20,
        m02 * m10 - m00 * m12,
        m10 * m21 - m11 * m20,
        m01 * m20 - m00 * m21,
        m00 * m11 - m01 * m10,
    ];
    let det = m00 * adjugate[0] + m01 * adjugate[3] + m02 * adjugate[6];
    if !det.is_finite() || det == 0.0 || !matrix.iter().all(|v| v.is_finite()) {
        return None;
    }
    let inverse = adjugate.map(|v| v / det);
    inverse.iter().all(|v| v.is_finite()).then_some(inverse)
}

fn apply_3x3(m: &[f64; 9], x: f64, y: f64) -> Option<(f64, f64)> {
    let w = m[6] * x + m[7] * y + m[8];
    if w == 0.0 {
        return None;
    }
    let out = (
        (m[0] * x + m[1] * y + m[2]) / w,
        (m[3] * x + m[4] * y + m[5]) / w,
    );
    (out.0.is_finite() && out.1.is_finite()).then_some(out)
}

impl<A: Frame, B: Frame> Homography<A, B> {
    /// Build from the nine elements, row by row.
    #[must_use]
    pub const fn from_matrix(m: [f64; 9]) -> Self {
        Self {
            m,
            frames: PhantomData,
        }
    }

    /// The nine elements, row by row.
    #[must_use]
    pub const fn matrix(&self) -> [f64; 9] {
        self.m
    }

    /// The projective form of an affine map.
    #[must_use]
    pub fn from_affine(a: &Affine<A, B>) -> Self {
        let [xx, yx, xy, yy, tx, ty] = a.matrix();
        Self::from_matrix([xx, xy, tx, yx, yy, ty, 0.0, 0.0, 1.0])
    }

    /// Map a point; `None` if it goes to infinity (`w' = 0`) or the result is not finite.
    #[must_use]
    pub fn apply_f64(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        apply_3x3(&self.m, x, y)
    }

    /// Compose: first `self` (`A` to `B`), then `next` (`B` to `C`).
    #[must_use]
    pub fn then<C: Frame>(&self, next: &Homography<B, C>) -> Homography<A, C> {
        let (a, b) = (&self.m, &next.m);
        let mut out = [0.0_f64; 9];
        for row in 0..3 {
            for col in 0..3 {
                // out = next * self (self acts first on column vectors).
                out[row * 3 + col] =
                    b[row * 3] * a[col] + b[row * 3 + 1] * a[3 + col] + b[row * 3 + 2] * a[6 + col];
            }
        }
        Homography::from_matrix(out)
    }

    /// The inverse map, or `None` if the matrix is singular or not finite.
    #[must_use]
    pub fn inverse(&self) -> Option<Homography<B, A>> {
        invert_3x3(&self.m).map(Homography::from_matrix)
    }
}

/// One step of a chain, without its frames: an affine map (six numbers) or a homography (nine).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Transform {
    /// `[xx, yx, xy, yy, tx, ty]`, as in [`Affine`].
    Affine([f64; 6]),
    /// Row-major 3x3, as in [`Homography`].
    Homography([f64; 9]),
}

impl Transform {
    /// `"affine"` or `"homography"`: the spelling used in DocIR JSON.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Affine(_) => "affine",
            Self::Homography(_) => "homography",
        }
    }

    /// The matrix elements, in the order of [`Transform::kind`]'s documentation.
    #[must_use]
    pub fn matrix(&self) -> &[f64] {
        match self {
            Self::Affine(m) => m,
            Self::Homography(m) => m,
        }
    }

    /// Build from the JSON spelling: a kind and the matrix elements.
    pub fn from_kind(kind: &str, matrix: &[f64]) -> Result<Self> {
        let wrong_length = |expected: usize| {
            WnjError::new(
                codes::CHAIN_INVALID,
                ErrorKind::InvalidInput,
                format!(
                    "a {kind} step has {expected} matrix elements, got {}",
                    matrix.len()
                ),
            )
        };
        match kind {
            "affine" => <[f64; 6]>::try_from(matrix)
                .map(Self::Affine)
                .map_err(|_| wrong_length(6)),
            "homography" => <[f64; 9]>::try_from(matrix)
                .map(Self::Homography)
                .map_err(|_| wrong_length(9)),
            other => Err(WnjError::new(
                codes::CHAIN_INVALID,
                ErrorKind::InvalidInput,
                format!("unknown step kind {other:?}; expected \"affine\" or \"homography\""),
            )),
        }
    }

    /// Map a point; `None` if the result is not finite or the point goes to infinity.
    #[must_use]
    pub fn apply(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        match self {
            Self::Affine(matrix) => {
                let (px, py) = Affine::<Source, Source>::from_matrix(*matrix).apply_f64(x, y);
                (px.is_finite() && py.is_finite()).then_some((px, py))
            }
            Self::Homography(matrix) => apply_3x3(matrix, x, y),
        }
    }

    /// The inverse transform, or `None` if singular or not finite.
    #[must_use]
    pub fn inverse(&self) -> Option<Self> {
        match self {
            Self::Affine(m) => Affine::<Source, Source>::from_matrix(*m)
                .inverse()
                .map(|inv| Self::Affine(inv.matrix())),
            Self::Homography(m) => invert_3x3(m).map(Self::Homography),
        }
    }

    fn check(&self) -> Result<()> {
        if self.inverse().is_some() {
            Ok(())
        } else {
            Err(WnjError::new(
                codes::CHAIN_INVALID,
                ErrorKind::InvalidInput,
                format!("a {} step must be finite and invertible", self.kind()),
            ))
        }
    }
}

/// A step of a [`Chain`]: the transform that leads into the frame `to`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Step {
    to: &'static str,
    transform: Transform,
}

impl Step {
    /// The frame this step arrives in.
    #[must_use]
    pub const fn to(&self) -> &'static str {
        self.to
    }

    /// The map from the previous frame to [`Step::to`].
    #[must_use]
    pub const fn transform(&self) -> &Transform {
        &self.transform
    }
}

/// A connected sequence of transforms starting in a named frame. Frames are named by the stable
/// names of `wanjiru_types::Frame` (`source`, `page`, `line`, `pdf`, `normalised`); a chain cannot be
/// built from an unknown name, from a step that stays in its frame, or from a singular transform.
#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    from: &'static str,
    steps: Vec<Step>,
}

fn chain_error(message: String) -> WnjError {
    WnjError::new(codes::CHAIN_INVALID, ErrorKind::InvalidInput, message)
}

impl Chain {
    /// A chain with no steps, starting in the frame named `from`.
    pub fn new(from: &str) -> Result<Self> {
        let from =
            known_frame_name(from).ok_or_else(|| chain_error(format!("unknown frame {from:?}")))?;
        Ok(Self {
            from,
            steps: Vec::new(),
        })
    }

    /// A chain with no steps, starting in frame `F`.
    #[must_use]
    pub fn starting_in<F: Frame>() -> Self {
        Self {
            from: F::NAME,
            steps: Vec::new(),
        }
    }

    /// The frame the chain starts in.
    #[must_use]
    pub const fn from_frame(&self) -> &'static str {
        self.from
    }

    /// The frame the chain ends in (the start frame for an empty chain).
    #[must_use]
    pub fn to_frame(&self) -> &'static str {
        self.steps.last().map_or(self.from, Step::to)
    }

    /// The steps, in order.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Append a step into the frame named `to`.
    pub fn push(&mut self, to: &str, transform: Transform) -> Result<()> {
        let to =
            known_frame_name(to).ok_or_else(|| chain_error(format!("unknown frame {to:?}")))?;
        if to == self.to_frame() {
            return Err(chain_error(format!(
                "a step must change frame, but {to:?} follows {to:?}"
            )));
        }
        transform.check()?;
        self.steps.push(Step { to, transform });
        Ok(())
    }

    /// Append a typed affine step; its source frame must be the frame the chain ends in.
    pub fn push_affine<A: Frame, B: Frame>(&mut self, map: &Affine<A, B>) -> Result<()> {
        self.expect_end(A::NAME)?;
        self.push(B::NAME, Transform::Affine(map.matrix()))
    }

    /// Append a typed homography step; its source frame must be the frame the chain ends in.
    pub fn push_homography<A: Frame, B: Frame>(&mut self, map: &Homography<A, B>) -> Result<()> {
        self.expect_end(A::NAME)?;
        self.push(B::NAME, Transform::Homography(map.matrix()))
    }

    fn expect_end(&self, frame: &str) -> Result<()> {
        if self.to_frame() == frame {
            Ok(())
        } else {
            Err(chain_error(format!(
                "the step starts in frame {frame:?} but the chain ends in {:?}",
                self.to_frame()
            )))
        }
    }

    /// Map a point from the start frame to the end frame; `None` if some step sends it to infinity.
    #[must_use]
    pub fn map_point(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        self.steps
            .iter()
            .try_fold((x, y), |(px, py), step| step.transform.apply(px, py))
    }

    /// The chain that undoes this one: it starts where this one ends, with the inverse steps in
    /// reverse order.
    pub fn inverse(&self) -> Result<Self> {
        let mut inverse = Self {
            from: self.to_frame(),
            steps: Vec::with_capacity(self.steps.len()),
        };
        for index in (0..self.steps.len()).rev() {
            let back_to = if index == 0 {
                self.from
            } else {
                self.steps[index - 1].to
            };
            let transform = self.steps[index]
                .transform
                .inverse()
                .ok_or_else(|| chain_error(format!("step {index} cannot be inverted")))?;
            inverse.steps.push(Step {
                to: back_to,
                transform,
            });
        }
        Ok(inverse)
    }

    /// This chain followed by `next`; `next` must start in the frame this chain ends in.
    pub fn compose(&self, next: &Chain) -> Result<Self> {
        self.expect_end(next.from)?;
        let mut steps = self.steps.clone();
        steps.extend_from_slice(&next.steps);
        Ok(Self {
            from: self.from,
            steps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wanjiru_math::rng::Rng;
    use wanjiru_types::{Line, Normalised, Page};

    fn close(a: (f64, f64), b: (f64, f64), tolerance: f64) -> bool {
        (a.0 - b.0).abs() <= tolerance && (a.1 - b.1).abs() <= tolerance
    }

    #[test]
    fn homography_basics() {
        let shift =
            Homography::<Source, Page>::from_matrix([1.0, 0.0, 5.0, 0.0, 1.0, -3.0, 0.0, 0.0, 1.0]);
        assert_eq!(shift.apply_f64(1.0, 1.0), Some((6.0, -2.0)));
        let back = shift.inverse().expect("invertible");
        assert!(close(
            back.apply_f64(6.0, -2.0).expect("finite"),
            (1.0, 1.0),
            1e-12
        ));
        // A genuine perspective map: the image of (1, 1) divides by w = 1 + 0.5 + 0.25.
        let persp =
            Homography::<Source, Page>::from_matrix([2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.5, 0.25, 1.0]);
        assert!(close(
            persp.apply_f64(1.0, 1.0).expect("finite"),
            (2.0 / 1.75, 2.0 / 1.75),
            1e-15
        ));
        // The horizon: w' = 0 has no finite image.
        assert_eq!(persp.apply_f64(-2.0, 0.0), None);
        // A singular matrix has no inverse.
        assert!(
            Homography::<Source, Page>::from_matrix([1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 0.0, 0.0, 1.0])
                .inverse()
                .is_none()
        );
    }

    #[test]
    fn homography_from_affine_agrees_with_the_affine() {
        let a = Affine::<Source, Page>::from_matrix([0.9, 0.1, -0.1, 0.9, 12.0, -7.0]);
        let h = Homography::from_affine(&a);
        for (x, y) in [(0.0, 0.0), (100.0, 50.0), (-30.0, 400.0)] {
            assert!(close(
                h.apply_f64(x, y).expect("finite"),
                a.apply_f64(x, y),
                1e-12
            ));
        }
    }

    #[test]
    fn homography_composition_applies_the_first_map_first() {
        let rot = Homography::<Source, Page>::from_affine(&Affine::rotation(0.3));
        let persp =
            Homography::<Page, Line>::from_matrix([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1e-3, 2e-3, 1.0]);
        let both = rot.then(&persp);
        let (x, y) = (40.0, 25.0);
        let (mx, my) = rot.apply_f64(x, y).expect("finite");
        let step_by_step = persp.apply_f64(mx, my).expect("finite");
        assert!(close(
            both.apply_f64(x, y).expect("finite"),
            step_by_step,
            1e-9
        ));
    }

    #[test]
    fn chains_are_built_and_checked() {
        let mut chain = Chain::starting_in::<Source>();
        assert_eq!(chain.to_frame(), "source");
        chain
            .push_affine(&Affine::<Source, Page>::from_matrix([
                0.5, 0.0, 0.0, 0.5, 0.0, 0.0,
            ]))
            .expect("source to page");
        chain
            .push_homography(&Homography::<Page, Line>::from_matrix([
                1.0, 0.0, -10.0, 0.0, 1.0, -20.0, 0.0, 0.0, 1.0,
            ]))
            .expect("page to line");
        assert_eq!((chain.from_frame(), chain.to_frame()), ("source", "line"));
        assert_eq!(chain.steps().len(), 2);
        assert_eq!(chain.steps()[1].transform().kind(), "homography");
        assert_eq!(chain.map_point(100.0, 100.0), Some((40.0, 30.0)));

        // A typed step must start where the chain ends.
        let wrong = Affine::<Source, Normalised>::from_matrix([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        assert_eq!(
            chain
                .push_affine(&wrong)
                .expect_err("frame mismatch")
                .code()
                .to_string(),
            "WNJ-E1004"
        );
        // No step may stay in its frame, name an unknown frame or be singular.
        assert!(
            chain
                .push("line", Transform::Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]))
                .is_err()
        );
        assert!(
            chain
                .push("plane", Transform::Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]))
                .is_err()
        );
        assert!(
            chain
                .push("pdf", Transform::Affine([1.0, 2.0, 2.0, 4.0, 0.0, 0.0]))
                .is_err()
        );
        assert!(
            chain
                .push(
                    "pdf",
                    Transform::Affine([f64::NAN, 0.0, 0.0, 1.0, 0.0, 0.0])
                )
                .is_err()
        );
        assert!(Chain::new("nowhere").is_err());
        assert_eq!(
            chain.steps().len(),
            2,
            "failed pushes leave the chain unchanged"
        );
    }

    #[test]
    fn inverse_and_composition() {
        let mut chain = Chain::new("source").expect("known frame");
        chain
            .push("page", Transform::Affine([0.0, 1.0, -1.0, 0.0, 300.0, 0.0]))
            .expect("push"); // quarter turn
        chain
            .push(
                "normalised",
                Transform::Affine([0.001, 0.0, 0.0, 0.001, 0.0, 0.0]),
            )
            .expect("push");
        let inverse = chain.inverse().expect("invertible");
        assert_eq!(
            (inverse.from_frame(), inverse.to_frame()),
            ("normalised", "source")
        );
        assert_eq!(inverse.steps()[0].to(), "page");
        for (x, y) in [(0.0, 0.0), (120.0, 340.0), (999.0, 1.0)] {
            let there = chain.map_point(x, y).expect("finite");
            let back = inverse.map_point(there.0, there.1).expect("finite");
            assert!(
                close(back, (x, y), 1e-9),
                "({x}, {y}) -> {there:?} -> {back:?}"
            );
        }
        // chain followed by its inverse is the identity map on the source frame.
        let round = chain.compose(&inverse).expect("connected");
        assert_eq!((round.from_frame(), round.to_frame()), ("source", "source"));
        assert!(close(
            round.map_point(7.0, 9.0).expect("finite"),
            (7.0, 9.0),
            1e-9
        ));
        // Composition needs matching frames.
        assert!(chain.compose(&chain).is_err());
        // Inverse of an empty chain is an empty chain.
        let empty = Chain::starting_in::<Page>();
        assert_eq!(empty.inverse().expect("trivial"), empty);
    }

    #[test]
    fn from_kind_checks_lengths() {
        assert!(Transform::from_kind("affine", &[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]).is_ok());
        assert!(
            Transform::from_kind("homography", &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
                .is_ok()
        );
        assert!(Transform::from_kind("affine", &[1.0; 9]).is_err());
        assert!(Transform::from_kind("homography", &[1.0; 6]).is_err());
        assert!(Transform::from_kind("similarity", &[1.0; 4]).is_err());
    }

    fn random_chain(rng: &mut Rng) -> Chain {
        const FRAMES: [&str; 5] = ["source", "page", "line", "pdf", "normalised"];
        let mut chain = Chain::new("source").expect("known");
        let steps = 1 + rng.below_usize(4);
        for _ in 0..steps {
            let mut to = FRAMES[rng.below_usize(FRAMES.len())];
            while to == chain.to_frame() {
                to = FRAMES[rng.below_usize(FRAMES.len())];
            }
            let uniform = |rng: &mut Rng, lo: f64, hi: f64| lo + (hi - lo) * rng.next_f64();
            let scale = uniform(rng, 0.5, 2.0);
            let theta = uniform(rng, -3.1, 3.1);
            let shear = uniform(rng, -0.1, 0.1);
            let (s, c) = (theta.sin(), theta.cos());
            let affine = [
                scale * c,
                scale * s,
                -scale * s + shear * scale * c,
                scale * c + shear * scale * s,
                uniform(rng, -100.0, 100.0),
                uniform(rng, -100.0, 100.0),
            ];
            let transform = if rng.below(3) == 0 {
                // A mild perspective: |w - 1| stays below 0.4 for coordinates up to 10^4 px.
                let p = uniform(rng, -2e-5, 2e-5);
                let q = uniform(rng, -2e-5, 2e-5);
                Transform::Homography([
                    affine[0], affine[2], affine[4], affine[1], affine[3], affine[5], p, q, 1.0,
                ])
            } else {
                Transform::Affine(affine)
            };
            chain.push(to, transform).expect("valid step");
        }
        chain
    }

    #[test]
    fn inverse_round_trip_over_random_chains_including_homographies() {
        let mut rng = Rng::seed_from_u64(20_261_030);
        let mut worst = 0.0_f64;
        let mut homographies = 0_u32;
        for _ in 0..20_000 {
            let chain = random_chain(&mut rng);
            homographies += u32::from(
                chain
                    .steps()
                    .iter()
                    .any(|s| s.transform().kind() == "homography"),
            );
            let inverse = chain.inverse().expect("random chains are invertible");
            let x = 10_000.0 * rng.next_f64();
            let y = 10_000.0 * rng.next_f64();
            // Skip points that a perspective step sends near its horizon, where the problem itself
            // is ill-conditioned.
            let Some((u, v)) = chain.map_point(x, y) else {
                continue;
            };
            if u.abs() > 1e6 || v.abs() > 1e6 {
                continue;
            }
            let (bx, by) = inverse.map_point(u, v).expect("finite");
            worst = worst.max((bx - x).abs()).max((by - y).abs());
        }
        assert!(
            homographies > 5_000,
            "homographies exercised: {homographies}"
        );
        assert!(worst < 1e-6, "worst round-trip error {worst:e} px");
    }
}
