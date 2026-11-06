# wanjiru-detect: contract

| | |
|---|---|
| Layer | L3 Domain logic (may depend on L0 to L2; no file-system or process access) |
| Status | Contract only. Baseline in Phase 3; own model in Phase 7; sparse inference in Phase 10. |
| First working code | Phase 3 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Prepare images for the detector, turn its probability map into text-line polygons (thresholding, connected components, unclipping) and report them in source pixels.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `Detector::new(session) -> Detector`, `Detector::detect(&Image) -> Result<Vec<Polygon<Source>>>`.
- `postprocess(&ProbabilityMap, &Params) -> Vec<Polygon<Page>>`.

## Invariants

1. Pre- and post-processing are pure functions; the network call is the only effect and goes through `wanjiru-runtime`.
2. Every polygon returned is simple (non-self-intersecting) and has at least three vertices.
3. No file-system or process access (layer rule L3).

## Oracles

What the crate is checked against, independently of its own tests:

- Precision, recall and F1 at IoU 0.5 on the annotated detection set.
- Synthetic maps with known polygons (post-processing is exact on them).

## Budget

- Entries in `docs/budgets.md`: detection latency per megapixel; post-processing share of the total.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`, `wanjiru-docir`, `wanjiru-imgproc`, `wanjiru-runtime`
- External: none
