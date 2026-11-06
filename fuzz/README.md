# fuzz/

Empty on purpose. Coverage-guided fuzz targets, one per reader of untrusted bytes, arrive with the readers
themselves: the image decoders and the PDF reader in Phase 2 (`crates/wanjiru-codecs/CONTRACT.md`,
`crates/wanjiru-pdf/CONTRACT.md`), the model loader in Phase 3 (`SECURITY.md`). The rule in `SECURITY.md` is
that each reader gets its fuzz target before the reader is used, and runs behind the sandboxed worker.

**The tool is not chosen.** `cargo-fuzz`, for one, needs a nightly toolchain, and this workspace pins a stable one
(`rust-toolchain.toml`). The choice is made when the first target is written.

**What exists instead, today.** Deterministic mangling tests for the three readers that exist in Phase 1:
`crates/wanjiru-docir/tests/robustness.rs` (JSON and the DocIR reader, validator and writer) and
`crates/wanjiru-eval/tests/robustness.rs` (the YAML subset and the evaluation harness). They take the repository's
real files, damage them with a seeded random stream, and assert that the reader returns. They are cheap and they
run on every `cargo test`. They have not found a crash so far. The one crash found in Phase 1, a stack overflow
in the YAML reader, came from a crafted file of 200,000 nested brackets (`docs/lab/2026-10-29.md`, A1), which is
input that damaging a few characters of a real file never produces. So they are not a substitute for
coverage-guided fuzzing, and they say nothing about what the readers do with a very large file.
