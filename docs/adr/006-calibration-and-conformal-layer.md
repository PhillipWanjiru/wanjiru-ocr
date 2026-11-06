# ADR-006: Calibration and the conformal layer are mandatory; calibrators ship in the model package

| | |
|---|---|
| Status | Accepted |
| Date | 2026-11-02 |
| Phase | 1 (the measuring side); Phase 6 (the engine side) |
| Supersedes | none |
| Enforced by | `wanjiru eval` (three-valued gates, `not_measured` never counts as passed); the model-package rule in `crates/wanjiru-models/CONTRACT.md` from Phase 3 |

## Context

- A business customer does not need a confidence. They need to know which fields to check and what
  error rate is left among the rest. That is a statement about selective accuracy at a given coverage,
  and it needs probabilities that mean what they say.
- Raw scores from recognisers are not probabilities, and they drift when a model is retrained.
- A promise such as "99.9 % of accepted fields are correct" can only be shown with enough documents.
  `wanjiru eval plan` gives the size: for 99 % accuracy to within 0.5 points at 95 % confidence it asks for
  1,522 independent fields; with ten fields per document and a correlation of 0.15 within a document,
  3,575 fields over 358 documents. Correlation inside a document is why the intervals are resampled
  by document (`wanjiru-eval`, invariant 3).

## Decision

1. Every task that emits a confidence has a calibrator, and a model package without one is refused for
   such tasks (`wanjiru-models`).
2. Calibrators (temperature scaling, isotonic regression) and conformal thresholds (split and Mondrian,
   by document class) are fitted on held-out calibration data and ship inside the model package, versioned
   with the weights they were fitted to.
3. Acceptance of a field follows the Chow rule: accept when the calibrated confidence reaches the
   threshold that the class's quality contract needs; send it to review otherwise. An accepted field
   carries a Clopper-Pearson certificate computed on the calibration set, and the audit in
   `wanjiru-calibrate` checks that nothing downstream changes the confidences it was given.
4. The quality contract states what has to be proven, and `wanjiru eval` returns `pass`, `fail` or
   `inconclusive`. A metric that cannot be measured yet (`ece_char`, `conformal`, until Phase 6) is
   `not_measured` and never counts as passed.
5. Calibration is fitted per document class on data of that class, never borrowed across classes unless a
   test shows the borrowed fit holds.

## Consequences

- Confidences are honest and testable. The review queue is a decision with a stated error rate, not a
  guess.
- Every document class needs calibration data in addition to test data, which is a real cost. The
  golden-set plan counts it (`data/datasheets/golden-v0.md`).
- Retraining a model means refitting and re-certifying its calibrator. A release gate checks that the
  package's calibrator belongs to its weights.
- Measured in Phase 1, as support for the three-valued gates: 95 % intervals for a ratio of sums over 360
  documents contain the true value in 93.5 % to 96.5 % of 1,500 simulated corpora when the bootstrap-t
  method is used; the percentile method fell short at that size (93.2 % at 360 documents, 88.3 % at 50).
  The harness therefore defaults to bootstrap-t. The survey is in `docs/lab/2026-10-29.md`.

## Alternatives considered

- **Raw softmax confidences with hand-picked thresholds.** Rejected: miscalibrated by construction, and
  the thresholds would have to be re-picked after every retraining.
- **Calibrate later, if customers ask.** Rejected: it is the same retrofit problem as determinism, and
  the model package format would have to change.
- **Ensembles or dropout variance as the confidence.** Not rejected as a signal, but not a substitute:
  their output also needs calibration.

## Revisit when

Calibration data cannot be obtained for key classes.
