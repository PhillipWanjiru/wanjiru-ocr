# Error codes

Every failure the Wanjiru System reports carries a stable code: `WNJ-E`, the digit of the layer that
reports it, and three digits. A code is never renumbered and never reused; a retired code stays
retired. Messages may change between versions, codes may not.

This file is the output of `wanjiru errors`, and a test fails when the two differ. To change it, change
the registry in `crates/wanjiru-types/src/error.rs` and write the file again:
`cargo run -p wanjiru-cli -- errors > docs/errors.md`.

| Code | Layer | Title | When it is reported |
|---|---|---|---|
| `WNJ-E0001` | L0 foundation | invalid geometry | A polygon with fewer than three vertices, or a non-finite coordinate. |
| `WNJ-E0002` | L0 foundation | transform not invertible | A transform that cannot be inverted. |
| `WNJ-E1001` | L1 core | malformed JSON | JSON text is not well formed. |
| `WNJ-E1002` | L1 core | duplicate JSON key | A JSON object repeats a key. |
| `WNJ-E1003` | L1 core | DocIR schema violation | A JSON value does not satisfy the DocIR schema. |
| `WNJ-E1004` | L1 core | invalid transform chain | A transform chain is not connected, or a step is malformed. |
| `WNJ-E1005` | L1 core | invalid confidence | A confidence outside `[0, 1]`, or not a number. |
| `WNJ-E1006` | L1 core | unsupported DocIR schema version | A DocIR document declares a schema version this build does not read. |
| `WNJ-E1007` | L1 core | JSON number not representable | A JSON number that cannot be represented (NaN, infinity) was passed to the canonical writer. |
| `WNJ-E4001` | L4 orchestration | contract parse error | A quality contract could not be parsed. |
| `WNJ-E4002` | L4 orchestration | contract invalid | A quality contract has a key or value the harness does not know. |
| `WNJ-E4003` | L4 orchestration | duplicate document id | Two documents in one corpus share an identifier. |
| `WNJ-E4004` | L4 orchestration | no documents of the contract's class | The corpus contains no document of the contract's class. |
| `WNJ-E4005` | L4 orchestration | invalid statistical argument | Invalid arguments to a statistical routine (empty sample, probability outside `[0, 1]`). |
| `WNJ-E4006` | L4 orchestration | invalid corpus record | A record of a ground-truth or baseline file is malformed, or a truth value breaks the syntax of its kind. |
| `WNJ-E4007` | L4 orchestration | prediction for an unknown document | A prediction names a document that is not in the ground truth. |
| `WNJ-E5001` | L5 surfaces | usage error | Command-line usage error. |
| `WNJ-E5002` | L5 surfaces | cannot read input | A file or directory could not be read. |
| `WNJ-E5003` | L5 surfaces | cannot write output | A file could not be written. |
