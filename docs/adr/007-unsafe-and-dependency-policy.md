# ADR-007: Unsafe policy and dependency policy, enforced in CI

| | |
|---|---|
| Status | Accepted |
| Date | 2026-11-02 |
| Phase | 1 |
| Supersedes | none |
| Enforced by | `cargo xtask check` (layers, contracts, unsafe, determinism, licences); `[workspace.lints]` in `Cargo.toml`; `clippy.toml`; `deny.toml`; `.github/workflows/` |

## Context

- Safe Rust is the reason to write the engine in Rust (ADR-001), and the places where it is not safe
  are known in advance: SIMD intrinsics, the C libraries behind the image and PDF decoders, the ONNX
  Runtime binding, the C ABI, and the process sandbox.
- A rule that lives in a document erodes. A rule that fails the build does not.
- Every dependency is code the owner did not write and will have to trust, update and defend. Its
  licence is a legal fact about every release.
- The blueprint's finding F-04: the real decision is the boundary strategy, and the policy is what
  makes the boundary visible.

## Decision

**Unsafe code.**

1. `unsafe` is allowed in six crates only: `wanjiru-simd`, `wanjiru-codecs`, `wanjiru-pdf`,
   `wanjiru-runtime`, `wanjiru-ffi` and `wanjiru-worker`. The list is one table in
   `tools/xtask/src/layers.rs`.
2. The other nineteen crates say `#![forbid(unsafe_code)]` at their root. A zone says
   `#![deny(unsafe_code)]` and allows it per module, each with a note.
3. Every `unsafe` block, `unsafe impl` and `unsafe` attribute has a `// SAFETY:` comment directly above
   it, saying why it is sound. The `CONTRACT.md` of a zone repeats this rule, and
   `cargo xtask check-unsafe` checks both the code and the contract text.

**Dependencies.**

4. The layer rule: a crate depends only on crates of a strictly lower layer, development dependencies
   included. The same-layer exceptions are listed in ADR-011. `cargo xtask check-layers` enforces it and
   checks that the manifests agree with `Cargo.toml`'s workspace table.
5. A crate's `CONTRACT.md` lists its allowed dependencies, workspace and external, and
   `cargo xtask check-contracts` compares the list with the manifest. An external crate enters the
   workspace only after the contract of the crate that uses it lists it, a row exists in
   `data/registry/licences.yaml`, and `cargo xtask licences` passes.
6. `wanjiru-types` and `wanjiru-math` depend on the standard library only. In Phase 1 every one of the
   25 crates has no external dependency.
7. Licences: `deny.toml` allows MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0 and,
   after review, MPL-2.0. GPL, AGPL and LGPL are excluded by omission. Sources are crates.io only; git
   dependencies and wildcard versions are denied; a yanked release is an error. `cargo xtask licences`
   reads the same file, so the check also runs where cargo-deny is not installed.

**Lints and profiles.**

8. `clippy::all` and `clippy::pedantic` warn, and CI runs with `-D warnings`. `unwrap_used`, `dbg_macro`,
   `todo` and `unimplemented` are denied. Libraries do not print (`print_stdout`, `print_stderr`).
9. Five pedantic lints are switched off, each for a stated reason: `module_name_repetitions`
   (`wanjiru_eval::eval_report` would be worse), `must_use_candidate` (noise on small pure functions),
   `missing_errors_doc` (errors are specified once, in `CONTRACT.md`), `missing_panics_doc` (panics are not
   permitted in library paths) and `cast_precision_loss` (counts converted for statistics stay below 2^53).
10. `overflow-checks = true` in the release profile: the input is untrusted, and an overflow is a bug, not a
    wrap. Phase 10 may revisit this with measurements.

## Consequences

- The risk is contained and auditable: the word `unsafe` appears in six crates, and each use is
  explained where it stands.
- The checker found real gaps in the repository's own contracts the first time it ran: five zone contracts
  lacked the SAFETY text. Each rule is also tested against deliberately broken in-memory repositories and
  against a broken copy of this one.
- Having no external crates in Phase 1 means owning a JSON reader and writer, a YAML subset, a TOML
  subset, SHA-256 and a random generator. Each has an independent oracle (the contracts list them).
  That is more code to maintain, and it is bounded: later phases add crates through the process in
  point 5.
- The checks read source text with a small lexer, not a parser. A macro that generates `unsafe` in
  another crate is outside what they see, so `#![forbid(unsafe_code)]` stays the second line of defence.
- Overflow checks cost some speed in kernels. The cost is measured (`docs/budgets.md`) before any
  decision to remove them.

## Alternatives considered

- **Review by convention.** Rejected: it works until the first deadline.
- **`cargo-geiger` for counts.** Useful as a report, but it does not enforce zones. It may be added
  beside the policy.
- **`unsafe` permitted everywhere with review.** Rejected: the audit surface would be the whole code base.
- **No `unsafe` at all.** Rejected: SIMD, the C libraries, the sandbox and the C ABI cannot be written
  without it.

## Revisit when

The policy blocks essential performance work repeatedly.
