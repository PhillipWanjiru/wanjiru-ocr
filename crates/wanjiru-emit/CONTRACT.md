# wanjiru-emit: contract

| | |
|---|---|
| Layer | L4 Orchestration (may depend on L0 to L3; budgets, scheduling, formats, metrics) |
| Status | Contract only. JSON, text and hOCR in Phase 3; ALTO, PAGE, CSV and Markdown in Phase 7; PDF/A in Phase 8. |
| First working code | Phase 3 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Render DocIR into the formats other tools expect. DocIR stays canonical; every format is a lossy view and says what it drops.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `emit(&Document, Format, &EmitOptions) -> Result<Vec<u8>>`.
- `Format::{Json, Text, Hocr, Alto, Page, PdfA, Csv, Markdown}`.

## Invariants

1. Emission is a pure function of the document and the options.
2. Every format documents the DocIR fields it cannot represent.
3. Escaping is correct for the target syntax (no unescaped `<` or `&` in markup).

## Oracles

What the crate is checked against, independently of its own tests:

- Schema validation of hOCR, ALTO and PAGE output with the published XSDs.
- veraPDF for PDF/A.
- Round trip DocIR to JSON to DocIR.

## Budget

- Entry in `docs/budgets.md`: emission time per 1,000 lines for each format.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-docir`
- External: none
