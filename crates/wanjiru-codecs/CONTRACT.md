# wanjiru-codecs: contract

| | |
|---|---|
| Layer | L2 I/O and runtime (may depend on L0 to L1 and `-sys` crates; parsers and FFI wrappers, sandbox first) |
| Status | Contract only. Implemented in Phase 2. |
| First working code | Phase 2 |
| Unsafe zone | yes |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Turn untrusted bytes into pixel buffers with metadata: JPEG, PNG, TIFF, WebP and others through vetted decoders, EXIF orientation, ICC profile handling, and hard resource limits (pixels, bytes, frames, decompression ratio). This is the first parser an attacker reaches, so it runs inside the worker process (`wanjiru-worker`) and is fuzzed from day one.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `decode(bytes, &Limits) -> Result<Decoded>` with `Decoded { image, exif, icc, frames }`.
- `Limits { max_pixels, max_bytes, max_frames, max_ratio, deadline }` with conservative defaults.
- `sniff(bytes) -> Format`, never trusting file extensions.

## Invariants

1. No decoder allocates before the declared dimensions have been checked against `Limits`.
2. A decode either returns a complete image or an error with a stable `WNJ-E2xxx` code; there are no partial results.
3. EXIF orientation is applied once and recorded in the returned transform chain.
4. Output is a pure function of the input bytes and the limits.

## Oracles

What the crate is checked against, independently of its own tests:

- `tests/corpora/evil`: decompression bombs, truncated and bit-flipped files, oversized dimensions, malformed EXIF; every one must fail cleanly within its limits.
- Pixel hashes against reference decoders (libjpeg-turbo, libpng) on a conformance corpus.
- cargo-fuzz targets per format, run for at least 10 minutes per push and for hours nightly.

## Budget

- Entries in `docs/budgets.md`: decode time and peak memory per format at 12 megapixels; the memory limit is enforced by the worker, not assumed.

## Unsafe policy

Unsafe zone, limited to the `-sys` wrappers and their safe façades. `#![deny(unsafe_code)]` at the root, `#[allow(unsafe_code)]` per module, SAFETY comments mandatory. Every `unsafe` block, `unsafe impl` and `unsafe` attribute has a `// SAFETY:` comment directly above it that says why it is sound; every `unsafe fn` and `unsafe trait` has a `# Safety` section in its documentation that says what the caller must guarantee. `cargo xtask check-unsafe` enforces both, and `unsafe` outside the six unsafe zones (`wanjiru-simd`, `wanjiru-runtime`, `wanjiru-codecs`, `wanjiru-pdf`, `wanjiru-ffi`, `wanjiru-worker`) is refused (ADR-007).

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-imgproc`
- External: none
- Planned external (needs an ADR and a row in `data/registry/licences.yaml` first): decoder crates or `-sys` bindings chosen by ADR in Phase 2
