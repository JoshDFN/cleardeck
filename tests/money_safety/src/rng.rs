//! A deterministic PRNG with no dependencies, so a fuzz seed reproduces exactly
//! the same sequence on any machine and in any future version of this crate.
//! SplitMix64 (Steele/Lea/Flood), the standard seeding generator.

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, n)`. `n == 0` yields 0.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        self.next_u64() % n
    }

    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        if hi <= lo {
            return lo;
        }
        lo + self.below(hi - lo)
    }

    pub fn index(&mut self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        (self.next_u64() % len as u64) as usize
    }

    /// True with probability `numerator/denominator`.
    pub fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.below(denominator) < numerator
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.index(items.len())]
    }

    /// Weighted choice over `(weight, value)` pairs.
    pub fn weighted<'a, T>(&mut self, items: &'a [(u64, T)]) -> &'a T {
        let total: u64 = items.iter().map(|(w, _)| *w).sum();
        let mut r = self.below(total.max(1));
        for (w, v) in items {
            if r < *w {
                return v;
            }
            r -= *w;
        }
        &items[items.len() - 1].1
    }
}
