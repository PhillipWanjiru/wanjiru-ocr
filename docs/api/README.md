# API description

The description of the local HTTP service of `wanjiru-server` (OpenAPI 3.1), and of nothing else.

**Status: not written.** The server's first working code is in Phase 8, and an API description written before
the thing it describes would only be a guess that later has to be unlearned. This folder exists because the
contract of `wanjiru-server` (`crates/wanjiru-server/CONTRACT.md`) names it as the place where the description
will live, and `cargo xtask check-contracts` requires that what a contract names exists.

What is already decided, and binds the description when it is written:

- The service is local. It listens on the loopback interface by default and makes no outbound connection
  (ADR-002, ADR-008).
- A result is a DocIR document (`crates/wanjiru-docir/schema/docir-v0.schema.json`); the service does not invent
  a second format.
- Errors carry the stable codes of `docs/errors.md`.
- The description is tested against the running server (contract tests), not only written beside it.

The C ABI and the Python module have their own description in their own crates when Phase 8 reaches them.
