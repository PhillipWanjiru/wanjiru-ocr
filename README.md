# Wanjiru System

A local-first OCR engine in Rust, built around a quality contract: every claim about accuracy, calibration and
latency is a test that can fail. First target: business paper, meaning invoices, receipts and identity documents
with a machine-readable zone. The name is a dedication ([DEDICATION.md](DEDICATION.md)).

> **Status: Phase 1 of 10 (foundation). There is no OCR yet.** The repository holds the structure later phases
> stand on and the harness that will judge them. The evaluation fixtures are fictional. Next is Phase 2: image
> and PDF decoding, SIMD kernels, the sandboxed worker and fuzzing (`crates/*/CONTRACT.md` say what lands when).

## What exists

- **Workspace:** 25 crates in six layers, each with a `CONTRACT.md`. Five have code (`types`, `math`, `docir`,
  `eval`, `cli`); twenty are contracts waiting for their phase.
- **DocIR v0** (`wanjiru-docir`): the format every stage reads and writes. JSON Schema, strict parser, RFC 8785
  canonical JSON with a stable SHA-256, geometry in source pixels with invertible transform chains (ADR-004).
- **`wanjiru eval`** (`wanjiru-eval`): exact and bootstrap-t intervals resampled by document, three-valued verdicts
  (pass, fail, inconclusive), a sample-size planner, independent validators for amounts, ISO dates, IBANs and ICAO
  machine-readable zones. Three draft quality contracts in `eval/contracts/`.
- **`cargo xtask check`:** the architecture rules below, as checks that fail the build.
- **Independent oracles** (`tools/oracles`): Python and Node re-implementations that generate the committed golden
  vectors, and a mutation check (17 planted faults, all caught).

## Quick start

Rust 1.97.0 is pinned in `rust-toolchain.toml`. There are no outside crates, so the build needs no network beyond
the toolchain.

```sh
rustup toolchain install        # the pinned toolchain
cargo test --workspace          # unit, golden-vector, transcript and robustness tests
cargo xtask check               # layers, contracts, unsafe, determinism, licences
cargo run --release -p wanjiru-cli -- eval \
  --contract eval/contracts/invoice-fields-v1.yaml \
  --truth eval/fixtures/synthetic-v0/truth.jsonl \
  --pred eval/fixtures/synthetic-v0/predictions.jsonl \
  --baseline eval/fixtures/synthetic-v0/baseline.jsonl
echo $?                         # 3 = inconclusive: 120 documents cannot prove a 99.9 % promise
```

`wanjiru help` lists the commands and `wanjiru errors` the error codes (`docs/errors.md`).

## Architecture

Dependencies point down only (`cargo xtask check-layers`; ADR-011 lists the exceptions).

| Layer | Crates (prefix `wanjiru-`) | Role |
|---|---|---|
| L5 Surfaces | `wanjiru` (facade), `cli`, `ffi`, `worker`, `server`, `py` | public API, C ABI, sandboxed decode worker, HTTP, Python |
| L4 Orchestration | `engine`, `emit`, `eval` | pipeline, output formats, evaluation |
| L3 Domain | `detect`, `recognize`, `decode`, `fields`, `layout`, `calibrate` | find, read, decode, extract fields, calibrate confidence |
| L2 I/O and runtime | `codecs`, `pdf`, `barcode`, `runtime`, `models` | parsers of untrusted input, model runtime and packages |
| L1 Core | `docir`, `simd`, `imgproc` | document IR, SIMD kernels, image processing |
| L0 Foundations | `types`, `math` | typed geometry, error codes, deterministic maths |

## Rules and what enforces them

| Rule | Enforced by |
|---|---|
| Lower layers only; each contract agrees with its manifest | `cargo xtask check-layers`, `check-contracts` |
| `unsafe` only in six crates, each block with a `SAFETY:` comment (ADR-007) | `cargo xtask check-unsafe` |
| The deterministic subset (`math`, `docir`, `eval`) is bit-identical across platforms: no platform libm, no hash-seeded containers, no clocks (ADR-005) | `cargo xtask check-determinism`, golden vectors, oracles |
| Only allowed licences in the dependency graph | `cargo xtask licences`, `cargo deny` in CI |
| Untrusted bytes are decoded in a separate process, with a fuzz target per parser (ADR-002, `SECURITY.md`) | from Phase 2 |
| No telemetry; recognition makes no network call (ADR-008) | contracts name every dependency |
| A performance budget is written before it is measured, and a miss stays a miss | `docs/budgets.md` |

Decisions are in `docs/adr/` (seven accepted, four proposed). What the checks themselves were tested against is in
`docs/lab/`.

## Known gaps

- No OCR, no models, no real documents. Nothing here is a claim about accuracy.
- One budget is missed: parsing, validating and canonicalising a 1 MB DocIR document takes 23.6 ms against a 20 ms
  target (`docs/budgets.md`). Timings come from shared machines, not a reference machine.
- The tests have run on x86-64 Linux only.
- CI (`.github/workflows`) was written without access to GitHub. Treat its first runs as unverified: `cargo deny`
  and `cargo audit` have never run, and the actions are pinned by major version, not by hash.
- The ADRs cite a design blueprint that is not in this repository.
- Open decisions for the owner, the licence among them: [docs/open-inputs.md](docs/open-inputs.md).

## Licence

Apache-2.0 ([LICENSE](LICENSE), [NOTICE](NOTICE)). Models and datasets keep their own licences (ADR-009, proposed).
