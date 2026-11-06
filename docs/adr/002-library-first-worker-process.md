# ADR-002: Library first; the desktop application is a public-API client, and risky decoding runs in a worker process

| | |
|---|---|
| Status | Accepted |
| Date | 2026-11-02 |
| Phase | 1 |
| Supersedes | none |
| Enforced by | the layer rule and the surface exceptions in `tools/xtask/src/layers.rs` (ADR-011); the worker from Phase 2 |

## Context

- The measuring instrument (`wanjiru eval`) and the continuous-integration jobs must run with no
  display and no user, so the engine cannot live inside an application.
- Decoders for images and PDFs are the code most exposed to hostile input, and they wrap C libraries
  (the `wanjiru-codecs` and `wanjiru-pdf` unsafe zones). A bug there must not take the user's
  application, or their open documents, with it.
- A desktop application will exist, because correcting results is the most valuable source of
  improvement data (ADR-008), but it is not what Phase 1 to 8 are about.

## Decision

1. The product is the library. The facade crate `wanjiru` is its public API. `wanjiru-cli`,
   `wanjiru-server`, `wanjiru-py` and `wanjiru-ffi` are thin surfaces over it: argument handling,
   input and output, and mapping of errors, with no recognition logic of their own.
2. The desktop application is a separate program (under `apps/`, Phase 9) that uses only the public
   API, the facade or the C ABI. It may not depend on an engine crate.
3. Untrusted bytes are decoded in `wanjiru-worker`, a separate process with no network and a
   system-call allow-list, speaking a length-prefixed protocol. A crash in a decoder kills the
   worker, not the application. The decode job arrives in Phase 2, engine hosting in Phase 3,
   installers in Phase 9.
4. State is explicit: configuration and session handles are values that are passed, and the lowest
   crates promise no global state and no input or output.

## Consequences

- Headless testing: every quality gate is a command line.
- Crash and compromise isolation sit in one place, where they can be tested with hostile files.
- The desktop client is an ordinary client of the API, so its mistakes cannot reach into the engine
  and the engine's cannot reach into its interface.
- Inter-process transfer of image data has a cost. It is measured in Phase 2 and entered in
  `docs/budgets.md`; until then there is no number.
- The surfaces depend on the facade, which sits on the same layer. That is a deliberate exception
  to the layer rule, proposed in ADR-011.
- Two ways to run the same decode (in the process, in the worker) must agree. A parity test, from
  Phase 2, compares the DocIR they produce.

## Alternatives considered

- **Desktop first, engine inside the application.** Rejected: no headless tests, a decoder crash
  takes the application down, and state creeps into the interface.
- **A service first.** Rejected: the documents are invoices and identity papers; local processing
  is the default (ADR-008).
- **A library without a worker.** Rejected for the decoders of untrusted formats. An embedder who
  cannot run a second process is the case in "Revisit when".

## Revisit when

Embedding constraints forbid worker processes. The likely response then is an in-process mode that
speaks the same protocol over a channel, with the loss of isolation stated.
