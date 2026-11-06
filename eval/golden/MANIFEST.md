# Golden set manifest

**Hashes and metadata only. The documents are never in this repository.** The images live in a private
data repository with access control and encryption at rest (`data/datasheets/golden-v0.md`, section 3).

Status: **empty**. Nothing has been collected yet. The first rows arrive with the first collection.

## What a row holds

One row per document. A row identifies a file by its hash and says where it belongs; it holds nothing
that identifies a person.

| Column | Meaning |
|---|---|
| `id` | the document identifier used in the truth and prediction files (`inv-0001`, `rec-0001`, `id-0001`) |
| `class` | `invoice`, `receipt` or `id`, the classes of the quality contracts |
| `script` | the script of the printed text: `latin`, `cyrillic`, `greek`, `arabic`, mixed ones joined by `+` |
| `quality` | `clean`, `phone` or `degraded` (the buckets of the datasheet) |
| `template` | a template family identifier, so that documents from one supplier or issuer can be counted together |
| `source` | the dataset or source row in `data/registry/datasets.yaml` |
| `consent` | a reference to the consent record in the private repository; a document without one is not listed |
| `sha256` | SHA-256 of the image file as stored, 64 lower-case hexadecimal digits |
| `keyed` | `single` or `double` (one document in five is double-keyed) |
| `split` | `dev`, `test` or `hidden` |

## Rules

1. **Append-only within a version.** A row is never edited. A correction, a removal or an erasure is a
   new version of the manifest with a dated note under *Versions* below, and the dataset's report says
   which published numbers it touches.
2. **The split is a seeded draw** made when the document is added, with the seed recorded under
   *Versions*. It is never changed to improve a result.
3. **The hidden split is opened once**, for the release decision, and the opening is a dated line under
   *Versions*. After that it is development data.
4. **The hash is checked, not trusted.** A script in the private repository recomputes every hash and
   compares it with this file before a run counts. A mismatch makes the run void.
5. **No row without a licence or consent trail.** Every `source` exists in the registry, and every
   `consent` resolves in the private repository.

## Rows

| id | class | script | quality | template | source | consent | sha256 | keyed | split |
|---|---|---|---|---|---|---|---|---|---|

## Versions

| Version | Date | Change |
|---|---|---|
| v0 | not yet | planned: 100 documents per class, one document in five double-keyed |
