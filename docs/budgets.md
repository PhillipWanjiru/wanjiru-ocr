# Performance budgets

What "fast enough" means for each part of the system, and what has been measured so far. Every contract
under `crates/*/CONTRACT.md` points here from its *Budget* section.

Status of this file: **Phase 1.** Eight entries are measured, in nine rows (the pieces that exist: geometry,
maths, DocIR, evaluation, the command line). The rest are defined and wait for their crate. One budget is missed and is
reported as missed (see *B-docir-01*).

## Rules

1. **The target comes before the measurement.** An entry gets its number, and the reason for it, before
   the first run. A number chosen after seeing the result is a description, not a budget, and is labelled
   *recorded*.
2. **A miss stays a miss.** A target changes only by a dated line in the entry with the reason, and the
   earlier target and the miss stay in the history. Nothing is tightened or loosened to turn a red row
   green.
3. **Report a middle and a spread, never a single time.** Every figure here is the median of repeated runs
   of the work, with the fastest and slowest repetition, and each program run is repeated three times on
   its own (see *Measuring*).
4. **Say where it ran.** A timing without its machine line is not quoted anywhere. The machine line is
   printed by the program itself.
5. **Release builds only.** A debug build is never timed.
6. **A regression rule needs a quiet machine.** The contract for `wanjiru-math` says a regression of more
   than 5 % blocks a merge once the reference machine is fixed (Phase 10). Until then nothing blocks;
   a drop beyond 5 % on one machine in one session is looked at by hand. See *The 5 % rule and noise*.
7. **No budget is met by dropping work.** A faster result that skips a check, a validation or a
   verification does not count.

## Measuring

```sh
cargo build --release -p wanjiru-cli --bins --examples
./target/release/examples/budgets
```

`--bins` matters: `B-cli-01` starts the `wanjiru` binary that is already built, and without it the program
can time an out-of-date build without saying so. The program prints the machine line and one table. Run it
three times with nothing else running on the machine; do not run it next to `cargo`, a test run, the
mutation check (`tools/oracles/mutants.py`) or the coverage survey. Entries that exist only in a later
phase are measured by that phase's benchmark and added here with the same columns.

## Where the numbers come from, and where they do not

All figures below were measured in the development sandbox, **not** on the reference machine, which does not
exist yet:

```
Machine: Intel(R) Xeon(R) Processor @ 2.10GHz; 2 logical CPUs; linux x86_64
Build: release
```

The sandbox is a shared virtual machine. It reported a processor at 2.8 GHz earlier in the phase and 2.1 GHz
for this run, which is a reason to compare only numbers measured in the same session. They are good for
finding a change that costs a factor of two, and for ranking the parts of one pipeline. They are not
good for quoting a latency to anyone, and they must not appear in a datasheet, a release note or a claim
about the engine.

## Measured in Phase 1

Three separate runs of the program on an idle machine. *Median* is the median of the three runs' medians;
*spread* is the smallest and largest of those three medians. A spread wider than about 5 % of the median
means the machine was noisy and the digit after it means little.

| ID | What | Target | Median | Spread | Status |
|---|---|---|---|---|---|
| B-types-01 | `Affine::then`, per call | none set | 10.0 ns | 9.8 to 10.0 ns | recorded |
| B-types-01 | `Affine::apply`, per call (includes the two multiplications that keep the point bounded) | none set | 9.9 ns | 9.9 to 10.0 ns | recorded |
| B-math-01 | Myers edit distance, 5,000 x 5,000 symbols over 26 letters | none set | 2.02 ms | 2.01 to 2.46 ms | recorded |
| B-math-02 | Clopper-Pearson interval, k = 50,000 of n = 10^5, 95 % (the middle of the range, where the search is longest) | none set | 4.34 ms | 4.24 to 4.41 ms | recorded |
| B-math-03 | SHA-256 over 16 MiB (about 246 MB/s, no hardware acceleration, by design) | none set | 68.19 ms | 67.56 to 69.21 ms | recorded |
| B-math-04 | `exp`, one million arguments in [-30, 30] (about 86 million per second) | none set | 11.57 ms | 10.41 to 11.63 ms | recorded |
| B-docir-01 | parse, validate and canonicalise one document of 1,000,484 bytes | **below 20 ms** | **23.62 ms** | 23.32 to 23.94 ms | **missed**, by 3.6 ms (18 %) |
| B-eval-01 | `evaluate`, 360 invoices with a baseline, 2,000 resamples | **under 2 s** | 87.12 ms | 86.85 to 92.47 ms | met, with a margin of about 23 times |
| B-cli-01 | `wanjiru version`, process start to exit (p95 between 1.09 and 1.80 ms; 201 runs per program run) | none set | 1.22 ms | 0.93 to 1.38 ms | recorded |

"None set" means the contract names the entry and asks for it to be recorded, but gives no number. Those
rows are a baseline: they say what a later change is compared with, and they are the first numbers that the
Phase 10 regression rule will use.

### B-docir-01: a miss, and what is known about it

The target (below 20 ms for a 1 MB document) was written into the contract before anything was measured. The
measured median is 23.62 ms over three runs, 3.6 ms or 18 % too slow. The three stages, each timed alone on
the already parsed value where that applies:

| Stage | Median of three runs | Spread |
|---|---|---|
| strict parse | 9.06 ms | 8.73 to 12.56 ms |
| schema validation | 8.42 ms | 7.20 to 8.62 ms |
| canonical form (RFC 8785) | 5.64 ms | 5.40 to 5.76 ms |

The stages add up to about 23.1 ms, which agrees with the total of 23.6 ms. The whole pipeline therefore
runs at about 42 MB/s, strict parsing alone at about 110 MB/s.

Two things were done before this measurement and are in the code now: an integer shortcut in the number
formatter (checked against the general path by a test and, for the general path, against Node's `JSON.stringify`
as the independent oracle), and a validator that extends one shared path buffer instead of allocating a new string for every key and index.
Figures reported earlier in this phase (89 ms before those changes and 57 ms after) were taken while other jobs
were using the same two processors; they are not comparable with this table and are not used. The code from
before the changes was not measured again, so this file makes **no claim** about how much the changes gained.

What would close the gap, in order of expected effect, each to be measured before it is believed:

1. **Compile the validator.** Validation walks the schema as data on every call. Turning the schema into a
   tree of checks once would remove most of its 8.4 ms. If the cost halved, the total would be about 19.4 ms:
   this is arithmetic on the table, not a result.
2. **Reduce allocation in the parser.** Strings and object keys are copied; borrowing from the input would
   save allocation and memory traffic.
3. **Check the target against real documents.** One megabyte is a deliberately large DocIR. In Phase 3 the
   first real pages will show how large a page's DocIR is. If a page is a few tens of kilobytes, the
   meaningful budget is per page, and Phase 3 restates it per page with the measured size and the reason.
   This entry keeps its miss either way (rule 2).

Nothing about the miss is urgent: no code depends on this path being fast yet, and being wrong about
the shape of the data is more costly than being slow.

### B-eval-01: a target that was easy

The target of under 2 s was met by a factor of about 23, so it was too loose to catch a regression. It is
not tightened here (rule 2), because tightening it after seeing the result would be choosing the target by
the measurement. The baseline above is the number the regression rule will use. The timing is of the
machinery only: the verdict of that fixture run (fail) is a property of the synthetic fixture, which was
built to show each verdict, and is not evidence about any engine.

### The 5 % rule and noise

Across the three runs, the median of one entry moved by 2 % (`B-types-01`) up to 22 % (`B-math-01`, 2.01 to
2.46 ms) and 49 % (`B-cli-01`, which starts a process). A merge gate that fires at 5 % on a machine like this
would block on noise. So the rule is not a threshold on one number: in Phase 10 it becomes a comparison of
repeated runs on the fixed machine, blocking only when the lower end of a bootstrap interval for the ratio
of medians is above 1.05 (the same method the evaluation harness uses for paired relative changes). Until that
exists, rule 6 stands: nothing blocks, and a suspicious drop is looked at by hand.

## Defined, waiting for their crate

Each row names what will be measured, when, and where the target comes from. **No target is invented here.**
Where the contract already gives a number it is quoted. Every other entry gets its number, with its
derivation, in the first change that adds the benchmark, before the first measurement (rule 1).

| ID | Crate | What is measured | From | Target |
|---|---|---|---|---|
| B-simd-NN (one per kernel) | `wanjiru-simd` | scalar time, vector time, speed-up | Phase 2 | at least 2x faster than scalar on the reference machine, or the kernel is deleted (contract) |
| B-imgproc-NN (one per operation) | `wanjiru-imgproc` | milliseconds per megapixel; bytes of scratch memory per megapixel | Phase 2 | set before the first measurement |
| B-codecs-01 | `wanjiru-codecs` | decode time and peak memory per format at 12 megapixels | Phase 2 | set before the first measurement; the memory limit is enforced by the worker, not assumed |
| B-pdf-01 | `wanjiru-pdf` | rasterisation time per page at 200 and 300 dpi; peak memory | Phase 2 | set before the first measurement |
| B-worker-01 | `wanjiru-worker` | start-up time; per-job round-trip overhead | Phase 2 | set before the first measurement |
| B-detect-01 | `wanjiru-detect` | detection latency per megapixel; post-processing share of the total | Phase 3 | set before the first measurement |
| B-recognize-01 | `wanjiru-recognize` | latency per line and per page; strip height and width limits | Phase 3 | set before the first measurement |
| B-decode-01 | `wanjiru-decode` | time per line for beam widths 1, 8 and 32; allocations per line | Phase 3 | set before the first measurement |
| B-models-01 | `wanjiru-models` | open and verify a 50 MB package | Phase 3 | set before the first measurement |
| B-runtime-01 | `wanjiru-runtime` | latency and peak memory per model; model load time | Phase 3 | set before the first measurement |
| B-emit-01 | `wanjiru-emit` | emission time per 1,000 lines for each format | Phase 3 | set before the first measurement |
| B-engine-01 | `wanjiru-engine` | latency per page by class at p50 and p95; peak memory per page | Phase 3 | the quality contracts: invoice p50 1,000 ms and p95 2,500 ms; receipt p50 800 ms and p95 2,000 ms; identity document p50 1,000 ms and p95 3,000 ms. The split among stages is written at the start of Phase 3 |
| B-fields-01 | `wanjiru-fields` | extraction time per page for the three schemas | Phase 5 | set before the first measurement |
| B-calibrate-01 | `wanjiru-calibrate` | calibration application cost per field | Phase 6 | set before the first measurement |
| B-barcode-01 | `wanjiru-barcode` | scan time per megapixel with and without a hint region | Phase 7 | set before the first measurement |
| B-layout-01 | `wanjiru-layout` | layout time per page by number of lines | Phase 7 | set before the first measurement |
| B-ffi-01 | `wanjiru-ffi` | call overhead per entry point | Phase 8 | set before the first measurement |
| B-py-01 | `wanjiru-py` | overhead of the binding over a direct call | Phase 8 | set before the first measurement |
| B-server-01 | `wanjiru-server` | overhead per request over a direct library call | Phase 8 | set before the first measurement |

The facade crate `wanjiru` has no entry: it has no code of its own and re-exports inline, so its overhead is
zero by construction. `B-engine-01` is the one that matters most to a person, because it is the latency they
feel. The quality contracts give it a ceiling, and the stage budgets of Phase 3 have to add up to
that ceiling, so each stage is a share of a number that already exists and is not a figure in isolation.

## The reference machine

The reference machine is an open decision of Phase 10: one named processor, memory, operating system and
power setting, with the fan and the clock behaviour recorded, used for every number that appears in a release
note. Until it is chosen:

- sandbox figures are labelled as such, as above;
- nothing in this file is promised to a user;
- the choice should be a machine the owner has access to for years (an open input: first operating system),
  because a regression rule is only worth having if the machine under it stays the same.
