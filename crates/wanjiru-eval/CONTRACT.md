# wanjiru-eval: contract

| | |
|---|---|
| Layer | L4 Orchestration (may depend on L0 to L3; budgets, scheduling, formats, metrics) |
| Status | Implemented in Phase 1; calibration metrics added in Phase 6. |
| First working code | Phase 1 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

The measuring instrument. Reads quality contracts, truth and prediction DocIR, computes character and word error rates, selective field accuracy and coverage, and latency quantiles, attaches an interval to every number, and returns three-valued gate verdicts (pass, fail, inconclusive). Reports are reproducible bit for bit.

## Public API

Implemented in this phase and described in the crate documentation; the list is the contract.

- `Contract::{from_yaml, from_json}`, `Contract::gate_count` and the types `FieldGate`, `Status`: the quality contract of a document class.
- `Corpus::parse` over three JSON Lines texts (reference, engine output, baseline output); `Truth`, `Case`.
- `cer::score`, `fields::tally`, `validators::{validate, mrz_check_digit}`: character error rate, selective accuracy and coverage per field.
- `intervals::{ratio_interval, relative_change, proportion, quantile}` with `Resampling`, `Method` (`BootstrapT`, `Percentile`) and `Interval`.
- `gates::{Verdict, Comparison, Gate, decide, overall}`: three-valued verdicts.
- `plan::{plan, minimum_detectable_difference}`: how many documents a golden set needs.
- `run::{Inputs, evaluate}` and `report::Report` (`wanjiru.eval-report/0`): `body`, `to_canonical_string`, `sha256`, `render_text`, `verdict`.
- `yaml::parse`: the YAML subset that contracts use, read into `Json`.

## Invariants

1. No file, network or clock access: every input is a string, so a report is a function of its inputs.
2. Every rate has an interval, and the report states the method, the level, the number of resamples, the seed and the counts behind it.
3. Character error rate and latency are resampled by document, never by character or field, because errors cluster within documents; the default is the bootstrap-t interval of a ratio of sums, the percentile interval on request.
4. A proportion of fields (accuracy among accepted fields, coverage) gets an exact Clopper-Pearson interval on its trials; a quantile gets an interval from order statistics. Neither is bootstrapped.
5. A gate is `pass` only if the interval proves it, `fail` only if the interval refutes it, otherwise `inconclusive`; a metric that cannot be measured yet (`ece_char`, `conformal`, until Phase 6) is `not_measured` and never counts as passed. The overall verdict is `fail` if any gate fails, else `inconclusive` if any is inconclusive or nothing was measured, else `pass`.
6. A missing prediction is a total failure: every reference character is an error and every field is not accepted. A field that is absent or sent to review is not accepted; it is counted, never dropped.
7. Reports contain no timestamps, host names or paths. The same inputs, settings and version give the same bytes, and the report ends with the SHA-256 of everything before it. Adding a baseline changes no other number (the paired comparison has its own stream).
8. The field validators (amount, ISO date, IBAN, MRZ check digits) are independent of `wanjiru-fields` on purpose.
9. Failures carry codes `WNJ-E4001` to `WNJ-E4007`.

## Oracles

What the crate is checked against, independently of its own tests:

- `tools/oracles/eval_oracle.py` is a second implementation of the whole harness (SciPy for Clopper-Pearson and the planner, exact fractions for order-statistic ranks, PyYAML for contracts, `datetime` for dates, its own dynamic programme for edit distance). On 12 generated corpora and on the four runs of `eval/fixtures/synthetic-v0` the library's reports agree with it: bootstrap intervals bit for bit, proportions to 1e-9, every count and verdict exactly (`tests/golden.rs`).
- Mutation check: 17 deliberate faults in the library (a missing correction, a swapped weight, a boundary off by one) are all detected: 10 by the unit tests (some also by the oracle comparison) and 7 only by the oracle comparison. The mutants are in `tools/oracles/mutants.py`, and the run fails on a survivor or on a mutant whose pattern is no longer in the source.
- Coverage simulation (`tests/coverage.rs`): the test asserts that 95 % bootstrap-t intervals contain the true error rate in 93.5 % to 96.5 % of 1,500 corpora of 360 documents. The survey for 50 to 1,000 documents is `#[ignore]`d and run nightly; its results are in `docs/lab/2026-10-29.md`.
- `tests/robustness.rs`: 24,000 mangled copies of the contracts and registries and 1,500 mangled evaluation runs return without a panic (checked to fail when a fault is injected into the YAML reader). The YAML reader refuses nesting deeper than 128, the limit of the JSON reader, with an error and not a stack overflow (unit test).
- Limit of this evidence: the oracle and the library were written by the same person, so the comparison finds implementation slips, not a shared misreading of a method.

## Budget

- Entry `B-eval-01` in `docs/budgets.md`: `evaluate` over 360 documents with 2,000 resamples. Target: under 2 s.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`, `wanjiru-docir`
- External: none
