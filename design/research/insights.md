# Research insights log

| | |
|---|---|
| Status | **Empty.** No session has been held, so there are no insights. This file holds the method for writing them and the register of hypotheses they will test. |
| Written | week 0 |
| Fed by | `plan.md`, `interview-guide.md`, `competitive-audit.md` |
| Feeds | `design/ux/*` (revised after week 5), the quality contracts at gate G1, the review interface of Phase 9, ADR-004 and ADR-008 |

An insight is a statement that survived contact with participants. A hypothesis is a statement nobody has tested.
The log keeps the two apart, and nothing moves from the second kind to the first without evidence written down
here.

## 1. How an insight is written

One entry per insight, in this form. An entry with an empty line is not finished.

```
I-001  <a plain sentence, in the participant's words where possible>
  Observed    what was seen or said, one line per observation, each tagged said / seen / inferred
  In          k of N participants (who: P03, P05, ...), by role: A k/n, B k/n, C k/n, D k/n
  Evidence    the sessions and the timestamps; quotations anonymised (nothing that identifies a person,
              an employer or a customer)
  Suggests    what it means; marked as interpretation
  Confidence  high / medium / low, by the rule in section 2
  Against     any participant or observation that points the other way, and what to make of it
  So          the change it asks for: a contract, a journey stage, a metric, an ADR, the plan
  Affects     the hypotheses it supports, refutes or changes (HP-..., HJ-..., HY-...)
```

## 2. Rules of evidence

- **Counts, not percentages.** "Four of ten participants" and never "40 %". Ten people do not make a
  percentage, and nothing here says how common a thing is in the population.
- **Seen beats said.** What a person did in a contextual inquiry counts for more than what they said in an
  interview. Where the two disagree, record the disagreement as its own insight.
- **The first reaction to the project is not evidence of need.** It is recorded separately, as courtesy
  is common and the question was asked last. It never supports a hypothesis on its own.
- **Confidence:**
  - *High*: seen (not only said) in at least two sessions in at least two roles, and nothing against it.
  - *Medium*: seen or said in at least three sessions, or seen in one session and said in two others.
  - *Low*: one or two sessions, or only said, or contradicted by a participant.
- **Disconfirming evidence goes in the entry**, under *Against*, in the same size of type as the evidence for.
- **A hypothesis may end in four states:** *supported*, *refuted*, *changed* (rewritten, with the old text
  kept) or *untested* (no session bore on it). A hypothesis that was never tested stays untested; it is not
  quietly promoted.
- **Stop rule** (from `plan.md`): recruiting stops for a role when its last two sessions bring no new theme
  among that role's ten most important.

## 3. The log

| ID | Insight | In | Confidence | Affects | So |
|---|---|---|---|---|---|
| | *(no entries)* | | | | |

## 4. Register of hypotheses

All of them were written in week 0 and none has been tested. The register is checked against the documents under
`design/ux/` by `crates/wanjiru-cli/tests/repository_data.rs`: an identifier that appears in one place and
not the other fails the test.

| ID | Statement (short form; the full text is in the source) | Source | Tested by | State |
|---|---|---|---|---|
| HP-A1 | The costliest error for the clerk is a wrong account or amount passing into a payment, not a slow batch | personas A | Q6 | untested |
| HP-A2 | Layout changes by supplier are why template-based tools failed for them | personas A | Q10, module | untested |
| HP-A3 | They decide what to check by a few cues, not by looking at everything | personas A | Q7 | untested |
| HP-A4 | They work by keyboard; a mouse trip is a defect | personas A | inquiry | untested |
| HP-A5 | Duplicate detection matters as much as reading accuracy (out of v1 scope; tests whether it should be) | personas A | module | untested |
| HP-B1 | Bad photographs dominate; "I cannot read this one", said early, beats a little more accuracy | personas B | Q8, module | untested |
| HP-B2 | They need a few fields from a receipt and ignore line items | personas B | module | untested |
| HP-B3 | Client confidentiality limits tool choice more than price | personas B | Q13 | untested |
| HP-B4 | They work on laptops without a graphics card; CPU latency is what they feel | personas B | screener, Q9 | untested |
| HP-C1 | For the reviewer a wrong accepted value is far worse than a delay | personas C | Q6, Q12 | untested |
| HP-C2 | They start from the check digits and the zone against the printed text | personas C | module | untested |
| HP-C3 | They cannot send documents outside their organisation at all | personas C | Q13 | untested |
| HP-C4 | A record of review is a hard requirement | personas C | Q11 | untested |
| HP-D1 | The owner is a variant of the bookkeeper with a buying role, not a fourth persona | personas D | module | untested |
| HP-D2 | The owner pays for time, not accuracy points; must install without IT | personas D | module | untested |
| HP-X1 | All prefer a short list of flagged fields, if they believe the flags | personas | Q7, reaction | untested |
| HP-X2 | None accept a tool that sends documents away without their decision | personas | Q13 | untested |
| HP-X3 | All review with the source image next to the value | personas | inquiry | untested |
| HJ-1 | The first job is saving time on the pile, at an acceptable number of later errors | jobs J1 | Q3, Q4, inquiry | untested |
| HJ-2 | Knowing which values to doubt decides adoption; a mostly-right tool without flags is rejected | jobs J2 | Q7, Q10 | untested |
| HJ-3 | Review speed is decided by keyboard flow and the image beside the value | jobs J3 | inquiry | untested |
| HJ-4 | The record matters most to the identity reviewer and the auditor | jobs J4 | Q11, Q12 | untested |
| HJ-5 | For some people in each role a local-only engine is a requirement | jobs J5 | Q13 | untested |
| HY-1 | The cheapest improvement is telling the person at capture that a document is unreadable | journey 2 | Q5, Q8 | untested |
| HY-2 | Waiting is tolerated if progress is visible and first results come early | journey 3 | inquiry | untested |
| HY-3 | Exceptions are wanted in order of risk, not in file order | journey 4 | inquiry, Q7 | untested |
| HY-4 | The value and its source image must be in one view | journey 5 | inquiry | untested |
| HY-5 | A correction should take effect at once for the rest of the batch | journey 5 | Q5 | untested |
| HY-6 | For identity documents the interface must never show "verified" on valid check digits alone | journey 6, 7 | mock result | untested |
| HY-7 | The export must match the next system's import; an absent field must be explicit | journey 7 | Q3, Q9 | untested |
| HY-8 | The record is used rarely and decides a lot when used | journey 8 | Q11 | untested |

## 5. Decisions this research can change

A research plan is worth its cost if something would be done differently because of it. These are the decisions
that wait for it, the hypotheses that bear on each, and what would change them.

| Decision | Where it lives | Waits on | What would change it |
|---|---|---|---|
| Which fields each contract scores, and how strictly | `eval/contracts/*.yaml`, frozen at gate G1 (week 18) | HP-A1, HP-B2, HP-C1, HJ-2 | A field participants never use, or a field they check every time and the contract does not name |
| How the review interface shows uncertainty | Phase 9 design; ADR-006 | HJ-2, HP-X1, HP-A3 | Participants distrust flags, or do not use them |
| Review layout and keyboard model | Phase 9 | HJ-3, HP-A4, HP-X3, HY-3, HY-4 | A different working pattern in the inquiries |
| Early rejection of unreadable input | `wanjiru-codecs` and `wanjiru-engine`, Phases 2 and 3 | HY-1, HP-B1 | Rejection comes too late to ask the sender, or the unreadable ones are rare |
| What leaves the machine, and when | ADR-008 (consent tiers), the golden-set collection | HJ-5, HP-X2, HP-B3, HP-C3 | A segment that demands tier 0 only, or one that would accept more |
| What a review record holds | DocIR provenance fields, ADR-004 (v0 is not frozen) | HJ-4, HP-C4, HY-8 | The record is not used, or needs more than DocIR carries |
| Whether matching or duplicates are in scope | the plan | HP-A5 | They turn out to be the bigger part of the job |
| Whether the owner is a persona | `personas.md` | HP-D1, HP-D2 | The owner's needs differ from the bookkeeper's |

## 6. Competitive audit summary

To be filled from the scoring sheets of `competitive-audit.md`. Nothing here is a finding until the audit is done.

| Tool | Problems found | Of severity 3 or 4 | Worst heuristic | Best thing | Also seen in a session? |
|---|---|---|---|---|---|
| | | | | | |

## 7. Follow-up with participants

If a participant agrees, a short call at 30 and at 90 days, three questions (do you still use it, what changed,
what would you not give up). This is the only way the project learns about retention without telemetry
(`design/ux/success-metrics.md`, HM-11).

| Participant | 30 days | 90 days |
|---|---|---|
| | | |

## 8. Closing the research

At the end of week 6, in writing and dated, by the owner:

- the number of sessions held by role, and any role that stopped short of the plan and why;
- the register with every hypothesis in a final state;
- the insights table;
- the decisions of section 5 with their outcome;
- what the research could not answer and what would be needed to answer it.
