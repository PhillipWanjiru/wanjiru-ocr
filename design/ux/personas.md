# Personas (hypotheses, not findings)

| | |
|---|---|
| Status | **Hypotheses.** Written in week 0 from the roles the plan names and from general knowledge of the work. No one has been interviewed. Nothing below describes a real person. |
| Becomes findings | after the research in `design/research/plan.md`; each statement is revised, kept or deleted by `insights.md` |
| Tests | the interview guide, section B (the last batch), section C (when it goes wrong) and the role modules |

## How to read this file

A persona is a way to keep one kind of person in mind while deciding. These three are **role-based**: they
stand for a way of working, not for an individual, and they carry no photograph, age, hometown or invented
biography, because those details invite design for an imagined person. The target market is not chosen
yet (an open input), so locale, language and currency are left out on purpose.

Every statement that could be wrong has an identifier (`HP-A1`, ...) and a test. A persona that research does
not support is rewritten or removed. The fourth role the research recruits (the small-business owner) is treated
as a buyer and a variant of the bookkeeper until the interviews say otherwise (`HP-D1`).

What all three have in common is a position the engine has to respect: they are **accountable for the data
that leaves their desk**. A tool that is mostly right, and gives no hint where it is wrong, is no use to
anyone who has to sign for the result.

---

## A. The accounts-payable clerk

*Hypothesis: works through a queue of supplier invoices that must be entered, matched and paid.*

| | |
|---|---|
| **Does** | Receives invoices by post, by email as PDFs and as photographs; enters the supplier, number, date, amounts and bank details in the finance system; matches them with orders; passes them for approval and payment. |
| **Wants** | The pile smaller by the end of the day; no payment to the wrong account or for the wrong amount; no duplicates. |
| **Measured on** | Invoices processed per day and errors found later (by the payment run, an auditor, or a supplier who was paid wrongly). |
| **Works** | At a desk with two screens, keyboard-first, in bursts around interruptions, with a peak at month-end and before the payment run. |
| **Judges a tool by** | Whether the green things can be trusted, so that only the amber ones need a look. "Mostly right" with no signal is worse than typing, because everything has to be checked anyway. |
| **Fears** | Being the person who paid the wrong IBAN. |
| **What the engine must give them** | The fields in the `invoice` contract (`total_amount`, `issue_date`, `iban`), plus a per-field confidence that means what it says, the image of where each value came from, and a review pass that runs on the keyboard. |

Hypotheses to test:

| ID | Statement | Test |
|---|---|---|
| HP-A1 | The costliest error is a wrong bank account or amount that passes into a payment, not a slow batch. | Q6 (a time a wrong number got through): what it cost, who found it. |
| HP-A2 | Layout changes by supplier are the main reason that template-based capture tools failed for them. | Q10 (have you tried anything that reads documents automatically); role module (a supplier who changes layout). |
| HP-A3 | They decide whether to look at a field by a small set of cues (amount against the order, a supplier's habit), not by looking at everything. | Q7 (how do you decide a value needs a second look). |
| HP-A4 | They work with the keyboard and would count a mouse trip as a defect. | Contextual inquiry: seen, not said. |
| HP-A5 | Duplicate detection matters to them at least as much as reading accuracy. | Role module (how are duplicates found). Note: out of scope for v1 of the engine; this tests whether it should be. |

---

## B. The bookkeeper

*Hypothesis: keeps the books for several small clients; deadlines come in waves; the paper arrives in a shoebox.*

| | |
|---|---|
| **Does** | Collects receipts and invoices from clients (photographed on phones, scanned, forwarded), enters or checks them in an accounting package, reconciles with bank lines, prepares the periodic returns. |
| **Wants** | To finish a client's month without chasing them for unreadable photographs; to keep one client's data apart from another's. |
| **Measured on** | Deadlines met; clients retained; time per client (often billed, so time saved is margin, and time wasted is the bookkeeper's own). |
| **Works** | At a laptop, sometimes at the client's; many small batches rather than one large one; the input quality ranges from a flatbed scan to a crumpled till receipt in poor light. |
| **Judges a tool by** | How it handles the worst receipt in the pile; whether it sorts the unreadable ones to the top so the client can be asked once; whether it keeps clients separate. |
| **Fears** | A client's financial data leaking, or going somewhere the client has not agreed to. |
| **What the engine must give them** | The `receipt` contract fields (`total`, `date`), a clear statement when something cannot be read (instead of a guess), batch processing that is quick on a laptop, and the guarantee that nothing leaves the machine (ADR-008). |

| ID | Statement | Test |
|---|---|---|
| HP-B1 | Bad photographs are the dominant problem; a tool that says "I cannot read this one" early is worth more than one that is slightly more accurate on the good ones. | Role module (a faded till receipt); Q8 (a document you cannot read). |
| HP-B2 | They need only a few fields from a receipt (total, date, supplier, tax) and ignore line items. | Role module (which fields do you need). |
| HP-B3 | Client confidentiality restricts which tools they may use more than price does. | Q13 (what may leave the building). |
| HP-B4 | They work on a laptop with no graphics card, so latency on a CPU is what they feel. | Screener (equipment); Q9. |

---

## C. The identity-verification reviewer

*Hypothesis: checks identity documents at onboarding and decides to accept, reject or escalate.*

| | |
|---|---|
| **Does** | Looks at a photograph of an identity document (and often a face image), checks that the data matches what the customer entered, uses the machine-readable zone for check digits and consistency, and records a decision. |
| **Wants** | Correct decisions, each defensible if challenged; a short queue. |
| **Measured on** | Decisions per hour with a ceiling on wrong accepts; audit results. |
| **Works** | In a regulated setting, often in a team, from a queue; every decision leaves a record. |
| **Judges a tool by** | Whether it shows its evidence (the zone, its check digits, the printed text, and whether they agree) and never decides alone. |
| **Fears** | Accepting a wrong or false record; being unable to show afterwards why. |
| **What the engine must give them** | The `id` contract fields (`mrz`, `document_number`, `date_of_expiry`), independent validators (check digits, dates), a visible comparison of the zone with the printed text, and a full record of what was read and changed. |

**Scope, stated plainly.** Version 1 **reads** identity documents. It does not judge whether a document is genuine,
whether a face matches, or whether a person is who they say. A reviewer who takes reading for verification
is a risk, and the interface says so wherever a result is shown (`HY-6` in `journey-map.md`).

| ID | Statement | Test |
|---|---|---|
| HP-C1 | A wrong value accepted without a human look is far worse to them than any delay, so the contract's accepted-field error limits (`id-mrz-v1`) are the thing that matters. | Q6; Q12 (whose name is on a mistake). |
| HP-C2 | The check digits and the consistency between the zone and the printed text are the first thing they use. | Role module (what do you check, in what order). |
| HP-C3 | They cannot send documents outside their organisation at all. | Q13. If true, tier 0 of ADR-008 is a requirement for them and not a preference. |
| HP-C4 | A record of review (who looked, what changed) is a hard requirement. | Q11 (who else looks at it). |

---

## D. The small-business owner (not a persona yet)

Does this work personally, in the evenings, and is also the one who decides what is bought.

| ID | Statement | Test |
|---|---|---|
| HP-D1 | The owner is a variant of the bookkeeper's job (the same documents, the same fear) with a different buying role, and not a fourth persona. | Role module (owner); compare with B after the interviews. |
| HP-D2 | The owner will pay for time saved, not for accuracy points; a tool must be installable without an IT department. | Role module (what would you pay for it not to be your evening). |

---

## What the personas have in common (to test)

| ID | Statement | Test |
|---|---|---|
| HP-X1 | All three would rather review a short list of flagged fields than every field of every document, provided they believe the flags. | Q7; the first reaction at the end of the interview. |
| HP-X2 | None of them will accept a tool that sends their documents away without their decision. | Q13. |
| HP-X3 | All three review with the source image visible next to the value. | Contextual inquiry. |

## What is deliberately missing

Age, gender, education, income, technical skill, favourite apps, and a stock-photo face. None of them is known,
and writing them down would make the personas look more certain than they are.
