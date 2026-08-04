//! The thing under test: ClearDeck's real engine, reached only through
//! `poker_core`'s public API.
//!
//! There is no re-implementation here on purpose. `src/table_canister` used to
//! carry a private copy of the evaluator in its own test file and the two copies
//! had already drifted, so its green tests proved nothing about the deployed
//! canister. This harness links the *same* `poker_core::evaluate_five_cards` /
//! `poker_core::evaluate_hand` that `table_canister::showdown` calls.

use poker_core::{evaluate_five_cards, evaluate_hand, Card as CdCard, HandRank};

use crate::cards::{to_cleardeck, CardIdx};

/// ClearDeck's 5-card evaluation.
pub fn ours_five(cards: &[CardIdx; 5]) -> HandRank {
    // Stack array, not a `Vec`: this is called 2,598,960 times by the exhaustive
    // sweep and the allocation dominated the profile.
    let converted: [CdCard; 5] = std::array::from_fn(|i| to_cleardeck(cards[i]));
    evaluate_five_cards(&converted)
}

/// ClearDeck's 7-card evaluation, entered exactly the way the canister enters it:
/// two hole cards plus a five-card community board.
pub fn ours_seven(cards: &[CardIdx; 7]) -> HandRank {
    let hole = (to_cleardeck(cards[0]), to_cleardeck(cards[1]));
    let community: Vec<CdCard> = cards[2..].iter().map(|&c| to_cleardeck(c)).collect();
    evaluate_hand(&hole, &community)
}

/// `evaluate_hand` with an arbitrary board length: `cards[0..2]` are the hole
/// cards, the rest is the community. Used only by the degenerate-input probes.
pub fn ours_hand(cards: &[CardIdx]) -> HandRank {
    assert!(cards.len() >= 2, "a hold'em player always has 2 hole cards");
    let hole = (to_cleardeck(cards[0]), to_cleardeck(cards[1]));
    let community: Vec<CdCard> = cards[2..].iter().map(|&c| to_cleardeck(c)).collect();
    evaluate_hand(&hole, &community)
}

/// `evaluate_five_cards` with an arbitrary card count. The name promises five;
/// nothing in the signature enforces it, and the degenerate probes measure what
/// the function actually does when handed six or seven.
pub fn ours_five_cards_raw(cards: &[CardIdx]) -> HandRank {
    let converted: Vec<CdCard> = cards.iter().map(|&c| to_cleardeck(c)).collect();
    evaluate_five_cards(&converted)
}

/// ClearDeck's own comparison: the derived `Ord` on `HandRank`. This, not any
/// numeric score, is what `table_canister` uses to decide who takes a pot, so
/// it is the thing the ordering checks must exercise.
pub fn ours_cmp(a: &HandRank, b: &HandRank) -> std::cmp::Ordering {
    a.cmp(b)
}

/// Renders a `HandRank` the way it appears on the Candid wire, for repro cases.
pub fn render(rank: &HandRank) -> String {
    format!("{rank:?}")
}
