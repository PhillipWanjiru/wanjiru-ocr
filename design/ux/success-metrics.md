# Success metrics (HEART, and how they are measured without telemetry)

| | |
|---|---|
| Status | **Draft, written in week 0.** The metrics and the methods are proposed; **no target below is a number yet**. Targets are set from the research (`design/research/plan.md`) before the first usability test and are then frozen, like the quality contracts (budget rule 1 of `docs/budgets.md`). |
| Frame | HEART, from Rodden, Hutchinson and Fu (Google, 2010): Happiness, Engagement, Adoption, Retention and Task success, each with a goal, a signal that would show it and a metric that quantifies the signal |
| Constraint | ADR-008: no telemetry without consent, and the default is none. A metric that needs data to leave a person's machine is measured only in sessions of people who agreed to it, or it is not measured. |

## The rule that shapes this file

The engine is local-first and the default is that nothing is reported. A metric therefore has to come from one
of three places:

1. **A session with consent.** A moderated usability test, an interview, a contextual inquiry. The researcher
   observes, times, counts, and asks the questions below. The participant signed the form.
2. **The golden set and the project's own tests.** Anything about the engine itself: accuracy, coverage,
   calibration and latency, measured on documents the project collected with consent, and properties that a
   test can check (that recognition makes no network call). It does not need a user.
3. **The person's own machine.** The software may keep local counters (documents read, fields reviewed,
   fields changed) and **show them to the person**. They leave the machine only if the person chooses to send
   them, by hand, after a preview, which ADR-008 allows. A report nobody sends is a metric nobody has; that is
   the price of the rule, and it is accepted.

Everything in the table says which of the three it uses. What none of them can give, and the file says so below,
is anything about how many people use the product, for how long, in the wild.

## The metrics

| ID | HEART | Goal | Signal | Metric | Source | Informs |
|---|---|---|---|---|---|---|
| HM-1 | Task success | A person finishes the batch correctly | They complete the scripted task with no wrong value passing | Completion rate over participants, with an exact (Clopper-Pearson) interval; count of tasks abandoned | 1: usability test | J1; the review interface |
| HM-2 | Task success | Review is faster than typing | Time on task | Median seconds per document, per task, compared with the participant's own typing of the same batch (within-person) | 1 | J1, J3 |
| HM-3 | Task success | Corrections are cheap | Effort per correction | Median keystrokes and seconds per corrected field | 1 (observed); 3 (local counter, shown to the person) | J3 |
| HM-4 | Task success | The flags are believed *and* right | A reviewer's catches against planted errors | **Seeded-error detection**: plant known errors at a known rate, some in flagged and some in unflagged positions; report the share caught in each, with exact intervals (method below) | 1 | J2, ADR-006 |
| HM-5 | Task success | The engine's accepted values are right | Selective accuracy and coverage | Per field, as in the contracts: the lower end of the interval for accuracy among accepted values; coverage | 2 | the quality contracts |
| HM-6 | Task success | Latency feels fine | Time a person waits | p50 and p95 per page by class (`B-engine-01`); and in sessions, the first moment the participant says or does anything about waiting | 2 and 1 | J1; the latency contracts |
| HM-7 | Happiness | The tool is usable | Questionnaire after the session | **System Usability Scale** (ten items, scored 0 to 100), plus the **Single Ease Question** (seven points) after each task | 1 | the review interface |
| HM-8 | Happiness | The person trusts what they are told | What the participant says about the flags | Coded interview answers on trust, counted as *k* of *N* participants, never as a percentage | 1 | J2 |
| HM-9 | Adoption | A person gets going without help | Install to first correctly read document | Minutes and the number of times help was needed, in a test starting from a clean machine | 1 | packaging (Phase 10), `docs/user` |
| HM-10 | Engagement | The person uses the review rather than skipping it | Share of accepted values a person looked at | Local counter shown to the person: the share of fields reviewed against fields accepted untouched. In sessions it is observed instead. | 3 and 1 | J2; the risk of rubber-stamping |
| HM-11 | Retention | The person is still using it later | Use after weeks | **Not measurable without telemetry.** Substitute: a follow-up call with each beta participant at 30 and 90 days, three questions, recorded in `insights.md` | 1 (follow-up) | roadmap, honestly labelled as qualitative |
| HM-12 | Task success | Nothing leaves the machine | A check anyone can run | A test that recognition makes no network call (ADR-008); a security-review checklist answered with evidence | 2 | J5 |

## The seeded-error method (HM-4)

The question behind J2 is whether people check what they should. Counting how many corrections they make
says little, since it depends on how many errors the engine happened to make. The method fixes that:

1. Take a batch of synthetic or consented documents with known correct values.
2. Replace a set of values with plausible wrong ones (a transposed digit, a neighbouring date) at a rate
   that the researcher fixes, for example one in ten. Put half in positions the interface flags and half in
   positions it does not flag (the flags are set by the researcher for the test, so that this is
   controlled).
3. Ask the participant to review the batch as they normally would.
4. Report two proportions: of the planted errors in flagged positions, how many were corrected; of the planted
   errors in unflagged positions, how many were corrected. Each with an exact interval.

The gap between the two is a measure of how much people rely on the flags, in both directions: a high catch
rate on unflagged positions means that they check everything, and the flags are decoration; a low one
means that they believe the flags, which is right only if the flags are right (HM-5). It is a measure of
**automation bias**, taken in a test setting and not in the field. An honest report says so.

## What small samples can and cannot do

Usability sessions have 5 to 12 people. The intervals are wide, and the report says so with numbers. For
example, the 95 % exact interval for a completion rate:

| Completed | Of | Interval |
|---|---|---|
| 8 | 8 | 63 % to 100 % |
| 7 | 8 | 47 % to 99.7 % |
| 6 | 8 | 35 % to 97 % |
| 5 | 5 | 48 % to 100 % |
| 11 | 12 | 62 % to 99.8 % |

Eight people who all finished do not show that nine in ten will. They show that finishing is not rare. The
sessions are for finding problems and the words people use for them; they are not for estimating how common
anything is (`plan.md`, section 3). Where a number is needed for the population, it comes from the golden set,
which is designed for that.

Differences between two designs, with the same small number of people, are not claimed unless the same
people did both and the interval for the paired difference excludes zero.

## Targets

None are set. They are set after the interviews, for each metric that has a number, in this order:

1. from what participants say is acceptable (for example a time per batch that makes the tool worth using);
2. from the quality contracts where the metric is an engine metric (HM-5, HM-6);
3. frozen in this file, dated, **before** the first usability test. A target changed afterwards is changed by a
   dated line with the reason, and the first target stays visible.

## What these metrics cannot tell

- How many people use the product, how often, or for how long. The telemetry rule rules it out. The project
  accepts that and uses follow-up calls, support conversations and the people who write to it.
- Whether a person's judgement was better with the tool than without it, outside a test. HM-4 measures how
  people behave in a controlled test.
- Anything about people who are not in the research. The participants are few, chosen by the researcher's reach,
  and the report says where they came from.
