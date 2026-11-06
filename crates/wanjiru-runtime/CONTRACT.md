# wanjiru-runtime: contract

| | |
|---|---|
| Layer | L2 I/O and runtime (may depend on L0 to L1 and `-sys` crates; parsers and FFI wrappers, sandbox first) |
| Status | Contract only. Trait and ONNX back end in Phase 3; native back end later (ADR-003). |
| First working code | Phase 3 |
| Unsafe zone | yes |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

All networks run behind one trait so that the engine never names a back end. The first back end wraps ONNX Runtime; a native Rust back end for the recogniser and detector operator subset follows when determinism, size or WebAssembly needs it (ADR-003).

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `trait ModelRuntime { fn load(&self, &ModelPackage) -> Result<Session>; }` and `Session::run(&Tensors) -> Result<Tensors>`.
- `Tensor` views with shape and strides, `Precision`, `DeviceKind`.
- `RuntimeInfo` reporting back end, version and determinism tier.

## Invariants

1. The runtime's thread pool is taken from the engine's single thread budget; no back end spawns threads of its own.
2. Every `Session::run` is bounded by a deadline and can be cancelled.
3. The determinism tier a back end claims is the one its tests prove, no more.

## Oracles

What the crate is checked against, independently of its own tests:

- Back-end parity: the same package gives outputs within the tolerance stated per tier on the golden inputs.
- Tier tests: repeated runs, different thread counts, different batch sizes.

## Budget

- Entries in `docs/budgets.md`: latency and peak memory per model on the reference machine; model load time.

## Unsafe policy

Unsafe zone, limited to the ONNX Runtime binding. Same rules as `wanjiru-codecs`. Every `unsafe` block, `unsafe impl` and `unsafe` attribute has a `// SAFETY:` comment directly above it that says why it is sound; every `unsafe fn` and `unsafe trait` has a `# Safety` section in its documentation that says what the caller must guarantee. `cargo xtask check-unsafe` enforces both, and `unsafe` outside the six unsafe zones (`wanjiru-simd`, `wanjiru-runtime`, `wanjiru-codecs`, `wanjiru-pdf`, `wanjiru-ffi`, `wanjiru-worker`) is refused (ADR-007).

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-math`
- External: none
- Planned external (needs an ADR and a row in `data/registry/licences.yaml` first): `ort` or direct `onnxruntime-sys` bindings (MIT/Apache-2.0), chosen by ADR-003 follow-up
