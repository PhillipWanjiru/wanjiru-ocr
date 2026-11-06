# wanjiru-fields: contract

| | |
|---|---|
| Layer | L3 Domain logic (may depend on L0 to L2; no file-system or process access) |
| Status | Contract only. Implemented in Phase 5. |
| First working code | Phase 5 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Schemas for invoices, receipts and ID documents; the constraint library (IBAN modulus 97, amounts and dates by locale, MRZ check digits with weights 7, 3, 1 and the composite digit, Luhn masks); field extraction from DocIR lines; cross-field arithmetic (line totals sum to the subtotal, subtotal plus tax equals total, due date not before issue date).

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `Schema`, `FieldSpec`, `Constraint`.
- `extract(&Document, &Schema) -> Vec<Field>` with evidence attached.
- `checks::{iban_mod97, mrz_check_digit, luhn, amount, date}`.

## Invariants

1. Constraints produce evidence and may re-rank hypotheses; they never rewrite recognised text.
2. Every check is a pure function with a documented standard it implements.
3. A field accepted for automatic processing carries every check that applies to its kind.

## Oracles

What the crate is checked against, independently of its own tests:

- Standard vectors: ISO 13616 example IBANs, the ICAO 9303 specimen, Luhn test numbers.
- The evaluation harness keeps its own, independent copies of these checks on purpose, so that a shared bug cannot hide from the measuring instrument.

## Budget

- Entry in `docs/budgets.md`: extraction time per page for the three schemas.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`, `wanjiru-docir`
- External: none
