#!/usr/bin/env python3
"""Reference data for crates/wanjiru-eval, produced by eval_oracle.py and PyYAML.

Writes, under crates/wanjiru-eval/tests/golden/:
  validators.txt  amount/date/IBAN/MRZ verdicts (own implementations in eval_oracle.py)
  intervals.jsonl bootstrap intervals, bit for bit (same generator, same operation order)
  proportions.jsonl  Clopper-Pearson intervals from SciPy's beta quantiles
  ranks.jsonl     order-statistic ranks by exact rational arithmetic
  quantiles.jsonl type-7 quantiles (cross-checked against NumPy)
  plan.jsonl      sample-size plans and detectable differences (SciPy's normal quantile)
  yaml.jsonl      PyYAML's reading of YAML texts, as JSON
  cases/NAME/     whole evaluations: inputs, settings and the report body computed by the oracle

The output is deterministic. Usage: python3 tools/oracles/make_eval_vectors.py [output-directory]
"""
import json
import math
import os
import random
import shutil
import sys
from dataclasses import dataclass

import numpy as np
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import eval_oracle as oracle  # noqa: E402

OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "..", "crates", "wanjiru-eval", "tests", "golden")
os.makedirs(OUT, exist_ok=True)


def write_lines(name, lines):
    with open(os.path.join(OUT, name), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{name}: {len(lines)} records")


def number(x):
    """A float for JSON: infinities become strings, because JSON has none."""
    if x is None:
        return None
    if x == math.inf:
        return "inf"
    if x == -math.inf:
        return "-inf"
    return x


def dumps(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False)


# ---------------------------------------------------------------- validators
def mrz_td3(rnd, personal_empty=False):
    def alnum(n, alphabet="ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"):
        return "".join(rnd.choice(alphabet) for _ in range(n))

    def date():
        return f"{rnd.randrange(0, 100):02d}{rnd.randrange(1, 13):02d}{rnd.randrange(1, 29):02d}"

    name = (alnum(rnd.randrange(3, 10), "ABCDEFGHIJKLMNOPQRSTUVWXYZ") + "<<" + alnum(rnd.randrange(3, 10), "ABCDEFGHIJKLMNOPQRSTUVWXYZ"))
    line1 = ("P<" + "UTO" + name).ljust(44, "<")
    number_ = alnum(rnd.randrange(5, 10)).ljust(9, "<")
    dob, expiry = date(), date()
    personal = ("<" * 14) if personal_empty else alnum(rnd.randrange(1, 15)).ljust(14, "<")
    personal_check = "<" if personal_empty else str(oracle.check_digit(personal))
    line2 = (number_ + str(oracle.check_digit(number_)) + "UTO" + dob + str(oracle.check_digit(dob)) + rnd.choice("MF<")
             + expiry + str(oracle.check_digit(expiry)) + personal + personal_check)
    composite = line2[0:10] + line2[13:20] + line2[21:43]
    return line1 + "\n" + line2 + str(oracle.check_digit(composite))


def mrz_td2(rnd):
    def alnum(n, alphabet="ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"):
        return "".join(rnd.choice(alphabet) for _ in range(n))

    line1 = ("I<UTO" + alnum(8, "ABCDEFGHIJKLMNOPQRSTUVWXYZ") + "<<" + alnum(6, "ABCDEFGHIJKLMNOPQRSTUVWXYZ")).ljust(36, "<")
    number_ = alnum(rnd.randrange(5, 10)).ljust(9, "<")
    dob = f"{rnd.randrange(0, 100):02d}{rnd.randrange(1, 13):02d}{rnd.randrange(1, 29):02d}"
    expiry = f"{rnd.randrange(0, 100):02d}{rnd.randrange(1, 13):02d}{rnd.randrange(1, 29):02d}"
    optional = alnum(rnd.randrange(0, 8)).ljust(7, "<")
    line2 = (number_ + str(oracle.check_digit(number_)) + "UTO" + dob + str(oracle.check_digit(dob)) + rnd.choice("MF<")
             + expiry + str(oracle.check_digit(expiry)) + optional)
    composite = line2[0:10] + line2[13:20] + line2[21:35]
    return line1 + "\n" + line2 + str(oracle.check_digit(composite))


def mrz_td1(rnd):
    def alnum(n, alphabet="ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"):
        return "".join(rnd.choice(alphabet) for _ in range(n))

    number_ = alnum(rnd.randrange(5, 10)).ljust(9, "<")
    optional1 = alnum(rnd.randrange(0, 15)).ljust(15, "<")
    line1 = "I<UTO" + number_ + str(oracle.check_digit(number_)) + optional1
    dob = f"{rnd.randrange(0, 100):02d}{rnd.randrange(1, 13):02d}{rnd.randrange(1, 29):02d}"
    expiry = f"{rnd.randrange(0, 100):02d}{rnd.randrange(1, 13):02d}{rnd.randrange(1, 29):02d}"
    optional2 = alnum(rnd.randrange(0, 11)).ljust(11, "<")
    line2 = dob + str(oracle.check_digit(dob)) + rnd.choice("MF<") + expiry + str(oracle.check_digit(expiry)) + "UTO" + optional2
    composite = line1[5:30] + line2[0:7] + line2[8:15] + line2[18:29]
    line2 += str(oracle.check_digit(composite))
    line3 = (alnum(8, "ABCDEFGHIJKLMNOPQRSTUVWXYZ") + "<<" + alnum(6, "ABCDEFGHIJKLMNOPQRSTUVWXYZ")).ljust(30, "<")
    return "\n".join((line1, line2, line3))


# ISO 3166-1 reserves AA, QM-QZ, XA-XZ and ZZ for private use, and no IBAN country uses them (the IBAN
# registry's only X-code is XK, left out here). An IBAN made with one of these prefixes cannot belong to
# a real account, whatever follows it.
PRIVATE_USE_PREFIXES = ["AA", "ZZ"] + ["Q" + c for c in "MNOPQRSTUVWXYZ"] + ["X" + c for c in "ABCDEFGHIJLMNOPQRSTUVWXYZ"]


def make_iban(rnd):
    country = rnd.choice(PRIVATE_USE_PREFIXES)
    bban = "".join(rnd.choice("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789") for _ in range(rnd.randrange(11, 31)))
    digits = "".join(str(int(c, 36)) for c in bban + country) + "00"
    check = 98 - int(digits) % 97
    return f"{country}{check:02d}{bban}"


def mutate(rnd, text, alphabet):
    if not text:
        return rnd.choice(alphabet)
    kind = rnd.randrange(5)
    i = rnd.randrange(len(text))
    if kind == 0:
        return text[:i] + rnd.choice(alphabet) + text[i + 1:]
    if kind == 1:
        return text[:i] + rnd.choice(alphabet) + text[i:]
    if kind == 2:
        return text[:i] + text[i + 1:]
    if kind == 3 and len(text) > 1:
        j = min(i + 1, len(text) - 1)
        return text[:i] + text[j] + text[i] + text[j + 1:] if j > i else text
    return text.lower() if rnd.random() < 0.3 else text[:i]


def validator_vectors():
    rnd = random.Random(2026_10_31)
    cases = {"amount": [], "date": [], "iban": [], "mrz": [], "text": ["", "a", "x y", " "]}
    cases["amount"] += ["", "-", ".5", "5.", "01", "-0", "-0.00", "0", "0.0", "+5", "1,234.50", "1 234", "1e3", "12.3.4",
                        "１２", "1234567890123456789", "123456789012345678", "-123456789012345678", "0.000001",
                        "100000000000000000", "-1", "-0.1", "00", "0.", "1.", "1.0", "999999999999999999", "9999999999999999999"]
    for _ in range(300):
        whole = str(rnd.choice([0, rnd.randrange(1, 10), rnd.randrange(10, 10**6), rnd.randrange(10**6, 10**12)]))
        text = whole + (("." + "".join(rnd.choice("0123456789") for _ in range(rnd.randrange(1, 7)))) if rnd.random() < 0.7 else "")
        cases["amount"].append(("-" if rnd.random() < 0.2 else "") + text)
    cases["amount"] += [mutate(rnd, a, "0123456789.-,+ eE") for a in list(cases["amount"])[40:340] for _ in range(1)]
    cases["date"] += ["2026-02-29", "2024-02-29", "2000-02-29", "1900-02-29", "2100-02-29", "1900-01-01", "2100-12-31",
                      "1899-12-31", "2101-01-01", "2026-13-01", "2026-00-10", "2026-04-31", "2026-4-3", "26-10-29",
                      "2026/10/29", "", "2026-10-2x", "2026-10-29 ", " 2026-10-29", "２０２６-10-29", "0000-01-01", "9999-12-31"]
    for _ in range(300):
        cases["date"].append(f"{rnd.randrange(1890, 2111):04d}-{rnd.randrange(0, 14):02d}-{rnd.randrange(0, 33):02d}")
    cases["date"] += [mutate(rnd, d, "0123456789-/ ") for d in list(cases["date"])[30:200]]
    cases["iban"] += ["DE89370400440532013000", "GB82WEST12345698765432", "FR1420041010050500013M02606", "NL91ABNA0417164300",
                      "CH9300762011623852957", "BE68539007547034", "NO9386011117947", "MT84MALT011000012345MTLCAST001S",
                      "de89370400440532013000", "DE89 3704 0044 0532 0130 00", "DE", "", "1289370400440532013000"]
    for _ in range(250):
        cases["iban"].append(make_iban(rnd))
    cases["iban"] += [mutate(rnd, i, "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ") for i in list(cases["iban"])[20:270] for _ in range(2)]
    cases["mrz"] += ["P<UTOERIKSSON<<ANNA<MARIA<<<<<<<<<<<<<<<<<<<\nL898902C36UTO7408122F1204159ZE184226B<<<<<10",
                     "I<UTOERIKSSON<<ANNA<MARIA<<<<<<<<<<<\nD231458907UTO7408122F1204159<<<<<<<6",
                     "I<UTOD231458907<<<<<<<<<<<<<<<\n7408122F1204159UTO<<<<<<<<<<<6\nERIKSSON<<ANNA<MARIA<<<<<<<<<<"]
    for _ in range(80):
        cases["mrz"].append(mrz_td3(rnd, personal_empty=rnd.random() < 0.3))
        cases["mrz"].append(mrz_td2(rnd))
        cases["mrz"].append(mrz_td1(rnd))
    cases["mrz"] += [mutate(rnd, m, "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789<\n ") for m in list(cases["mrz"])[3:243] for _ in range(3)]
    cases["mrz"] += ["", "\n", "ABC", "P<UTO\nX", "<" * 44 + "\n" + "<" * 44]
    lines = []
    for kind, values in cases.items():
        for value in values:
            verdict = "ok" if oracle.VALIDATORS[kind](value) else "err"
            lines.append(f"{kind} {verdict} {value.encode('utf-8').hex() or '-'}")
    ok = sum(1 for line in lines if line.split()[1] == "ok")
    print(f"validators: {ok} valid, {len(lines) - ok} invalid")
    write_lines("validators.txt", lines)


# ---------------------------------------------------------------- resampling intervals
def interval_vectors():
    rnd = random.Random(31)
    lines = []
    errors = 0
    for case in range(260):
        n = rnd.choice([2, 3, 5, 8, 13, 20, 34, 55, 80])
        shape = rnd.randrange(4)
        x = [rnd.randrange(0, 400) if shape == 0 else rnd.randrange(30, 400) for _ in range(n)]
        if shape == 2:
            y = [min(xi, int(rnd.paretovariate(1.2))) for xi in x]  # heavy tail
        elif shape == 3:
            y = [0] * n  # perfect engine
        else:
            y = [sum(1 for _ in range(xi) if rnd.random() < rnd.choice([0.005, 0.02, 0.1])) for xi in x]
        level = rnd.choice([0.9, 0.95, 0.99])
        resamples = rnd.choice([100, 250, 400])
        seed = rnd.randrange(0, 2**64)
        method = rnd.choice(["bootstrap-t", "percentile"])
        kind = "ratio" if case < 160 else "change"
        # seeds are 64-bit: as text, because JSON numbers are doubles
        entry = {"kind": kind, "level": level, "resamples": resamples, "seed": str(seed), "method": method}
        try:
            if kind == "ratio":
                entry.update(y=y, x=x)
                est = oracle.ratio_interval(y, x, level, resamples, seed, method)
            else:
                base = [max(0, xi // 40 + rnd.randrange(-1, 3)) for xi in x] if shape != 3 else [0] * n
                new = [max(0, b + rnd.randrange(-1, 3)) for b in base] if shape != 3 else y
                if rnd.random() < 0.15:
                    base = [0] * n
                entry.update(new=new, base=base)
                est = oracle.relative_change(new, base, level, resamples, seed, method)
            entry["expected"] = {"estimate": number(est[0]), "lower": number(est[1]), "upper": number(est[2])}
        except oracle.Unusable:
            entry["error"] = True
            errors += 1
        lines.append(dumps(entry))
    # settings the Rust crate must refuse
    for settings in ({"level": 1.0, "resamples": 200}, {"level": 0.0, "resamples": 200}, {"level": 0.95, "resamples": 99},
                     {"level": 0.95, "resamples": 1_000_001}):
        lines.append(dumps({"kind": "ratio", "y": [1, 2, 3], "x": [10, 20, 30], "seed": "1", "method": "bootstrap-t", "error": True, **settings}))
    write_lines("intervals.jsonl", lines)
    print(f"intervals: {errors} unusable by design")


def proportion_vectors():
    rnd = random.Random(32)
    lines = []
    for _ in range(500):
        n = rnd.choice([0, 1, 2, 3, 5, 10, 25, 100, 360, 1000, 3575])
        k = rnd.choice([0, n, max(0, n - 1), min(n, 1), rnd.randrange(0, n + 1)]) if n else 0
        level = rnd.choice([0.8, 0.9, 0.95, 0.99, 0.999])
        estimate, lower, upper = oracle.clopper_pearson(k, n, level)
        lines.append(dumps({"k": k, "n": n, "level": level, "expected": [estimate, lower, upper]}))
    write_lines("proportions.jsonl", lines)


def rank_vectors():
    rnd = random.Random(33)
    lines = []
    for _ in range(120):
        n = rnd.choice([1, 2, 5, 10, 20, 21, 59, 60, 100, 200, 359, 360, 500])
        q = rnd.choice([0.5, 0.9, 0.95, 0.99, 0.25, 0.75, 0.1])
        level = rnd.choice([0.9, 0.95, 0.99])
        lower, upper = oracle.rank_interval(n, q, level)
        lines.append(dumps({"n": n, "q": q, "level": level, "expected": [lower, upper]}))
    write_lines("ranks.jsonl", lines)


def quantile_vectors():
    rnd = random.Random(34)
    lines = []
    for _ in range(150):
        n = rnd.choice([1, 2, 3, 7, 20, 100, 359])
        values = sorted(round(rnd.lognormvariate(5, 0.8), rnd.choice([0, 1, 3])) for _ in range(n))
        q = rnd.choice([0.5, 0.95, 0.99, 0.01, rnd.random()])
        expected = oracle.quantile7(values, q)
        numpy_value = float(np.quantile(np.array(values), q, method="linear"))
        assert abs(expected - numpy_value) <= 1e-12 * max(1.0, abs(expected)), (values, q, expected, numpy_value)
        lines.append(dumps({"values": values, "q": q, "expected": expected}))
    write_lines("quantiles.jsonl", lines)


def plan_vectors():
    rnd = random.Random(35)
    lines = []
    while len(lines) < 120:
        p = rnd.choice([0.9, 0.95, 0.99, 0.995, rnd.uniform(0.5, 0.9999)])
        margin = rnd.choice([0.0025, 0.005, 0.01, 0.02, rnd.uniform(0.001, 0.05)])
        confidence = rnd.choice([0.9, 0.95, 0.99])
        m = rnd.randrange(1, 31)
        icc = rnd.choice([0.0, 0.05, 0.15, 0.3, rnd.random() / 2])
        result = oracle.plan(p, margin, confidence, m, icc)
        z = result["z"]
        raw = z * z * p * (1 - p) / (margin * margin)
        scaled = raw * result["design_effect"]
        if any(abs(v - round(v)) < 1e-6 * max(1.0, v) for v in (raw, scaled)):
            continue  # too close to an integer for the last bit of a quantile to be irrelevant
        entry = {"p": p, "margin": margin, "confidence": confidence, "fields_per_document": m, "icc": icc, "expected": result}
        effective = rnd.choice([50.0, 358.0, 1532.0, rnd.uniform(10, 5000)])
        alpha, power = rnd.choice([(0.05, 0.8), (0.05, 0.9), (0.1, 0.8), (0.01, 0.95)])
        entry["difference"] = {"effective_n": effective, "alpha": alpha, "power": power,
                               "expected": oracle.minimum_detectable_difference(p, effective, alpha, power)}
        lines.append(dumps(entry))
    write_lines("plan.jsonl", lines)


# ---------------------------------------------------------------- YAML
def yaml_vectors():
    rnd = random.Random(36)
    words = ["alpha", "beta", "gamma", "delta", "x", "ключ", "größe", "naïve", "日本語", "with space", "a: b", "a # b", "-dash", "[x]",
             "{y}", "it's", 'say "hi"', "true", "false", "null", "~", "yes", "no", "on", "off", "123", "1.5", "0x1F", "007", "",
             " lead", "trail ", "colon:", ":colon", "# hash", "- item", "?", "!bang", "&anchor", "*alias", "%pct", "@at", "`tick", "a,b", "a|b",
             "http://example.org/a#frag", "12:30", "2026-10-31", ".5", "5.", "+1", "-1", "1_000", "0b101", "Null", "NULL", "True", "FALSE"]

    def scalar():
        r = rnd.randrange(6)
        if r in (0, 5):
            return rnd.choice(words)
        if r == 1:
            return rnd.randrange(-10**6, 10**6)
        if r == 2:
            return round(rnd.uniform(-1000, 1000), rnd.choice([0, 1, 3, 6])) + 0.0
        if r == 3:
            return rnd.choice([True, False, None])
        return rnd.choice([1e-5, 2.5e-7, 123456789.125, 1.5e20, -4.25e-3, 0.1, 100.0])

    def key():
        return rnd.choice([w for w in words if w != "" and w.strip() == w] + [f"k{rnd.randrange(100)}"])

    def node(depth, rich):
        r = rnd.randrange(10)
        if depth >= 3 or r < 4:
            return scalar()
        if r < 7:
            if rich:
                return [node(depth + 1, rich) for _ in range(rnd.randrange(1, 4))]
            return [scalar() for _ in range(rnd.randrange(1, 4))]
        return {key(): node(depth + 1, rich) for _ in range(rnd.randrange(1, 4))}

    lines = []
    for index in range(200):
        flow = index % 3 == 0
        value = {key(): node(1, flow) for _ in range(rnd.randrange(1, 5))}
        text = yaml.dump(value, default_flow_style=flow, allow_unicode=True, width=10**6, indent=rnd.choice([2, 2, 4]), sort_keys=False)
        lines.append(dumps({"yaml": text, "json": yaml.safe_load(text)}))
    handwritten = [
        "contract: x\nclass: invoice\ncorpus: golden/x\nmetrics:\n  field_exact_match:\n    amount: {min: 0.995, coverage: 0.95}\ngates:\n  regression_budget_cer_rel: 0.02\n",
        "a: 1\nb: [1, 2, {c: d}]\nc:\n  - x\n  - y\n# comment\nd: 'it''s'\ne: \"tab\\there\"\n",
        "---\nk: v\n",
        "- 1\n- two\n- [3, 4]\n",
        "empty:\nlist: []\nmap: {}\nnull_value: null\ntilde: ~\n",
    ]
    for text in handwritten:
        lines.append(dumps({"yaml": text, "json": yaml.safe_load(text)}))
    write_lines("yaml.jsonl", lines)


# ---------------------------------------------------------------- whole evaluations
LETTERS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .,:-/€éüßñ"


def random_text(rnd):
    return [("".join(rnd.choice(LETTERS) for _ in range(rnd.randrange(8, 40))).strip() or "x") for _ in range(rnd.randrange(2, 5))]


def corrupt(rnd, line, rate):
    out = []
    for c in line:
        r = rnd.random()
        if r < rate / 3:
            continue
        if r < 2 * rate / 3:
            out.append(rnd.choice(LETTERS))
            continue
        out.append(c)
        if r < rate:
            out.append(rnd.choice(LETTERS))
    return "".join(out)


def truth_value(rnd, kind):
    if kind == "amount":
        return f"{rnd.randrange(0, 5000)}.{rnd.randrange(0, 100):02d}"
    if kind == "date":
        return f"{rnd.randrange(2015, 2027)}-{rnd.randrange(1, 13):02d}-{rnd.randrange(1, 29):02d}"
    if kind == "iban":
        return make_iban(rnd)
    if kind == "mrz":
        return mrz_td3(rnd, personal_empty=rnd.random() < 0.2)
    return "".join(rnd.choice("abcdefghij ") for _ in range(rnd.randrange(3, 12))).strip() or "x"


def wrong_value(rnd, kind, value):
    """A plausible mistake; sometimes one that breaks the syntax of the kind."""
    if rnd.random() < 0.25:
        return rnd.choice(["12,50", "n/a", "2026-13-40", "DE00", "ABC"]) if kind != "text" else value + "?"
    digits = [i for i, c in enumerate(value) if c.isdigit()]
    if not digits:
        return value + "x"
    i = rnd.choice(digits)
    return value[:i] + str((int(value[i]) + 1) % 10) + value[i + 1:]


def docir(doc_id, klass, lines, fields, total_ms):
    sha = "0123456789abcdef" * 4
    line_objects = []
    for i, text in enumerate(lines):
        y = 10 * i
        line_objects.append({"id": f"l{i}", "text": text, "polygon": [[0, y], [100, y], [100, y + 8], [0, y + 8]],
                             "confidence": {"value": 0.9, "calibrated": False}, "words": []})
    field_objects = [{"name": name, "kind": kind, "value": value, "confidence": {"value": 0.9, "calibrated": False},
                      "decision": "accept" if accept else "review", "evidence": [], "lines": ["l0"] if lines else []}
                     for name, kind, value, accept in fields]
    return {"schema": "wanjiru.docir/0", "id": doc_id, "class": klass, "source": {"sha256": sha, "media_type": "image/png"},
            "pages": [{"index": 0, "size": {"w": 100, "h": 200}, "chain": {"from": "source", "steps": []}, "lines": line_objects}],
            "fields": field_objects,
            "run": {"engine": "oracle 0", "config_sha256": "1" * 64, "timing_ms": {} if total_ms is None else {"total": total_ms}}}


@dataclass
class Scenario:
    name: str
    klass: str
    n: int
    contract: str
    fields: dict
    seed: int
    truth_text: bool = True
    other: int = 0
    engine: dict = None
    baseline: dict = None
    level: float = 0.95
    resamples: int = 300
    method: str = "bootstrap-t"
    field_presence: float = 0.9
    truth_fields: list = None


def engine_output(rnd, document, spec, scenario, with_timing):
    """The (possibly absent) DocIR document an engine with quality `spec` produces for a truth record."""
    if rnd.random() < spec.get("missing", 0.0):
        return None
    rate = spec["cer"] * (6.0 if rnd.random() < 0.1 else 1.0)
    reference = document.get("text")
    lines = [corrupt(rnd, line, rate) for line in (reference.split("\n") if reference is not None else random_text(rnd))]
    fields = []
    for name, expected in document.get("fields", {}).items():
        kind = spec["kinds"].get(name)
        if kind is None or rnd.random() < spec.get("absent", 0.03):
            continue
        value = expected if rnd.random() < spec["accuracy"] else wrong_value(rnd, kind, expected)
        fields.append((name, kind, value, rnd.random() < spec["accept"]))
    for name, kind in spec["kinds"].items():  # a gated field the truth does not mention
        if name not in document.get("fields", {}) and rnd.random() < spec.get("spurious", 0.15):
            fields.append((name, kind, truth_value(rnd, kind), rnd.random() < 0.7))
    if rnd.random() < 0.05:  # a field nobody gates
        fields.append(("surplus", "text", "extra", rnd.random() < 0.5))
    total = None
    if with_timing and rnd.random() < spec.get("timing", 1.0):
        total = round(rnd.lognormvariate(spec["ms"], 0.5), 2)
    return docir(document["id"], document["class"], lines, fields, total)


def build_case(scenario):
    """The truth, predictions and baseline texts of a scenario, and the settings to evaluate them with."""
    rnd = random.Random(scenario.seed)
    kinds = {name: spec[0] for name, spec in scenario.fields.items()}
    truths = []
    for i in range(scenario.n):
        record = {"id": f"{scenario.klass[:3]}-{i:04d}", "class": scenario.klass}
        if scenario.truth_text:
            record["text"] = "\n".join(random_text(rnd))
        fields = {}
        for name in (scenario.truth_fields if scenario.truth_fields is not None else scenario.fields):
            kind = scenario.fields[name][0] if name in scenario.fields else "text"
            if rnd.random() < scenario.field_presence:
                fields[name] = truth_value(rnd, kind)
        record["fields"] = fields
        truths.append(record)
    for i in range(scenario.other):
        truths.append({"id": f"oth-{i:04d}", "class": "receipt" if scenario.klass != "receipt" else "invoice",
                       "text": "\n".join(random_text(rnd)), "fields": {}})
    rnd.shuffle(truths)

    def outputs(spec):
        spec = dict(spec, kinds=kinds)
        lines = []
        for record in truths:
            out = engine_output(rnd, record, spec, scenario, with_timing=spec.get("ms") is not None)
            if out is not None:
                lines.append(dumps(out))
        return "\n".join(lines) + "\n"

    truth_text = "\n".join(dumps(t) for t in truths) + "\n"
    predictions = outputs(scenario.engine)
    baseline = None if scenario.baseline is None else outputs(scenario.baseline)
    settings = {"level": scenario.level, "resamples": scenario.resamples, "seed": scenario.seed * 7 + 1, "method": scenario.method}
    return truth_text, predictions, baseline, settings


def make_case(scenario):
    truth_text, predictions, baseline, settings = build_case(scenario)
    report = oracle.evaluate(scenario.contract, truth_text, predictions, baseline, settings["level"], settings["resamples"],
                             settings["seed"], settings["method"])
    directory = os.path.join(OUT, "cases", scenario.name)
    os.makedirs(directory, exist_ok=True)
    files = {"contract.yaml": scenario.contract, "truth.jsonl": truth_text, "predictions.jsonl": predictions}
    if baseline is not None:
        files["baseline.jsonl"] = baseline
    for name, text in files.items():
        with open(os.path.join(directory, name), "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
    with open(os.path.join(directory, "settings.json"), "w", encoding="utf-8", newline="\n") as f:
        f.write(dumps(settings) + "\n")
    with open(os.path.join(directory, "expected.json"), "w", encoding="utf-8", newline="\n") as f:
        f.write(dumps(report) + "\n")
    gates = {g["verdict"] for g in report["gates"]}
    print(f"cases/{scenario.name}: {scenario.n} documents, verdict {report['verdict']}, gate verdicts {sorted(gates)}")
    return report


def contract_text(name, klass, fields, *, ece=True, conformal=True, latency=(1000, 2500), budget=0.02, status="draft"):
    lines = [f"contract: {name}", f"class: {klass}", f"status: {status}", f"corpus: golden/{name}", "metrics:"]
    if fields:
        lines.append("  field_exact_match:")
        for field, (kind, minimum, coverage) in fields.items():
            kind_part = "" if kind == field else f"kind: {kind}, "
            lines.append(f"    {field}: {{{kind_part}min: {minimum}, coverage: {coverage}}}")
    if ece:
        lines.append("  ece_char: {max: 0.02}")
    if conformal:
        lines.append("  conformal: {alpha: 0.01, coverage_tolerance: 0.01}")
    if latency:
        parts = []
        if latency[0] is not None:
            parts.append(f"p50_max: {latency[0]}")
        if latency[1] is not None:
            parts.append(f"p95_max: {latency[1]}")
        lines.append(f"  latency_ms: {{{', '.join(parts)}}}")
    lines.append("gates:")
    if budget is not None:
        lines.append(f"  regression_budget_cer_rel: {budget}")
    lines.append("  confidence_interval: bootstrap-by-document")
    return "\n".join(lines) + "\n"


def case_vectors():
    cases_dir = os.path.join(OUT, "cases")
    shutil.rmtree(cases_dir, ignore_errors=True)
    invoice_fields = {"total_amount": ("amount", 0.99, 0.9), "issue_date": ("date", 0.99, 0.9), "iban": ("iban", 0.995, 0.8)}
    invoice_contract = contract_text("invoice-oracle", "invoice", invoice_fields)
    strong = {"cer": 0.004, "accuracy": 0.995, "accept": 0.97, "ms": 5.5}
    weaker = {"cer": 0.02, "accuracy": 0.9, "accept": 0.9, "ms": 6.5}
    reports = {}
    reports["invoice-strong"] = make_case(Scenario("invoice-strong", "invoice", 160, invoice_contract, invoice_fields, seed=101,
                                                   other=12, engine=strong, baseline=weaker))
    reports["invoice-weak"] = make_case(Scenario("invoice-weak", "invoice", 120, invoice_contract, invoice_fields, seed=102,
                                                 engine=weaker, baseline=strong, resamples=400))
    receipt_fields = {"total": ("amount", 0.98, 0.9), "date": ("date", 0.98, 0.9)}
    reports["receipt-small"] = make_case(Scenario("receipt-small", "receipt", 14, contract_text("receipt-oracle", "receipt", receipt_fields),
                                                  receipt_fields, seed=103, engine=strong, baseline=weaker, field_presence=1.0))
    id_fields = {"mrz": ("mrz", 0.999, 0.9), "document_number": ("text", 0.99, 0.9)}
    reports["id-mrz"] = make_case(Scenario("id-mrz", "id", 90, contract_text("id-oracle", "id", id_fields, latency=(None, 3000), budget=None),
                                           id_fields, seed=104, engine={"cer": 0.002, "accuracy": 0.99, "accept": 0.95, "ms": 6.0},
                                           field_presence=0.97))
    reports["missing-outputs"] = make_case(Scenario("missing-outputs", "invoice", 100, invoice_contract, invoice_fields, seed=105,
                                                    engine=dict(strong, missing=0.3, absent=0.1, accept=0.8),
                                                    baseline=dict(weaker, missing=0.1), level=0.9))
    perfect = {"cer": 0.0, "accuracy": 1.0, "accept": 1.0, "ms": 5.0, "absent": 0.0, "spurious": 0.0}
    reports["zero-baseline-worse"] = make_case(Scenario("zero-baseline-worse", "invoice", 40, invoice_contract, invoice_fields, seed=106,
                                                        engine=weaker, baseline=perfect, field_presence=1.0))
    reports["zero-baseline-equal"] = make_case(Scenario("zero-baseline-equal", "invoice", 40, invoice_contract, invoice_fields, seed=107,
                                                        engine=perfect, baseline=perfect, field_presence=1.0))
    reports["no-reference-text"] = make_case(Scenario("no-reference-text", "invoice", 60, invoice_contract, invoice_fields, seed=108,
                                                      truth_text=False, engine=strong, baseline=weaker))
    reports["latency-sparse"] = make_case(Scenario("latency-sparse", "invoice", 50, invoice_contract, invoice_fields, seed=109,
                                                   engine=dict(strong, timing=0.2), baseline=weaker))
    lenient_fields = {"total_amount": ("amount", 0.9, 0.8), "issue_date": ("date", 0.9, 0.8), "iban": ("iban", 0.9, 0.7)}
    reports["lenient-pass"] = make_case(Scenario("lenient-pass", "invoice", 150, contract_text("invoice-lenient", "invoice", lenient_fields, budget=0.5),
                                                 lenient_fields, seed=112, engine=strong, baseline=weaker))
    reports["percentile-method"] = make_case(Scenario("percentile-method", "invoice", 90, invoice_contract, invoice_fields, seed=110,
                                                      engine=weaker, baseline=strong, method="percentile", level=0.99))
    reports["field-absent-in-truth"] = make_case(Scenario("field-absent-in-truth", "invoice", 50, invoice_contract, invoice_fields, seed=111,
                                                          engine=strong, truth_fields=["total_amount", "issue_date"]))
    # the corpus must make every code path of the harness run at least once
    seen = {g["verdict"] for report in reports.values() for g in report["gates"]}
    assert seen == {"pass", "fail", "inconclusive", "not_measured"}, seen
    assert {r["verdict"] for r in reports.values()} >= {"pass", "fail", "inconclusive"}, {r["verdict"] for r in reports.values()}


REPOSITORY = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")


def fixture_vectors():
    """eval/fixtures/synthetic-v0: three classes in one truth file, an engine and a baseline run. All fictional."""
    directory = os.path.join(REPOSITORY, "eval", "fixtures", "synthetic-v0")
    os.makedirs(directory, exist_ok=True)

    def scenario(name, klass, n, seed, engine, baseline):
        path = os.path.join(REPOSITORY, "eval", "contracts", f"{name}.yaml")
        with open(path, encoding="utf-8") as f:
            text = f.read()
        fields = {g["name"]: (g["kind"], g["min"], g["coverage"]) for g in oracle.parse_contract(text)["fields"]}
        return Scenario(name, klass, n, text, fields, seed=seed, engine=engine, baseline=baseline, field_presence=0.95)

    strong = {"cer": 0.004, "accuracy": 0.995, "accept": 0.97, "ms": 5.5}
    weaker = {"cer": 0.02, "accuracy": 0.9, "accept": 0.9, "ms": 6.5}
    weak = {"cer": 0.03, "accuracy": 0.9, "accept": 0.9, "ms": 6.2}
    careful = dict(strong, accuracy=0.998, accept=0.985)

    # The runs are meant to show the three verdicts a person can meet, so each class is given the engine
    # that tells that story and the first seed from its base seed on that does tell it. That is a choice
    # of demonstration data, not a measurement; the generator refuses to write the files if no seed within
    # forty of the base does, so that a change to the generator cannot quietly change the story.
    plan_of_runs = [
        # name, class, documents, base seed, engine, baseline, the verdict the run is there to show
        ("invoice-fields-v1", "invoice", 120, 201, careful, weaker, "inconclusive"),  # good, but 120 documents cannot prove 99.9 %
        ("receipt-fields-v1", "receipt", 100, 202, weak, strong, "fail"),  # worse than the baseline and below its promises
        ("id-mrz-v1", "id", 80, 203, strong, weaker, "fail"),  # MRZ accuracy under its 99.9 % promise, proven by the data
    ]
    scenarios, story = [], {}
    for name, klass, n, base, engine, baseline, wanted in plan_of_runs:
        for candidate in range(base, base + 40):
            s = scenario(name, klass, n, candidate, engine, baseline)
            truth_text, predictions, baseline_text, _ = build_case(s)
            verdict = oracle.evaluate(s.contract, truth_text, predictions, baseline_text, 0.95, 2000, 1, "bootstrap-t")["verdict"]
            if verdict == wanted:
                break
        else:
            raise AssertionError(f"{name}: no seed from {base} to {base + 39} gives the verdict {wanted}")
        scenarios.append(s)
        story[name] = {"class": klass, "documents": n, "seed": candidate, "verdict": wanted,
                       "engine": engine, "baseline": baseline}
        print(f"fixtures/{name}: seed {candidate} shows {wanted}")
    combined = {"truth.jsonl": "", "predictions.jsonl": "", "baseline.jsonl": ""}
    for s in scenarios:
        truth_text, predictions, baseline, _ = build_case(s)
        combined["truth.jsonl"] += truth_text
        combined["predictions.jsonl"] += predictions
        combined["baseline.jsonl"] += baseline
    for name, text in combined.items():
        with open(os.path.join(directory, name), "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
        print(f"fixtures/synthetic-v0/{name}: {len(text.splitlines())} records")
    story["demo-lenient-invoice"] = {"class": "invoice", "documents": 120, "seed": "that of invoice-fields-v1", "verdict": "pass",
                                      "engine": "that of invoice-fields-v1", "baseline": "that of invoice-fields-v1"}
    with open(os.path.join(directory, "scenarios.json"), "w", encoding="utf-8", newline="\n") as f:
        f.write(dumps(story) + "\n")
    lenient = contract_text("demo-lenient-invoice", "invoice",
                            {"total_amount": ("amount", 0.9, 0.8), "issue_date": ("date", 0.9, 0.8), "iban": ("iban", 0.9, 0.7)},
                            ece=False, conformal=False, budget=0.5)
    lenient = "# A demonstration contract with thresholds a small corpus can meet: it passes on the synthetic fixtures.\n" + lenient
    with open(os.path.join(directory, "demo-lenient-invoice.yaml"), "w", encoding="utf-8", newline="\n") as f:
        f.write(lenient)
    # What the independent implementation says about each contract on these files, with the settings
    # `wanjiru eval` uses when none are given. The Rust test compares the report of the library with these.
    reports_dir = os.path.join(OUT, "fixtures")
    shutil.rmtree(reports_dir, ignore_errors=True)
    os.makedirs(reports_dir)
    contracts = {s.name: s.contract for s in scenarios}
    contracts["demo-lenient-invoice"] = lenient
    for name, text in contracts.items():
        report = oracle.evaluate(text, combined["truth.jsonl"], combined["predictions.jsonl"], combined["baseline.jsonl"],
                                 0.95, 2000, 1, "bootstrap-t")
        with open(os.path.join(reports_dir, name + ".json"), "w", encoding="utf-8", newline="\n") as f:
            f.write(dumps(report) + "\n")
        print(f"fixtures/{name}: verdict {report['verdict']}")
        assert report["verdict"] == story[name]["verdict"], (name, report["verdict"])


if __name__ == "__main__":
    validator_vectors()
    interval_vectors()
    proportion_vectors()
    rank_vectors()
    quantile_vectors()
    plan_vectors()
    yaml_vectors()
    case_vectors()
    fixture_vectors()
