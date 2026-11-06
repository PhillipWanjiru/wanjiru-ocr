# ADR-008: Telemetry off by default; consent tiers for corrections

| | |
|---|---|
| Status | Accepted |
| Date | 2026-11-02 |
| Phase | 1 |
| Supersedes | none |
| Enforced by | contracts (`wanjiru-cli`, `wanjiru-server`, `wanjiru-worker`); the dependency policy of ADR-007 (a network client would have to be named in a contract and have a licence row); the review of every dataset row |

## Context

- Version 1 reads invoices, receipts and identity documents. These are financial records and personal
  data. A customer who finds the engine phoning home will not use it, and in some places may not be allowed
  to.
- Corrections made by people are the most valuable training signal the project can own, but only if
  consent and privacy are designed in from the start (blueprint, Chapter 22).
- The golden set and every fixture must be collectable and publishable without harming anybody.

## Decision

1. Recognition makes no network call. The library, the command line and the worker have no network code;
   the worker has no network system calls at all. No telemetry is collected without consent, and the
   default is that none is.
2. Corrections follow consent tiers:

   | Tier | What happens | Consent | Risk |
   |---|---|---|---|
   | 0, local only (default) | Corrections update the user's lexicons, constraint presets and a local recalibration | none needed; the data never leaves the device | low |
   | 1, shared corrections | De-identified line crops with corrected text, metadata stripped, previewed by the user | explicit per-project opt-in, revocable | medium: personal data in images |
   | 2, federated | On-device fine-tuning with secure aggregation and differentially private updates | explicit and documented | high engineering complexity; research track |

   Enterprise contracts default to tier 0, with a contractual guarantee that no data leaves.
3. A dataset manifest links every training example to its consent record, so an erasure request can be
   honoured before the next training run (`data/registry/datasets.yaml`).
4. A real identity document never enters any repository. Fixtures are fictional: the Machine Readable Zone
   fixtures name the issuing state `UTO`, the fictional state that ICAO uses in its specimens, and the IBANs
   use country prefixes from the private-use range of ISO 3166-1, which no real country has, so none of them
   can be a real account.
5. Reports and logs written by the project's own tools contain no host names, user names or paths.

## Consequences

- Trust, and a plain answer to a security questionnaire.
- A smaller data flywheel. Improvement data will be scarce at first: the project depends on licensed and
  synthetic data, on the collected golden set (with a consent form: `design/research/consent-form.md`) and
  on volunteers.
- Every feature that wants data has to say which tier it uses, and a feature that needs more than
  tier 0 needs its own decision record.
- A crash report that the user previews and sends by hand is compatible with this decision. One that is
  sent automatically is not.

## Alternatives considered

- **Opt-out telemetry.** Rejected: the documents are identity papers and financial records.
- **No correction feature.** Rejected: it is the best source of improvement and the reason the desktop
  application exists.
- **Cloud-first processing.** Rejected with ADR-002.

## Revisit when

Customers demand opt-out collection.
