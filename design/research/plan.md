# Discovery research plan

| | |
|---|---|
| Status | Written, not started. No participant has been recruited and no session has been held. |
| Phase | 1, weeks 0 to 6 (the UX track) |
| Owner | the project owner, who is also the only researcher (an open input: hours per week) |
| Produces | `insights.md`, and the revision of `design/ux/personas.md`, `jobs-to-be-done.md`, `journey-map.md` and `success-metrics.md` |

Everything under `design/ux/` is written as **hypotheses**, from the roles the plan names and from general
knowledge of the work. This plan is how those hypotheses get tested or thrown away.

## 1. What this research is for

Version 1 reads business documents: invoices, receipts and identity documents. The code of Phases 1 to 8
has a great deal of freedom in what it builds first, and the desktop application of Phase 9 is where the
people who use the engine meet it. Discovery research decides, before either is built:

- which fields and which documents matter most, and in what order a person wants to see them;
- how a reviewer wants to be told that a value is uncertain, and what makes them believe or doubt it;
- how much correction effort a person will accept per document, which sets the error budgets of the
  quality contracts from the person's side and not only from the engine's;
- what must never happen (a wrong value accepted without anyone looking), which sets the contracts' hard
  limits;
- what a customer will and will not allow to be collected (ADR-008).

## 2. Research questions

| # | Question | Decides |
|---|---|---|
| RQ1 | How do the people in the four roles handle a batch of documents today, from arrival to the data leaving? | the journey map, the shape of batch processing |
| RQ2 | Where in that work do errors happen, how are they found, and what do they cost? | the quality contracts' error budgets, the review queue |
| RQ3 | What makes a person trust or distrust a value that a machine read? | how confidence and evidence are shown (ADR-006, ADR-010) |
| RQ4 | What do they use now, and what do they put up with? | the competitive audit, the thresholds for switching |
| RQ5 | What can leave their premises, and under what conditions? | the consent tiers (ADR-008), the golden-set collection |
| RQ6 | What does "faster" mean to them: seconds per document, documents per hour, fewer keystrokes, fewer late corrections? | the metrics in `success-metrics.md` |

## 3. Method

**Eight to twelve semi-structured interviews** of 45 to 60 minutes, and **three contextual inquiries** of 60
to 90 minutes, in which the researcher watches real work in the participant's own setting and asks about it
as it happens. Interviews tell what people say and remember; inquiries show what they do. Where the two
disagree, the inquiry wins and the disagreement is itself a finding.

The guide is `interview-guide.md`. The form every participant signs first is `consent-form.md`.

### Participants

| Role | Why | Minimum |
|---|---|---|
| Accounts-payable clerk | enters supplier invoices; the plan's first persona | 2 interviews, 1 inquiry |
| Bookkeeper (several small clients) | receipts and invoices of many shapes, month-end pressure | 2 interviews, 1 inquiry |
| Identity-verification reviewer | checks identity documents; where a wrong accepted value is the worst error | 2 interviews, 1 inquiry |
| Small-business owner | often does the work personally; also the buyer | 2 interviews |

Spread within a role: company size, the tool in use now (a spreadsheet, an accounting package, a capture
service, nothing), and the share of paper against electronic documents. The target market is an open input;
until it is named, recruit where the researcher can reach people in person.

**Screener** (five questions, asked before scheduling): your role; how many documents of this kind you handle
in a typical week; what you use to get their contents into another system; whether you decide on corrections
yourself or someone else does; whether you could do the session in your workplace or by video. Exclude
people who work for a competing OCR product, and the researcher's close family, whose answers would be
courtesy.

### How many is enough

Eight to twelve interviews across four roles is a small number, and the plan does not hide it. The
often-quoted result that five participants find about 85 % of the problems in a usability test comes from
a model: if each participant independently finds a given problem with probability *p*, then *n*
participants find it with probability 1 − (1 − *p*)^*n*. With *p* = 0.31, the average Nielsen and Landauer
reported, two participants find a problem with probability 0.52, three 0.67, five 0.84. That model is for a
single user group and a single test; here there are four groups, so the realistic claim is that two or three
people per role will find the common problems of that role and will not find the rare ones. Stop recruiting
a role when its last two sessions produce no theme that is new among that role's ten most important, and
say so in `insights.md`.

Counts in the findings are written "*k* of *N* participants", never as percentages, and nothing here is a
statement about how common a problem is in the population. Qualitative research finds problems and the
words people use for them; it does not measure prevalence.

## 4. Conduct

- **Before.** Send the consent form with the invitation. Confirm that the participant will not show any
  customer's data that they are not allowed to show; the session works on their own workflow, and for an
  inquiry the researcher looks at what is on the screen, takes no photographs of documents, and notes only
  the structure of what is seen (which fields, in what order), not the values.
- **During.** One researcher asks; a second person takes notes if there is one. Audio is recorded only with
  the participant's recorded agreement. The interviewer follows the guide, but follows the person when they
  say something unexpected.
- **After.** Within 24 hours: clean the notes, write the one-paragraph summary and the five most
  surprising things, and mark each observation as *said*, *seen* or *inferred*.

## 5. Privacy and ethics

The research follows the same rules as the product (ADR-008). Participation is voluntary and can be
withdrawn during the session and for 14 days afterwards, which deletes the participant's data. Recordings
and notes are stored encrypted, under pseudonyms (`P01`, `P02`), with the key to the pseudonyms kept apart.
Quotations are anonymised and checked for anything that identifies a person or a company. Raw recordings are
deleted when the analysis is finished and in any case within 12 months. The consent form has to be
checked against the law that applies where the participants are before it is used (the form says so).

## 6. Analysis

1. Write each observation on its own line with the participant, the role and the tag *said*, *seen* or
   *inferred*.
2. Group the observations into themes by affinity, twice: once alone, once again a week later without
   looking at the first, and reconcile the two.
3. Turn each theme into an insight in the form given in `insights.md`: what was observed, in how many
   sessions, what it suggests, how sure we are, and what follows for the design.
4. Revise the personas, jobs, journey and metrics. A hypothesis that no session supported is removed or
   marked untested, never kept because it was in the draft.

## 7. Schedule

| Weeks | Work |
|---|---|
| 0 to 1 | screener, recruitment, consent form checked, pilot of the guide with one friendly participant |
| 2 to 4 | interviews and the three inquiries, notes cleaned the day after each |
| 5 | second affinity pass; first draft of `insights.md` |
| 6 | revise the `design/ux/` documents; decide what is carried into the golden-set plan; close in writing |

## 8. Risks

| Risk | Mitigation |
|---|---|
| Recruiting only friendly or similar people | quota by role and by tool in use; ask each participant to introduce someone who works differently |
| Leading questions and courtesy answers | the guide asks about the last time something happened, not about opinions of an imagined product; no concept is shown before the last section |
| One researcher cannot take notes and listen | record with consent and note timestamps; keep sessions to 60 minutes |
| Hours: this plan assumes 30 focused hours a week for the project, and research competes with code | the research is sized for about 40 hours in total, spread over six weeks |
| Findings that contradict the plan | they are the point; they go in `insights.md` first, and the plan changes after |
