# wanjiru-layout: contract

| | |
|---|---|
| Layer | L3 Domain logic (may depend on L0 to L2; no file-system or process access) |
| Status | Contract only. Implemented in Phase 7. |
| First working code | Phase 7 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Group lines into regions, determine reading order, detect table structure and pair keys with values, so that fields can be extracted from layouts the engine has never seen.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `analyse(&Document) -> Layout`.
- `Region`, `Table` (cells with spans), `KeyValue`.
- `reading_order(&[Region]) -> Vec<usize>`.

## Invariants

1. Layout analysis only adds structure and references to existing lines; it never changes recognised text.
2. Reading order is a permutation of the input regions.
3. Table cells tile the table region without overlap.

## Oracles

What the crate is checked against, independently of its own tests:

- Reading-order accuracy (Kendall tau) and table cell F1 on the annotated layout set.
- Synthetic layouts with known structure.

## Budget

- Entry in `docs/budgets.md`: layout time per page by number of lines.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`, `wanjiru-docir`, `wanjiru-imgproc`
- External: none
