# Competitive audit: protocol and scoring sheet

| | |
|---|---|
| Status | **Not performed.** This file is the protocol and the empty scoring sheet. No tool has been used and no score below is a finding. |
| Phase | 1, weeks 1 to 5 (the UX track, alongside `plan.md`) |
| Why it is not done yet | A heuristic evaluation means using each tool. The assistant that wrote this file cannot operate a browser or an application in this plan, and a review from product pages and documentation would grade the marketing, not the tool. The owner (or a reviewer the owner chooses) does it with the protocol below. |
| Method | Heuristic evaluation against Nielsen's ten usability heuristics, each problem rated 0 to 4 for severity |
| Output | the filled scoring sheet, and a short section in `insights.md` |

## 1. What it is for

To learn, before building a review screen, what existing tools already do well, where they make a reviewer's
work slower or riskier, and what the people in `plan.md` put up with. It answers RQ4 and informs RQ3.
It is **not** a feature comparison, a market study, or a claim about any product's accuracy. Accuracy is
measured on the golden set or not at all (`data/datasheets/golden-v0.md`).

## 2. Which tools

The plan names five. Choose them **after the first interviews**, from what the participants say they use now
(interview question 9), so that the audit covers the real alternatives and not the most famous names. Fill the
list below, then keep it fixed.

Aim for a spread of kinds:

| Kind | Why it is in the set | Tool chosen | Version and date used |
|---|---|---|---|
| A capture service with a review screen (cloud, per-document pricing) | the closest thing to what version 1 does | | |
| An open-source OCR engine with whatever front end it comes with | the engine this project is measured against technically | | |
| The receipt or invoice capture built into an accounting package | what most small businesses already have | | |
| A phone scanning app | where many receipts start | | |
| An enterprise document-processing suite (reviewer view only, if a trial is available) | the bar a bookkeeper's employer may expect | | |

If a kind has no tool a participant used, drop it and take a second tool from a kind that did. Record why.

## 3. Evaluators

Nielsen and Molich found that one evaluator finds only a minority of the problems that several evaluators
find together, and that three to five is a good number. This audit will likely have **one** evaluator, the
owner, which is the weakest reasonable setting. To make up for it:

- do two passes a day or more apart, the second without looking at the first;
- have one other person, even a non-specialist, do pass one on at least two of the tools if at all possible;
- say in the report that the findings come from one or two people and are a list of candidate problems, not a
  census of them.

## 4. Rules for using the tools

- **Terms of use.** Read each tool's terms before signing up. Use free tiers or trials. Do not circumvent
  limits, do not scrape, do not reverse-engineer.
- **No real documents.** Use only the synthetic fixtures (`eval/fixtures/synthetic-v0`) or documents invented
  for the audit. A real invoice, receipt or identity document never goes into a third-party service for this
  purpose, whatever its terms say.
- **Record the version and the date** of every tool and screen. Interfaces change; a score without a date is
  not reproducible.
- **Screenshots stay private** (they carry the tools' designs); the audit publishes descriptions and scores,
  not copies.
- **Say when you could not do a task** and why (needs a paid plan, not available in the region), instead of
  guessing.

## 5. The tasks

The same scripted tasks on every tool, so the scores can be compared. Each is done from a clean account.

| # | Task | Which heuristics it exercises most |
|---|---|---|
| T1 | Add a batch of ten invoices (mixed quality) and get the data out | 1, 7 |
| T2 | Find the value in the batch that the tool read wrongly and correct it | 1, 5, 6, 9 |
| T3 | Find out why the tool flagged a field, or why it did not | 1, 2, 6 |
| T4 | Review a batch only by keyboard | 7 |
| T5 | Recover from a document the tool cannot read | 3, 9 |
| T6 | Review one identity document whose machine-readable zone and printed text disagree (synthetic specimen) | 2, 5, 9 |
| T7 | Export the data and find out what was exported, to where, and who saw the document | 1, 2, 10 |
| T8 | Find help for something you did not understand | 10 |

For each task note the number of steps, anything surprising, and whether the tool showed what it had done
(heuristic 1).

## 6. The heuristics

Nielsen's ten usability heuristics, in the plain wording used here:

| # | Heuristic | The question to ask |
|---|---|---|
| H1 | Visibility of system status | Does the person always know what the tool is doing and what state the batch is in? |
| H2 | Match between system and the real world | Does it speak in the person's words (invoice, supplier, due date) and not its own? |
| H3 | User control and freedom | Can they undo, go back, cancel and leave an operation without losing work? |
| H4 | Consistency and standards | Does the same thing look and act the same everywhere, and like other tools of this kind? |
| H5 | Error prevention | Does the design stop mistakes before they happen (for instance a wrong value accepted by default)? |
| H6 | Recognition rather than recall | Is what they need in front of them (the image next to the value) or must they remember it? |
| H7 | Flexibility and efficiency of use | Are there shortcuts for the experienced person, and does a long batch stay fast? |
| H8 | Aesthetic and minimalist design | Is each screen free of information that competes with the task? |
| H9 | Help users recognise, diagnose and recover from errors | Are error messages in plain words, precise, and helpful about what to do next? |
| H10 | Help and documentation | Is help findable, short and about the task? |

**Additional checks for this product** (not part of Nielsen's list; mark them *extra* in the sheet):

| # | Check |
|---|---|
| X1 | Is the evidence (the part of the image a value came from) shown next to the value? |
| X2 | Is uncertainty shown, and does the tool say what the uncertainty means? |
| X3 | Can a reviewer tell which values a person has checked, and which were accepted without a look? |
| X4 | Is it clear what leaves the person's machine, and when? |
| X5 | Does the export carry where each value came from? |

## 7. Severity

Rate each problem after finding all of them, from 0 to 4, weighing three things: how often it happens, how
much it hurts when it does, and whether people get past it once they know.

| Rating | Meaning |
|---|---|
| 0 | Not a usability problem |
| 1 | Cosmetic: fix it if there is time |
| 2 | Minor: low priority |
| 3 | Major: important to fix, high priority |
| 4 | Catastrophe: must be fixed before release; for this project, anything that lets a wrong value pass without anyone being told counts as a 4 |

## 8. Scoring sheet

One copy per tool. One row per problem. Leave the sheet empty until the tool has been used.

**Tool:** ____________  **Version / date:** ____________  **Evaluator:** ____________  **Pass:** 1 / 2

| ID | Task | Heuristic (H1 to H10 or X1 to X5) | What happened (a sentence, no screenshot needed) | Frequency (rare, sometimes, always) | Severity (0 to 4) | What a better design would do |
|---|---|---|---|---|---|---|
| | | | | | | |
| | | | | | | |
| | | | | | | |

**What the tool does well** (record at least three, so the audit is not only a list of faults):

1.
2.
3.

**Summary row for the comparison table in `insights.md`:**

| Tool | Problems found | Of severity 3 or 4 | Worst heuristic | Best thing |
|---|---|---|---|---|
| | | | | |

## 9. Combining

1. Merge the two passes (and the second evaluator, if there is one) by problem, not by score: the same problem
   found twice is one problem.
2. For a problem rated differently by two people, discuss it and record both numbers and the agreed one.
3. Count problems by heuristic and by tool. A count of problems is a rough size, not a ranking of products: a
   tool can have many cosmetic problems and one catastrophe.
4. Write up the patterns (the same problem across tools is a *category* problem and a design opportunity) in
   `insights.md`, tagged with the tasks and tools behind them.

## 10. What this audit cannot say

- Which tool is better. It says where people are likely to stumble, not how often they do.
- Anything about accuracy, speed on a real workload, or price.
- Anything about a tool's features that it did not show during the eight tasks.
- It is one or two evaluators' opinion until a participant confirms a problem in an interview or an inquiry.
  Where a problem from the audit is also seen in a session (`plan.md`), it counts for more, and `insights.md`
  records both sources.
