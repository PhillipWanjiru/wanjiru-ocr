# Architecture decision records

A decision that is expensive to reverse, or that other decisions lean on, gets a record here: what was
decided, why, what it costs, what else was considered, and what would make us reconsider. The records
are short on purpose. The code says *how*; these say *why*.

| ADR | Decision | Status |
|---|---|---|
| [001](001-rust-engine-and-c-abi.md) | Rust engine; the C ABI is the only binary contract | Accepted |
| [002](002-library-first-worker-process.md) | Library first; the desktop application is a public-API client; risky decoding runs in a worker process | Accepted |
| [003](003-runtime-trait-onnx-first.md) | Model runtime abstraction with ONNX Runtime first | Proposed |
| [004](004-docir-source-pixels-transform-chains.md) | DocIR is canonical; geometry in source pixels with invertible transform chains | Accepted |
| [005](005-determinism-tiers-and-own-maths.md) | Three determinism tiers; an own maths library for the part that must be exact | Accepted |
| [006](006-calibration-and-conformal-layer.md) | Calibration and the conformal layer are mandatory; calibrators ship in the model package | Accepted |
| [007](007-unsafe-and-dependency-policy.md) | Unsafe policy and dependency policy, enforced in CI | Accepted |
| [008](008-telemetry-and-consent.md) | Telemetry off by default; consent tiers for corrections | Accepted |
| [009](009-permissive-licence-per-model-licences.md) | A permissive licence for the code; each model, dataset and font keeps its own | Proposed |
| [010](010-ctc-primary-generators-behind-a-verifier.md) | A CTC recogniser is primary; generative readers only behind the optical verifier | Proposed |
| [011](011-same-layer-exceptions.md) | Same-layer dependencies: the kernel crate and the surfaces | Proposed |

ADR-001 to 010 are the initial decision set of the blueprint (Appendix B.3). ADR-011 was found while
writing the layer checker.

## Statuses

- **Proposed**: written down, waiting for evidence or for the owner. Work may follow it, at the risk of
  rework, and the record says what turns it into Accepted.
- **Accepted**: in force. The repository either enforces it already (the "Enforced by" line says how) or
  was built on it.
- **Superseded by ADR-nnn**: replaced. The old record stays, with a pointer, because the reasons it gave
  are part of the history.

"Accepted" in Phase 1 means the decision is in force in the repository and its checks run. It is not a
verdict that the decision is right for ever: every record names its own triggers for revisiting it.

## How to write one

Copy [`0000-template.md`](0000-template.md) to the next number, with a name that says the decision, not
the topic. Say what you decided in the title. Give the evidence you have and say plainly what you do not.
List the alternatives that were really considered, not ones added for show, and write the triggers for
reopening the question as things that can be observed.

Change a decision with a new record that supersedes it; do not edit the old one except to mark it
superseded. A change to a `CONTRACT.md` that is also a decision names its ADR in the same commit
(`CONTRACT.md` says so at the top of every crate's contract).
