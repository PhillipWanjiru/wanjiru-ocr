# wanjiru-types: contract

| | |
|---|---|
| Layer | L0 Foundation (may depend on the standard library only; no I/O, no globals) |
| Status | Implemented in Phase 1. |
| First working code | Phase 1 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

The shared vocabulary of the engine: points, rectangles, polygons and affine maps in coordinate frames that the compiler keeps apart, units with explicit conversions, and the error-code taxonomy. Everything above this crate speaks these types, so a pixel coordinate can never be passed where a PDF-point coordinate is expected.

## Public API

Implemented in this phase and described in the crate documentation; the list is the contract.

- `Point<F>`, `Size<F>`, `Rect<F>` and `Polygon<F>`: `f32` coordinates tagged with a frame `F`; `Rect::{intersection, union, iou, contains}`, `Polygon::{signed_area, bounding_rect, contains, transform}`.
- `Affine<A, B>`: an `f64` map from frame `A` to frame `B`, with `apply`, `apply_f64`, `then`, `inverse`, `determinant` and `condition_number`.
- The `Frame` trait, the zero-sized markers `Source`, `Page`, `Line`, `PdfPoints` and `Normalised`, and `known_frame_name`.
- `Dpi`, `points_to_pixels`, `pixels_to_points`, `mm_to_points`, `pdf_to_source`.
- `Layer`, `ErrorCode` (`WNJ-E<layer><number>`), `ErrorKind`, `WnjError`, `Result<T>` and the registry `codes::ALL`, from which `docs/errors.md` is generated.

## Invariants

1. Pixel `(i, j)` covers `[i, i+1) x [j, j+1)` in continuous coordinates and its centre is `(i + 0.5, j + 0.5)`. Every geometry in the engine is continuous: a map from destination to source pixels is applied to centres, not to indices.
2. Transforms are computed in `f64`; polygon and rectangle coordinates are stored as `f32`. A forward-then-inverse round trip is exact to 1e-6 px in `f64` (points up to 8,000 px, condition number up to 50). With `f32` storage the error is about 6e-8 times the coordinate times the condition number: below 0.05 px in the same range.
3. NaN in gives NaN out; an uninvertible map gives `None`; a degenerate polygon is refused with an error.
4. Frames are zero-sized types; mixing frames is a compile error, not a runtime check.
5. Error codes are unique, sit in the block of their layer (`WNJ-E1xxx` belongs to L1, and so on) and are never reused: a retired code stays retired.
6. Library code contains no `unwrap`, `expect` or `panic!`; the one `assert!` is in a `const fn`, where it turns an out-of-range error code into a compile error.

## Oracles

What the crate is checked against, independently of its own tests:

- Round trip over 10^5 random maps (rotation, anisotropic scale, rotation, translation; condition number up to 50) on points up to 8,000 px: worst displacement below 1e-6 px in `f64` and below 0.05 px with `f32` storage (unit tests in `src/geometry.rs`).
- Composition is associative to 1e-9 (relative to 1 + |entry|) on 1,000 random triples of maps (unit tests). An exact-arithmetic oracle follows in Phase 2, in a directory named tests/oracles.
- The error registry has unique codes, each in the block of its layer (unit test); `docs/errors.md` is that registry as a table, and a test of `wanjiru-cli` fails if the file and the output of `wanjiru errors` differ.
- Planned for Phase 2: compile-fail tests, in a directory named tests/ui, showing that mixing frames does not compile.

## Budget

- No heap allocation in any geometry operation (`Polygon` owns one `Vec`; everything else is `Copy`).
- Entry `B-types-01` in `docs/budgets.md`: `Affine::then` and `Affine::apply`.

## Unsafe policy

`#![forbid(unsafe_code)]`. No exceptions.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: none
- External: none
