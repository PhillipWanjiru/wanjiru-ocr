#!/usr/bin/env python3
"""Mutation check of the evaluation harness: does anything notice when the arithmetic is wrong?

A test suite that passes proves little until it is shown to fail on a wrong program. This script makes
17 deliberate faults in `crates/wanjiru-eval` one at a time (a missing correction, a swapped weight, a
boundary off by one, a counter wired to the wrong field) and runs the tests against each. A fault is

  - "killed by unit tests"   if a test inside the crate fails,
  - "killed by the oracle"   if only `tests/golden.rs` fails, which compares the library's reports with
                             the independent implementation in `tools/oracles/eval_oracle.py`,
  - "survived"               if nothing fails. A survivor is a gap in the tests, and the script exits 1.

The comparison between the first two is the point: faults that only the oracle catches are faults the
unit tests, written by the same hand as the code, could not see.

Usage (from anywhere; it edits the sources of the crate in place and always restores them):

    python3 tools/oracles/mutants.py            # all mutants
    python3 tools/oracles/mutants.py MRZ IBAN   # those whose description contains a word

It needs a working `cargo test` for the workspace and takes a few minutes: each mutant is a rebuild.
Each pattern must occur exactly once in its file; a pattern that no longer does is reported as stale
and fails the run, so that a refactor cannot quietly turn this check into a no-op.
"""

import atexit
import os
import pathlib
import subprocess
import sys

REPOSITORY = pathlib.Path(__file__).resolve().parents[2]
CRATE = REPOSITORY / "crates" / "wanjiru-eval"

# (file under src/, text to replace, replacement, what the fault is)
MUTANTS = [
    ("intervals.rs", "((n / (n - 1.0)) * residual_squares).sqrt()", "((n / n) * residual_squares).sqrt()", "standard error without the n/(n-1) correction"),
    ("intervals.rs", "quantile_sorted(&values, alpha / 2.0),\n        quantile_sorted(&values, 1.0 - alpha / 2.0),", "quantile_sorted(&values, alpha),\n        quantile_sorted(&values, 1.0 - alpha),", "one-sided quantile levels"),
    ("intervals.rs", "(ratio - high_q * se, ratio - low_q * se)", "(ratio + low_q * se, ratio + high_q * se)", "bootstrap-t interval not reflected"),
    ("intervals.rs", "if resampled_x == 0 {\n            continue;\n        }", "if resampled_x == 0 {\n            resampled_x = 1;\n        }", "empty resamples not skipped"),
    ("intervals.rs", "estimate: ratio.estimate - 1.0,", "estimate: ratio.estimate,", "relative change forgets the -1"),
    ("run.rs", "seed: resampling.seed ^ PAIRED_STREAM,", "seed: resampling.seed,", "paired comparison on the same random stream"),
    ("run.rs", "p95: quantile(&milliseconds, 0.95, resampling.level),", "p95: quantile(&milliseconds, 0.9, resampling.level),", "p95 computed at 0.9"),
    ("fields.rs", "(Some(_), Some(_), None) => t.reviewed += 1,", "(Some(_), Some(_), None) => t.absent += 1,", "reviewed field counted as absent"),
    ("fields.rs", "if field.kind == gate.kind && field.value == *expected {", "if field.value == *expected {", "field kind not compared"),
    ("cer.rs", ".join(\"\\n\")", ".join(\" \")", "lines joined by a space"),
    ("validators.rs", "const WEIGHTS: [u32; 3] = [7, 3, 1];", "const WEIGHTS: [u32; 3] = [7, 1, 3];", "MRZ check-digit weights"),
    ("validators.rs", "(year % 4 == 0 && year % 100 != 0) || year % 400 == 0", "year % 4 == 0", "leap years"),
    ("validators.rs", "if remainder == 1 {", "if remainder == 0 {", "IBAN remainder"),
    ("gates.rs", "Comparison::AtMost if upper <= threshold => Verdict::Pass,", "Comparison::AtMost if upper < threshold => Verdict::Pass,", "strict at-most (boundary)"),
    ("plan.rs", "documents: fields.div_ceil(u64::from(fields_per_document)),", "documents: fields / u64::from(fields_per_document),", "documents rounded down"),
    ("report.rs", "(\"accepted_invalid\", count(t.accepted_invalid)),", "(\"accepted_invalid\", count(t.accepted)),", "report member wired to the wrong counter"),
    ("yaml.rs", "if c == '#' && previous.is_none_or(char::is_whitespace) {", "if c == '#' {", "comment starts anywhere"),
]


def failures(output):
    """Split the names of failed tests into those of the crate's own tests and those of tests/golden.rs."""
    section = None
    unit, golden = [], []
    for line in output.splitlines():
        if line.startswith("     Running unittests"):
            section = unit
        elif line.startswith("     Running tests/golden.rs"):
            section = golden
        elif line.startswith("     Running"):
            section = None
        elif line.startswith("test ") and line.endswith(" FAILED") and section is not None:
            section.append(line.split()[1])
    return unit, golden


def run_tests():
    result = subprocess.run(
        ["cargo", "test", "--offline", "-p", "wanjiru-eval", "--lib", "--test", "golden", "--no-fail-fast"],
        cwd=REPOSITORY,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,  # one stream, so that "Running ..." lines stay in front of their tests
        text=True,
        timeout=1800,
        env=dict(os.environ),
    )
    return result.returncode, result.stdout


def main():
    words = [w.lower() for w in sys.argv[1:]]
    originals = {}

    def restore():
        for path, text in originals.items():
            if path.read_text() != text:
                path.write_text(text)

    atexit.register(restore)

    chosen = [m for m in MUTANTS if not words or any(w in m[3].lower() for w in words)]
    unit_only, oracle_only, survivors, stale, broken, unattributed = [], [], [], [], [], []
    for number, (name, old, new, what) in enumerate(chosen, 1):
        path = CRATE / "src" / name
        original = originals.setdefault(path, path.read_text())
        label = f"[{number:2}/{len(chosen)}] {name}: {what}"
        if original.count(old) != 1:
            print(f"{label}\n      STALE: the pattern occurs {original.count(old)} times, expected once")
            stale.append(what)
            continue
        path.write_text(original.replace(old, new))
        try:
            code, output = run_tests()
        finally:
            path.write_text(original)
        if "error: could not compile" in output or "error[E" in output:
            print(f"{label}\n      DID NOT COMPILE: the fault must be a valid program")
            broken.append(what)
            continue
        unit, golden = failures(output)
        if code == 0:
            print(f"{label}\n      SURVIVED")
            survivors.append(what)
        elif unit:
            print(f"{label}\n      killed by unit tests ({len(unit)}; first: {unit[0]})" + (f" and the oracle ({len(golden)})" if golden else ""))
            unit_only.append(what)
        elif golden:
            print(f"{label}\n      killed by the oracle only ({len(golden)}; first: {golden[0]})")
            oracle_only.append(what)
        else:
            print(f"{label}\n      the tests failed, but not in a way this script can attribute (a crash? read the output of cargo test)")
            unattributed.append(what)

    print()
    print(f"{len(chosen)} faults: {len(unit_only)} killed by unit tests, "
          f"{len(oracle_only)} killed only by the oracle comparison, {len(survivors)} survived, "
          f"{len(stale)} stale, {len(broken)} did not compile, {len(unattributed)} unattributed")
    for what in oracle_only:
        print(f"  oracle only: {what}")
    for what in survivors:
        print(f"  SURVIVED:    {what}")
    return 0 if not (survivors or stale or broken or unattributed) else 1


if __name__ == "__main__":
    sys.exit(main())
