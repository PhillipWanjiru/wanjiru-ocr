# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project follows semantic versioning
once it has a release; until then every `0.x` version may change anything.

## [Unreleased]

### Added (Phase 1, the foundation)

- A workspace of 25 crates in six layers, each with a `CONTRACT.md` (`crates/*`). Five have code:
  `wanjiru-types`, `wanjiru-math`, `wanjiru-docir`, `wanjiru-eval`, `wanjiru-cli`.
- `wanjiru-types`: geometry in typed coordinate frames, units and the pixel-centre convention, the error taxonomy
  (`WNJ-E<layer><number>`) and the registry from which `docs/errors.md` is generated.
- `wanjiru-math`: Myers edit distance, exact binomial and Clopper-Pearson intervals, summation in a fixed order,
  xoshiro256** seeded by SplitMix64, SHA-256 and `exp` without the platform's maths library.
- `wanjiru-docir`: DocIR v0 with its JSON Schema, a strict JSON parser, canonical JSON (RFC 8785) and transform
  chains between frames.
- `wanjiru-eval` and the `wanjiru eval` command: selective accuracy and coverage, character error rate,
  bootstrap-t intervals by document, three-valued verdicts, a sampling-size planner, and independent validators for
  amounts, ISO dates, IBANs and ICAO machine-readable zones.
- `wanjiru docir check|canon`, `wanjiru errors`, `wanjiru version`.
- Three draft quality contracts (`eval/contracts`) for invoices, receipts and identity documents, and 300
  fictional synthetic documents with four transcripts of runs.
- `cargo xtask check`: layers, contracts against manifests, unsafe policy, determinism and licences.
- Independent oracles (Python, Node) and committed golden vectors; a mutation check with 17 planted faults.
- ADRs 001 to 011 (seven accepted, four proposed); registries for licences, datasets, models and fonts; the datasheet
  and manifest of the planned golden set; `docs/budgets.md` with eight measured entries; `docs/lab` notes.
- Discovery research plan, interview guide, consent-form template and competitive-audit protocol; personas,
  jobs, journey map and success metrics written as hypotheses (`design/`).
- Continuous-integration workflows (`ci`, `security`, `nightly`) and a Dependabot configuration, not yet run on
  GitHub.
- Placeholder folders for later phases (`apps/`, `training/`, `models/`, `include/`, `tests/`, `fuzz/`, `benches/`,
  `examples/`, `packaging/`), each saying what will live there and why it is empty.
- `docs/open-inputs.md`: the owner's open questions, moved out of the README, which is now short and technical.

### Fixed

- The YAML reader crashed (stack overflow) on a contract with very deep nesting. It now refuses nesting deeper
  than 128, the limit of the JSON reader (`docs/lab/2026-10-29.md`, A1).

### Known problems

- The DocIR budget `B-docir-01` is missed: 23.6 ms against a target below 20 ms (`docs/budgets.md`).
- There is no OCR, no model and no real document in the repository.
