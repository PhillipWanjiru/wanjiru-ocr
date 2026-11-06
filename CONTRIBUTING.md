# Contributing

The project has one maintainer today and is pre-release. This page says how to build, what a change has to
satisfy, and how decisions are made, so that a change can be judged by the same rules as every other.

## Build and test

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask check
```

All four must pass. `rust-toolchain.toml` pins the toolchain; changing it is a reviewed change on its own, with the
full suite run and any recorded hashes updated in the same commit.

## What every change must respect

1. **Layers.** A crate depends on lower layers only (L0 to L5, `crates/*/CONTRACT.md`). The few same-layer
   exceptions are listed in `tools/xtask/src/layers.rs` and ADR-011.
2. **Contracts.** Each crate's `CONTRACT.md` says what it offers, what must hold, how that is checked and what it may
   depend on. A change to behaviour changes the contract first. A promise without a check is not a promise.
3. **Dependencies.** None is added casually. A new outside crate needs a line in the contract of the crate that uses it,
   a row in `data/registry/licences.yaml` with its licence verified at the source, a licence on the allow-list
   (`deny.toml`), and an ADR when the choice is not obvious (ADR-007).
4. **Unsafe.** Only in the six crates named in ADR-007, each block with a `// SAFETY:` comment, each `unsafe fn` with a
   `# Safety` section.
5. **Determinism.** `wanjiru-math`, `wanjiru-docir` and `wanjiru-eval` give bit-identical results everywhere:
   no platform maths functions, no hash-seeded containers, no clock, summation in a fixed order (ADR-005). A deliberate
   exception carries `// determinism: allow <reason>` on its line.
6. **Errors.** A failure has a stable code in `wanjiru-types` (`codes::ALL`); `docs/errors.md` is generated from it and a test
   fails if they differ. Codes are never renumbered or reused.
7. **No telemetry, no network** in the engine (ADR-008). A real identity document, bank detail or invoice never enters
   the repository, not even as a fixture; fixtures are fictional.

## Tests: how a claim is checked

- A test that compares code with expected values written by the same person proves little. Where a standard or a
  well-known library exists (RFC 8785, ICAO 9303, SciPy, `hashlib`, PyYAML), an independent implementation in
  `tools/oracles` generates golden vectors, which are committed; the Rust tests need neither Python nor Node.
- Regenerate vectors with `python3 -I tools/oracles/make_*_vectors.py` and read the diff. Re-bless transcripts with
  `WANJIRU_BLESS=1 cargo test -p wanjiru-cli --test transcripts` and read the diff.
- If you change `wanjiru-eval`, run `python3 tools/oracles/mutants.py`. It plants known faults and fails if any survive
  or the list has gone stale. Do not run it next to other `cargo` commands: it edits sources in place and restores them.
- A budget is written down before it is measured (`docs/budgets.md`). A miss is recorded as a miss.

## Decisions

A decision that is expensive to reverse gets an ADR (`docs/adr/0000-template.md`): context, decision, consequences,
alternatives considered, and when to revisit it. *Proposed* means the owner has not accepted it. A change that
contradicts an accepted ADR changes the ADR first.

## Pull requests

- One change of one kind. Say what it changes and why, and which contract, check or budget it touches.
- Keep the diff reviewable. Run `cargo fmt`. Do not mix a refactor with a behaviour change.
- Say what you did not verify.
- By submitting a contribution you agree that it is licensed under the Apache License 2.0, as section 5 of that licence
  provides.

## Conduct and security

Be kind and be exact; see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Security problems are reported privately
([SECURITY.md](SECURITY.md)).
