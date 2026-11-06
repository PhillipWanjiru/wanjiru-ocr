# models/

Package manifests, one per model package, and nothing else. The model registry arrives in Phase 4 and package
signing in Phase 8 (`crates/wanjiru-models/CONTRACT.md`). The first rows of `data/registry/models.yaml` arrive
in Phase 3 with the baseline recogniser. Phase 1 uses no model, so this folder is empty.

**Weights are not committed.** `.gitignore` excludes `*.onnx`, `*.safetensors` and `*.bin` under this folder,
because weights are large and carry the licence terms of their training data and of their teachers. They are
distributed as signed packages.

**What a package is** (from the `wanjiru-models` contract): a directory with the weights, a manifest named
`<task>-<script>-v<major>` (for example `rec-latin-v1`), the character map, the calibrator and the model card.
Its licence, what the licence permits, its data lineage and its calibrator are recorded in
`data/registry/models.yaml`.

**What blocks a release.** A model with no row in the registry is not used. A row with `verified: false` cannot
be part of a release (ADR-009).
