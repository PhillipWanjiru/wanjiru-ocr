# training/

Empty on purpose. Training code for the project's own models arrives with the first of them: the
recogniser's character map and own models in Phase 4 (`crates/wanjiru-recognize/CONTRACT.md`), the detector's
own model in Phase 7 (`crates/wanjiru-detect/CONTRACT.md`).

**The stack is not chosen.** ADR-003 (proposed) starts from the assumption that models are trained in PyTorch
and exported to ONNX. That is its starting point, not a decision.

**Rules that already apply, because they are written elsewhere.**

- No real document enters this repository, for training either (ADR-008, `data/datasheets/golden-v0.md`,
  `.gitignore`).
- Every dataset used to train has a row in `data/registry/datasets.yaml`. Every model the project ships, tests
  against or trains from has a row in `data/registry/models.yaml`, teachers included, because a model's weights
  inherit the terms of its training data and of its teachers.
- Weights are not committed. A package's manifest lives in `models/`, its weights outside git.
- A model ships with a fitted calibrator (ADR-006) and is judged by `wanjiru eval` against a quality
  contract in `eval/contracts/`, not by its training loss.
