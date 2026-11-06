# ADR-003: Model runtime abstraction with ONNX Runtime first

| | |
|---|---|
| Status | Proposed |
| Date | 2026-11-02 |
| Phase | 1 (decision); Phase 3 (first back end) |
| Supersedes | none |
| Enforced by | nothing yet: `wanjiru-runtime` is contract only |

Proposed, not accepted, because the decision is only as good as the measurements of Phase 3 and the
choice of binding is open. It becomes Accepted when an ONNX back end has run the golden-set models on
the hardware matrix with the results in `docs/budgets.md`.

## Context

- Models are trained in PyTorch and exported to ONNX. Time to a competitive baseline matters.
- Hardware coverage matters: CPU everywhere, then CUDA, Core ML and DirectML.
- Determinism, binary size and WebAssembly support will matter later (ADR-005).
- A bespoke runtime is a multi-year liability if it is started early.

## Decision

1. All networks run behind a `ModelRuntime` trait in `wanjiru-runtime`; no other crate names a back
   end.
2. The first back end wraps ONNX Runtime. Which binding (`ort` or direct `onnxruntime-sys`) is a
   follow-up decision: it needs a reviewed row in `data/registry/licences.yaml` and a place in the
   dependency list of `wanjiru-runtime` before the first line is written.
3. A native Rust back end for the recogniser and detector operator subset is developed later. Both
   back ends must pass the same parity tests.
4. `wanjiru-runtime` is an unsafe zone limited to the ONNX Runtime binding (ADR-007). The engine's
   single thread budget includes the runtime's threads.

## Consequences

- A fast start and broad hardware reach.
- Runtime threading must be integrated into the engine's thread budget.
- Determinism guarantees are limited to what the runtime documents and what the tests prove. In the
  strictest mode only the CPU path is allowed, because accelerators differ from it at the bit level.
- Every model package declares the operators it needs, so that the native back end's coverage can be
  measured against real models and not assumed.

## Alternatives considered

- **A native runtime from day one.** Rejected: schedule risk.
- **`rten` or `tract` as the only back end.** Rejected: operator coverage and performance are
  unproven for these models.

## Revisit when

A determinism requirement cannot be met by the runtime; binary size or WebAssembly needs cannot be
met; runtime licensing or security issues arise; or native kernels are within 10 % of runtime
performance on the golden hardware matrix.
