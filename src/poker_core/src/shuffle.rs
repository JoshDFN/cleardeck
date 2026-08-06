//! Provably-fair deterministic shuffle.
//!
//! Players verify past hands by re-running this function with the revealed seed,
//! so its output for a given seed is a PUBLIC COMMITMENT. The normative
//! description of the algorithm lives in **`docs/SHUFFLE-SPEC.md`**; this file and
//! that document must always describe the same procedure, because a third party
//! reimplementing from the document has to reproduce this code bit for bit.
//!
//! # History (read before editing)
//!
//! The version extracted from `src/table_canister/src/lib.rs` at commit `ceacc37`
//! computed the swap index as:
//!
//! ```text
//! let j = (random_value as usize) % (i + 1);   // BUG
//! ```
//!
//! `random_value` is a `u64` and `usize` is **32 bits on wasm32-unknown-unknown**,
//! so on-chain the draw was silently truncated to its low 32 bits while every
//! native reimplementation (and every native test) used all 64. The same source
//! produced two different shuffles and no third party could reproduce a deal from
//! the revealed seed. See `docs/FINDING-02-shuffle-not-verifiable.md`.
//!
//! Two rules follow from that, and both are load-bearing:
//!
//! 1. **A value drawn from the hash chain must never pass through `usize`.** The
//!    only platform-sized integer in this file is the deck index `i`, which is
//!    widened to `u64` before it meets any drawn value. The single narrowing cast
//!    is `(value % n) as usize`, whose result is provably `< n <= deck.len()`.
//! 2. **Tests must run on wasm32.** A native-only golden test cannot catch this
//!    class of defect; that is precisely how it survived. See
//!    `tests/wasm32_golden.rs`.

use crate::card::Card;
use sha2::{Digest, Sha256};

/// Exclusive upper bound on accepted draws for modulus `n`: `floor(2^64 / n) * n`.
///
/// A 64-bit draw reduced mod `n` is uniform only if the draw came from a range
/// whose size is an exact multiple of `n`. `2^64` is not a multiple of `n` unless
/// `n` is a power of two, so the tail `[bound, 2^64)` -- the final, short bucket --
/// must be rejected rather than reduced.
///
/// Returned as `u128` because the bound is exactly `2^64` whenever `n` is a power
/// of two (nothing is rejected then), which does not fit in a `u64`. `u128` is 128
/// bits on every target, so this is width-independent by construction.
///
/// # Panics
///
/// Panics if `n == 0`, which is unreachable from [`shuffle_deck`] (`n = i + 1`
/// with `i >= 1`).
#[inline]
pub fn draw_bound(n: u64) -> u128 {
    assert!(n != 0, "draw_bound: modulus must be non-zero");
    let n = n as u128;
    (TWO_POW_64 / n) * n
}

/// `2^64`, the number of distinct values a 64-bit draw can take.
const TWO_POW_64: u128 = 1u128 << 64;

/// Draws a uniformly distributed value in `0..n` by rejection sampling.
///
/// `next_draw` yields successive 64-bit values from the hash chain. Values at or
/// above [`draw_bound`] are rejected and `next_draw` is called again; the first
/// accepted value is reduced mod `n`.
///
/// The rejection probability is `(2^64 mod n) / 2^64`, which over the `n` a 52-card
/// shuffle uses (`2..=52`) peaks at `41 / 2^64` (~2.2e-18, at `n = 43`): the loop
/// is expected to run exactly once, and
/// the branch exists so that the algorithm is defensible rather than merely
/// deterministic. It is exercised directly by the unit tests below, because no
/// reachable seed can exercise it.
///
/// # Panics
///
/// Panics if `n == 0`.
#[inline]
pub fn uniform_below(n: u64, mut next_draw: impl FnMut() -> u64) -> u64 {
    let bound = draw_bound(n);
    loop {
        let value = next_draw();
        if u128::from(value) < bound {
            return value % n;
        }
    }
}

/// One link of the hash chain: `SHA256(chain || [counter])`, and the low 8 bytes
/// of that digest read little-endian as the 64-bit draw.
///
/// Returns `(next_chain_state, draw)`. Every digest produced -- including the ones
/// whose draw is rejected -- advances the chain, which is what makes a rejection
/// re-draw a *different* value instead of looping forever.
#[inline]
fn chain_step(chain: &[u8], counter: u8) -> ([u8; 32], u64) {
    let mut hasher = Sha256::new();
    hasher.update(chain);
    hasher.update([counter]);
    let digest: [u8; 32] = hasher.finalize().into();

    let draw = u64::from_le_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]);
    (digest, draw)
}

/// Shuffles `deck` in place with a Fisher-Yates pass driven by a SHA-256 hash
/// chain rooted at `seed`.
///
/// Deterministic: the same seed always produces the same deck order, on every
/// target, in every language. That is the whole point -- the canister commits to
/// `SHA256(seed)` before the deal and reveals `seed` when the hand ends, so a
/// player can re-derive the exact cards they were dealt.
///
/// # Algorithm
///
/// Normative text and a worked example: `docs/SHUFFLE-SPEC.md`.
///
/// ```text
/// chain <- seed
/// for i from deck.len()-1 down to 1:
///     n <- i + 1
///     bound <- floor(2^64 / n) * n
///     repeat:
///         chain <- SHA256(chain || [i mod 256])
///         draw  <- u64 little-endian from chain[0..8]
///     until draw < bound
///     j <- draw mod n
///     swap deck[i] and deck[j]
/// ```
pub fn shuffle_deck(deck: &mut Vec<Card>, seed: &[u8]) {
    // The chain state is a 32-byte digest after the first step; before that it is
    // the seed, which is variable length.
    let mut chain: Vec<u8> = seed.to_vec();

    for i in (1..deck.len()).rev() {
        // `i` is the ONLY platform-sized integer in this loop, and it is widened
        // here, before it meets anything drawn from the chain. `i < deck.len()`
        // and a deck is 52 cards, so no target-dependent truncation is possible
        // in either direction.
        let n = i as u64 + 1;

        // Domain separator for this step. For a 52-card deck `i <= 51`, so this
        // is exactly `i`; the mask makes the expression total instead of relying
        // on that bound.
        let counter = (i & 0xFF) as u8;

        // `uniform_below` returns a value already reduced mod `n`.
        let j = uniform_below(n, || {
            let (digest, value) = chain_step(&chain, counter);
            chain = digest.to_vec();
            value
        });

        // `j < n <= deck.len()`, so this is the one narrowing cast in the file and
        // it cannot truncate on any target with a pointer at least 16 bits wide.
        // NEVER cast a raw draw to `usize`.
        let j = j as usize;

        deck.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::create_deck;

    #[test]
    fn draw_bound_is_the_largest_multiple_of_n_below_two_pow_64() {
        for n in 1u64..=52 {
            let bound = draw_bound(n);
            assert_eq!(bound % u128::from(n), 0, "bound must be a multiple of n={}", n);
            assert!(bound <= TWO_POW_64, "bound must not exceed 2^64 for n={}", n);
            assert!(
                TWO_POW_64 - bound < u128::from(n),
                "bound must be the LARGEST such multiple for n={}",
                n
            );
        }
    }

    #[test]
    fn draw_bound_rejects_nothing_for_powers_of_two() {
        for shift in 0..=6 {
            let n = 1u64 << shift;
            assert_eq!(
                draw_bound(n),
                TWO_POW_64,
                "a power-of-two modulus has no short bucket (n={})",
                n
            );
        }
    }

    #[test]
    fn draw_bound_for_a_52_card_deck_matches_the_spec_constant() {
        // Written out so docs/SHUFFLE-SPEC.md can be checked against code.
        // 2^64 = 18446744073709551616 = 52 * 354745078340568300 + 16
        // floor(2^64 / 52) * 52 = 18446744073709551600
        assert_eq!(draw_bound(52), 18_446_744_073_709_551_600u128);
        assert_eq!(TWO_POW_64 - draw_bound(52), 16, "n=52 rejects exactly 16 values");
        // The first step of a 52-card shuffle uses n = 52; the last uses n = 2.
        assert_eq!(draw_bound(2), TWO_POW_64, "n=2 rejects nothing");
        assert_eq!(TWO_POW_64 - draw_bound(51), 1, "n=51 rejects exactly 1 value");
        // The worst case over the moduli a 52-card shuffle uses.
        assert_eq!(TWO_POW_64 - draw_bound(43), 41, "n=43 rejects exactly 41 values");
        assert_eq!(
            (2u64..=52).map(|n| TWO_POW_64 - draw_bound(n)).max(),
            Some(41),
            "the documented worst-case rejection count changed"
        );
    }

    #[test]
    fn uniform_below_rejects_the_short_bucket_and_redraws() {
        // n = 52 accepts [0, 2^64 - 4). Feed a rejected value first, then an
        // accepted one, and prove the rejected draw is not what gets reduced.
        let rejected = u64::MAX - 1; // >= bound, must be discarded
        let accepted = 100u64;
        let mut queue = vec![accepted, rejected].into_iter().collect::<Vec<_>>();
        let got = uniform_below(52, || queue.pop().expect("extra draw requested"));
        assert_eq!(got, accepted % 52);
        assert!(queue.is_empty(), "both draws must have been consumed");

        // Sanity: the value we called "rejected" really is in the short bucket,
        // and reducing it would have given a DIFFERENT answer, so the test would
        // fail if the rejection rule were dropped.
        assert!(u128::from(rejected) >= draw_bound(52));
        assert_ne!(rejected % 52, accepted % 52);
    }

    #[test]
    fn uniform_below_accepts_the_value_immediately_below_the_bound() {
        let bound = draw_bound(52);
        let last_accepted = (bound - 1) as u64;
        let mut calls = 0;
        let got = uniform_below(52, || {
            calls += 1;
            last_accepted
        });
        assert_eq!(calls, 1, "a value below the bound must be accepted at once");
        assert_eq!(got, last_accepted % 52);
    }

    #[test]
    fn uniform_below_is_exhaustively_uniform_over_a_small_modulus() {
        // Every accepted residue must be reachable, and reduction must be plain
        // `mod n`. Walk a small window at the bottom of the range.
        for n in 2u64..=52 {
            let mut seen = vec![false; n as usize];
            for v in 0..(n * 3) {
                let got = uniform_below(n, || v);
                assert!(got < n);
                seen[got as usize] = true;
            }
            assert!(seen.iter().all(|s| *s), "not every residue reachable for n={}", n);
        }
    }

    /// The chain is single-threaded: EVERY digest advances it, including rejected
    /// ones. A verifier that only advances on acceptance would diverge, so pin it.
    #[test]
    fn chain_step_advances_deterministically() {
        let (c1, d1) = chain_step(b"seed", 51);
        let (c2, d2) = chain_step(&c1, 51);
        assert_ne!(c1, c2, "re-hashing with the same counter must move the chain");
        assert_ne!(d1, d2);

        // Independently recomputed: SHA256("seed" || 0x33).
        let mut h = Sha256::new();
        h.update(b"seed");
        h.update([51u8]);
        let expected: [u8; 32] = h.finalize().into();
        assert_eq!(c1, expected);
        assert_eq!(d1, u64::from_le_bytes(expected[0..8].try_into().unwrap()));
    }

    #[test]
    fn shuffle_is_a_permutation_and_is_deterministic() {
        let original = create_deck();
        let mut a = original.clone();
        let mut b = original.clone();
        shuffle_deck(&mut a, b"cleardeck-spec-example-seed");
        shuffle_deck(&mut b, b"cleardeck-spec-example-seed");
        assert_eq!(a, b, "same seed must give the same deck");
        assert_eq!(a.len(), 52);
        for card in &original {
            assert!(a.contains(card), "card lost by the shuffle: {:?}", card);
        }
        assert_ne!(a, original, "a 52-card shuffle must move something");
    }

    /// Guards the exact defect this file was rewritten to fix. Truncating the draw
    /// to 32 bits before the modulo changes the result, so a shuffle computed the
    /// old way must NOT match the current one.
    #[test]
    fn the_32_bit_truncating_shuffle_is_no_longer_what_we_compute() {
        fn shuffle_as_wasm32_used_to(deck: &mut Vec<Card>, seed: &[u8]) {
            let mut hash_input = seed.to_vec();
            for i in (1..deck.len()).rev() {
                let mut hasher = Sha256::new();
                hasher.update(&hash_input);
                hasher.update([i as u8]);
                let hash_result = hasher.finalize();
                let random_value = u64::from_le_bytes([
                    hash_result[0], hash_result[1], hash_result[2], hash_result[3],
                    hash_result[4], hash_result[5], hash_result[6], hash_result[7],
                ]);
                // The old line, with `usize` pinned to the 32 bits wasm32 gave it.
                let j = ((random_value as u32) as u64 % (i as u64 + 1)) as usize;
                deck.swap(i, j);
                hash_input = hash_result.to_vec();
            }
        }

        let seed = b"finding-02";
        let mut now = create_deck();
        shuffle_deck(&mut now, seed);
        let mut then = create_deck();
        shuffle_as_wasm32_used_to(&mut then, seed);
        assert_ne!(
            now, then,
            "the truncating shuffle and the fixed shuffle must differ; if they \
             agree, the width-independent arithmetic has been reverted"
        );
    }
}
