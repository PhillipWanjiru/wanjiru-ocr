# wanjiru-barcode: contract

| | |
|---|---|
| Layer | L2 I/O and runtime (may depend on L0 to L1 and `-sys` crates; parsers and FFI wrappers, sandbox first) |
| Status | Contract only. Implemented in Phase 7. |
| First working code | Phase 7 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Decode 1D and 2D codes found on invoices and receipts (EAN/UPC, Code 128, QR with payment payloads) and expose the decoded value as independent evidence for OCR fields: an IBAN in a QR code that disagrees with the printed IBAN is a review trigger, not a silent choice.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `scan(&Image) -> Vec<Symbol>` with symbology, payload, corners (as a `Polygon<Source>`) and check status.
- `parse_payment_payload(&Symbol) -> Option<PaymentFields>`.

## Invariants

1. A symbol is reported only if its own check digits or error-correction pass.
2. Corners are in source pixels.

## Oracles

What the crate is checked against, independently of its own tests:

- ISO/IEC test symbols and the public conformance sets for each symbology.
- Check-digit routines against standard vectors.

## Budget

- Entry in `docs/budgets.md`: scan time per megapixel with and without a hint region.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-imgproc`, `wanjiru-docir`
- External: none
