//! Money-safety invariants that can be evaluated on the HOST, with no replica.
//!
//! ============================================================================
//! The full harness lives in `tests/money_safety/` (its own detached crate).
//! ============================================================================
//!
//! That crate runs the REAL table canister wasm against the REAL mainnet ICP
//! ledger wasm, installed at the exact canister id this canister hardcodes
//! (`ryjl3-tyaaa-aaaaa-aaaba-cai`), on PocketIC. It defines M1..M6 as named
//! assertions, has one hand-written test per invariant, a seeded hostile-sequence
//! fuzzer with a shrinker, and a minimal reproducer per violation found:
//!
//! ```text
//! cd tests/money_safety
//! cargo test --test invariants     # one test per invariant, M1..M6
//! cargo test --test regressions    # minimal reproducers
//! cargo test --test fuzz           # the fuzzer; writes money-fuzz-report.json
//! ```
//!
//! It is detached from this workspace on purpose: `pocket-ic` pulls in tokio,
//! reqwest and rustls, and none of that belongs in the lockfile of a canister
//! that custodies real ICP and ckBTC on mainnet.
//!
//! What is left HERE is the part that needs no replica: the arithmetic of the
//! payout basis, evaluated against the real `TableState`, the real
//! `collect_contributions` and the real `poker_core::apply_side_pots`. These run
//! in milliseconds under a plain `cargo test --workspace`, so the mechanism behind
//! the fund-safety findings is pinned even in an environment with no PocketIC
//! binary.
//!
//! NOTHING here is a fix. Every assertion below states what the engine currently
//! does, including where that is wrong, so that changing the behaviour requires
//! deliberately editing this file and `docs/SECURITY-FINDINGS.md` together.

use table_canister::{
    collect_contributions, Currency, GamePhase, Player, PlayerStatus, TableConfig, TableState,
};

// ---------------------------------------------------------------------------
// the invariants, as named functions over a table state
// ---------------------------------------------------------------------------

/// M1b (POT BREAKDOWN), leg 1: `side_pots` is a breakdown of `pot`, so when it is
/// populated it must sum to `pot`.
fn side_pots_sum_to_pot(state: &TableState) -> Result<(), String> {
    if state.side_pots.is_empty() {
        return Ok(());
    }
    let sum = state
        .side_pots
        .iter()
        .fold(0u64, |a, sp| a.saturating_add(sp.amount));
    if sum == state.pot {
        Ok(())
    } else {
        Err(format!(
            "sum(side_pots)={sum} != pot={}: the payout basis disagrees with the money collected",
            state.pot
        ))
    }
}

/// M1b (POT BREAKDOWN), leg 2: mid-hand, every e8 in `pot` is attributed to a
/// seated player's `total_bet_this_hand`, so the bet-level split can allocate it.
fn pot_is_fully_attributed(state: &TableState) -> Result<(), String> {
    let wagered = state
        .players
        .iter()
        .flatten()
        .fold(0u64, |a, p| a.saturating_add(p.total_bet_this_hand));
    if state.pot == wagered {
        Ok(())
    } else {
        Err(format!(
            "pot={} but the seated players' total_bet_this_hand sums to {wagered} (delta {})",
            state.pot,
            state.pot as i128 - wagered as i128
        ))
    }
}

/// M3 (NO RAKE): the side pots the engine would pay out of must account for every
/// chip that was wagered. ClearDeck takes no rake, so this is exact.
fn payout_basis_covers_every_wagered_chip(state: &TableState) -> Result<(), String> {
    let contributions = collect_contributions(&state.players);
    let wagered = contributions
        .iter()
        .fold(0u64, |a, c| a.saturating_add(c.total_bet_this_hand));
    let mut side_pots = state.side_pots.clone();
    let _ = poker_core::apply_side_pots(&mut side_pots, &contributions, state.pot);
    let payable = side_pots
        .iter()
        .fold(0u64, |a, sp| a.saturating_add(sp.amount));
    if payable == wagered {
        Ok(())
    } else {
        Err(format!(
            "the payout basis totals {payable} but {wagered} was wagered (delta {})",
            payable as i128 - wagered as i128
        ))
    }
}

/// M4 (NO NEGATIVE / NO OVERFLOW): no chip figure may be so large that summing it
/// saturates. `u64` cannot go negative, so a clamp is the reachable failure.
fn no_chip_total_saturates(state: &TableState) -> Result<(), String> {
    state
        .players
        .iter()
        .flatten()
        .try_fold(0u64, |a, p| a.checked_add(p.chips))
        .ok_or_else(|| "sum of chip stacks overflows u64".to_string())?;
    state
        .players
        .iter()
        .flatten()
        .try_fold(0u64, |a, p| a.checked_add(p.total_bet_this_hand))
        .ok_or_else(|| "sum of total_bet_this_hand overflows u64".to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// scaffolding
// ---------------------------------------------------------------------------

fn config() -> TableConfig {
    TableConfig {
        small_blind: 1_000_000,
        big_blind: 2_000_000,
        min_buy_in: 200_000_000,
        max_buy_in: 1_000_000_000,
        max_players: 6,
        action_timeout_secs: 30,
        ante: 0,
        time_bank_secs: 30,
        currency: Currency::ICP,
    }
}

fn player(seat: u8, chips: u64, wagered: u64, folded: bool) -> Player {
    Player {
        principal: candid::Principal::from_slice(&[seat + 1]),
        seat,
        chips,
        hole_cards: None,
        current_bet: 0,
        total_bet_this_hand: wagered,
        has_folded: folded,
        has_acted_this_round: true,
        is_all_in: chips == 0,
        status: PlayerStatus::Active,
        last_seen: 0,
        timeout_count: 0,
        time_bank_remaining: 30,
        is_sitting_out_next_hand: false,
        broke_at: None,
        sitting_out_since: None,
    }
}

/// A table mid-hand with `stakes` = (chips, wagered, folded) per seat.
fn mid_hand(stakes: &[(u64, u64, bool)]) -> TableState {
    let cfg = config();
    let players: Vec<Option<Player>> = (0..cfg.max_players as usize)
        .map(|i| {
            stakes
                .get(i)
                .map(|(chips, wagered, folded)| player(i as u8, *chips, *wagered, *folded))
        })
        .collect();
    // saturating, so a deliberately overflowing fixture can be built at all
    let pot = stakes.iter().fold(0u64, |a, (_, w, _)| a.saturating_add(*w));
    TableState {
        id: 0,
        config: cfg,
        players,
        community_cards: Vec::new(),
        deck: Vec::new(),
        deck_index: 0,
        pot,
        side_pots: Vec::new(),
        current_bet: 0,
        min_raise: 2_000_000,
        phase: GamePhase::Flop,
        dealer_seat: 0,
        small_blind_seat: 0,
        big_blind_seat: 1,
        action_on: 0,
        action_timer: None,
        shuffle_proof: None,
        hand_number: 1,
        last_aggressor: None,
        bb_has_option: false,
        first_hand: false,
        auto_deal_at: None,
        last_action: None,
        departed_stakes: None,
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// The baseline: a consistent mid-hand table satisfies every host-checkable
/// invariant. If this ever fails, the invariants themselves are wrong.
#[test]
fn a_consistent_table_satisfies_every_host_checkable_invariant() {
    let mut state = mid_hand(&[
        (100_000_000, 50_000_000, false),
        (300_000_000, 50_000_000, false),
        (0, 20_000_000, false),
    ]);
    let contributions = collect_contributions(&state.players);
    let _ = poker_core::apply_side_pots(&mut state.side_pots, &contributions, state.pot);

    side_pots_sum_to_pot(&state).expect("M1b leg 1");
    pot_is_fully_attributed(&state).expect("M1b leg 2");
    payout_basis_covers_every_wagered_chip(&state).expect("M3");
    no_chip_total_saturates(&state).expect("M4");
}

/// M1b leg 1, PINNED DEFECT (docs/FINDING-01-chip-destruction.md).
///
/// `advance_to_next_street` builds `side_pots` on the PreFlop -> Flop transition
/// and nothing rebuilds it afterwards, so any post-flop money makes the breakdown
/// stale. `determine_winners` then pays out of the stale breakdown and zeroes
/// `state.pot` without reconciling, destroying the difference.
#[test]
fn m1b_a_stale_side_pot_breakdown_understates_the_pot() {
    // Built when the pot was 4_000_000 (the pre-flop money).
    let mut state = mid_hand(&[
        (100_000_000, 2_000_000, false),
        (100_000_000, 2_000_000, false),
    ]);
    let contributions = collect_contributions(&state.players);
    let _ = poker_core::apply_side_pots(&mut state.side_pots, &contributions, state.pot);
    side_pots_sum_to_pot(&state).expect("consistent at the moment it is built");

    // Post-flop: 30_000_000 each, into `pot` and `total_bet_this_hand`, exactly as
    // `player_action` does it. `side_pots` is NOT touched.
    for p in state.players.iter_mut().flatten() {
        p.chips -= 30_000_000;
        p.total_bet_this_hand += 30_000_000;
        state.pot += 30_000_000;
    }

    pot_is_fully_attributed(&state).expect("the pot is still fully attributed");
    let err = side_pots_sum_to_pot(&state).expect_err(
        "PINNED: the stale breakdown must disagree with the pot. If it now agrees, \
         FINDING 01 has been fixed -- update docs/SECURITY-FINDINGS.md and this test together.",
    );
    assert!(err.contains("sum(side_pots)=4000000"), "{err}");
    assert!(err.contains("pot=64000000"), "{err}");

    // And this is the money that gets destroyed: the engine pays out of 4_000_000
    // and then sets pot = 0.
    let payable = state
        .side_pots
        .iter()
        .map(|sp| sp.amount)
        .sum::<u64>();
    assert_eq!(
        state.pot - payable,
        60_000_000,
        "0.6 ICP would be destroyed by this hand"
    );
}

/// M1b leg 2, PINNED DEFECT (docs/SECURITY-FINDINGS.md FINDING 05).
///
/// Vacating a seat mid-hand (`leave_table`, or `cash_out` after a fold) removes
/// the seat, so `collect_contributions` -- which enumerates `state.players` --
/// stops seeing that player's stake, while the money stays in `state.pot`. The
/// bet-level split then has an unattributed remainder, and
/// `poker_core::apply_side_pots` appends the whole of it to the LAST (highest) bet
/// level: the pot only the deepest stack can win.
#[test]
fn m1b_vacating_a_seat_mid_hand_moves_the_stake_into_the_deepest_stacks_pot() {
    // Seat 0 is a short stack all-in for 50. Seats 1 and 2 have 200 in each.
    let stakes = [(0u64, 50u64, false), (100, 200, false), (100, 200, false)];
    let with_all_seats = mid_hand(&stakes);
    assert_eq!(with_all_seats.pot, 450);
    pot_is_fully_attributed(&with_all_seats).expect("everything is attributed to begin with");

    let mut before = with_all_seats.clone();
    let contributions = collect_contributions(&before.players);
    let _ = poker_core::apply_side_pots(&mut before.side_pots, &contributions, before.pot);
    let main_before = before.side_pots[0].amount;
    let last_before = before.side_pots.last().unwrap().amount;
    assert_eq!(main_before, 150, "3 x 50 at the lowest bet level");

    // --- the honest comparison: seat 1 FOLDS but keeps its seat --------------
    let mut folded = with_all_seats.clone();
    if let Some(p) = folded.players[1].as_mut() {
        p.has_folded = true;
    }
    let contributions = collect_contributions(&folded.players);
    let _ = poker_core::apply_side_pots(&mut folded.side_pots, &contributions, folded.pot);
    assert_eq!(
        folded.side_pots[0].amount, 150,
        "folding does not move money out of the main pot"
    );

    // --- the defect: seat 1 VACATES ----------------------------------------
    let mut vacated = with_all_seats.clone();
    vacated.players[1] = None; // exactly what leave_table does
    let err = pot_is_fully_attributed(&vacated).expect_err(
        "PINNED: the vacated seat's stake must become unattributed. If it no longer does, \
         FINDING 05 has been fixed -- update docs/SECURITY-FINDINGS.md and this test together.",
    );
    assert!(err.contains("delta 200"), "{err}");

    let contributions = collect_contributions(&vacated.players);
    let _ = poker_core::apply_side_pots(&mut vacated.side_pots, &contributions, vacated.pot);
    assert_eq!(
        vacated.side_pots[0].amount, 100,
        "the main pot, which the honest all-in short stack can win, SHRANK from 150 to 100"
    );
    assert_eq!(
        vacated.side_pots.last().unwrap().amount,
        last_before + 50,
        "and the 50 reappeared in the pot only the deepest stack is eligible for"
    );
    assert!(
        !vacated.side_pots[0].eligible_players.contains(&1),
        "seat 1 is gone from the eligibility list, as expected"
    );
}

/// M3 (NO RAKE), PINNED DEFECT: `poker_core::apply_side_pots` trusts `state.pot`
/// over the players' actual contributions in BOTH directions, so the payout basis
/// can total more than was wagered (chips from nothing) or less (chips destroyed).
/// This is docs/SECURITY-FINDINGS.md FINDING 02, reachable via FINDING 05.
#[test]
fn m3_the_payout_basis_can_diverge_from_what_was_actually_wagered() {
    // Direction A: `pot` overstates the contributions -> the excess is minted into
    // the highest bet level.
    let mut overstated = mid_hand(&[
        (0, 50_000_000, false),
        (0, 100_000_000, false),
        (0, 100_000_000, false),
    ]);
    overstated.pot += 100_000_000; // e.g. a vacated seat's stake
    let err = payout_basis_covers_every_wagered_chip(&overstated).expect_err(
        "PINNED: an overstated pot must inflate the payout basis. If it no longer does, \
         FINDING 02 direction A has been fixed -- update docs/SECURITY-FINDINGS.md too.",
    );
    assert!(err.contains("delta 100000000"), "{err}");

    // Direction B: `pot` understates the contributions -> the basis is capped and
    // chips are destroyed.
    let mut understated = mid_hand(&[(0, 100, false), (0, 10, false), (0, 1, false)]);
    understated.pot = 60;
    let err = payout_basis_covers_every_wagered_chip(&understated).expect_err(
        "PINNED: an understated pot must shrink the payout basis. If it no longer does, \
         FINDING 02 direction B has been fixed -- update docs/SECURITY-FINDINGS.md too.",
    );
    assert!(err.contains("delta -"), "{err}");
}

/// M4: the arithmetic on the payout path must not silently clamp. `u64::MAX`
/// contributions are the boundary; `saturating_add` in the engine means a total
/// that would overflow is reported as `u64::MAX` instead of as an error, so the
/// invariant has to detect it independently with `checked_add`.
#[test]
fn m4_a_chip_total_that_would_overflow_is_detected_not_clamped() {
    let state = mid_hand(&[
        (u64::MAX, u64::MAX, false),
        (u64::MAX, u64::MAX, false),
    ]);
    let err = no_chip_total_saturates(&state)
        .expect_err("two u64::MAX stacks must be reported as an overflow");
    assert!(err.contains("overflows u64"), "{err}");

    // And the engine's own saturating fold reports a total that is NOT the truth.
    let saturating = state
        .players
        .iter()
        .flatten()
        .fold(0u64, |a, p| a.saturating_add(p.chips));
    assert_eq!(
        saturating,
        u64::MAX,
        "saturating_add reports u64::MAX, hiding the overflow -- which is exactly why the \
         invariant re-sums with checked_add"
    );
}

/// Documents the boundary of what this file can prove, so nobody mistakes a green
/// `cargo test --workspace` for money safety.
#[test]
fn the_ledger_anchored_invariants_are_not_checked_here() {
    // M1 (CONSERVATION) and M2 (LEDGER REALITY) compare the canister against the
    // ICP ledger, M5 needs a real upgrade, and M6 needs same-round concurrent
    // messages. None of that exists on the host.
    let harness = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("<repo>/src/table_canister")
        .join("tests")
        .join("money_safety")
        .join("Cargo.toml");
    assert!(
        harness.exists(),
        "the PocketIC money-safety harness must exist at {}: M1, M2, M5 and M6 are only \
         checked there, and this host-level file is not a substitute for it",
        harness.display()
    );
}
