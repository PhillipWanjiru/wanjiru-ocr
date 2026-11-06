# ADR-011: Same-layer dependencies: the kernel crate and the surfaces

| | |
|---|---|
| Status | Proposed |
| Date | 2026-11-02 |
| Phase | 1 (found while writing the checker); to be decided before Phase 2 starts |
| Supersedes | none |
| Enforced by | `SAME_LAYER_EXCEPTIONS` in `tools/xtask/src/layers.rs`; `cargo xtask check-layers` |

Not part of the blueprint's initial set of ten. It is here because the blueprint's rule ("dependencies
point down only") and its own crate list cannot both hold, and a checker that enforces the rule has to say
what it does about that.

## Context

The layer rule is: a crate depends only on crates of a strictly lower layer. Writing a checker that
enforces it on the real crate graph found two groups of dependencies the table forbids and the design
needs.

1. `wanjiru-imgproc` (L1) calls `wanjiru-simd` (L1). Resampling and binarisation are the natural callers of
   the SIMD kernels, and the kernel crate is an unsafe zone, so it must stay a separate crate.
2. `wanjiru-ffi`, `wanjiru-cli`, `wanjiru-worker`, `wanjiru-server` and `wanjiru-py` (all L5) are thin
   layers over the facade `wanjiru`, which the blueprint's crate list also puts on L5.

The checker currently allows exactly these six pairs, each with its reason printed in the notes of
`cargo xtask check-layers`, and forbids every other same-layer dependency and every cycle.

## Decision (proposed)

1. Keep the six exceptions as an explicit list in one place (`tools/xtask/src/layers.rs`). Each has a
   reason, a new one needs a change to the list and to this ADR, and an exception is allowed, not
   required.
2. For the surfaces, say it as a rule in the layer table: a surface may depend on the facade and on lower
   layers, and on no other surface.
3. For the kernels, decide at the start of Phase 2, before there is code to move, between:
   - **(a)** accept `wanjiru-imgproc -> wanjiru-simd` as it stands; and
   - **(b)** move the kernel trait into L0 (`wanjiru-types` or `wanjiru-math`), make `wanjiru-imgproc`
     generic over it, and let the engine choose the implementation. The dependency then points downwards, and
     `wanjiru-imgproc` can be tested with the scalar implementation alone.
   The deciding measurement is the cost of the generic dispatch in the resampling and binarisation budgets
   (`docs/budgets.md`, from Phase 2). If it is not measurable, (b) is the cleaner design.

## Consequences

- The checker and the documents agree, and the exceptions are visible, not silent.
- The layer descriptions in the blueprint and in the `CONTRACT.md` headers need one sentence about the
  exceptions. The `CONTRACT.md` of `wanjiru-imgproc` carries the open question until this ADR is accepted.
- Choosing (b) changes the contracts of three crates, once, before any of them has code.

## Alternatives considered

- **Re-layer: move `wanjiru-simd` to L0 and give the facade its own layer below the surfaces.** No
  exception is needed afterwards. Rejected for now because it renumbers layers in 25 contracts and in the
  blueprint for the sake of two relationships, and because L0 was defined as the crates that need nothing
  but the standard library.
- **Allow same-layer dependencies in general, forbidding only cycles.** Rejected: it removes the rule's
  power to show structure.
- **Merge the surfaces into one crate.** Rejected: they have different unsafe policies, targets and
  dependencies.

## Revisit when

A third kind of same-layer dependency appears, or the list of exceptions grows beyond these six.
