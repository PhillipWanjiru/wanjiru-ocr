#!/usr/bin/env python3
"""An independent implementation of the evaluation harness (crates/wanjiru-eval).

It shares no code with the Rust crate. Where an exact answer exists, it is computed another way:
Clopper-Pearson intervals from SciPy's beta quantiles (Rust: bisection on the binomial tails),
order-statistic ranks by exact rational arithmetic (Rust: floating-point binomial sums), the
normal quantile from SciPy, calendar dates from `datetime`, the edit distance by a plain dynamic
programme, YAML from PyYAML. Only the random generator (xoshiro256** seeded by SplitMix64, with
Lemire's bounded integers) and the order of floating-point operations of the bootstrap are
ported literally, because a bootstrap interval is reproducible only if both are: the Rust crate
and this file agree to the last bit, on every case in tests/golden.

`evaluate(...)` returns the report body as a dict with the same members as the Rust report
(without `report_sha256`). make_eval_vectors.py runs it on generated corpora and commits the result.
"""
import datetime
import hashlib
import json
import math
import re
from fractions import Fraction

import yaml
from scipy.stats import beta, norm

MASK = (1 << 64) - 1
INF = math.inf


# ---------------------------------------------------------------- the random generator
def _splitmix(state):
    state = (state + 0x9E3779B97F4A7C15) & MASK
    z = state
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return state, z ^ (z >> 31)


def _rotl(x, k):
    return ((x << k) | (x >> (64 - k))) & MASK


class Rng:
    """xoshiro256** (Blackman and Vigna) seeded through SplitMix64."""

    def __init__(self, seed):
        state = seed & MASK
        self.s = []
        for _ in range(4):
            state, out = _splitmix(state)
            self.s.append(out)

    def next_u64(self):
        s = self.s
        result = (_rotl((s[1] * 5) & MASK, 7) * 9) & MASK
        t = (s[1] << 17) & MASK
        s[2] ^= s[0]
        s[3] ^= s[1]
        s[1] ^= s[2]
        s[0] ^= s[3]
        s[2] ^= t
        s[3] = _rotl(s[3], 45)
        return result

    def below(self, n):
        x = self.next_u64()
        m = x * n
        low = m & MASK
        if low < n:
            threshold = ((-n) & MASK) % n
            while low < threshold:
                x = self.next_u64()
                m = x * n
                low = m & MASK
        return m >> 64


# ---------------------------------------------------------------- resampling intervals
class Unusable(Exception):
    """The Rust crate answers these inputs with WNJ-E4005."""


def quantile7(sorted_values, p):
    """Hyndman-Fan type 7, in the operation order of wanjiru_math::stats::quantile_sorted."""
    n = len(sorted_values)
    h = (n - 1) * min(max(p, 0.0), 1.0)
    lo_f = math.floor(h)
    lo = int(lo_f)
    hi = min(lo + 1, n - 1)
    return sorted_values[lo] + (h - lo_f) * (sorted_values[hi] - sorted_values[lo])


def _linearised_se(pairs, n, ratio, sum_x):
    residual_squares = 0.0
    for y, x in pairs:
        d = float(y) - ratio * float(x)
        residual_squares += d * d
    nf = float(n)
    return math.sqrt((nf / (nf - 1.0)) * residual_squares) / float(sum_x)


def _check_settings(level, resamples):
    if not (0.0 < level < 1.0):
        raise Unusable("level")
    if not (100 <= resamples <= 1_000_000):
        raise Unusable("resamples")


def ratio_interval(y, x, level, resamples, seed, method):
    """sum(y)/sum(x) with an interval from resampling the n pairs; method 'bootstrap-t' or 'percentile'."""
    _check_settings(level, resamples)
    n = len(y)
    if n != len(x) or n < 2:
        raise Unusable("sample")
    sum_x, sum_y = sum(x), sum(y)
    if sum_x == 0:
        raise Unusable("denominators")
    ratio = float(sum_y) / float(sum_x)
    se = _linearised_se(zip(y, x), n, ratio, sum_x)
    if method == "bootstrap-t" and se == 0.0:
        return ratio, ratio, ratio
    alpha = 1.0 - level
    rng = Rng(seed)
    values = []
    for _ in range(resamples):
        drawn = [rng.below(n) for _ in range(n)]
        ry = rx = 0
        for i in drawn:
            ry += y[i]
            rx += x[i]
        if rx == 0:
            continue
        r = float(ry) / float(rx)
        if method == "percentile":
            values.append(r)
        else:
            rse = _linearised_se(((y[i], x[i]) for i in drawn), n, r, rx)
            if rse > 0.0 and math.isfinite(rse):
                values.append((r - ratio) / rse)
    if len(values) < 20:
        raise Unusable("too few usable resamples")
    values.sort()
    low_q = quantile7(values, alpha / 2.0)
    high_q = quantile7(values, 1.0 - alpha / 2.0)
    if method == "percentile":
        lower, upper = low_q, high_q
    else:
        lower, upper = ratio - high_q * se, ratio - low_q * se
    return ratio, max(lower, 0.0), upper


def relative_change(new, base, level, resamples, seed, method):
    sum_new, sum_base = sum(new), sum(base)
    if sum_base == 0:
        _check_settings(level, resamples)
        if len(new) != len(base) or len(new) < 2:
            raise Unusable("sample")
        if sum_new == 0:
            return 0.0, -1.0, INF
        return INF, INF, INF
    estimate, lower, upper = ratio_interval(new, base, level, resamples, seed, method)
    return estimate - 1.0, lower - 1.0, upper - 1.0


# ---------------------------------------------------------------- exact intervals
def clopper_pearson(k, n, level):
    """(estimate, lower, upper); estimate None and [0, 1] without trials. SciPy's beta quantiles."""
    if n == 0:
        return None, 0.0, 1.0
    alpha = 1.0 - level
    lower = 0.0 if k == 0 else float(beta.ppf(alpha / 2.0, k, n - k + 1))
    upper = 1.0 if k == n else float(beta.ppf(1.0 - alpha / 2.0, k + 1, n - k))
    return k / n, lower, upper


def _binomial_cdf_table(n, q):
    """[P(Binomial(n, q) <= j) for j in 0..n] as exact rationals; q is a float read exactly."""
    qf = Fraction(q)
    pf = 1 - qf
    pmf = pf**n
    table = []
    total = Fraction(0)
    for j in range(0, n + 1):
        total += pmf
        table.append(total)
        if j < n:
            pmf = pmf * (n - j) / (j + 1) * qf / pf
    return table


def rank_interval(n, q, level):
    """1-based ranks (lower, upper) of the order-statistic interval for the q-quantile, None if unbounded."""
    alpha = 1.0 - level
    tail = Fraction(alpha) / 2
    cdf = _binomial_cdf_table(n, q)  # cdf[j] = P(B <= j)
    lower = None
    if cdf[0] <= tail:
        lower = max(l for l in range(1, n + 1) if cdf[l - 1] <= tail)
    upper = None
    if 1 - cdf[n - 1] <= tail:  # P(B >= n)
        upper = min(u for u in range(1, n + 1) if 1 - cdf[u - 1] <= tail)  # P(B >= u) = 1 - P(B <= u - 1)
    return lower, upper


def quantile_interval(sorted_values, q, level):
    n = len(sorted_values)
    if n == 0:
        return {"n": 0, "estimate": None, "lower": None, "upper": None}
    lower, upper = rank_interval(n, q, level)
    return {
        "n": n,
        "estimate": quantile7(sorted_values, q),
        "lower": None if lower is None else sorted_values[lower - 1],
        "upper": None if upper is None else sorted_values[upper - 1],
    }


# ---------------------------------------------------------------- validators
def valid_amount(text):
    m = re.fullmatch(r"-?(0|[1-9][0-9]*)(\.[0-9]+)?", text)
    if not m:
        return False
    digits = len(m.group(1)) + (len(m.group(2)) - 1 if m.group(2) else 0)
    if digits > 18:
        return False
    if text.startswith("-") and float(text) == 0.0 and set(text.replace("-", "").replace(".", "")) <= {"0"}:
        return False
    return True


def valid_date(text):
    if not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", text):
        return False
    year, month, day = int(text[0:4]), int(text[5:7]), int(text[8:10])
    if not 1900 <= year <= 2100:
        return False
    try:
        datetime.date(year, month, day)
    except ValueError:
        return False
    return True


def valid_iban(text):
    if not re.fullmatch(r"[A-Z]{2}[0-9]{2}[A-Z0-9]{11,30}", text):
        return False
    rearranged = text[4:] + text[:4]
    return int("".join(str(int(c, 36)) for c in rearranged)) % 97 == 1


def _mrz_value(c):
    if c.isdigit() and c.isascii():
        return int(c)
    if "A" <= c <= "Z":
        return ord(c) - 55
    if c == "<":
        return 0
    raise ValueError(c)


def check_digit(text):
    return sum(_mrz_value(c) * (7, 3, 1)[i % 3] for i, c in enumerate(text)) % 10


def valid_mrz(text):
    lines = text.split("\n")
    if not all(re.fullmatch(r"[A-Z0-9<]*", line) for line in lines):
        return False
    shape = tuple(len(line) for line in lines)

    def ok(line, a, b, at):
        return line[at] == str(check_digit(line[a:b]))

    if shape == (44, 44):
        l2 = lines[1]
        if not (ok(l2, 0, 9, 9) and ok(l2, 13, 19, 19) and ok(l2, 21, 27, 27)):
            return False
        empty_personal = set(l2[28:42]) == {"<"} and l2[42] in "<0"
        if not empty_personal and not ok(l2, 28, 42, 42):
            return False
        return ok_composite(l2, l2[0:10] + l2[13:20] + l2[21:43], 43)
    if shape == (36, 36):
        l2 = lines[1]
        if not (ok(l2, 0, 9, 9) and ok(l2, 13, 19, 19) and ok(l2, 21, 27, 27)):
            return False
        return ok_composite(l2, l2[0:10] + l2[13:20] + l2[21:35], 35)
    if shape == (30, 30, 30):
        l1, l2 = lines[0], lines[1]
        if l1[14] == "<":
            return False
        if not (ok(l1, 5, 14, 14) and ok(l2, 0, 6, 6) and ok(l2, 8, 14, 14)):
            return False
        return ok_composite(l2, l1[5:30] + l2[0:7] + l2[8:15] + l2[18:29], 29)
    return False


def ok_composite(line, covered, at):
    return line[at] == str(check_digit(covered))


VALIDATORS = {"amount": valid_amount, "date": valid_date, "iban": valid_iban, "mrz": valid_mrz, "text": lambda t: t != ""}


# ---------------------------------------------------------------- planner
def plan(p, margin, confidence, fields_per_document, icc):
    z = float(norm.ppf(1.0 - (1.0 - confidence) / 2.0))
    raw = z * z * p * (1.0 - p) / (margin * margin)
    design_effect = 1.0 + (float(fields_per_document) - 1.0) * icc
    fields = math.ceil(raw * design_effect)
    return {
        "z": z,
        "independent_fields": math.ceil(raw),
        "design_effect": design_effect,
        "fields": fields,
        "documents": -(-fields // fields_per_document),
    }


def minimum_detectable_difference(p, effective_n, alpha, power):
    z = float(norm.ppf(1.0 - alpha / 2.0)) + float(norm.ppf(power))
    return z * math.sqrt(2.0 * p * (1.0 - p) / effective_n)


# ---------------------------------------------------------------- edit distance and CER
def edit_distance(a, b):
    previous = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        current = [i]
        for j, cb in enumerate(b, 1):
            current.append(min(previous[j] + 1, current[j - 1] + 1, previous[j - 1] + (ca != cb)))
        previous = current
    return previous[-1]


def hypothesis(document):
    return "\n".join(line["text"] for page in document["pages"] for line in page["lines"])


def cer_score(reference, document):
    n = len(reference)
    return n, (n if document is None else edit_distance(reference, hypothesis(document)))


# ---------------------------------------------------------------- contracts
CLASSES = ("invoice", "receipt", "id", "other")
KINDS = ("amount", "date", "iban", "mrz", "text")


def parse_contract(text):
    c = yaml.safe_load(text)
    metrics = c.get("metrics", {})
    fields = []
    for name in sorted(metrics.get("field_exact_match", {})):
        g = metrics["field_exact_match"][name]
        kind = g.get("kind", name)
        assert kind in KINDS, (name, kind)
        fields.append({"name": name, "kind": kind, "min": float(g["min"]), "coverage": float(g["coverage"])})
    latency = metrics.get("latency_ms", {})
    conformal = metrics.get("conformal")
    return {
        "id": c["contract"],
        "class": c["class"],
        "status": c.get("status", "draft"),
        "corpus": c["corpus"],
        "fields": fields,
        "ece_char_max": None if "ece_char" not in metrics else float(metrics["ece_char"]["max"]),
        "conformal": None if conformal is None else (float(conformal["alpha"]), float(conformal["coverage_tolerance"])),
        "p50_max": None if "p50_max" not in latency else float(latency["p50_max"]),
        "p95_max": None if "p95_max" not in latency else float(latency["p95_max"]),
        "budget": None if "regression_budget_cer_rel" not in c.get("gates", {}) else float(c["gates"]["regression_budget_cer_rel"]),
    }


# ---------------------------------------------------------------- gates
def decide(comparison, threshold, lower, upper):
    if comparison == ">=":
        if lower >= threshold:
            return "pass"
        if upper < threshold:
            return "fail"
    else:
        if upper <= threshold:
            return "pass"
        if lower > threshold:
            return "fail"
    return "inconclusive"


def gate(gid, comparison, threshold, estimate, lower, upper, note=None):
    return {"id": gid, "comparison": comparison, "threshold": threshold, "estimate": estimate,
            "lower": lower, "upper": upper, "verdict": decide(comparison, threshold, lower, upper), "note": note}


def not_measured(gid, comparison, threshold, why):
    return {"id": gid, "comparison": comparison, "threshold": threshold, "estimate": None,
            "lower": -INF, "upper": INF, "verdict": "not_measured", "note": why}


def overall(gates):
    measured = [g for g in gates if g["verdict"] != "not_measured"]
    if any(g["verdict"] == "fail" for g in gates):
        return "fail"
    if not measured or any(g["verdict"] == "inconclusive" for g in measured):
        return "inconclusive"
    return "pass"


# ---------------------------------------------------------------- the harness
PAIRED_STREAM = 0x9E3779B97F4A7C15


def _records(text):
    return [line.rstrip("\r") for line in text.split("\n") if line.strip()]


def _num(x):
    """JSON number or null, as the Rust report writes them."""
    return x if x is not None and math.isfinite(x) else None


def _interval(t):
    return {"estimate": _num(t[0]), "lower": _num(t[1]), "upper": _num(t[2])}


def _proportion(k, n, level):
    estimate, lower, upper = clopper_pearson(k, n, level)
    return {"k": k, "n": n, "estimate": estimate, "lower": lower, "upper": upper}


def sha256_text(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def evaluate(contract_text, truth_text, predictions_text, baseline_text, level, resamples, seed, method):
    """The report body (a dict) for the given files and settings; raises Unusable or ValueError for bad input."""
    contract = parse_contract(contract_text)
    truths = {}
    for line in _records(truth_text):
        record = json.loads(line)
        assert record["id"] not in truths, "duplicate truth id"
        truths[record["id"]] = record

    def read_documents(text):
        documents = {}
        for line in _records(text):
            document = json.loads(line)
            assert document["id"] in truths, "unknown prediction id"
            assert document["id"] not in documents, "duplicate prediction id"
            documents[document["id"]] = document
        return documents

    predictions = read_documents(predictions_text)
    baseline = None if baseline_text is None else read_documents(baseline_text)
    cases = [(t, predictions.get(t["id"]), None if baseline is None else baseline.get(t["id"]))
             for _, t in sorted(truths.items()) if t["class"] == contract["class"]]
    if not cases:
        raise ValueError("no documents of the class")

    counts = {
        "truth": len(cases),
        "predicted": sum(1 for _, p, _ in cases if p is not None),
        "baseline": None if baseline is None else sum(1 for _, _, b in cases if b is not None),
        "with_reference": sum(1 for t, _, _ in cases if "text" in t),
    }

    # character error rate and the comparison with the baseline
    referenced = [(t, p, b) for t, p, b in cases if "text" in t]
    scores = [cer_score(t["text"], p) for t, p, _ in referenced]
    lengths = [s[0] for s in scores]
    edits = [s[1] for s in scores]
    measurable = len(referenced) >= 2 and sum(lengths) > 0
    cer = regression = None
    if measurable:
        rate = ratio_interval(edits, lengths, level, resamples, seed, method)
        cer = {"documents": len(referenced), "reference_chars": sum(lengths), "edits": sum(edits), "rate": _interval(rate)}
        if baseline is not None:
            base_edits = [cer_score(t["text"], b)[1] for t, _, b in referenced]
            change = relative_change(edits, base_edits, level, resamples, seed ^ PAIRED_STREAM, method)
            regression = {"baseline_edits": sum(base_edits), "edits": sum(edits), "relative_change": _interval(change)}

    # fields
    fields_report = {}
    summaries = []
    for g in contract["fields"]:
        valid = VALIDATORS[g["kind"]]
        trials = accepted = correct = reviewed = absent = invalid = without_truth = 0
        for t, p, _ in cases:
            expected = t.get("fields", {}).get(g["name"])
            if expected is not None:
                assert valid(expected), f"truth value of {g['name']} in {t['id']} is invalid"
                trials += 1
            predicted = None if p is None else next((f for f in p["fields"] if f["name"] == g["name"]), None)
            accepted_field = predicted if predicted is not None and predicted["decision"] == "accept" else None
            if accepted_field is not None and not valid(accepted_field["value"]):
                invalid += 1
            if expected is not None:
                if predicted is None:
                    absent += 1
                elif accepted_field is None:
                    reviewed += 1
                else:
                    accepted += 1
                    if accepted_field["kind"] == g["kind"] and accepted_field["value"] == expected:
                        correct += 1
            elif accepted_field is not None:
                without_truth += 1
        accuracy = _proportion(correct, accepted, level)
        coverage = _proportion(accepted, trials, level)
        fields_report[g["name"]] = {
            "kind": g["kind"], "trials": trials, "accepted": accepted, "correct": correct, "reviewed": reviewed,
            "absent": absent, "accepted_invalid": invalid, "accepted_without_truth": without_truth,
            "accuracy": accuracy, "coverage": coverage,
        }
        summaries.append((g, trials, accepted, accuracy, coverage))

    # latency
    milliseconds = sorted(p["run"]["timing_ms"]["total"] for _, p, _ in cases
                          if p is not None and "total" in p["run"]["timing_ms"])
    latency = None
    if milliseconds:
        latency = {"p50": quantile_interval(milliseconds, 0.5, level), "p95": quantile_interval(milliseconds, 0.95, level)}

    # gates, in the order of the Rust harness
    gates = []
    for g, trials, accepted, accuracy, coverage in summaries:
        note = "no truth document carries this field" if trials == 0 else ("the engine accepted no value" if accepted == 0 else None)
        gates.append(gate(f"field_exact_match.{g['name']}.min", ">=", g["min"], accuracy["estimate"], accuracy["lower"], accuracy["upper"], note))
        note = "no truth document carries this field" if trials == 0 else None
        gates.append(gate(f"field_exact_match.{g['name']}.coverage", ">=", g["coverage"], coverage["estimate"], coverage["lower"], coverage["upper"], note))
    if contract["ece_char_max"] is not None:
        gates.append(not_measured("ece_char.max", "<=", contract["ece_char_max"], "calibration of character confidences is measured from Phase 6"))
    if contract["conformal"] is not None:
        alpha, tolerance = contract["conformal"]
        gates.append(not_measured("conformal.coverage", ">=", 1.0 - alpha - tolerance, "conformal coverage is measured from Phase 6"))
    for gid, key, which in (("latency_ms.p50_max", "p50_max", "p50"), ("latency_ms.p95_max", "p95_max", "p95")):
        if contract[key] is None:
            continue
        if latency is None:
            gates.append(not_measured(gid, "<=", contract[key], "no output carries run.timing_ms.total"))
        else:
            q = latency[which]
            gates.append(gate(gid, "<=", contract[key], q["estimate"], -INF if q["lower"] is None else q["lower"],
                              INF if q["upper"] is None else q["upper"]))
    if contract["budget"] is not None:
        gid = "gates.regression_budget_cer_rel"
        if regression is not None:
            e, lo, hi = relative_change(edits, [cer_score(t["text"], b)[1] for t, _, b in referenced], level, resamples, seed ^ PAIRED_STREAM, method)
            gates.append(gate(gid, "<=", contract["budget"], e, lo, hi))
        elif baseline is None:
            gates.append(not_measured(gid, "<=", contract["budget"], "no baseline run was supplied"))
        else:
            gates.append(not_measured(gid, "<=", contract["budget"],
                                      "the character error rate could not be measured: it needs two documents with reference text"))

    return {
        "schema": "wanjiru.eval-report/0",
        "contract": {"id": contract["id"], "class": contract["class"], "status": contract["status"], "corpus": contract["corpus"]},
        "inputs": {
            "contract_sha256": sha256_text(contract_text), "truth_sha256": sha256_text(truth_text),
            "predictions_sha256": sha256_text(predictions_text),
            "baseline_sha256": None if baseline_text is None else sha256_text(baseline_text),
        },
        "method": {"level": level, "resamples": resamples, "seed": seed, "ratios": method,
                   "proportions": "clopper-pearson", "quantiles": "order-statistics"},
        "documents": counts,
        "cer": cer,
        "fields": fields_report,
        "latency_ms": latency,
        "regression": regression,
        "gates": [dict(g, estimate=_num(g["estimate"]), lower=_num(g["lower"]), upper=_num(g["upper"])) for g in gates],
        "verdict": overall(gates),
    }
