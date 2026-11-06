# Journey map: from "a document arrives" to "verified data leaves" (a hypothesis)

| | |
|---|---|
| Status | **Hypothesis.** Drawn in week 0 from the roles in `personas.md` and the jobs in `jobs-to-be-done.md`. Nobody has been observed. Every cell below is an assumption until `insights.md` says otherwise. |
| Scope | One batch of documents, one person, one sitting, plus the question that comes months later. Not the whole company's process. |
| Becomes findings | After the contextual inquiries (`design/research/plan.md`): the stages are corrected from what is *seen*, and the "feels" row is replaced by what was said. |

## The map

Eight stages. For each: what the person is trying to do, what they do, where, what they are likely to be
thinking, how they are likely to feel, what goes wrong, and what the system must do. The "feels" row is
the weakest part of any journey map and is only a prompt for the interviews.

| | 1. Arrive | 2. Gather | 3. Read | 4. Triage | 5. Review | 6. Sign off | 7. Hand off | 8. Afterwards |
|---|---|---|---|---|---|---|---|---|
| **Goal** | Documents reach me | Make a batch I can work through | Get the machine to do the typing | See what needs me | Check and fix what needs me | Accept the batch as right | Get the data to the next system | Answer a question about a value |
| **Does** | Receives post, email, phone photos | Sorts, names, chases missing or unreadable ones | Starts the batch and waits | Looks at the list of documents and fields | Looks at a value next to its image; corrects or confirms | Marks the batch reviewed | Exports a file or sends to the system | Looks up a document, a value or a batch |
| **Where** | Inbox, tray, phone, shared drive | Desk, folders, the tool's import | The tool | The tool's queue | The tool's review view, keyboard | The tool | Export, import in the accounting or case system | The tool's records, files |
| **Thinks** | "Is this all of them?" | "Which of these are no good?" | "How long will this take?" | "Where do I start?" | "Is that really what it says?" | "Can I put my name on this?" | "Did it all arrive?" | "How did this get in?" |
| **Feels** (assumed) | neutral | impatient | impatient, curious | wary | focused | anxious | relieved | worried |
| **Pain points** (hypotheses) | Mixed formats; photographs of poor quality | The bad photographs are found late, when the client is no longer reachable | A blank wait with no sign of progress | A flat list in file order, with no sense of what matters most | The value and its image are far apart; every flag looks the same | No record of what was and was not looked at | The export does not match what the next system wants; a missing value is an empty cell | The record is incomplete or cannot be found |
| **System must** | Accept what people actually have (PDF, JPEG, PNG, phone photos) | Say at once when a document cannot be read, and why, so the sender can be asked once | Show progress, deliver first results early, never block on the slowest document | Sort exceptions by risk (what is at stake, how uncertain) and show the count | Show the evidence beside the value; flags that mean something; a short keyboard path | Record who looked at which value; show what was accepted untouched | Export in the formats the next system takes; make absent values explicit | Keep DocIR and the review log; find by document, value or batch |
| **Engine link** | `wanjiru-codecs`, `wanjiru-pdf` (Phase 2) | quality and rejection signals from `wanjiru-engine` (Phase 3) | `wanjiru-engine`, budget `B-engine-01` | calibration (ADR-006), `wanjiru-calibrate` (Phase 6) | DocIR evidence (ADR-004), review UI (Phase 9) | DocIR review fields; `wanjiru-emit` (Phase 3) | `wanjiru-emit` | DocIR (ADR-004), reports (ADR-005) |
| **Measure** | share of inputs accepted | share of unreadable documents flagged before review | time to first result; time to last | time from start to first correction | seconds and keystrokes per corrected field; seeded-error detection | share of accepted values touched by a person | export errors in the next system | time to answer a "how did this get here" question |

## The hypotheses

Each is testable and numbered, and each tells which question or observation would show it wrong.

| ID | Stage | Statement | How it can be wrong |
|---|---|---|---|
| HY-1 | 2 | The cheapest improvement is telling the person *at the moment of capture* (or the moment of import) that a document is unreadable, because the sender can still be asked. | Participants never have the sender at hand, or the bad ones are a small share. Test: interview Q5, Q8, role module for bookkeepers. |
| HY-2 | 3 | Waiting is tolerated if progress is visible and the first results come early; a single spinner for a whole batch is not. | Batches are small enough that nobody waits. Test: contextual inquiry (batch sizes seen). |
| HY-3 | 4 | People want exceptions ordered by risk (value at stake and uncertainty), not in file order. | They work strictly in document order for audit reasons. Test: inquiry, Q7. |
| HY-4 | 5 | The value and its source image must be in one view. | They are happy to look at the page in one window and the data in another. Test: inquiry (**HP-X3**). |
| HY-5 | 5 | A correction should take effect for the rest of the batch immediately (the same supplier's date format, the same recurring mistake). | Corrections are rarely repeated within a batch. Test: Q5 (did anything have to be done twice). |
| HY-6 | 6 and 7 | For identity documents the interface must never show a "verified" state on the strength of valid check digits: reading is not verifying. | Reviewers do not confuse the two. Test: show a mock result at the end of the interview; see whether they call it "verified". |
| HY-7 | 7 | The export has to match the next system's import, and a field that was not found must be explicit. | Participants export by copying and pasting. Test: Q3, Q9. |
| HY-8 | 8 | The record is used rarely and decides a lot when it is used. | It is used often, in small ways (to find a document). Test: Q11. |

## What the map says about the build order

The map is a hypothesis, and the build order of the plan does not depend on it. What it does is order the
questions: stages 4 and 5 (triage and review) carry most of the pain and most of the weight in the contracts,
and they are the ones the research most needs to get right before Phase 9 designs a screen. If the inquiries
show stage 2 (gathering) is where the time is lost, the case for early feedback on image quality
(`wanjiru-codecs`, Phase 2) is stronger than the plan says, and the plan changes.

## Not on this map

Matching with orders or bank lines, duplicate detection, payment, approvals, categorising a receipt, and
anything after the data has left. The research notes where people spend time on them. They are not v1 of the
engine.
