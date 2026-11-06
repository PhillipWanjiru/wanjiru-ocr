# wanjiru-server: contract

| | |
|---|---|
| Layer | L5 Surfaces (may depend on L0 to L4; the only crates that touch the outside world) |
| Status | Contract only. Implemented in Phase 8. |
| First working code | Phase 8 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

A localhost-only HTTP interface to the facade for tools that cannot link a library, with the same result types as everything else.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `serve(&ServerConfig) -> Result<()>`; routes `POST /v1/process`, `GET /v1/version`.

## Invariants

1. Binds to the loopback interface by default; any other bind requires an explicit option and a token.
2. Request size, rate and concurrency are limited.
3. No telemetry (ADR-008).

## Oracles

What the crate is checked against, independently of its own tests:

- Request fuzzing; limit tests; contract tests against the OpenAPI description in `docs/api`.

## Budget

- Overhead per request over a direct library call, recorded in `docs/budgets.md`.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-docir`, `wanjiru`
- External: none
- Planned external (needs an ADR and a row in `data/registry/licences.yaml` first): a small HTTP server crate, chosen by ADR in Phase 8

## Open questions

- `wanjiru-server -> wanjiru` is a same-layer dependency on the facade (ADR-011, Proposed).
