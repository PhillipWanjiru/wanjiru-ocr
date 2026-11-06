#!/usr/bin/env python3
"""Independent reference vectors for crates/wanjiru-math.

Every vector is produced by a library or an algorithm that shares no code with the Rust crate:
hashlib (SHA-256), decimal (50-digit exp), SciPy (normal cdf and quantile, binomial, Clopper-Pearson), math.fsum (exact sums) and a pure-Python dynamic programme (edit
distance). The generator is deterministic; its output is committed under
crates/wanjiru-math/tests/golden/ and read by crates/wanjiru-math/tests/golden.rs.

Usage: python3 tools/oracles/make_math_vectors.py [output-directory]
"""
import hashlib
import math
import os
import random
import struct
import sys
from decimal import Decimal, getcontext

OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
    os.path.dirname(__file__), "..", "..", "crates", "wanjiru-math", "tests", "golden")
os.makedirs(OUT, exist_ok=True)
MASK = (1 << 64) - 1


def bits(x: float) -> str:
    return f"{struct.unpack('>Q', struct.pack('>d', x))[0]:016x}"


def write(name, lines):
    with open(os.path.join(OUT, name), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{name}: {len(lines)} records")


# ---------------------------------------------------------------- SHA-256
rnd = random.Random(1)
lengths = list(range(0, 131)) + [191, 192, 193, 255, 256, 257, 511, 512, 1000]
lines = []
for n in lengths:
    data = bytes(rnd.randrange(256) for _ in range(n))
    lines.append(f"{data.hex() or '-'} {hashlib.sha256(data).hexdigest()}")
write("sha256.txt", lines)

# ---------------------------------------------------------------- exp at 60 digits
getcontext().prec = 60
rnd = random.Random(2)
xs = [rnd.uniform(-40, 40) for _ in range(1200)]
xs += [rnd.uniform(-745, 709) for _ in range(600)]
xs += [rnd.uniform(-1e-3, 1e-3) for _ in range(100)]
xs += [k * math.log(2) + d for k in (-1000, -100, -10, -1, 1, 10, 100, 1000) for d in (-1e-9, 0.0, 1e-9, 0.3, -0.3)]
xs += [0.0, 1.0, -1.0, 0.5, -0.5, 709.0, -708.0, -744.0, 1e-300, -1e-300]
lines = []
for x in xs:
    ref = float(Decimal(x).exp())
    lines.append(f"{bits(x)} {bits(ref)}")
write("exp.txt", lines)

# ---------------------------------------------------------------- normal cdf and quantile (SciPy: accurate tails)
from scipy import stats  # noqa: E402

rnd = random.Random(3)


def lower_tail_decimal(x: float) -> float:
    """Phi(x) for x <= -2 from Laplace's continued fraction at 60 digits, an independent derivation:
    SciPy's own erfc-based value carries a conditioning error of about x^2 * 1e-16 in this range."""
    a = Decimal(-x)
    t = a
    for n in range(600, 0, -1):
        t = a + Decimal(n) / t
    phi = (-(a * a) / 2).exp() / (2 * Decimal(math.pi)).sqrt()
    return float(phi / t)


lines = []
for _ in range(1200):
    x = rnd.uniform(-37.0, 8.4)
    ref = lower_tail_decimal(x) if x <= -2.0 else float(stats.norm.cdf(x))
    lines.append(f"cdf {bits(x)} {bits(ref)}")
for _ in range(1000):
    x = rnd.uniform(-8.4, 8.4)
    ref = lower_tail_decimal(x) if x <= -2.0 else float(stats.norm.cdf(x))
    lines.append(f"cdf {bits(x)} {bits(ref)}")
for _ in range(1200):
    p = 10.0 ** rnd.uniform(-300, -0.0001) if rnd.random() < 0.5 else rnd.uniform(1e-6, 1 - 1e-6)
    lines.append(f"inv {bits(p)} {bits(float(stats.norm.ppf(p)))}")
for p in (0.5, 0.8, 0.9, 0.95, 0.975, 0.99, 0.995, 0.999, 0.025, 0.005):
    lines.append(f"inv {bits(p)} {bits(float(stats.norm.ppf(p)))}")
write("normal.txt", lines)

# ---------------------------------------------------------------- binomial and Clopper-Pearson (SciPy)
rnd = random.Random(4)
lines = []
for _ in range(1500):
    n = rnd.choice([1, 2, 5, 10, 30, 100, 360, 1000, 3570, 10000, 100000])
    k = rnd.randrange(0, n + 1)
    p = rnd.choice([0.001, 0.01, 0.1, 0.5, 0.9, 0.99, 0.999, rnd.random()])
    lines.append(f"{k} {n} {bits(p)} {bits(float(stats.binom.cdf(k, n, p)))} {bits(float(stats.binom.sf(k, n, p)))}")
write("binomial.txt", lines)

lines = []
for _ in range(1500):
    n = rnd.choice([1, 2, 3, 5, 10, 20, 50, 100, 360, 1000, 3570, 20000])
    k = rnd.choice([0, n, rnd.randrange(0, n + 1), max(0, n - 1), min(n, 1), n // 2])
    alpha = rnd.choice([0.05, 0.01, 0.1, 0.2, 0.5])
    lo = 0.0 if k == 0 else float(stats.beta.ppf(alpha / 2, k, n - k + 1))
    hi = 1.0 if k == n else float(stats.beta.ppf(1 - alpha / 2, k + 1, n - k))
    lines.append(f"{k} {n} {bits(alpha)} {bits(lo)} {bits(hi)}")
write("clopper_pearson.txt", lines)

lines = []
skipped = 0
for n in [1, 2, 5, 10, 19, 20, 21, 59, 60, 100, 360, 1000, 5000]:
    for q in [0.05, 0.25, 0.5, 0.75, 0.9, 0.95, 0.99]:
        for alpha in [0.05, 0.1]:
            tail = alpha / 2
            eps = 1e-9 * tail  # skip records decided by a floating-point tie

            def clear(value, expect_below):
                return value < tail - eps if expect_below else value > tail + eps

            lower, upper, ok = "-", "-", True
            c0 = stats.binom.cdf(0, n, q)
            if c0 <= tail:
                l_ = max(i for i in range(1, n + 1) if stats.binom.cdf(i - 1, n, q) <= tail)
                lower = str(l_)
                ok &= clear(stats.binom.cdf(l_ - 1, n, q), True) and (l_ == n or clear(stats.binom.cdf(l_, n, q), False))
            else:
                ok &= clear(c0, False)
            s_ = stats.binom.sf(n - 1, n, q)
            if s_ <= tail:
                u_ = min(i for i in range(1, n + 1) if stats.binom.sf(i - 1, n, q) <= tail)
                upper = str(u_)
                ok &= clear(stats.binom.sf(u_ - 1, n, q), True) and (u_ == 1 or clear(stats.binom.sf(u_ - 2, n, q), False))
            else:
                ok &= clear(s_, False)
            if ok:
                lines.append(f"{n} {bits(q)} {bits(alpha)} {lower} {upper}")
            else:
                skipped += 1
print(f"quantile_ranks: skipped {skipped} tie cases")
write("quantile_ranks.txt", lines)


# ---------------------------------------------------------------- edit distance (pure-Python DP)
def dp(a, b):
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i] + [0] * len(b)
        for j, cb in enumerate(b, 1):
            cur[j] = min(prev[j - 1] + (ca != cb), prev[j] + 1, cur[j - 1] + 1)
        prev = cur
    return prev[-1]


rnd = random.Random(5)
alphabets = ["ab", "abcd", "abcdefghijklmnopqrstuvwxyz", "áéíóúñüßçÅØÆ0123456789", "αβγδεζηθικλμνξοπρστυφχψω", "日本語中文字漢字한국어"]
lines = []
for _ in range(450):
    alpha = rnd.choice(alphabets)
    la = rnd.choice([0, 1, 2, 5, 20, 63, 64, 65, 100, 128, 129, 150])
    a = "".join(rnd.choice(alpha) for _ in range(la))
    if rnd.random() < 0.5 and a:
        chars = list(a)
        for _ in range(rnd.randrange(0, 6)):
            if not chars:
                break
            at = rnd.randrange(len(chars))
            op = rnd.randrange(3)
            if op == 0:
                chars[at] = rnd.choice(alpha)
            elif op == 1:
                del chars[at]
            else:
                chars.insert(at, rnd.choice(alpha))
        b = "".join(chars)
    else:
        b = "".join(rnd.choice(alpha) for _ in range(rnd.choice([0, 1, 2, 5, 20, 63, 64, 65, 100, 128, 129, 150])))
    lines.append(f"{a.encode().hex() or '-'} {b.encode().hex() or '-'} {dp(a, b)}")
write("edit_distance.txt", lines)


# ---------------------------------------------------------------- xoshiro256** and SplitMix64 in Python
def splitmix(state):
    state = (state + 0x9E3779B97F4A7C15) & MASK
    z = state
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return state, z ^ (z >> 31)


def rotl(x, k):
    return ((x << k) | (x >> (64 - k))) & MASK


class Xoshiro:
    def __init__(self, seed):
        st = seed
        self.s = []
        for _ in range(4):
            st, out = splitmix(st)
            self.s.append(out)

    def next_u64(self):
        s = self.s
        result = (rotl((s[1] * 5) & MASK, 7) * 9) & MASK
        t = (s[1] << 17) & MASK
        s[2] ^= s[0]
        s[3] ^= s[1]
        s[1] ^= s[2]
        s[0] ^= s[3]
        s[2] ^= t
        s[3] = rotl(s[3], 45)
        return result

    def next_f64(self):
        return (self.next_u64() >> 11) * (1.0 / 9007199254740992.0)

    def below(self, n):
        if n == 0:
            return 0
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


lines = []
for seed in (0, 1, 42, MASK, 0xDEADBEEF, 20261029):
    g = Xoshiro(seed)
    lines.append(f"u64 {seed} " + " ".join(f"{g.next_u64():016x}" for _ in range(16)))
    g = Xoshiro(seed)
    lines.append(f"f64 {seed} " + " ".join(bits(g.next_f64()) for _ in range(16)))
    for bound in (1, 2, 7, 100, 1000, 3570, (1 << 63) + 12345):
        g = Xoshiro(seed)
        lines.append(f"below {seed} {bound} " + " ".join(str(g.below(bound)) for _ in range(16)))
write("rng.txt", lines)

# ---------------------------------------------------------------- exact sums (math.fsum)
rnd = random.Random(6)
lines = []
for _ in range(150):
    n = rnd.randrange(1, 130)
    xs = []
    for _ in range(n):
        xs.append(rnd.uniform(-1, 1) * 2.0 ** rnd.randrange(-30, 30))
    if rnd.random() < 0.3:
        xs += [1e16, -1e16]
        rnd.shuffle(xs)
    lines.append(" ".join(bits(x) for x in xs) + " = " + bits(math.fsum(xs)))
write("summation.txt", lines)
