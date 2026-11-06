# ADR-010: A CTC recogniser is primary; generative readers only behind the optical verifier

| | |
|---|---|
| Status | Proposed |
| Date | 2026-11-02 |
| Phase | 1 (decision); Phase 3 (baseline recogniser); Phase 4 onwards (verifier measured) |
| Supersedes | none |
| Enforced by | nothing yet: `wanjiru-recognize` and `wanjiru-decode` are contract only |

Proposed, not accepted, because there is no recogniser in the repository yet, and the claim that the
verifier works is a measurement. It becomes Accepted when the verifier has been measured on a set of
hard negatives, with the intervals `wanjiru eval` reports.

## Context

- There are two families of line readers. A **CTC** recogniser outputs a distribution over characters for
  each horizontal position and decodes a monotonic alignment, so it cannot write what no part of the line
  supports. An **attention decoder** or a document vision-language model has an implicit language model.
  That helps on degraded text, and it is also how a reader silently rewrites what it sees.
- On an invoice or an identity document the worst error is the plausible one: a fluent amount or a
  well-formed number that is wrong, and that nobody looks at twice.
- The engine owes the user character boxes (provenance) and honest confidences (ADR-006). An alignment
  gives the first, and calibration works on scores that are tied to the pixels.
- Joint CTC and attention decoding already uses CTC prefix scores to veto attention hypotheses the pixels
  do not support. That veto is the seed of the verifier.

## Decision

1. The primary recogniser is a CTC line recogniser (`wanjiru-recognize`: an open-component baseline in
   Phase 3, own models in Phase 4), decoded by `wanjiru-decode` with constraints (beam search first,
   language model and automata in Phase 5).
2. A generator (a compact document VLM, a language-model post-corrector, a language-model-heavy decoder)
   may only **propose**. Its text enters DocIR only if the optical verifier supports it. For a line
   crop `x` and a proposed string `s`, the verifier computes `L(s) = log P_ctc(s | x)` with the CTC forward
   algorithm, in `O(T |s|)` and with no autoregressive pass, and accepts `s` only if the gap between
   its own best string's `L` and `L(s)` is at most a threshold `τ`.
3. `τ` is calibrated per slice (document class, script, line type) so that the probability of accepting a
   wrong string is at most a target `α`, on a labelled set of hard negatives built from real generator
   hallucinations and synthetic rewrites. It is a Neyman-Pearson-style test, and its error rates are reported
   with intervals like every other number.
4. The verifier is independent of the generator: a different architecture and different training data. The
   correlation of the two error sets (the phi coefficient) is measured and kept low.
5. A rejected proposal goes to review. A typed field still has to pass the validators (amount, date, IBAN,
   check digits) whoever proposed it.
6. Generators remain welcome as teachers: pseudo-labels are kept only where several teachers agree and the
   verifier gives the string a high likelihood.

## Consequences

- Faithfulness is a measured property: nothing is emitted that the pixels do not support, to a stated error
  rate, and the alignment gives each character its box.
- Some degraded or handwritten text that a VLM would read will be sent to review instead. That is the
  intended trade: a visible miss over a silent error.
- A CTC recogniser has a weaker implicit language model, so more depends on explicit lexicons, constraints and
  the decoder. This is why `wanjiru-decode` has its own phase.
- The hard-negative set and the independence measurement cost effort. They are part of the golden-set plan.
- Follow-up: the contracts of `wanjiru-recognize` and `wanjiru-decode` do not name the verifier today. The
  contract of the crate that holds it is amended in the commit that adds it.

## Alternatives considered

- **An attention decoder or a VLM as the primary reader.** Rejected: fluent errors, confidences that are
  hard to calibrate, no alignment for provenance, and slower.
- **A generator without verification, with spot checks.** Rejected: the errors are silent, and a spot check
  cannot bound their rate.
- **An ensemble of generators that must agree.** Rejected as a substitute: they share priors and tend to
  hallucinate the same plausible string.

## Revisit when

A generator proves reliable without verification on the target classes: on the hard-negative set its
accepted-wrong rate, with an interval, meets the class's quality contract by itself.
