# wanjiru-worker: contract

| | |
|---|---|
| Layer | L5 Surfaces (may depend on L0 to L4; the only crates that touch the outside world) |
| Status | Contract only. Decode job in Phase 2; full engine hosting in Phase 3; installers in Phase 9. |
| First working code | Phase 2 |
| Unsafe zone | yes |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

A separate process with no network and a tight system-call allow-list that parses untrusted bytes and returns pixels and metadata over a length-prefixed protocol. A crash in a decoder kills the worker, not the application (ADR-002).

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `worker_main()`; the protocol types `Request`, `Response`, `JobError` with a version field.
- `spawn(&WorkerConfig) -> WorkerHandle` for hosts.

## Invariants

1. The worker has no network access and can touch only the file descriptors it is handed.
2. Memory, CPU time and wall-clock limits are enforced by the operating system, not by the worker's good behaviour.
3. The protocol is versioned, length-prefixed and fuzzed; a malformed frame ends the session.

## Oracles

What the crate is checked against, independently of its own tests:

- Sandbox tests per platform: forbidden system calls are blocked; resource limits trip.
- Crash-isolation test: a decoder aborted on purpose does not take down the host.
- Protocol fuzzing.

## Budget

- Worker start-up time and per-job round-trip overhead recorded in `docs/budgets.md`.

## Unsafe policy

Unsafe zone, limited to the sandbox set-up (seccomp, sandbox_init, job objects) and file-descriptor handling. Every `unsafe` block, `unsafe impl` and `unsafe` attribute has a `// SAFETY:` comment directly above it that says why it is sound; every `unsafe fn` and `unsafe trait` has a `# Safety` section in its documentation that says what the caller must guarantee. `cargo xtask check-unsafe` enforces both, and `unsafe` outside the six unsafe zones (`wanjiru-simd`, `wanjiru-runtime`, `wanjiru-codecs`, `wanjiru-pdf`, `wanjiru-ffi`, `wanjiru-worker`) is refused (ADR-007).

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-docir`, `wanjiru-imgproc`, `wanjiru-codecs`, `wanjiru-pdf`
- External: none
- Planned external (needs an ADR and a row in `data/registry/licences.yaml` first): platform sandbox bindings, chosen by ADR in Phase 2
