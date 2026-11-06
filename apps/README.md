# apps/

Empty on purpose. The desktop application arrives in Phase 9 (ADR-002; the installers it needs are listed
for the same phase in `crates/wanjiru-worker/CONTRACT.md`).

**The rule.** The application is a separate program that uses only the public API: the `wanjiru` facade or
the C ABI (`include/wanjiru.h`). It may not depend on an engine crate. That keeps it a client like any other,
and it is what makes the library the product (ADR-002).

**What does not exist yet.** A check for that rule. `cargo xtask check` looks only at the crates under
`crates/`. How a program outside that folder is held to the rule is decided in the change that adds the first
one.

**Where the design starts.** Nothing here is designed. The research in `design/` is a set of hypotheses, not
findings, and `design/ux/success-metrics.md` says how the application will be judged once there is someone
to judge it.
