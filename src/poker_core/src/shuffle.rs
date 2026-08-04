//! Provably-fair deterministic shuffle.
//!
//! MOVED VERBATIM out of `src/table_canister/src/lib.rs` (commit ceacc37).
//! Players verify past hands by re-running this function with the revealed
//! seed, so its output for a given seed is a PUBLIC COMMITMENT. Any change to
//! the hashing or index arithmetic breaks verifiability of every hand already
//! played. It is covered by `tests/golden_vectors.rs`.

use crate::card::Card;
use sha2::{Digest, Sha256};

/// Shuffles the deck using Fisher-Yates algorithm with SHA256 hash chaining.
///
/// This is a deterministic shuffle - the same seed always produces the same deck order.
/// The algorithm is provably fair because:
/// 1. The seed comes from IC's VRF (Verifiable Random Function)
/// 2. SHA256 hash chaining ensures each swap is unpredictable without the seed
/// 3. Anyone can verify by re-running this function with the revealed seed
///
/// # Algorithm
/// For each position i from 51 down to 1:
///   1. Hash(previous_hash || i) to get deterministic randomness
///   2. Select position j = random_value mod (i+1)
///   3. Swap cards at positions i and j
pub fn shuffle_deck(deck: &mut Vec<Card>, seed: &[u8]) {
    let mut hash_input = seed.to_vec();

    for i in (1..deck.len()).rev() {
        let mut hasher = Sha256::new();
        hasher.update(&hash_input);
        hasher.update(&[i as u8]);
        let hash_result = hasher.finalize();

        // SHA256 always produces 32 bytes, so this slice is always valid
        let random_value = u64::from_le_bytes([
            hash_result[0], hash_result[1], hash_result[2], hash_result[3],
            hash_result[4], hash_result[5], hash_result[6], hash_result[7],
        ]);
        let j = (random_value as usize) % (i + 1);

        deck.swap(i, j);
        hash_input = hash_result.to_vec();
    }
}
