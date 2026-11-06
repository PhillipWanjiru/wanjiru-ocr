# wanjiru-decode: contract

| | |
|---|---|
| Layer | L3 Domain logic (may depend on L0 to L2; no file-system or process access) |
| Status | Contract only. CTC and beam search in Phase 3; language model, MBR, lattices and automata in Phase 5. |
| First working code | Phase 3 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Turn per-frame probabilities into text hypotheses: CTC best path and prefix beam search, character language model fusion, minimum-Bayes-risk selection, lattices and constraint automata. Constraints are evidence, not coercion: they re-rank hypotheses and never invent characters.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `ctc_forward(log_probs, labels) -> f64` (the oracle), `best_path`, `prefix_beam_search(&Frames, &Params) -> Vec<Hypothesis>`.
- `CharLm`, `Lattice`, `Automaton`, `mbr_select`.

## Invariants

1. Beam search is deterministic: ties are broken by `total_cmp` and then by lexicographic order of the hypothesis.
2. The sum of probabilities over all labellings equals 1 within 1e-12 (checked on toy alphabets).
3. A constraint can only select among hypotheses that the optical model proposed; it cannot add symbols (`#[test]` with an adversarial automaton).

## Oracles

What the crate is checked against, independently of its own tests:

- CTC forward in log space versus brute-force enumeration over all alignments on alphabets of up to 4 symbols and 8 frames (verified numerically in Part I, maximum difference about 1e-16).
- Beam search with an unbounded beam equals exhaustive search on toy problems.
- Language-model scores against an offline reference implementation.

## Budget

- Entries in `docs/budgets.md`: decode time per line for beam widths 1, 8, 32; allocation count per line.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`, `wanjiru-docir`
- External: none
