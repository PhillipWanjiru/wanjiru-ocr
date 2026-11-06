#!/usr/bin/env python3
"""Independent reference vectors for crates/wanjiru-docir.

Nothing in the Rust crate is used here. The references are:

* Node.js, an ECMAScript engine: the number formatting and string escaping that RFC 8785 prescribes
  are by definition what `String(x)` and `JSON.stringify` do, and JavaScript sorts strings by UTF-16
  code unit, the key order the RFC requires (tools/oracles/jcs.js);
* Python's `json` module, with hooks that make it as strict as wanjiru-docir (no duplicate keys, no
  NaN or Infinity, no overflowing numbers, no lone surrogates), for what is valid JSON;
* the `jsonschema` package (draft 2020-12) for what satisfies crates/wanjiru-docir/schema/docir-v0.schema.json;
* NumPy, with ordinary 3x3 matrices, for transform chains;
* hashlib for SHA-256.

The generator is deterministic. Its output is committed under crates/wanjiru-docir/tests/golden/ and
read by crates/wanjiru-docir/tests/golden.rs, so running the Rust tests needs neither Python nor Node.

Usage: python3 -I tools/oracles/make_docir_vectors.py
"""
import copy
import hashlib
import json
import math
import os
import random
import shutil
import struct
import subprocess
import sys

import jsonschema
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
CRATE = os.path.join(ROOT, "crates", "wanjiru-docir")
OUT = os.path.join(CRATE, "tests", "golden")
SCHEMA_PATH = os.path.join(CRATE, "schema", "docir-v0.schema.json")
NODE = shutil.which("node")
if NODE is None:
    sys.exit("node is required: it is the ECMAScript reference for number formatting")
os.makedirs(OUT, exist_ok=True)


def write(name, lines):
    with open(os.path.join(OUT, name), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{name}: {len(lines)} records")


def bits_of(x: float) -> int:
    return struct.unpack(">Q", struct.pack(">d", x))[0]


def float_of(bits: int) -> float:
    return struct.unpack(">d", struct.pack(">Q", bits))[0]


def node(numbers_bits, texts):
    request = {"numbers": [f"{b:016x}" for b in numbers_bits], "texts": [t.encode("utf-8").hex() for t in texts]}
    done = subprocess.run([NODE, os.path.join(HERE, "jcs.js")], input=json.dumps(request), capture_output=True,
                          text=True, check=True)
    response = json.loads(done.stdout)
    return response["numbers"], [None if c is None else bytes.fromhex(c).decode("utf-8") for c in response["canonical"]]


# ---------------------------------------------------------------- numbers
rng = random.Random(8785)
bits = []


def finite_bits(b):
    return ((b >> 52) & 0x7FF) != 0x7FF


while len(bits) < 5000:  # random bit patterns: every exponent, subnormals included
    b = rng.getrandbits(64)
    if finite_bits(b):
        bits.append(b)
for _ in range(2500):  # short and long decimals
    digits = "".join(rng.choice("0123456789") for _ in range(rng.randint(1, 17)))
    bits.append(bits_of(float(f"{digits}e{rng.randint(-40, 40)}")))
    bits.append(bits_of(-float(f"{digits}e{rng.randint(-40, 40)}")))
for k in range(-30, 31):  # powers of ten and their neighbours, around both exponent-notation switches
    base = bits_of(float(f"1e{k}"))
    bits.extend(base + d for d in range(-3, 4))
for edge in ("1e21", "1e-6", "1e-7", "999999999999999900000", "9007199254740993", "18446744073709551616"):
    base = bits_of(float(edge))
    bits.extend(base + d for d in range(-3, 4))
for _ in range(1200):  # exact ties: values whose decimal expansion ends exactly at the digit after the last
    j = rng.randrange(1 << 50)
    bits.append(bits_of((2**52 + j) / 4.0))        # spacing 0.25, 16 digits plus .25 / .75
    bits.append(bits_of((2**51 + j) / 2.0))        # spacing 0.5
    b = rng.randint(1, 25)
    m = rng.randrange(1, 1 << rng.randint(1, 53)) | 1
    bits.append(bits_of(m / float(1 << b)))        # dyadic fractions with up to 25 fractional bits
    bits.append(bits_of(float(m) * 2.0 ** rng.randint(0, 20)))
for _ in range(300):  # subnormals
    bits.append(rng.getrandbits(52))
bits = [b for b in bits if finite_bits(b)]
bits = list(dict.fromkeys(bits))
strings, _ = node(bits, [])
write("numbers.txt", [f"{b:016x} {s}" for b, s in zip(bits, strings)])

# ---------------------------------------------------------------- JSON text: validity and canonical form


def no_dups(pairs):
    seen = {}
    for key, value in pairs:
        if key in seen:
            raise ValueError("duplicate key")
        seen[key] = value
    return seen


def refuse_constant(name):
    raise ValueError(name)


def checked_float(text):
    x = float(text)
    if math.isinf(x):
        raise ValueError("overflow")
    return x


def checked_int(text):
    return checked_float(text)


def no_lone_surrogates(value):
    if isinstance(value, str):
        value.encode("utf-8")
    elif isinstance(value, list):
        for v in value:
            no_lone_surrogates(v)
    elif isinstance(value, dict):
        for k, v in value.items():
            k.encode("utf-8")
            no_lone_surrogates(v)


def python_accepts(text: str) -> bool:
    try:
        value = json.loads(text, object_pairs_hook=no_dups, parse_constant=refuse_constant,
                           parse_float=checked_float, parse_int=checked_int)
        no_lone_surrogates(value)
        return True
    except (ValueError, UnicodeEncodeError, RecursionError, OverflowError):
        return False


HAND_WRITTEN = [
    # valid
    "null", "true", "false", "0", "-0", "0.0", "-0.0", "1", "-1", "1.5", "1e5", "1E5", "1e+5", "1e-5", "1.5e300",
    "5e-324", "1e308", "123456789012345678901234567890", "9007199254740993", "0.1e1", "-1e-0", "10E-1",
    '"abc"', '""', '"\\u0041"', '"\\ud83d\\ude00"', '"\\/"', '"\\u0000"', '"\\u001f"', '"\\u007f"', '"é"', '"😀"',
    '"\\u2028"', '"\\uffff"', "[]", "{}", "[[]]", '{"a":{}}', "[1,2,3]", '{"a":1,"b":2}', "  [ 1 , 2 ]  ", '{"":1}',
    '{"a":1,"A":2}', "[0.1,0.2,0.30000000000000004]", '{"\\u00e9":1,"e\\u0301":2}', '{"\\ud83d\\ude00":1,"\\ufb33":2}',
    "\n\t\r [ ]\n", '{"a":[{"b":[{"c":[]}]}]}', "[true,false,null]", "[1e2,1E2,100,1.0e2]",
    # invalid
    "", " ", "nul", "tru", "fals", "01", "00", "-01", "-", "+1", ".5", "5.", "1.e5", "1e", "1e+", "--1", "0x10",
    "NaN", "Infinity", "-Infinity", "nan", "True", "None", "[1,]", "[,1]", "[1,,2]", '{"a":1,}', "{,}", '{"a":1,,"b":2}',
    "{'a':1}", '{"a" 1}', '{"a":}', '{"a"}', '{"a":1 "b":2}', "[1 2]", "[1;2]", '"abc', "'a'", '"a" "b"', '"\\x41"',
    '"\\u12"', '"\\u12g4"', '"\\q"', '"a\tb"', '"a\nb"', "// c\n1", "/* c */1", "1 2", "[1] x", "[1][2]", "﻿[]",
    '{"a":1}}', "[[]", "[", "]", "{", "}", ":", ",", '"\\ud800"', '"\\udc00"', '"\\ud800\\u0041"', '"\\ud800x"',
    '"\\ud800\\ud800"', '"\\udc00\\ud800"', "\x00", "\f1", "1\x0b", " 1", "1e999", "-1e999", '{"a":1,"a":2}',
    '{"a":1,"\\u0061":2}', '{"a":{"b":1,"b":2}}', "[1e999]", '{"k":NaN}', "[Infinity]",
]

KEY_POOL = ["a", "b", "aa", "ab", "B", "_", "1", "10", "2", "", "é", "€", "\u0080", "דּ", "\U0001f600", "\r", "\n",
            '"', "\\", "/", "é", "￿", "\U0010ffff", "z" * 20, "ключ", "日本", " ", "\x7f", "\x1f", "$ref"]


def esc(rng, ch):
    """A JSON spelling of one character, chosen at random among the legal ones."""
    code = ord(ch)
    if ch == '"':
        return '\\"'
    if ch == "\\":
        return "\\\\"
    short = {"\b": "\\b", "\f": "\\f", "\n": "\\n", "\r": "\\r", "\t": "\\t"}
    if ch in short:
        return short[ch] if rng.random() < 0.7 else f"\\u{code:04{'x' if rng.random() < 0.5 else 'X'}}"
    if ch == "/":
        return "\\/" if rng.random() < 0.3 else "/"
    if code < 0x20:
        return f"\\u{code:04{'x' if rng.random() < 0.5 else 'X'}}"
    if code > 0xFFFF:
        if rng.random() < 0.5:
            return ch
        v = code - 0x10000
        hi, lo = 0xD800 + (v >> 10), 0xDC00 + (v & 0x3FF)
        form = "x" if rng.random() < 0.5 else "X"
        return f"\\u{hi:04{form}}\\u{lo:04{form}}"
    if code > 0x7E and rng.random() < 0.4:
        return f"\\u{code:04{'x' if rng.random() < 0.5 else 'X'}}"
    return ch


POOL = (list("abcdefghijklmnopqrstuvwxyzABCDEF0123456789 _-.,:;!?()[]{}") * 3 +
        ['"', "\\", "/", "\b", "\f", "\n", "\r", "\t", "\x01", "\x1f", "\x7f", "é", "€", "\u0080", " ", "דּ",
         "￿", "\U0001f600", "\U00010437", "\U0010ffff", "ß", "日", "本"])


def string_literal(rng, text=None):
    if text is None:
        text = "".join(rng.choice(POOL) for _ in range(rng.randint(0, 12)))
    return '"' + "".join(esc(rng, c) for c in text) + '"'


def ws(rng):
    return rng.choice(["", "", "", " ", "  ", "\n", "\t", "\r\n"])


def number_literal(rng):
    r = rng.random()
    if r < 0.2:
        return str(rng.randint(-10**6, 10**6))
    if r < 0.4:
        return repr(rng.uniform(-1e3, 1e3))
    if r < 0.5:
        return repr(10.0 ** rng.randint(-30, 30) * rng.random())
    if r < 0.7:
        sign = rng.choice(["", "-"])
        mant = str(rng.randint(0, 999)) if rng.random() < 0.5 else "0"
        frac = "".join(rng.choice("0123456789") for _ in range(rng.randint(0, 20)))
        exp = rng.choice(["", "e", "E"])
        tail = "" if not exp else exp + rng.choice(["", "+", "-"]) + str(rng.randint(0, 330))
        text = sign + mant + ("." + frac if frac else "") + tail
        return text if python_accepts(text) else "0"
    return rng.choice(["0", "-0", "0.0", "1", "-1", "0.1", "1e0", "1E2", "5e-324", "1.7976931348623157e308",
                       "9007199254740993", "4.5", "100", "1e21", "1e-7", "123456789.123456789", "0.000001"])


def value_text(rng, depth=0):
    r = rng.random()
    if depth > 4 or r < 0.35:
        c = rng.random()
        if c < 0.12:
            return rng.choice(["null", "true", "false"])
        if c < 0.55:
            return number_literal(rng)
        return string_literal(rng)
    if r < 0.65:
        n = rng.randint(0, 4)
        return "[" + ws(rng) + ("," + ws(rng)).join(value_text(rng, depth + 1) + ws(rng) for _ in range(n)) + "]"
    keys = rng.sample(KEY_POOL, rng.randint(0, 6))
    members = [string_literal(rng, k) + ws(rng) + ":" + ws(rng) + value_text(rng, depth + 1) + ws(rng) for k in keys]
    return "{" + ws(rng) + ",".join(members) + "}"


def mutate_text(rng, text):
    chars = list(text)
    alphabet = list('[]{},:"\\ 0-.eE+tfnul\né \x00 ') + ["דּ", "\U0001f600"]
    for _ in range(rng.randint(1, 3)):
        if not chars:
            chars = [rng.choice(alphabet)]
            continue
        at = rng.randrange(len(chars))
        op = rng.randrange(3)
        if op == 0:
            del chars[at]
        elif op == 1:
            chars.insert(at, rng.choice(alphabet))
        else:
            chars[at] = rng.choice(alphabet)
    return "".join(chars)


rng = random.Random(9)
cases = list(HAND_WRITTEN)
valid_texts = [value_text(rng) for _ in range(900)]
cases.extend(valid_texts)
cases.extend(mutate_text(rng, t) for t in valid_texts[:700])
cases = list(dict.fromkeys(cases))
accepted = [c for c in cases if python_accepts(c)]
_, canonical = node([], accepted)
canonical_of = dict(zip(accepted, canonical))
lines = []
for text in cases:
    hex_text = text.encode("utf-8").hex() or "-"
    if text in canonical_of:
        if canonical_of[text] is None:
            sys.exit(f"Node rejects a text that Python accepts: {text!r}")
        lines.append(f"ok {hex_text} {canonical_of[text].encode('utf-8').hex() or '-'}")
    else:
        lines.append(f"err {hex_text}")
write("json_cases.txt", lines)
print(f"  accepted {len(accepted)}, rejected {len(cases) - len(accepted)}")

# ---------------------------------------------------------------- DocIR instances and the schema
with open(SCHEMA_PATH, encoding="utf-8") as f:
    SCHEMA = json.load(f)
jsonschema.Draft202012Validator.check_schema(SCHEMA)
VALIDATOR = jsonschema.Draft202012Validator(SCHEMA)

FRAMES = ["source", "page", "line", "pdf", "normalised"]
rng = random.Random(10)


def f32(x):
    """A float32 as its shortest decimal, the form wanjiru-docir writes."""
    return float(str(np.float32(round(x, 2))))


def uniform_text(rng, n):
    pool = "abcdefghijklmnopqrstuvwxyz ABC0123456789,.-/éü€日本😀"
    return "".join(rng.choice(pool) for _ in range(n))


def gen_polygon(rng):
    x, y = rng.uniform(0, 900), rng.uniform(0, 1300)
    w, h = rng.uniform(5, 300), rng.uniform(5, 40)
    corners = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
    return [[f32(px), f32(py)] for px, py in corners[: rng.choice([3, 4, 4, 4])]]


def gen_confidence(rng):
    return {"value": rng.choice([0.0, 1.0, rng.random(), rng.random(), round(rng.random(), 3)]),
            "calibrated": rng.random() < 0.3}


def gen_chain(rng):
    steps, here = [], "source"
    for _ in range(rng.randint(0, 3)):
        to = rng.choice([f for f in FRAMES if f != here])
        theta, s = rng.uniform(-0.1, 0.1), rng.uniform(0.5, 2.0)
        affine = [s * math.cos(theta), s * math.sin(theta), -s * math.sin(theta), s * math.cos(theta),
                  rng.uniform(-50, 50), rng.uniform(-50, 50)]
        if rng.random() < 0.35:
            matrix = [affine[0], affine[2], affine[4], affine[1], affine[3], affine[5],
                      rng.uniform(-2e-5, 2e-5), rng.uniform(-2e-5, 2e-5), 1.0]
            steps.append({"to": to, "kind": "homography", "matrix": matrix})
        else:
            steps.append({"to": to, "kind": "affine", "matrix": affine})
        here = to
    return {"from": "source", "steps": steps}


def gen_doc(rng):
    pages, line_ids, serial = [], [], 0
    for index in range(rng.randint(1, 2)):
        lines = []
        for _ in range(rng.randint(0, 3)):
            serial += 1
            line_ids.append(f"l{serial}")
            lines.append({"id": line_ids[-1], "text": uniform_text(rng, rng.randint(0, 25)),
                          "polygon": gen_polygon(rng), "confidence": gen_confidence(rng),
                          "words": [{"text": uniform_text(rng, rng.randint(1, 8)), "polygon": gen_polygon(rng),
                                     "confidence": gen_confidence(rng)} for _ in range(rng.randint(0, 2))]})
        pages.append({"index": index, "size": {"w": f32(rng.uniform(100, 3000)), "h": f32(rng.uniform(100, 4000))},
                      "chain": gen_chain(rng), "lines": lines})
    fields = []
    for n in range(rng.randint(0, 3)):
        kind = rng.choice(["amount", "date", "iban", "mrz", "text"])
        field = {"name": f"field_{n}", "kind": kind, "value": uniform_text(rng, rng.randint(1, 20)),
                 "confidence": gen_confidence(rng), "decision": rng.choice(["accept", "review"]),
                 "evidence": [{"page": rng.randrange(len(pages)), "polygon": gen_polygon(rng),
                               "text": uniform_text(rng, rng.randint(0, 10))} for _ in range(rng.randint(0, 2))],
                 "lines": rng.sample(line_ids, min(len(line_ids), rng.randint(0, 2)))}
        if kind == "amount" and rng.random() < 0.7:
            field["currency"] = rng.choice(["EUR", "USD", "KES", "GBP", "CHF"])
        fields.append(field)
    return {"schema": "wanjiru.docir/0", "id": uniform_text(rng, rng.randint(1, 30)) or "x",
            "class": rng.choice(["invoice", "receipt", "id", "other"]),
            "source": {"sha256": "".join(rng.choice("0123456789abcdef") for _ in range(64)),
                       "media_type": rng.choice(["image/png", "image/jpeg", "application/pdf", "image/tiff"])},
            "pages": pages, "fields": fields,
            "run": {"engine": "wanjiru " + rng.choice(["0.1.0", "0.2.0-dev"]),
                    "config_sha256": "".join(rng.choice("0123456789abcdef") for _ in range(64)),
                    "timing_ms": {k: rng.choice([0, 0.5, rng.uniform(0, 500), 1e-7])
                                  for k in rng.sample(["decode", "detect", "recognise", "total"], rng.randint(0, 3))}}}


ENUM_WORDS = ["invoice", "receipt", "id", "other", "Invoice", "amount", "date", "iban", "mrz", "text", "accept",
              "review", "source", "page", "line", "pdf", "normalised", "affine", "homography", "wanjiru.docir/1"]
RETYPES = [None, True, False, "x", "", 0, 1, -1, 1.5, [], {}, [1, 2, 3], {"a": 1}]


def all_paths(node, prefix=()):
    yield prefix
    if isinstance(node, dict):
        for k, v in node.items():
            yield from all_paths(v, prefix + (k,))
    elif isinstance(node, list):
        for i, v in enumerate(node):
            yield from all_paths(v, prefix + (i,))


def get(node, path):
    for p in path:
        node = node[p]
    return node


def mutate_doc(rng, doc):
    doc = copy.deepcopy(doc)
    for _ in range(rng.choice([1, 1, 1, 2])):
        path = rng.choice(list(all_paths(doc)))
        if not path:
            doc["extra"] = 1
            continue
        parent, key = get(doc, path[:-1]), path[-1]
        node = parent[key]
        ops = ["retype", "retype", "delete", "extra"]
        if isinstance(node, (int, float)) and not isinstance(node, bool):
            ops += ["number"] * 3
        if isinstance(node, str):
            ops += ["string"] * 2 + ["enum"] * 2
        if isinstance(node, list):
            ops += ["list"] * 3
        op = rng.choice(ops)
        if op == "retype":
            parent[key] = copy.deepcopy(rng.choice(RETYPES))
        elif op == "delete":
            if isinstance(parent, dict):
                del parent[key]
            else:
                parent.pop(key)
        elif op == "extra":
            target = node if isinstance(node, dict) else parent if isinstance(parent, dict) else doc
            target["extra"] = 0
        elif op == "number":
            parent[key] = rng.choice([-1, 0, 1, 2, 0.5, 1.0000001, -1e-7, 1e308, 3.0, 64, 65])
        elif op == "string":
            parent[key] = rng.choice(["", "x" * 64, "x" * 65, "x" * 256, "x" * 257, node.upper(), node + "x", "é" * 64])
        elif op == "enum":
            parent[key] = rng.choice(ENUM_WORDS)
        else:
            choice = rng.randrange(4)
            if choice == 0 and node:
                node.pop()
            elif choice == 1:
                node.clear()
            elif choice == 2 and node:
                node.append(copy.deepcopy(node[0]))
            else:
                node.append(None)
    return doc


def compact(doc):
    return json.dumps(doc, separators=(",", ":"), ensure_ascii=True, allow_nan=False)


model_valid = [gen_doc(rng) for _ in range(260)]
for doc in model_valid:
    assert VALIDATOR.is_valid(doc), "generator produced a schema-invalid document"
mutated = [mutate_doc(rng, doc) for doc in model_valid for _ in range(2)]
instances = [("model", d) for d in model_valid]
for doc in mutated:
    instances.append(("schema-valid" if VALIDATOR.is_valid(doc) else "schema-invalid", doc))
texts = [compact(d) for _, d in instances]
_, canonical = node([], [t for (kind, _), t in zip(instances, texts) if kind == "model"])
canonical_iter = iter(canonical)
lines, counts = [], {"model": 0, "schema-valid": 0, "schema-invalid": 0}
for (kind, _), text in zip(instances, texts):
    counts[kind] += 1
    if kind == "model":
        lines.append(f"{kind} {next(canonical_iter).encode('utf-8').hex()} {text}")
    else:
        lines.append(f"{kind} - {text}")
write("schema_cases.txt", lines)
print(f"  {counts}")
assert counts["schema-valid"] >= 80 and counts["schema-invalid"] >= 250, counts

# ---------------------------------------------------------------- the sample invoice
sample = {
    "schema": "wanjiru.docir/0",
    "id": "inv-0001",
    "class": "invoice",
    "source": {"sha256": hashlib.sha256(b"fictional invoice scan, 2026").hexdigest(), "media_type": "image/png"},
    "pages": [{
        "index": 0,
        "size": {"w": 1654, "h": 2339},
        "chain": {"from": "source", "steps": [
            {"to": "page", "kind": "affine", "matrix": [0.5, 0, 0, 0.5, 0, 0]},
            {"to": "line", "kind": "homography", "matrix": [1, 0, -10, 0, 1, -20, 0.00001, 0, 1]}]},
        "lines": [
            {"id": "l0", "text": "Rechnung Nr. 2026-001", "polygon": [[100.5, 120], [620.25, 120], [620.25, 152.5], [100.5, 152.5]],
             "confidence": {"value": 0.98, "calibrated": False},
             "words": [{"text": "Rechnung", "polygon": [[100.5, 120], [260, 120], [260, 152.5], [100.5, 152.5]],
                        "confidence": {"value": 0.99, "calibrated": False}}]},
            {"id": "l1", "text": "Gesamtbetrag 1 234,50 € — fällig am 2026-11-30 😀",
             "polygon": [[100.5, 1900], [900.75, 1900], [900.75, 1940], [100.5, 1940]],
             "confidence": {"value": 0.9731234567890123, "calibrated": False}, "words": []}],
    }],
    "fields": [
        {"name": "total_amount", "kind": "amount", "value": "1234.50", "currency": "EUR",
         "confidence": {"value": 0.97, "calibrated": False}, "decision": "accept",
         "evidence": [{"page": 0, "polygon": [[420, 1900], [600.5, 1900], [600.5, 1940], [420, 1940]], "text": "1 234,50"}],
         "lines": ["l1"]},
        {"name": "issue_date", "kind": "date", "value": "2026-11-30",
         "confidence": {"value": 0.61, "calibrated": False}, "decision": "review", "evidence": [], "lines": ["l1"]}],
    "run": {"engine": "wanjiru 0.1.0", "config_sha256": hashlib.sha256(b"config").hexdigest(),
            "timing_ms": {"decode": 1.25, "recognise": 0.0000001, "total": 12.5}},
}
assert VALIDATOR.is_valid(sample)
for name, digest in (("sample-source.sha256", sample["source"]["sha256"]), ("sample-config.sha256", sample["run"]["config_sha256"])):
    with open(os.path.join(OUT, name), "w", encoding="utf-8", newline="\n") as f:
        f.write(digest + "\n")
pretty = json.dumps(sample, indent=2, ensure_ascii=False)
with open(os.path.join(OUT, "sample-invoice.json"), "w", encoding="utf-8", newline="\n") as f:
    f.write(pretty + "\n")
_, canonical = node([], [pretty])
canonical_text = canonical[0]
with open(os.path.join(OUT, "sample-invoice.canonical.json"), "w", encoding="utf-8", newline="\n") as f:
    f.write(canonical_text)  # no trailing newline: the bytes are exactly what is hashed
with open(os.path.join(OUT, "sample-invoice.sha256"), "w", encoding="utf-8", newline="\n") as f:
    f.write(hashlib.sha256(canonical_text.encode("utf-8")).hexdigest() + "\n")
# The same document without wall-clock timings, for content_sha256.
without_timing = copy.deepcopy(sample)
without_timing["run"]["timing_ms"] = {}
_, canonical = node([], [json.dumps(without_timing)])
with open(os.path.join(OUT, "sample-invoice.content.sha256"), "w", encoding="utf-8", newline="\n") as f:
    f.write(hashlib.sha256(canonical[0].encode("utf-8")).hexdigest() + "\n")
print("sample-invoice.*: written")

# ---------------------------------------------------------------- transform chains with NumPy
rng = random.Random(11)


def matrix3(step):
    m = step["matrix"]
    if step["kind"] == "affine":
        return np.array([[m[0], m[2], m[4]], [m[1], m[3], m[5]], [0.0, 0.0, 1.0]])
    return np.array(m, dtype=float).reshape(3, 3)


def apply3(matrix, point):
    q = matrix @ np.array([point[0], point[1], 1.0])
    if abs(q[2]) < 1e-9:
        return None
    return [q[0] / q[2], q[1] / q[2]]


lines = []
for _ in range(300):
    chain = gen_chain(rng)
    if not chain["steps"]:
        continue
    chain["steps"] = [dict(s) for s in chain["steps"]]
    points, forward, back = [], [], []
    for _ in range(8):
        p = [rng.uniform(0, 10000), rng.uniform(0, 10000)]
        q = p
        for step in chain["steps"]:
            q = apply3(matrix3(step), q) if q is not None else None
        if q is None or max(abs(q[0]), abs(q[1])) > 1e6:
            continue
        r = q
        for step in reversed(chain["steps"]):
            r = apply3(np.linalg.inv(matrix3(step)), r) if r is not None else None
        if r is None:
            continue
        points.append(p)
        forward.append(q)
        back.append(r)
    if points:
        lines.append(json.dumps({"chain": chain, "points": points, "forward": forward, "back": back}, separators=(",", ":")))
write("chains.jsonl", lines)
