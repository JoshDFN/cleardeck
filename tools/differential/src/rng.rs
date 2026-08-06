//! A deterministic, dependency-free PRNG so every run is byte-for-byte
//! reproducible from its seed.
//!
//! SplitMix64 (Steele/Lea/Flood) is used rather than `rand` for two reasons:
//! `rand`'s stream is not stable across semver bumps, and `poker_core`'s own
//! golden-vector tests already use SplitMix64, so a repro printed by this
//! harness can be replayed there without a second RNG to reason about.

#[derive(Clone, Copy, Debug)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` with rejection sampling, so there is no modulo bias
    /// that could quietly bias which hands get sampled.
    #[inline]
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "below(0) is undefined");
        let zone = u64::MAX - (u64::MAX % n) - 1;
        loop {
            let v = self.next_u64();
            if v <= zone {
                return v % n;
            }
        }
    }

    /// Deals `k` distinct cards from a 52-card deck by partial Fisher-Yates.
    /// Distinctness is structural, not probabilistic: a duplicate card can never
    /// be produced, so a duplicate in a repro case means a harness bug.
    pub fn deal(&mut self, k: usize) -> Vec<u8> {
        assert!(k <= 52, "cannot deal {k} cards from 52");
        let mut deck: [u8; 52] = [0; 52];
        for (i, slot) in deck.iter_mut().enumerate() {
            *slot = i as u8;
        }
        for i in 0..k {
            let j = i + self.below((52 - i) as u64) as usize;
            deck.swap(i, j);
        }
        deck[..k].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deal_never_repeats_a_card() {
        let mut rng = SplitMix64::new(1);
        for _ in 0..20_000 {
            let cards = rng.deal(7);
            let mut sorted = cards.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), 7, "duplicate card dealt: {cards:?}");
            assert!(cards.iter().all(|&c| c < 52));
        }
    }

    #[test]
    fn same_seed_same_stream() {
        let a: Vec<u64> = (0..8)
            .scan(SplitMix64::new(0xDEAD_BEEF), |r, _| Some(r.next_u64()))
            .collect();
        let b: Vec<u64> = (0..8)
            .scan(SplitMix64::new(0xDEAD_BEEF), |r, _| Some(r.next_u64()))
            .collect();
        assert_eq!(a, b);
    }
}
