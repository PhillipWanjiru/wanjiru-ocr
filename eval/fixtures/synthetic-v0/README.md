# synthetic-v0

Three hundred fictional documents, the output of a simulated engine and of a simulated baseline for each of
them, and one demonstration contract. They exist so that `wanjiru eval` can be run end to end, by a person
or by CI, before any real document has been collected. They contain no real person, company, account or
document, and they say **nothing** about how well any engine reads real paper: the "documents" are random
characters and the "engine" is a random number generator with stated error rates.

| file | what it is |
|---|---|
| `truth.jsonl` | 300 reference records: `id`, `class`, `text` (the reference transcription) and `fields` |
| `predictions.jsonl` | the engine's 300 DocIR documents (`wanjiru.docir/0`) |
| `baseline.jsonl` | the baseline's 300 DocIR documents |
| `demo-lenient-invoice.yaml` | a contract with thresholds this much data can actually prove |
| `scenarios.json` | the parameters that produced each class |
| `expected/*.txt` | the transcripts of the four runs below: command, exit code, standard output |

120 invoices (`inv-…`), 100 receipts (`rec-…`) and 80 identity documents (`id-…`) share the three files; each
contract picks its own class.

## What is in the data

- **Text** is random characters (Latin letters, digits, a few accented letters, punctuation and `€`) in two
  to four lines per document. It is not language; it is only something to measure edit distance on.
- **Amounts** are decimal strings, dates run from 2015 to 2026, **IBANs** are 15 to 34 characters with a
  correct mod-97 check but a country prefix from ISO 3166-1's private-use range (`AA`, `QM`–`QZ`,
  `XA`–`XZ` without `XK`, `ZZ`), which no real IBAN country uses, so none of them can be a real account.
- **MRZ** lines follow ICAO 9303 (TD1, TD2 and TD3, with correct check digits) and name the issuing state
  `UTO`, the fictional state ICAO uses in its specimens; names and numbers are random letters.
- About 5 % of the reference records lack each field (so there are fewer trials than documents), and the
  simulated engine fails to produce 3 % of the fields it should, so that coverage has something to measure.

## How the simulated engine behaves

Each field is read correctly with probability `accuracy` and then accepted (rather than sent to review)
with probability `accept`, independently of whether it is correct; it is missing with probability 0.03.
Characters are corrupted at rate `cer`, six times as often in one document in ten (errors in real
engines come in bursts, which is what makes a document-level bootstrap necessary). Latency is
log-normal. The parameters are in `scenarios.json`.

## The four runs

Run from the repository root; `expected/` holds the full output of each.

| contract | verdict | exit | what it shows |
|---|---|---|---|
| `eval/contracts/invoice-fields-v1.yaml` | inconclusive | 3 | a careful engine; 120 documents cannot prove 99.9 % |
| `eval/contracts/receipt-fields-v1.yaml` | fail | 1 | an engine worse than its baseline and under its promises |
| `eval/contracts/id-mrz-v1.yaml` | fail | 1 | MRZ accuracy under 99.9 %, proven by the data |
| `eval/fixtures/synthetic-v0/demo-lenient-invoice.yaml` | pass | 0 | thresholds that 120 documents can prove |

```sh
cargo run --release -p wanjiru-cli -- eval \
  --contract eval/contracts/invoice-fields-v1.yaml \
  --truth eval/fixtures/synthetic-v0/truth.jsonl \
  --pred eval/fixtures/synthetic-v0/predictions.jsonl \
  --baseline eval/fixtures/synthetic-v0/baseline.jsonl
echo $?   # 3
```

The *inconclusive* run is the one to read twice. The engine is good, and the report says so with intervals;
what it refuses to do is call 120 documents proof of a 99.9 % promise. That is the reason for the golden-set
plan (`wanjiru eval plan`, about 360 documents per class).

## Checks that use these files

- `crates/wanjiru-cli/tests/transcripts.rs` runs the real binary on them and compares the transcripts.
- `crates/wanjiru-eval/tests/golden.rs` compares the library's report on them with the one an independent
  Python implementation produced (`crates/wanjiru-eval/tests/golden/fixtures/`).
- `wanjiru docir check predictions.jsonl baseline.jsonl` must report 600 valid documents.

## Regenerating

```sh
python3 tools/oracles/make_eval_vectors.py                          # rewrites these files and the golden vectors
WANJIRU_BLESS=1 cargo test -p wanjiru-cli --test transcripts        # rewrites expected/; read the diff
```

The generator is deterministic. It picks, for each class, the first seed from a base seed on that makes the
run show its intended verdict, and refuses to write anything if none does, so a change to the generator
cannot quietly change what these files demonstrate. That is a choice of demonstration data, not a
measurement.

## What this is not

It is not a benchmark, not a sample of real invoices, and not evidence about any engine. The first real
golden set (datasheet in `data/datasheets/`, plan in `docs/`) will replace it in every conclusion and stay
only as a test of the machinery.
