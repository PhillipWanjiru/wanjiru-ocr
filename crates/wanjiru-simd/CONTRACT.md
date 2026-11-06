# wanjiru-simd: contract

| | |
|---|---|
| Layer | L1 Core (may depend on L0; pure computation) |
| Status | Contract only. First kernels in Phase 2; tuned kernels in Phase 10. |
| First working code | Phase 2 |
| Unsafe zone | yes |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Vectorised kernels (grey conversion, resampling taps, integral images, dot products, morphology) behind safe functions. Every kernel has a scalar reference in the same crate; dispatch happens once per process from CPU feature detection, and results must not depend on which path ran (tier 1 kernels) or must stay within a stated tolerance (tier 2).

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- Safe functions over slices, for example `rgb_to_grey(&[u8], &mut [u8])`, `integral_u64(...)`, `dot_fixed_tree(...)`.
- `Isa` and `active_isa()` for diagnostics; `force_scalar()` for tests and reproducible builds.

## Invariants

1. Every public function has a scalar reference; for tier 1 kernels the vector path equals the scalar path bit for bit on all inputs.
2. No public `unsafe fn`. Unsafe blocks are confined to private modules, each with a `// SAFETY:` comment stating the proof obligation.
3. No kernel reads or writes outside its slices; lengths are checked once at the safe boundary.
4. Dispatch is deterministic for a given CPU and never changes mid-process.

## Oracles

What the crate is checked against, independently of its own tests:

- Differential test: vector path versus scalar path on random and adversarial inputs, all lengths from 0 to 4,096 and every alignment.
- Miri on the scalar paths; sanitiser runs (address, undefined behaviour) on the vector paths in the nightly workflow.

## Budget

- Each kernel has an entry in `docs/budgets.md` with the scalar time, the vector time and the speed-up; a kernel that is not at least 2x faster than scalar on the reference machine is deleted, not kept.

## Unsafe policy

Unsafe zone. `#![deny(unsafe_code)]` at the crate root; each module that needs intrinsics carries `#[allow(unsafe_code)]` and a module-level note. `clippy::undocumented_unsafe_blocks` is denied. Every `unsafe` block, `unsafe impl` and `unsafe` attribute has a `// SAFETY:` comment directly above it that says why it is sound; every `unsafe fn` and `unsafe trait` has a `# Safety` section in its documentation that says what the caller must guarantee. `cargo xtask check-unsafe` enforces both, and `unsafe` outside the six unsafe zones (`wanjiru-simd`, `wanjiru-runtime`, `wanjiru-codecs`, `wanjiru-pdf`, `wanjiru-ffi`, `wanjiru-worker`) is refused (ADR-007).

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`
- External: none
