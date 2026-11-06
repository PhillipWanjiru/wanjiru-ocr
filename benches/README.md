# benches/

Empty on purpose. Budgets are measured today by one program, `crates/wanjiru-cli/examples/budgets.rs`, and
recorded with the machine line in `docs/budgets.md`: release build only, median and spread of three runs,
a target written before it is measured, and a miss stays a miss.

**What is not possible yet.** A regression gate. The 5 % rule needs a quiet, named reference machine, and
naming it is an open decision of Phase 10 (`docs/budgets.md`, "The reference machine"). Until then no
benchmark blocks a merge, and a figure measured on a shared sandbox is not a reference figure.

**What waits for the engine.** Whole-pipeline benchmarks need an engine that runs: Phase 3
(`crates/wanjiru-engine/CONTRACT.md`). Their targets are the latency entries of the quality contracts in
`eval/contracts/`, not numbers written in this folder.
