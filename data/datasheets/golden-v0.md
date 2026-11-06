# Datasheet: golden set, version 0

Follows the headings of "Datasheets for Datasets" (Gebru et al.), written **before** the collection, as a
protocol. Every statement in the future tense is a commitment of the collection, and every "not yet" is
an open input. When the first documents are collected the datasheet is revised, not replaced, and the
revision is dated.

| | |
|---|---|
| Status | Planned. Nothing has been collected. |
| Registry row | `golden-v0` in `data/registry/datasets.yaml` |
| Manifest | `eval/golden/MANIFEST.md` (hashes and metadata only; the documents are never in this repository) |
| Quality contracts proven on it | `eval/contracts/invoice-fields-v1.yaml`, `receipt-fields-v1.yaml`, `id-mrz-v1.yaml` |
| Owner | the project owner |

## 1. Motivation

*Why does this dataset exist?* To prove or refuse the quality contracts with intervals instead of
anecdotes. A contract such as "99.9 % of accepted fields are correct" is a claim about documents the
engine has not seen, and it can only be judged on documents that were never used to build or tune it.

*Who makes it and who pays?* The project owner, from the project's own budget (an open input: budget).

*What it is not.* It is not a training set, and it is not a benchmark for comparing with other
engines' published numbers. `eval/fixtures/synthetic-v0` stays in the repository only to test the
machinery; it says nothing about real paper.

## 2. Composition

*What is in it?* Photographs and scans of real business documents, with the correct reading of every
field the contracts name.

| Class | Fields the contracts score | Target for v0 |
|---|---|---|
| `invoice` | `total_amount`, `issue_date`, `iban` | 100 documents |
| `receipt` | `total`, `date` | 100 documents |
| `id` (identity document with a machine-readable zone) | `mrz`, `document_number`, `date_of_expiry` | 100 documents |

The plan states the first target twice, as 200 documents in the phase table and as 100 per class in the
exit criteria. This datasheet uses 100 per class, the stricter reading; the owner may cut it to 200
without changing anything else. The full golden set is about 360 documents per class (`wanjiru eval plan`:
3,575 fields over 358 documents for 99 % accuracy to within 0.5 points at 95 % confidence, with ten
fields per document and a within-document correlation of 0.15), reached by Phase 6. 100 documents per
class is enough to find out what is wrong and not enough to prove a 99.9 % promise: the harness will say
*inconclusive*, and that is the correct answer.

*Cells.* Documents are spread over document class, script, quality bucket and template family. Scripts
start with Latin, accented letters included, then extended Latin, Cyrillic and Greek, then Arabic
(the plan's recommendation; the owner has not yet named the target market, so the locale pack is open).
Quality buckets, proposed: **clean** (flatbed scan or a flat, well-lit photograph), **phone** (hand-held
photograph with ordinary skew, shadow and glare) and **degraded** (crumpled, faded, low light, blur, a
fold across a field). Each cell has a minimum count, set by allocation proportional to cell size times
the spread of the outcome in that cell (Neyman) once a first pilot has shown the spreads. Before the pilot
the allocation is equal.

*Personal data.* Yes, by nature: names, addresses, account numbers and, for identity documents, much
more. See sections 3 and 6.

*Known gaps and biases.* The set will reflect the owner's sources: their country, languages, industries
and the printers and phone cameras of the people who volunteer documents. That is not a defect to hide: it
defines where the quality statement holds, and every report that quotes a number from this set must say
which sources it covers.

## 3. Collection process

*How are documents acquired?* In order of preference:

1. documents the owner has the right to use, with the counterparties' personal data handled as below;
2. documents from volunteers, each with a signed or recorded consent (`design/research/consent-form.md`
   is the form for research sessions; a collection needs its own, which the same template can be adapted
   to);
3. specimen or sample documents that an issuing authority publishes for that purpose. They are valid for
   checking formats and check digits and are weak evidence about print and photograph quality.

Real identity documents are the hardest case in law and in security. Until the owner has taken advice
for the country concerned, identity documents come only from item 2, from the volunteer's own document, with
the volunteer's consent to the photograph and a written agreement on retention, and never from item 1.

*Consent.* Before collection, each volunteer is told what is photographed, why, who sees it, how long it
is kept, how to withdraw, and that withdrawal removes the document from every future version. The consent
reference is a column of the manifest. A document without a consent reference is not in the set.

*Storage.* Images live in a private data repository with access control and encryption at rest. This
repository holds the manifest. Nothing in the manifest identifies a person.

## 4. Preprocessing, cleaning and labelling

*Labelling.* Every field is transcribed by a person into the truth format of `wanjiru eval` (JSON Lines:
`id`, `class`, `text`, `fields`). One document in five is **double-keyed**: transcribed independently by
two people, with disagreements settled by a third look at the image. The rate of disagreement between
keyers is reported with the set, as character error rate between the two transcriptions: it is the floor
under any engine's measured error, because a truth that is wrong in one field in a thousand cannot prove a
promise of one in a thousand.

*Fields that are not on the document* are recorded as absent, never invented; the harness counts them
(fewer trials than documents).

*Splits.* Each document belongs to exactly one of **development** (used to build and tune), **test**
(used for the quality contracts, never for tuning) and **hidden** (opened once, for the release
decision). The split is fixed at collection time, by a seeded draw recorded in the manifest, and never
changed to improve a result.

*Raw data.* The photographs are kept as taken. No automatic enhancement is applied to the stored image.

## 5. Uses

*Intended.* Proving or refusing the quality contracts; measuring regressions between versions with paired
intervals; calibrating confidences, with a calibration split carved out of **development**, never out of
test (ADR-006).

*Not intended.* Training or tuning on the test or hidden split. Publishing the documents. Comparing with
other engines on different data. Any use that the consent given does not cover.

*Reading a result.* A report states the number of documents behind every number, the interval, the
method and the seed, and prints the smallest difference the set can detect. "No change detected" is read
with that number in view.

## 6. Distribution, maintenance and risks

*Distribution.* The documents are not distributed. The manifest, which holds hashes and metadata, lives in
this repository. Anyone who needs to reproduce a result can ask for access to the private repository under
an agreement that carries the consent terms.

*Maintenance.* The owner maintains it. Versions are numbered; a document is added or removed only in a
new version, with a note. An erasure request is honoured by removing the document from the private
repository and the manifest in the next version, before the next calibration or training run, and by
reporting which published numbers it touched.

*Risks.* A leak of the private repository would expose real people's documents. The mitigations are the
storage rules in section 3, a short list of people with access, and keeping the set as small as the
statistics allow. A second risk is the set's drift from the documents customers actually send: it is
refreshed from real use (consented, tier 1 of ADR-008) once there is any.

## Open inputs

| Input | Why it matters here |
|---|---|
| Target market and locale pack | decides the scripts, number and date formats and the identity documents |
| Document source and consent route | decides section 3 |
| Budget | decides how many keyers, how much double-keying, and how large the set can grow |
| Focused hours per week | the plan assumes 30; collection and keying are the largest part of Phase 1 for one person |
