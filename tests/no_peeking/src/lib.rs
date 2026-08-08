//! # The no-peeking harness
//!
//! `docs/NO-PEEKING-FEASIBILITY.md` measured the platform and recommended one
//! construction: a **sealed dealer**, a separate canister with an empty controller
//! list, holding the seed and the cards while the fund-holding table holds the
//! money and the betting. It proved every management-canister premise of that
//! construction against a replica. It did **not** build one, and it said so: the
//! open questions were the showdown split, the street-advance rule, and what
//! happens to a hand in flight if the dealer freezes.
//!
//! This crate answers those by executing them.
//!
//! | file | what it proves |
//! |---|---|
//! | `tests/no_peek.rs` | **The negative.** Every exported method of both canisters, called as the table's controller, as an opponent, as a stranger and anonymously, yields no card of a live hand. Plus the canister-snapshot read that defeated the obvious fix, run against both canisters. |
//! | `tests/full_hand.rs` | A whole hand — blinds, betting, an all-in, side pots, showdown, settlement — where the table never holds a card, and the shuffle is still reproducible by an outsider from the revealed seed. |
//! | `tests/disconnect.rs` | The failure mode these protocols die on. A silent player, a dead table, and a dealer that cannot be topped up. |
//! | `tests/measurements.rs` | Cycles per hand and rounds per deal, against the live engine's own numbers. |
//!
//! # Run it
//!
//! ```text
//! cd tests/no_peeking
//! cargo test -- --nocapture --test-threads=2
//! ```
//!
//! A PocketIC server binary is required: `$POCKET_IC_BIN`, else
//! `$(dfx cache show)/pocket-ic`. LOCAL ONLY; nothing in this crate can reach
//! mainnet, because nothing in it knows a mainnet canister id.

pub mod build;
pub mod peek;
pub mod wasm_exports;
pub mod world;

/// Print a line that is part of the evidence, not debug noise.
#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => {{
        println!($($arg)*);
    }};
}
