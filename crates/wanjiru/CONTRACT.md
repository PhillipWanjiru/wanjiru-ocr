# wanjiru: contract

| | |
|---|---|
| Layer | L5 Surfaces (may depend on L0 to L4; the only crates that touch the outside world) |
| Status | Contract only. Version 0 in Phase 3; version 1 (frozen surface) in Phase 8. |
| First working code | Phase 3 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

The one crate that applications, bindings and the desktop app are allowed to depend on. It re-exports a small, deliberately narrow API and hides every internal crate (dogfooding rules).

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `Engine`, `Options`, `Document` and the result types, re-exported.
- `version()`, `build_info()`.

## Invariants

1. Nothing is public here that is not documented, tested and covered by the semver check.
2. Internal crates are never re-exported wholesale.

## Oracles

What the crate is checked against, independently of its own tests:

- `cargo public-api` snapshot and semver checks in CI.
- Doc tests for every public item.

## Budget

- No code of its own; overhead of the facade over the engine is zero by construction (inlined re-exports).

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-docir`, `wanjiru-engine`, `wanjiru-emit`, `wanjiru-models`
- External: none
