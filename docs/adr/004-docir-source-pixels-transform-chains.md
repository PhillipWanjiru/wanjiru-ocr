# ADR-004: DocIR is canonical; geometry is stored in source pixels with invertible transform chains

| | |
|---|---|
| Status | Accepted |
| Date | 2026-11-02 |
| Phase | 1 |
| Supersedes | none |
| Enforced by | `crates/wanjiru-docir` (schema `wanjiru.docir/0`, strict reader, canonical form), `crates/wanjiru-types` (frames as types); `wanjiru docir check` |

## Context

- Every result must say where it came from. A reviewer needs the box on the customer's original, a
  redaction must cut the original's pixels, and an audit must be able to follow a word back to the
  image. The models never see the original: they see a deskewed, cropped, rescaled copy.
- The output formats (hOCR, ALTO, PAGE, a PDF text layer) each lose something. None of them can be the
  core.
- A result is evidence, so two programs must be able to agree on its bytes and its hash.

## Decision

1. DocIR (`wanjiru.docir/0`) is the single result type: pages, lines, words and typed fields, each with
   a confidence and a provenance. Every emitter produces a view of DocIR, and nothing is emitted that
   DocIR cannot represent.
2. All geometry is stored in source pixels, and every step that produced the image a model saw (deskew,
   crop, rescale, perspective correction) is recorded as a `Chain` of steps between named frames. A
   chain can be applied, composed and inverted, and its inverse is again a chain.
3. Pixel `(i, j)` covers `[i, i+1) x [j, j+1)` and its centre is `(i + 0.5, j + 0.5)`. Maps are applied
   to centres, never to indices. Frames are zero-sized types, so mixing them is a compile error.
4. The canonical form is JSON as RFC 8785 defines it: keys in UTF-16 order, no insignificant
   whitespace, numbers written as ECMAScript writes them. The SHA-256 of that text identifies a result.
   `content_sha256` ignores `run.timing_ms`, so a re-run gives the same hash.
5. The reader is strict: duplicate keys, NaN, infinities, lone surrogates and nesting deeper than 128 are
   refused. A later schema version is refused with `WNJ-E1006`, a foreign one with `WNJ-E1003`, and a
   violation is reported with the file, the line and the path of the value.

## Consequences

- Provenance and redaction are possible: an output element can be traced to source pixels, and the
  cut is made in the source frame.
- Compact: a handful of numbers per stage instead of a dense field.
- Measured in Phase 1 (unit tests with committed seeds): affine maps in `wanjiru-types` return a
  point to within 1e-6 px in `f64` for points up to 8,000 px and a condition number up to 50, and to
  within about 6e-8 times the coordinate times the condition number (below 0.05 px in that range) when
  the coordinates are stored as `f32`. DocIR chains of projective steps stay within 1e-6 px for points up
  to 10^4 px and a condition number below 10^3.
- The blueprint's resampling chapter writes the pixel-centre rule in index space; the code works in
  continuous coordinates. The two differ by half a pixel, and mixing them is the classic bug, so the
  rule above is the contract and the `wanjiru-types` invariants repeat it.
- Chains describe affine and projective steps only. Lens distortion and page curl need a different
  step, a sampled displacement field with a stated error bound. DocIR v0 does not have one.
- Canonical JSON is bulkier than a binary form. A binary form can come later as a lossless
  re-encoding of the canonical one, with the hash taken over the canonical text.

## Alternatives considered

- **Dense inverse maps per stage.** Rejected: heavy in memory, and an inverse built by interpolation
  accumulates error stage by stage.
- **Geometry only in the final, processed frame.** Rejected: no overlay on the original, no redaction,
  no provenance.
- **A format such as hOCR or ALTO as the core.** Rejected: each is a lossy view.

## Revisit when

Dense inverses prove inaccurate in practice. The yardstick is the bound above: a round trip over
0.05 px with `f32` storage inside the stated range.
