# ADR-001: Rust engine; the C ABI is the only binary contract

| | |
|---|---|
| Status | Accepted |
| Date | 2026-11-02 |
| Phase | 1 |
| Supersedes | none |
| Enforced by | `rust-toolchain.toml`, `[workspace.package]` in `Cargo.toml`; the C ABI itself from Phase 8 (`wanjiru-ffi`) |

## Context

- The engine parses bytes it cannot trust (images, PDFs, identity documents) and runs numeric kernels
  where speed matters. The blueprint's finding F-04 says the question "Rust or C" hides the real
  decision, which is where `unsafe` code, C dependencies and process boundaries sit.
- One person builds this, with experience in Rust and systems programming (the owner's answer before
  Phase 1 began).
- The consumers are a command line, a server, a desktop application and language bindings (Python
  first). All of them need a binary surface that does not change when a compiler does. Rust has no
  stable ABI, and C++ ABIs differ between compilers.
- Version 1 reads business documents: invoices, receipts and identity documents.

## Decision

1. The engine is written in Rust: edition 2024, the toolchain pinned in `rust-toolchain.toml`, the
   minimum supported version in `Cargo.toml` (`rust-version`).
2. The only binary contract is a C ABI, owned by `wanjiru-ffi`: opaque handles, explicit ownership,
   `wnj_` function names and `WNJ_` constants, versioned structs, and a panic barrier on every entry
   point. Every other crate is a source-level interface and may change between versions.
3. No Rust type crosses the C boundary. Failures cross as `WNJ-E…` codes from the registry in
   `wanjiru-types` (`docs/errors.md`) plus a message.
4. `unsafe` code and C dependencies are confined to named crates (ADR-007). Language bindings go
   through the facade `wanjiru` or the C ABI, never through crate internals.

## Consequences

- Memory safety is the default for the large majority of the code. The places where it is not are
  six crates, listed in one table in `tools/xtask/src/layers.rs` and checked by `cargo xtask check`.
- One stable surface serves every language; it can be versioned and tested on its own.
- A C ABI cannot express generics, lifetimes or rich enums, so every facade type that crosses it
  needs a flat C counterpart. That work is Phase 8 and is accepted.
- The ramp-up cost for a second contributor is real. It is acceptable while the builder is one
  person experienced in Rust, and is the first thing to re-examine if that changes.
- Until Phase 8 `wanjiru-ffi` is contract only: its `CONTRACT.md` lists the intended surface and
  `cargo xtask check` keeps the contract and the manifest in agreement.

## Alternatives considered

- **A C or C++ core with Rust at the edges.** Rejected: the code that parses untrusted input would be
  memory-unsafe by default, so the unsafe budget would be the whole code base.
- **A Rust ABI between components (dynamic Rust plug-ins).** Rejected: it breaks with every compiler
  version, and bindings cannot use it.
- **A managed language (Go, C#, Java).** Rejected: SIMD kernels and bit-exact numerical control are
  harder, and every consumer would carry a runtime.
- **Zig.** Not chosen: the ecosystem for image, PDF and ONNX bindings is smaller, and the builder's
  experience is in Rust.

## Revisit when

A target platform cannot run Rust, or a certification regime demands C.
