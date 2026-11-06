# wanjiru-models: contract

| | |
|---|---|
| Layer | L2 I/O and runtime (may depend on L0 to L1 and `-sys` crates; parsers and FFI wrappers, sandbox first) |
| Status | Contract only. Registry in Phase 4; signing in Phase 8. |
| First working code | Phase 3 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

A model package is a directory with weights, a manifest (`<task>-<script>-v<major>`, for example `rec-latin-v1`), the character map, the calibrator and the model card. This crate reads and verifies packages; weights live outside git (manifests only in `models/`).

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `ModelPackage::open(path) -> Result<ModelPackage>`.
- `Manifest` (task, script, version, licence, input contract, hashes).
- `verify(&ModelPackage, &TrustRoots) -> Result<()>`.

## Invariants

1. A package whose files do not match the hashes in its manifest is rejected before any weight is read.
2. A package without a calibrator is rejected for tasks that emit confidences (ADR-006).
3. Manifests are canonical JSON and are hashed and signed in that form.

## Oracles

What the crate is checked against, independently of its own tests:

- Tamper tests: flip one byte in each file, expect rejection.
- Signature test vectors; manifest schema fixtures.

## Budget

- Entry in `docs/budgets.md`: package open and verify time for a 50 MB package.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`
- External: none
- Planned external (needs an ADR and a row in `data/registry/licences.yaml` first): a signature crate (Ed25519), chosen by ADR in Phase 8
