# wanjiru-calibrate: contract

| | |
|---|---|
| Layer | L3 Domain logic (may depend on L0 to L2; no file-system or process access) |
| Status | Contract only. Implemented in Phase 6. |
| First working code | Phase 6 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Make confidences honest (ADR-006): temperature and isotonic calibration, split and Mondrian conformal prediction, Clopper-Pearson certificates for accepted fields, Chow-rule thresholds, and an audit that any downstream rewrite of recognised text is explained by evidence.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `Calibrator::fit(&[(score, correct)]) -> Calibrator`, `Calibrator::apply`.
- `conformal_set(&Calibrator, alpha) -> PredictionSet`.
- `certificate(k, n, alpha) -> Interval`.
- `chow_threshold(c_rev, c_err) -> f64`.

## Invariants

1. Calibration data and evaluation data are disjoint by document.
2. A calibrator ships inside the model package it was fitted for.
3. Conformal sets have finite-sample marginal coverage at least 1 - alpha under exchangeability; the test suite checks this by simulation.

## Oracles

What the crate is checked against, independently of its own tests:

- Coverage of conformal sets over 2,000 simulated experiments lies within the binomial tolerance band.
- ECE, Brier and reliability bins against an offline reference implementation.

## Budget

- Entry in `docs/budgets.md`: calibration application cost per field.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`, `wanjiru-docir`
- External: none
