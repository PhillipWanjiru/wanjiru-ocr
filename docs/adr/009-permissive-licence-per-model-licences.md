# ADR-009: A permissive licence for the code; each model, dataset and font keeps its own licence

| | |
|---|---|
| Status | Proposed |
| Date | 2026-11-02 |
| Phase | 1 |
| Supersedes | none |
| Enforced by | `license = "Apache-2.0"` in `[workspace.package]`, `LICENSE`, `deny.toml`, `cargo xtask licences`; the registries in `data/registry/` |

Proposed, not accepted, because the owner has not yet said what business model the project follows
(an open question in the plan), and the licence follows from it. Apache-2.0 is the working default.
Nothing in the repository depends on the answer except the text of `LICENSE`, one line in
`[workspace.package]` and `NOTICE`.

## Context

- A library earns its place by being embedded. A licence that frightens an embedder's lawyers keeps the
  library out of the products that would use it.
- The code, the model weights, the training data and the fonts are four different things with four
  different legal regimes. Weights inherit the terms of the data and of any teacher model they were
  trained from, and a font's licence may forbid embedding in a document.
- A release is a legal fact about everything in it, so the licence of each part has to be known before it
  ships, and recorded.

## Decision

1. The code is released under Apache-2.0: permissive, with an express patent grant. The licence does not
   grant rights to the name or to the dedication text (Apache-2.0, section 6).
2. Every model, dataset and font has its own row in `data/registry/` (`models.yaml`, `datasets.yaml`,
   `fonts.yaml`) with its source, version, hash, licence and what the licence permits (commercial use,
   redistribution, derivation). The register of licences themselves is `data/registry/licences.yaml`.
3. A row that has not been verified against the source's own licence text is marked unverified, and an
   unverified row cannot be part of a release. In Phase 1 every row is unverified.
4. A model whose training data or teacher forbids commercial use is not part of the default package set.
   It may exist for research, labelled as such.
5. Dependencies follow the allow-list in `deny.toml` (ADR-007).

## Consequences

- Adoption and embedding are easy: no copyleft obligations pass to users of the library, and the patent
  grant is explicit.
- Anyone, including a competitor, may embed the code. What stays the project's own is the name, the
  models, the data and the quality evidence, not the source.
- A model's licence can differ from the code's. The package manifest says which, and the registries are
  where to look.
- The registries are a maintenance task, and a release checklist item (Phase 10).

## Alternatives considered

- **MIT.** Simpler, but has no express patent grant.
- **MPL-2.0.** File-level copyleft keeps improvements to a file open and is a fair compromise. It adds
  friction for embedders and is allowed as a dependency, not chosen for the project.
- **AGPL, or GPL with a commercial licence.** Protects against embedding in proprietary products and
  supports dual licensing. It also keeps the library out of exactly those products. The right choice only if the
  business model is dual licensing.
- **Source-available or proprietary.** Possible, and then this ADR is wrong. It is the question to answer
  first.

## Revisit when

A strategic partner requires a different licence, or when the owner decides the business model (the
open question in the plan).
