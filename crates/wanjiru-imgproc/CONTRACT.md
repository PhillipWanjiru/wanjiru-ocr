# wanjiru-imgproc: contract

| | |
|---|---|
| Layer | L1 Core (may depend on L0; pure computation) |
| Status | Contract only. Implemented in Phase 2. |
| First working code | Phase 2 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Pure image computation under the pixel-centre convention: image buffers, colour conversion, resampling, binarisation (Sauvola with integral images), skew estimation, connected components with deterministic labelling, and morphology. Every operation reports the transform it applied so that DocIR provenance stays exact.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `Image<P>` buffers for grey and RGB with stride and bounds checks.
- `resample`, `to_grey`, `sauvola`, `estimate_skew`, `deskew`, `label_components`, `erode`, `dilate`.
- Each geometric operation returns `(Image, Affine<A, B>)`.

## Invariants

1. Resampling maps the centre of a destination pixel to the source: `x_src = x_dst / s` in continuous coordinates, which is `(i + 0.5) / s - 0.5` for pixel indices; a constant image stays constant; integer scale factors preserve the mean to within one grey level.
2. Connected-component labels are assigned in raster order of each component's first pixel (union-find with deterministic tie-breaks).
3. Integral images use `u64`; no operation overflows for images up to 2^28 pixels.
4. No hidden global state; threads are provided by the caller.

## Oracles

What the crate is checked against, independently of its own tests:

- Golden hashes of every operation on the synthetic test set (`tests/determinism`).
- Metamorphic: four 90-degree rotations are the identity; scale-up then scale-down bounded error; skew estimate recovers a known applied rotation within 0.05 degrees.
- Cross-check against OpenCV or Pillow in `tests/oracles` (offline, tolerance stated per operation).

## Budget

- Entries per operation in `docs/budgets.md`: milliseconds per megapixel and bytes of scratch memory per megapixel.

## Unsafe policy

`#![forbid(unsafe_code)]`. Fast paths come from `wanjiru-simd`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`
- External: none

## Open questions

- The layer table allows an L1 crate to depend only on L0, yet resampling and binarisation are the natural callers of `wanjiru-simd` (also L1). The workspace therefore lists `wanjiru-imgproc -> wanjiru-simd` as a proposed same-layer exception (ADR-011, status Proposed). Decide before Phase 2 starts: accept the exception, or move the kernel trait into L0.
