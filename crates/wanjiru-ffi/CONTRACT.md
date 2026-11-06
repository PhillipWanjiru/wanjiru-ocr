# wanjiru-ffi: contract

| | |
|---|---|
| Layer | L5 Surfaces (may depend on L0 to L4; the only crates that touch the outside world) |
| Status | Contract only. Implemented in Phase 8. |
| First working code | Phase 8 |
| Unsafe zone | yes |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

The stable binary contract (ADR-001): opaque handles, explicit ownership, `wnj_` function names and `WNJ_` constants, versioned structs, and a panic barrier on every entry point.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `extern "C"` functions named in `include/wanjiru.h`, for example `wnj_engine_new`, `wnj_process`, `wnj_result_free`.
- `WNJ_ABI_VERSION`, `wnj_version()`.

## Invariants

1. No panic crosses the boundary; every entry point catches unwinds and returns an error code.
2. Every allocation that crosses the boundary has exactly one documented owner and one `free` function.
3. The header and the exported symbols match exactly; a CI test fails on any difference.
4. Structs are versioned by a leading size field; old callers keep working.

## Oracles

What the crate is checked against, independently of its own tests:

- ABI snapshot test; C test programs built with `-Wall -Wextra -Werror` and run under sanitisers.
- Fuzzing of the entry points with malformed handles and sizes.

## Budget

- Call overhead per entry point recorded in `docs/budgets.md`.

## Unsafe policy

Unsafe zone: every `extern "C"` function is unsafe by nature. Unsafe code is limited to pointer validation at the boundary; everything behind it is safe Rust. Every `unsafe` block, `unsafe impl` and `unsafe` attribute has a `// SAFETY:` comment directly above it that says why it is sound; every `unsafe fn` and `unsafe trait` has a `# Safety` section in its documentation that says what the caller must guarantee. `cargo xtask check-unsafe` enforces both, and `unsafe` outside the six unsafe zones (`wanjiru-simd`, `wanjiru-runtime`, `wanjiru-codecs`, `wanjiru-pdf`, `wanjiru-ffi`, `wanjiru-worker`) is refused (ADR-007).

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-docir`, `wanjiru`
- External: none

## Open questions

- `wanjiru-ffi -> wanjiru` is a same-layer dependency on the facade (ADR-011, Proposed).
