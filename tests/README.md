# tests/

Empty on purpose. A test that belongs to one crate lives in that crate's own `tests/` directory, and its
`CONTRACT.md` names it. This folder is for tests that belong to none.

The first will be `tests/determinism`, from Phase 2: the bit-exact hashes of the deterministic subset's results,
recorded for the pinned toolchain. `rust-toolchain.toml` says why it matters: changing the toolchain is a
reviewed change that records the new hashes in the same commit.

That does not mean nothing pins determinism today. The Phase 1 golden vectors do it crate by crate
(`crates/*/tests/golden/`, `eval/fixtures/synthetic-v0/expected/`), checked against independent oracles
(`tools/oracles/`), and `cargo xtask check-determinism` forbids platform maths, hash-seeded containers and clocks
in the deterministic subset (ADR-005). They have been run on one platform only: Linux, x86-64.
