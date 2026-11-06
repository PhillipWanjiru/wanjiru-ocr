//! A seeded random generator specified by its algorithm, not by a library version.
//!
//! `Rng` is xoshiro256** (Blackman and Vigna, 2018), seeded through SplitMix64. The first outputs for
//! every seed are fixed forever; tests pin them, and `tools/oracles` re-implements the generator in
//! Python so that the evaluation harness can be reproduced outside Rust.

/// SplitMix64: used to expand a 64-bit seed into the 256-bit state of [`Rng`].
#[derive(Copy, Clone, Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// Start the sequence at `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Next 64-bit output.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// xoshiro256** with 256 bits of state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    /// Seed the generator: four SplitMix64 outputs become the state.
    #[must_use]
    pub fn seed_from_u64(seed: u64) -> Self {
        let mut sm = SplitMix64::new(seed);
        Self {
            s: [sm.next_u64(), sm.next_u64(), sm.next_u64(), sm.next_u64()],
        }
    }

    /// Next 64-bit output.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// A uniform value in `[0, 1)` with 53 random bits.
    pub fn next_f64(&mut self) -> f64 {
        #[allow(
            clippy::cast_precision_loss,
            reason = "53 bits fit a f64 mantissa exactly"
        )]
        let bits = (self.next_u64() >> 11) as f64;
        bits * (1.0 / 9_007_199_254_740_992.0)
    }

    /// An unbiased uniform integer in `[0, n)`, by Lemire's multiply-and-reject method.
    /// Returns 0 when `n == 0`.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let mut x = self.next_u64();
        let mut m = u128::from(x) * u128::from(n);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "low 64 bits of the product, by design"
        )]
        let mut low = m as u64;
        if low < n {
            let threshold = n.wrapping_neg() % n;
            while low < threshold {
                x = self.next_u64();
                m = u128::from(x) * u128::from(n);
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "low 64 bits of the product, by design"
                )]
                {
                    low = m as u64;
                }
            }
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "the high 64 bits are below n"
        )]
        let high = (m >> 64) as u64;
        high
    }

    /// An unbiased uniform index in `[0, n)`.
    pub fn below_usize(&mut self, n: usize) -> usize {
        usize::try_from(self.below(n as u64)).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_the_published_sequence_for_seed_zero() {
        let mut sm = SplitMix64::new(0);
        assert_eq!(sm.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(sm.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(sm.next_u64(), 0x06C4_5D18_8009_454F);
        assert_eq!(sm.next_u64(), 0xF88B_B8A8_724C_81EC);
    }

    #[test]
    fn same_seed_same_stream_different_seed_different_stream() {
        let mut a = Rng::seed_from_u64(42);
        let mut b = Rng::seed_from_u64(42);
        let mut c = Rng::seed_from_u64(43);
        let (xa, xb, xc): (Vec<u64>, Vec<u64>, Vec<u64>) = (
            (0..8).map(|_| a.next_u64()).collect(),
            (0..8).map(|_| b.next_u64()).collect(),
            (0..8).map(|_| c.next_u64()).collect(),
        );
        assert_eq!(xa, xb);
        assert_ne!(xa, xc);
    }

    #[test]
    fn below_is_in_range_and_roughly_uniform() {
        let mut rng = Rng::seed_from_u64(1);
        let mut counts = [0_u32; 7];
        for _ in 0..70_000 {
            let v = rng.below(7);
            assert!(v < 7);
            counts[usize::try_from(v).expect("small")] += 1;
        }
        for c in counts {
            // Expected 10,000 each; the standard deviation is about 93.
            assert!((9_500..10_500).contains(&c), "{counts:?}");
        }
        assert_eq!(rng.below(0), 0);
        assert_eq!(rng.below(1), 0);
    }

    #[test]
    fn unit_interval_is_half_open_with_mean_one_half() {
        let mut rng = Rng::seed_from_u64(5);
        let mut sum = 0.0;
        for _ in 0..100_000 {
            let x = rng.next_f64();
            assert!((0.0..1.0).contains(&x));
            sum += x;
        }
        assert!((sum / 100_000.0 - 0.5).abs() < 0.005);
    }
}
