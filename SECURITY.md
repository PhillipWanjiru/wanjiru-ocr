# Security policy

## Status

Pre-release. There is no released version and no version is supported. This file says how to report a problem
and what the project counts as one, so that the rules exist before the first release does.

## Reporting a vulnerability

Do not open a public issue for a security problem.

- Use GitHub's private vulnerability reporting for this repository (the *Security* tab, then *Report a
  vulnerability*), once the repository is public and the feature is switched on, **or**
- write to the security contact: **[to be set by the owner before the repository is made public]**.

Please include what you found, how to reproduce it, the version or commit, and what you think it allows.
**Never attach a real identity document, bank detail or invoice.** A synthetic or fictional input that triggers
the problem is all that is needed, and the project's fixtures (`eval/fixtures/synthetic-v0`) show how to make one.

## What counts

The engine reads files from people who may not be trusted: images, PDFs, JSON, YAML, model packages. These are
in scope:

- memory unsafety, in an unsafe zone or anywhere else (`unsafe` is allowed in six crates only; ADR-007);
- a crash, a hang or unbounded memory use on a crafted input;
- an escape from the worker's sandbox, or any network activity during recognition (ADR-002, ADR-008);
- a way to make two different documents canonicalise to the same bytes or hash (DocIR integrity);
- a model package or dataset accepted without a valid signature or consent record, once those checks exist;
- a dependency with a known vulnerability or a licence outside the allow-list that the checks miss.

Not security problems, and best reported as ordinary issues: a wrong reading, a low confidence, a slow page.
Quality is measured on the golden set and not handled as a vulnerability.

## What the code reads today

In Phase 1 the only untrusted input the code parses is JSON, JSON Lines and the YAML subset of the quality
contracts. All three readers are strict (no duplicate keys, no NaN, no lone surrogates, bounded nesting), have
no `unsafe`, and are in crates that forbid it. Image decoders, the PDF reader and the model loader arrive in Phases 2
and 3, each behind the sandboxed worker, and each with a fuzz target before it is used (`fuzz/`).

## What the project does about it

- `unsafe` is confined, documented with a `SAFETY:` comment per block, and checked on every build
  (`cargo xtask check-unsafe`).
- Dependencies are named in contracts, have a licence row and pass the allow-list. `cargo deny` and an
  advisory check are set to run weekly in CI (`.github/workflows/security.yml`); that workflow has not run yet,
  and until it does the project has no dependency to check.
- Recognition makes no network call, and the worker is designed to have no network system calls at all (ADR-008).
- Reports and logs written by the project's tools contain no host names, user names or paths.

## Response (targets, to be confirmed by the owner)

The project has one maintainer. The aim, which the maintainer must be able to keep:

| Step | Target |
|---|---|
| Acknowledge the report | 7 days |
| Say whether it is accepted, and how severe | 30 days |
| Fix or mitigate, and publish an advisory | 90 days, sooner if it is being exploited |

The reporter is credited unless they prefer not to be.

## Good-faith research

Testing your own copy of the code on synthetic input, reading the source and reporting what you find is welcome and
will not be met with legal action. Do not test against other people's systems or data, and do not keep or share
any personal data you meet by accident.
