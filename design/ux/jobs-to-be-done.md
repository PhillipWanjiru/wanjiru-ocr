# Jobs to be done (hypotheses, not findings)

| | |
|---|---|
| Status | **Hypotheses**, written in week 0 before any research. See `personas.md` for why. |
| Format | "When ..., I want ..., so that ...": a situation, a motivation and an outcome, with no solution in it, as used in jobs-to-be-done practice |
| Tested by | `design/research/plan.md`; each job's last row says how |
| Feeds | the quality contracts (`eval/contracts`), DocIR's provenance fields, the review interface of Phase 9, the consent tiers (ADR-008) |

A job statement describes a situation and a result, not a feature. If a statement mentions a button, a screen or
a model, it is a solution and does not belong here. The five jobs below are in the order that the hypotheses
rank them; the research may reorder or cut them.

For each job the table gives the people it belongs to, what "done" looks like in terms that can be measured,
the forces that make people switch to a new way of doing it, the forces that hold them back, and the part of the
system that serves it.

---

## J1. Get the data out of a pile

> **When** a batch of documents arrives and the contents have to go into another system,
> **I want** the key fields captured without retyping them,
> **so that** finishing the batch takes much less time than typing it.

| | |
|---|---|
| For | A, B (and C at onboarding) |
| Done looks like | Fewer minutes per batch than before, with no more errors found later than before. *Not* "the engine reads 99 % of characters": a person cannot feel that. |
| Pulls people to a new way | Month-end pressure; a pile that will not shrink; the tedium of typing. |
| Holds them back | Having to check everything anyway; a tool that was tried and abandoned; a setup cost per supplier or per template. |
| Served by | The `invoice`, `receipt` and `id` contracts; batch processing in `wanjiru-cli` and the engine; latency budget `B-engine-01`. |
| Hypotheses | **HJ-1** The first job is saving time on the pile, and the person's measure is time per batch at an acceptable number of later errors. Test: Q3, Q4 (the last batch, where it took longest); contextual inquiry. |

## J2. Know which values to doubt

> **When** a machine has read a document and I am responsible for the result,
> **I want** to be told exactly which values are uncertain and to see where on the page each came from,
> **so that** I check only what needs checking and can trust the rest.

| | |
|---|---|
| For | A, B, C (most strongly C) |
| Done looks like | When the engine says a value is accepted, it is right at least as often as the contract says (selective accuracy), and the share of values accepted without a look (coverage) is high enough to matter. The person's own check: how often they overrule a value the tool marked as safe. |
| Pulls people to a new way | A pile too large to check everything; having been burned by a confident tool. |
| Holds them back | A flag they do not believe; the habit of checking everything; a score they cannot interpret. |
| Served by | Calibration and conformal prediction (ADR-006); selective accuracy and coverage in `wanjiru eval`; DocIR's evidence fields (ADR-004); the review interface. |
| Hypotheses | **HJ-2** This is the job that decides adoption: people will trust a tool only if the flags are right, and a mostly-right tool without flags is rejected. Test: Q7, Q10; the first-reaction question. Related: **HP-A3**, **HP-X1**. |

## J3. Fix a wrong value cheaply

> **When** the machine and I disagree about a value,
> **I want** to correct it in a few keystrokes without leaving the page,
> **so that** fixing a mistake costs less than typing the field myself.

| | |
|---|---|
| For | A, B, C |
| Done looks like | The median correction takes a small number of seconds and keystrokes (set by research); the total correction time for a batch is lower than typing from scratch. |
| Pulls people to a new way | Review screens that need a mouse and a page change per field. |
| Holds them back | A correction that gets lost, or has to be repeated at the next step. |
| Served by | The review interface (Phase 9); constraint presets and local re-calibration from corrections (ADR-008, tier 0). |
| Hypotheses | **HJ-3** Review speed is decided by keyboard flow and by the source image sitting next to the value. Test: contextual inquiry (**HP-A4**, **HP-X3**). |

## J4. Show how a value got there

> **When** someone asks, weeks later, how a number came to be in the system,
> **I want** a record of what was read, what was changed and by whom,
> **so that** I can answer without redoing the work.

| | |
|---|---|
| For | C (hard requirement, hypothesis **HP-C4**), A (audits), B (client queries) |
| Done looks like | For any exported value, the person can retrieve the page, the region, the original reading, the corrections and the reviewer, with no gaps. |
| Pulls people to a new way | An audit or a dispute that went badly because nobody could say. |
| Holds them back | Extra steps at the time, if the record is not automatic. |
| Served by | DocIR with source-pixel geometry and transform chains (ADR-004); report hashing (ADR-005); `emit`. |
| Hypotheses | **HJ-4** The record matters most to the identity reviewer and the auditor, and little to the bookkeeper in an ordinary month. Test: Q11, Q12. |

## J5. Keep the data where it belongs

> **When** a document holds personal or confidential data,
> **I want** to know that nothing leaves my machine unless I decided it may,
> **so that** I can use the tool without breaking the rules I work under or asking permission each time.

| | |
|---|---|
| For | A (company policy), B (client trust), C (law) |
| Done looks like | A plain statement, which a security reviewer accepts, of what the software sends where (nothing, by default), and a way to check it. |
| Pulls people to a new way | Fear of a cloud service; a policy that forbids one. |
| Holds them back | Tools that only work in a cloud, so the choice is the tool or the rule. |
| Served by | ADR-002 (library first), ADR-008 (no network call during recognition; telemetry off; consent tiers), the worker's lack of network system calls. |
| Hypotheses | **HJ-5** For at least some of the people in each role, a local-only engine is a requirement and not a preference, and is the reason they will consider a new tool at all. Test: Q13; **HP-B3**, **HP-C3**, **HP-X2**. |

---

## Cross-check with the plan

The plan names five job statements. These are they. The research may find a job that is missing (the likeliest
candidates: matching a document with an order or a bank line, duplicate detection, and categorising a receipt).
Those are not engine jobs in v1, and the research notes them and passes them on without changing the scope
(`HP-A5`).

## How the jobs connect to the contracts

| Job | Contract or measure it turns into |
|---|---|
| J1 | latency by class at p50 and p95 (`B-engine-01`), coverage at the stated accuracy, time per batch |
| J2 | selective accuracy and coverage per field (`eval/contracts/*.yaml`); calibration error (`ece_char`, from Phase 6) |
| J3 | corrections per document and time per correction, from usability tests (`success-metrics.md`) |
| J4 | DocIR completeness: every exported value traces to a region (a test, from Phase 3) |
| J5 | the network-free guarantee: a check, to be written, that no crate on the recognition path links a network client. `cargo xtask check-contracts` already forces every dependency to be named in a contract, and ADR-007 gives each one a licence row, which is what makes such a check possible |
