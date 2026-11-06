# examples/

Empty on purpose. Small programs that use only the public API of the `wanjiru` facade, so that a person can
copy one and have something that works. They need a facade that can process a document, which is Phase 3
(`crates/wanjiru/CONTRACT.md`: version 0 in Phase 3, version 1 with a frozen surface in Phase 8).

Until then the nearest thing to an example is the quick start in the repository's `README.md`, which runs the
evaluation harness (`wanjiru eval`) on the synthetic fixtures. Examples that belong to one crate, such as
`crates/wanjiru-cli/examples/budgets.rs`, live in that crate.
