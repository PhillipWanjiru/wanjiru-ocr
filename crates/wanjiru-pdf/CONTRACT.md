# wanjiru-pdf: contract

| | |
|---|---|
| Layer | L2 I/O and runtime (may depend on L0 to L1 and `-sys` crates; parsers and FFI wrappers, sandbox first) |
| Status | Contract only. Rasterisation in Phase 2; text-layer extraction and PDF/A support in Phase 8. |
| First working code | Phase 2 |
| Unsafe zone | yes |

This file is the contract of the crate. A change to the public API, an invariant, an oracle, the budget or the dependency list changes this file in the same commit, and an ADR when the change is a decision.

## Purpose

Decide per page whether a PDF page is born-digital, scanned or mixed; rasterise pages at a chosen resolution; and probe the embedded text layer so that the engine can skip OCR where the text is trustworthy and compare against it where it is not.

## Public API

Planned. Nothing is stable until the exit check of the phase that implements the crate.

- `classify_page(&Page) -> PageClass`.
- `rasterise(page, dpi, &Limits) -> Result<(Image, Affine<PdfPoints, Source>)>`.
- `text_layer(page) -> Option<TextLayer>` with glyph boxes in PDF points.

## Invariants

1. The returned affine maps PDF points (y up) to source pixels (y down) exactly under the pixel-centre convention.
2. Rasterisation obeys the same `Limits` as image decoding; a page with an absurd media box fails with a stable code.
3. Parsing happens only inside the worker process.

## Oracles

What the crate is checked against, independently of its own tests:

- Classification precision and recall on a labelled set of born-digital, scanned and mixed pages (target recorded at Phase 2 exit).
- Rasterisation hashes on the conformance PDFs; `tests/corpora/evil` for malformed and hostile PDFs.
- Text-layer boxes round trip through the affine to within 0.05 px.

## Budget

- Entries in `docs/budgets.md`: rasterisation time per page at 200 and 300 dpi; peak memory.

## Unsafe policy

Unsafe zone, limited to the `-sys` wrapper of the PDF library. Same rules as `wanjiru-codecs`. Every `unsafe` block, `unsafe impl` and `unsafe` attribute has a `// SAFETY:` comment directly above it that says why it is sound; every `unsafe fn` and `unsafe trait` has a `# Safety` section in its documentation that says what the caller must guarantee. `cargo xtask check-unsafe` enforces both, and `unsafe` outside the six unsafe zones (`wanjiru-simd`, `wanjiru-runtime`, `wanjiru-codecs`, `wanjiru-pdf`, `wanjiru-ffi`, `wanjiru-worker`) is refused (ADR-007).

## Allowed dependencies

Checked by `cargo xtask check-contracts` against `Cargo.toml` and against the layer rule.

- Workspace: `wanjiru-types`, `wanjiru-imgproc`, `wanjiru-docir`
- External: none
- Planned external (needs an ADR and a row in `data/registry/licences.yaml` first): a PDF rendering library (PDFium or MuPDF, decided by ADR against the licence allow-list)
