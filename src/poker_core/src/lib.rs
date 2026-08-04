//! `poker_core` -- the pure, host-testable heart of the ClearDeck poker engine.
//!
//! Everything in this crate was MOVED out of `src/table_canister/src/lib.rs`
//! (baseline commit `ceacc37`) so that tests, differential harnesses, fuzzers and
//! hand-replay tools bind to the SAME code the deployed canister runs. Before
//! this crate existed, `src/table_canister/tests/unit_tests.rs` carried its own
//! private re-implementation of all of it, and the two copies had already
//! diverged -- so its green tests proved nothing about the deployed engine.
//!
//! Invariants for anyone editing this crate:
//!
//! * **No `ic-cdk`.** This crate must compile for the host target. Diagnostics are
//!   returned to the caller as strings instead of being printed.
//! * **The Candid wire is frozen.** `Suit`, `Rank`, `Card`, `HandRank` and
//!   `SidePot` are re-exported by `table_canister` and appear in its `.did` and in
//!   its stable-memory serialisation. Field names, variant names and variant
//!   ORDER must not change: `HandRank`'s derived `Ord` is what decides who wins a
//!   pot, and its discriminant order is what encodes a hand on the wire.
//! * **Behaviour is pinned by golden vectors.** `tests/golden_vectors.rs` replays
//!   ~15,700 vectors captured from the pre-refactor engine. If you intend to
//!   change behaviour you must regenerate them deliberately, never casually.

pub mod card;
pub mod hand;
pub mod shuffle;
pub mod side_pots;

pub use card::{create_deck, Card, Rank, Suit};
pub use hand::{
    check_straight, combinations, detect_straight, evaluate_five_cards, evaluate_hand,
    get_straight_high, HandRank,
};
pub use shuffle::shuffle_deck;
pub use side_pots::{
    apply_side_pots, build_side_pots, build_side_pots_logged, Contribution, SidePot,
};
