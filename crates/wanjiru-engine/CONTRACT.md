# wanjiru-engine: contract

| | |
|---|---|
| Layer | L4 Orchestration (may depend on L0 to L3; budgets, scheduling, formats, metrics) |
| Status | Contract only. Pipeline in Phase 3; cascade and compute-follows-ink in Phase 10. |
| First working code | Phase 3 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Run the stages in order under one time, memory and thread budget, with cooperative cancellation, and assemble the DocIR result with its provenance and its manifest (configuration, versions, hashes).

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `Engine::new(Config) -> Result<Engine>`, `Engine::process(Input, &Options) -> Result<Document>`.
- `Budget { deadline, max_memory, threads }`, `CancelToken`.
- `Manifest` embedded in every result.

## Invariants

1. Every result records the effective configuration, so any output can be reproduced from its own record.
2. A cancelled or over-budget run returns promptly with a stable code and releases all memory.
3. The engine owns the only thread pool; stages receive slices of the budget.

## Oracles

What the crate is checked against, independently of its own tests:

- End-to-end golden outputs per determinism tier (hash comparison).
- Cancellation latency test; budget-exceeded tests; thread-count invariance for tier 1 stages.

## Budget

- Latency per page by class at p50 and p95 against the quality contracts (`eval/contracts`); peak memory per page.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`, `wanjiru-docir`, `wanjiru-imgproc`, `wanjiru-codecs`, `wanjiru-pdf`, `wanjiru-barcode`, `wanjiru-runtime`, `wanjiru-models`, `wanjiru-detect`, `wanjiru-recognize`, `wanjiru-decode`, `wanjiru-fields`, `wanjiru-layout`, `wanjiru-calibrate`
- External: none
