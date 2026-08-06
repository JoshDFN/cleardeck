//! Guards on the seam between `table_canister` and `poker_core`.
//!
//! WHAT USED TO BE HERE: this file carried private copies of `Suit`, `Rank`,
//! `Card`, `HandRank`, `create_deck`, `shuffle_deck`, `combinations`,
//! `check_straight`, `get_straight_high`, `evaluate_five_cards`, `evaluate_hand`
//! and a `calculate_side_pots(contributions, total_pot)` whose signature did not
//! even match the canister's `calculate_side_pots(&mut TableState)`. Its 34 tests
//! were green while testing nothing the canister ships, and the copies had
//! already drifted from the real code.
//!
//! Those 34 assertions now live in `src/poker_core/tests/unit_tests.rs`, running
//! against the real engine. What remains here is what only this crate can check:
//!
//!  1. `table_canister`'s Candid-facing poker types ARE `poker_core`'s types, not
//!     structural look-alikes. If someone ever re-declares them locally, the
//!     `.did` and the stable-memory layout can drift while everything still
//!     compiles -- on a canister custodying real ICP/ckBTC.
//!  2. `collect_contributions`, the only logic left in the side-pot adapter.

use poker_core::{Contribution, SidePot};
use table_canister::{collect_contributions, Player, PlayerStatus};

// =============================================================================
// 1. RE-EXPORT IDENTITY
// =============================================================================

/// Compiles only if the two paths name the SAME type, not merely equal-looking
/// ones: a local re-declaration would fail here.
fn assert_same_type<T>(_a: &T, _b: &T) {}

#[test]
fn candid_facing_poker_types_are_the_poker_core_types() {
    let core_card = poker_core::Card {
        suit: poker_core::Suit::Spades,
        rank: poker_core::Rank::Ace,
    };
    let canister_card = table_canister::Card {
        suit: table_canister::Suit::Spades,
        rank: table_canister::Rank::Ace,
    };
    assert_same_type(&core_card, &canister_card);
    assert_eq!(core_card, canister_card);

    let core_rank = poker_core::HandRank::RoyalFlush;
    let canister_rank = table_canister::HandRank::RoyalFlush;
    assert_same_type(&core_rank, &canister_rank);
    assert_eq!(core_rank, canister_rank);

    let core_suit = poker_core::Suit::Hearts;
    let canister_suit = table_canister::Suit::Hearts;
    assert_same_type(&core_suit, &canister_suit);

    let core_pot = SidePot {
        amount: 1,
        eligible_players: vec![0],
    };
    let canister_pot = table_canister::SidePot {
        amount: 1,
        eligible_players: vec![0],
    };
    assert_same_type(&core_pot, &canister_pot);
}

#[test]
fn hand_rank_ordering_is_intact_through_the_re_export() {
    // Who wins a pot is decided by `HandRank`'s derived `Ord`, i.e. by its variant
    // ORDER. Re-check it on the canister's path, not just poker_core's.
    use table_canister::HandRank as H;
    assert!(H::RoyalFlush > H::StraightFlush(14));
    assert!(H::StraightFlush(5) > H::FourOfAKind(14, 13));
    assert!(H::FourOfAKind(2, 3) > H::FullHouse(14, 13));
    assert!(H::FullHouse(2, 3) > H::Flush(vec![14, 13, 12, 11, 9]));
    assert!(H::Flush(vec![7, 5, 4, 3, 2]) > H::Straight(14));
    assert!(H::Straight(5) > H::ThreeOfAKind(14, vec![13, 12]));
    assert!(H::ThreeOfAKind(2, vec![4, 3]) > H::TwoPair(14, 13, 12));
    assert!(H::TwoPair(3, 2, 4) > H::Pair(14, vec![13, 12, 11]));
    assert!(H::Pair(2, vec![5, 4, 3]) > H::HighCard(vec![14, 13, 12, 11, 9]));
}

// =============================================================================
// 2. THE SIDE-POT ADAPTER
// =============================================================================

fn seated(seat: u8, total_bet_this_hand: u64, has_folded: bool) -> Option<Player> {
    Some(Player {
        principal: candid::Principal::anonymous(),
        seat,
        chips: 0,
        hole_cards: None,
        current_bet: 0,
        total_bet_this_hand,
        has_folded,
        has_acted_this_round: false,
        is_all_in: false,
        status: PlayerStatus::Active,
        last_seen: 0,
        timeout_count: 0,
        time_bank_remaining: 0,
        is_sitting_out_next_hand: false,
        broke_at: None,
        sitting_out_since: None,
    })
}

#[test]
fn collect_contributions_uses_the_vector_index_as_the_seat() {
    // The original code enumerated `state.players` and used the INDEX, not
    // `Player::seat`. If those two ever disagree, using `Player::seat` instead
    // would hand the pot to the wrong player, so pin the index behaviour by
    // deliberately setting a mismatched `seat` field.
    let players = vec![seated(9, 100, false), seated(8, 50, false)];
    let got = collect_contributions(&players);
    assert_eq!(
        got,
        vec![
            Contribution::new(0, 100, false),
            Contribution::new(1, 50, false),
        ]
    );
}

#[test]
fn collect_contributions_skips_empty_seats_but_keeps_their_index() {
    let players = vec![None, seated(1, 30, false), None, seated(3, 70, true)];
    let got = collect_contributions(&players);
    assert_eq!(
        got,
        vec![Contribution::new(1, 30, false), Contribution::new(3, 70, true)]
    );
}

#[test]
fn collect_contributions_drops_players_who_bet_nothing() {
    let players = vec![seated(0, 0, false), seated(1, 25, false), seated(2, 0, true)];
    let got = collect_contributions(&players);
    assert_eq!(got, vec![Contribution::new(1, 25, false)]);
}

#[test]
fn collect_contributions_preserves_the_folded_flag() {
    let players = vec![seated(0, 10, true), seated(1, 10, false)];
    let got = collect_contributions(&players);
    assert!(got[0].has_folded);
    assert!(!got[1].has_folded);
}

#[test]
fn an_all_zero_table_yields_no_contributions() {
    // This is the input that makes `apply_side_pots` return early and leave the
    // previous hand's side pots in place, exactly as the original did.
    let players = vec![None, seated(1, 0, false), seated(2, 0, true)];
    assert!(collect_contributions(&players).is_empty());

    let mut existing = vec![SidePot {
        amount: 4321,
        eligible_players: vec![5],
    }];
    let warnings = poker_core::apply_side_pots(&mut existing, &[], 0);
    assert!(warnings.is_empty());
    assert_eq!(existing.len(), 1);
    assert_eq!(existing[0].amount, 4321, "early return must not clear pots");
}
