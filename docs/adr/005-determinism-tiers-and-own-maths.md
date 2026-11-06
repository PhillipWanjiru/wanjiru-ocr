# ADR-005: Three determinism tiers, and an own maths library for the part that must be exact

| | |
|---|---|
| Status | Accepted |
| Date | 2026-11-02 |
| Phase | 1 |
| Supersedes | none |
| Enforced by | `cargo xtask check-determinism`; `clippy.toml` (`disallowed-types`, `disallowed-methods`); golden hashes and oracle comparisons in `wanjiru-math`, `wanjiru-docir` and `wanjiru-eval` |

## Context

- An engine that cannot say whether two runs will agree cannot be audited, and a regression cannot be
  told from noise. Retrofitting determinism is painful: the decisions about reduction order and
  mathematical functions have to be made before the code exists.
- The blueprint defines three tiers. **Tier 1:** bit-exact on the same machine and thread count.
  **Tier 2:** identical text across machines of the same architecture. **Tier 3:** near-identical
  across architectures.
- `f64::exp`, `ln`, `powf`, `sin` and their relatives call the platform's maths library. Its last bit
  differs between systems and versions, and the language does not promise otherwise. The operations
  IEEE 754 defines exactly are `+ - * /` and `sqrt` (correctly rounded), plus `floor`, `ceil`, `abs`
  and comparisons.
- Some parts of this project issue verdicts and hashes that people will compare across machines:
  the quality gates, the report hash, the DocIR hash. Those can be held to more than Tier 1.

## Decision

1. Every crate states in its `CONTRACT.md` what its outputs promise. Neural inference promises Tier 2
   or Tier 3, and Tier 1 only on the CPU path (ADR-003).
2. The deterministic subset, `wanjiru-math`, `wanjiru-docir` and `wanjiru-eval`, promises the same
   bytes on every platform, thread count and optimisation level. In non-test code it uses only the exact
   operations above and its own implementations of the rest: edit distance, exact binomial and
   Clopper-Pearson intervals, normal distribution functions, SHA-256, and `exp` built from IEEE
   operations. `ln`, `tanh` and fixed-point arithmetic arrive in Phase 10.
3. Summation order is fixed by the function (fixed-tree and compensated sums), never by a scheduler.
   Random streams are specified by the algorithm (xoshiro256** seeded by SplitMix64, bounded integers by
   Lemire's method), not by a library version. Containers whose order matters are `BTreeMap` and
   `BTreeSet`; `HashMap` and `HashSet` are refused by clippy, and `SystemTime::now` too.
4. Reports contain no timestamps, host names or paths, and end with their own SHA-256.
5. `wanjiru-types` is not in the subset: its rotation helpers use `sin_cos`. Its bounds are stated in
   tolerances, not in bytes (ADR-004).
6. `cargo xtask check-determinism` bans the platform functions by name in the subset's non-test code. A
   deliberate use carries `// determinism: allow <reason>` on its line.

## Consequences

- Gate verdicts and report hashes are identical on every machine. The golden transcripts of
  `wanjiru eval` in `eval/fixtures/synthetic-v0/expected/` are compared byte for byte in CI.
- Every special function is an obligation: it has an independent oracle (`hashlib`, SciPy, a 50-digit
  `decimal` reference; the numbers are in the contracts) and a stated error bound.
- The check is lexical. It sees names, not types, and it exempts `#[cfg(test)]` code on purpose. It is a
  guard against accidents, not a proof, which is why the oracles remain necessary.
- The subset is slower than the platform functions would be. The costs are entered as budgets
  `B-math-01` to `B-math-04`.
- Found in Phase 1, and the reason for independent oracles: Rust's shortest-digits formatting and
  ECMAScript's disagree on exact ties (`1424953923781206.25` is `...206.2` in ECMAScript and `...206.3`
  in Rust). RFC 8785 needs the ECMAScript form. The Rust unit tests passed; a comparison with Node.js
  on the RFC's own vector list found it, and `docir::canonical` now re-rounds those ties. See
  `docs/lab/2026-10-29.md`.

## Alternatives considered

- **Use the platform's maths library and compare with tolerances.** Rejected for the subset: a
  tolerance cannot make a hash stable, and a gate verdict that flips with the platform is worse than
  a slow one. It is the right choice for the neural part, which is Tier 2 or 3.
- **A crate for correctly rounded functions (for example `libm` ports).** Not adopted in Phase 1: no
  external crates are allowed yet (ADR-007), and the subset needs few functions. It can be reconsidered
  when `ln` and `tanh` arrive.
- **Tier 1 for the whole engine.** Rejected: it forbids accelerators and constrains kernels
  everywhere, for a property only an audit mode needs.

## Revisit when

Tier 1 cost exceeds its value for customers.
