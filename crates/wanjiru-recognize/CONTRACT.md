# wanjiru-recognize: contract

| | |
|---|---|
| Layer | L3 Domain logic (may depend on L0 to L2; no file-system or process access) |
| Status | Contract only. Baseline in Phase 3; character map and own models in Phase 4. |
| First working code | Phase 3 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Rectify each detected line to a fixed-height strip, run the recogniser and hand per-frame probabilities to the decoder. The rectification is recorded as a transform chain so that every character position maps back to the source image.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `rectify(&Image, &Polygon<Source>) -> Result<(Image, Chain)>`.
- `Recogniser::run(&Image) -> Result<FrameLogits>`.
- `CharMap` between model classes and Unicode.

## Invariants

1. The line chain is invertible; character boxes computed in the strip map back inside the original polygon.
2. The character map is part of the model package; the code never hard-codes an alphabet.
3. No file-system or process access.

## Oracles

What the crate is checked against, independently of its own tests:

- CER on the frozen line set, with an interval (via `wanjiru eval`).
- Rectification round trip on synthetic curved lines.

## Budget

- Entries in `docs/budgets.md`: recognition latency per line and per page; strip height and width limits.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`, `wanjiru-docir`, `wanjiru-imgproc`, `wanjiru-runtime`
- External: none
