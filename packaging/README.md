# packaging/

Empty on purpose. Installers and release packaging arrive in Phase 9, with the desktop application
(`crates/wanjiru-worker/CONTRACT.md`, ADR-002). Signing of model packages is Phase 8
(`crates/wanjiru-models/CONTRACT.md`).

Two rules already in force will constrain it. Telemetry is off by default and recognition makes no network call
(ADR-008), so an installer may add neither. Every model, dataset and font keeps its own licence (ADR-009,
proposed), recorded in `data/registry/`; the licence of the project's own code is still an open decision for the
owner.
