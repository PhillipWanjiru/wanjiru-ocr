//! Levenshtein distance: the bit-parallel algorithm of Myers (1999), blocked for any length, and the
//! textbook dynamic programme that serves as its oracle.
//!
//! Costs are 1 for insertion, deletion and substitution. Sequences are slices of any ordered symbol
//! type (`char`, `u8`, `u32`, words as `&str`).

use std::collections::BTreeMap;

/// Textbook dynamic programme: `O(n m)` time, `O(min(n, m))` memory. This is the oracle for
/// [`levenshtein`]; use it in tests, not in hot paths.
#[must_use]
pub fn levenshtein_dp<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let mut prev: Vec<usize> = (0..=short.len()).collect();
    let mut cur = vec![0_usize; short.len() + 1];
    for (i, x) in long.iter().enumerate() {
        cur[0] = i + 1;
        for (j, y) in short.iter().enumerate() {
            let substitute = prev[j] + usize::from(x != y);
            cur[j + 1] = substitute.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[short.len()]
}

/// Levenshtein distance by the bit-parallel algorithm of Myers, in blocks of 64 pattern symbols:
/// `O(n * ceil(m / 64))` time for lengths `n >= m`, with `O(m / 64)` words of state per distinct symbol.
#[must_use]
pub fn levenshtein<T: Ord>(a: &[T], b: &[T]) -> usize {
    // The pattern is the shorter sequence: fewer 64-bit blocks per text symbol.
    let (pattern, text) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let m = pattern.len();
    if m == 0 {
        return text.len();
    }
    let blocks = m.div_ceil(64);
    let last_bit: u64 = 1 << ((m - 1) % 64);

    // Peq: for every distinct pattern symbol, a bit-vector of `blocks` words marking its positions.
    let mut peq: BTreeMap<&T, Vec<u64>> = BTreeMap::new();
    for (i, symbol) in pattern.iter().enumerate() {
        peq.entry(symbol).or_insert_with(|| vec![0; blocks])[i / 64] |= 1 << (i % 64);
    }
    let zeros = vec![0_u64; blocks];
    let mut pv = vec![!0_u64; blocks];
    let mut mv = vec![0_u64; blocks];
    let mut score = m;

    for symbol in text {
        let eq = peq.get(symbol).map_or(&zeros[..], Vec::as_slice);
        // Horizontal delta entering the first block: D[0][j] - D[0][j-1] = +1 for global alignment.
        let mut carry: i8 = 1;
        for block in 0..blocks {
            let high = if block + 1 == blocks {
                last_bit
            } else {
                1 << 63
            };
            let (plus, minus, out) = step_block(pv[block], mv[block], eq[block], carry, high);
            pv[block] = plus;
            mv[block] = minus;
            carry = out;
        }
        // `carry` is now the horizontal delta at the pattern's last row.
        match carry {
            1 => score += 1,
            -1 => score -= 1,
            _ => {}
        }
    }
    score
}

/// Levenshtein distance between two strings, over Unicode scalar values.
#[must_use]
pub fn levenshtein_chars(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    levenshtein(&a, &b)
}

/// One block of the Myers recurrence. `carry` is the horizontal delta (-1, 0, +1) entering the
/// block's top row; the returned delta is the one leaving at `high`, the block's last relevant row.
#[inline]
fn step_block(pv: u64, mv: u64, eq: u64, carry: i8, high: u64) -> (u64, u64, i8) {
    let xv = eq | mv;
    let eq_in = if carry < 0 { eq | 1 } else { eq };
    let xh = ((eq_in & pv).wrapping_add(pv) ^ pv) | eq_in;
    let mut ph = mv | !(xh | pv);
    let mut mh = pv & xh;
    let out = i8::from(ph & high != 0) - i8::from(mh & high != 0);
    ph <<= 1;
    mh <<= 1;
    if carry < 0 {
        mh |= 1;
    } else if carry > 0 {
        ph |= 1;
    }
    (mh | !(xv | ph), ph & xv, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    fn random_seq(rng: &mut Rng, alphabet: u64, len: usize) -> Vec<u8> {
        (0..len)
            .map(|_| u8::try_from(rng.below(alphabet)).expect("small alphabet"))
            .collect()
    }

    #[test]
    fn textbook_examples() {
        assert_eq!(levenshtein_chars("kitten", "sitting"), 3);
        assert_eq!(levenshtein_chars("flaw", "lawn"), 2);
        assert_eq!(levenshtein_chars("", "abc"), 3);
        assert_eq!(levenshtein_chars("abc", ""), 3);
        assert_eq!(levenshtein_chars("", ""), 0);
        assert_eq!(levenshtein_chars("same", "same"), 0);
        assert_eq!(levenshtein_dp(&['a', 'b'], &['b', 'a']), 2);
    }

    #[test]
    fn counts_unicode_scalars_not_bytes() {
        assert_eq!(levenshtein_chars("é", "e"), 1);
        assert_eq!(levenshtein_chars("naïve", "naive"), 1);
        assert_eq!(levenshtein_chars("Ωmega", "omega"), 1);
        assert_eq!(levenshtein_chars("日本語", "日本"), 1);
    }

    #[test]
    fn myers_equals_dynamic_programme_on_one_hundred_thousand_random_pairs() {
        let mut rng = Rng::seed_from_u64(20_261_029);
        for case in 0..100_000_u32 {
            // Mostly short sequences (cheap oracle), with a long tail that crosses the 64- and
            // 128-symbol block boundaries.
            let (max_len, alphabet) = if case % 33 == 0 {
                (300, 2 + rng.below(5))
            } else {
                (70, 2 + rng.below(5))
            };
            let la = usize::try_from(rng.below(max_len + 1)).expect("fits");
            let lb = usize::try_from(rng.below(max_len + 1)).expect("fits");
            let a = random_seq(&mut rng, alphabet, la);
            // Half of the time make `b` a noisy copy of `a`, so that distances are small and
            // alignments are non-trivial.
            let b = if case % 2 == 0 {
                let mut b = a.clone();
                for _ in 0..rng.below(8) {
                    if b.is_empty() {
                        break;
                    }
                    let at = usize::try_from(rng.below(b.len() as u64)).expect("fits");
                    match rng.below(3) {
                        0 => b[at] = u8::try_from(rng.below(alphabet)).expect("small"),
                        1 => {
                            b.remove(at);
                        }
                        _ => b.insert(at, u8::try_from(rng.below(alphabet)).expect("small")),
                    }
                }
                b
            } else {
                random_seq(&mut rng, alphabet, lb)
            };
            let fast = levenshtein(&a, &b);
            let slow = levenshtein_dp(&a, &b);
            assert_eq!(fast, slow, "case {case}: a={a:?} b={b:?}");
        }
    }

    #[test]
    fn block_boundaries_are_exact() {
        let mut rng = Rng::seed_from_u64(7);
        for len in [
            0_usize, 1, 2, 63, 64, 65, 127, 128, 129, 191, 192, 193, 256, 257,
        ] {
            for other in [0_usize, 1, 63, 64, 65, 128, 130, 257] {
                let a = random_seq(&mut rng, 3, len);
                let b = random_seq(&mut rng, 3, other);
                assert_eq!(
                    levenshtein(&a, &b),
                    levenshtein_dp(&a, &b),
                    "lengths {len} and {other}"
                );
                assert_eq!(levenshtein(&a, &a), 0);
                assert_eq!(levenshtein(&a, &b), levenshtein(&b, &a), "symmetry");
            }
        }
    }

    #[test]
    fn adversarial_repeats() {
        let all_a = vec![b'a'; 200];
        let mut one_b = all_a.clone();
        one_b[100] = b'b';
        assert_eq!(levenshtein(&all_a, &one_b), 1);
        let half = vec![b'a'; 100];
        assert_eq!(levenshtein(&all_a, &half), 100);
        let zigzag: Vec<u8> = (0..200)
            .map(|i| u8::try_from(i % 2).expect("bit"))
            .collect();
        let shifted: Vec<u8> = (0..200)
            .map(|i| u8::try_from((i + 1) % 2).expect("bit"))
            .collect();
        assert_eq!(
            levenshtein(&zigzag, &shifted),
            levenshtein_dp(&zigzag, &shifted)
        );
    }

    #[test]
    fn triangle_inequality_holds_on_samples() {
        let mut rng = Rng::seed_from_u64(99);
        for _ in 0..500 {
            let a = random_seq(&mut rng, 4, 40);
            let b = random_seq(&mut rng, 4, 45);
            let c = random_seq(&mut rng, 4, 50);
            assert!(levenshtein(&a, &c) <= levenshtein(&a, &b) + levenshtein(&b, &c));
        }
    }

    #[test]
    fn works_on_words() {
        let a = ["the", "quick", "brown", "fox"];
        let b = ["the", "slow", "brown", "dog", "jumps"];
        assert_eq!(levenshtein(&a, &b), 3);
    }
}
