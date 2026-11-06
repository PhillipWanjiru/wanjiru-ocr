# wanjiru-cli: contract

| | |
|---|---|
| Layer | L5 Surfaces (may depend on L0 to L4; the only crates that touch the outside world) |
| Status | Implemented in Phase 1 (`wanjiru eval`, `wanjiru docir`); `run` in Phase 3; `calibrate` in Phase 6. |
| First working code | Phase 1 |
| Unsafe zone | no |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

The command-line surface. In Phase 1 it exposes the evaluation harness and DocIR utilities; later it grows the recognition commands. It is a client of the library like any other.

## Public API

Implemented in this phase and described in the crate documentation; the list is the contract.

- `wanjiru eval --contract C --truth T --pred P [--baseline B] [--level L] [--resamples N] [--seed N] [--method bootstrap-t|percentile] [--format text|json] [--out F] [--strict]`.
- `wanjiru eval plan [--p P] [--margin M] [--confidence C] [--fields-per-doc N] [--icc R] [--documents N] [--alpha A] [--power W]` and `wanjiru eval contract FILE`.
- `wanjiru docir check FILE...` (JSON Lines for names ending in `.jsonl`) and `wanjiru docir canon FILE [--hash]`.
- `wanjiru errors [CODE]`, `wanjiru version`, `wanjiru help`.

## Invariants

1. Exit codes are stable: 0 pass, 1 fail, 2 unusable input or usage, 3 inconclusive (1 under `--strict`).
2. The report goes to standard output in the chosen format; `--out` additionally writes the canonical JSON. Diagnostics go to standard error as `error[WNJ-Exxxx]: message (location)`. Nothing else is printed.
3. All behaviour is in `app::run`, which returns text and an exit code and reaches files only through `Environment`.
4. No network access and no telemetry (ADR-008).

## Oracles

What the crate is checked against, independently of its own tests:

- Transcript tests start the real binary on `eval/fixtures/synthetic-v0` and compare command, exit code and output with `expected/*.txt`; the four runs show `pass`, `fail` and `inconclusive`. The numbers behind them are checked against the Python oracle by `wanjiru-eval`'s golden test.
- Exit-code, usage-error and `--strict` tests; `docir check` accepts all 600 documents of the fixtures and names the file, line and code of a broken one.
- The `eval plan` defaults reproduce the 3,575 fields and 358 documents of the blueprint.
- `tests/repository_data.rs` reads the registries of `data/registry` (schema, required fields, licences against `deny.toml`, datasets against their datasheets) and checks that the research register of hypotheses in `design/research/insights.md` and the documents under `design/ux` name the same identifiers.

## Budget

- Entry `B-cli-01` in `docs/budgets.md`: start-up time of `wanjiru version`; and `B-eval-01`, which this command exercises. Both are measured by `crates/wanjiru-cli/examples/budgets.rs`, which prints the machine it ran on.

## Unsafe policy

`#![forbid(unsafe_code)]`.

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`, `wanjiru-docir`, `wanjiru-eval`
- External: none
