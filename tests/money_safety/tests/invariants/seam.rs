//! THE SEAM. Canister-level tests that drive the real state machine.
//!
//! docs/DEFECTS.md H-04: seven of seven mutations to the seam in
//! `src/table_canister/src/lib.rs` survived with all 101 tests green. The canister
//! could stop using the extracted engine entirely, or deal every hand from a fixed
//! deck, and the suite still reported green, because
//! `src/table_canister/tests/integration_test.rs` was comments only and nothing
//! else ever ran a hand through the canister and looked at the numbers.
//!
//! Each test below names the mutation it is there to kill. If you change one, check
//! that the mutation it names still dies -- the procedure and the table are in
//! `docs/DEFECTS.md` H-04.
//!
//! These are PINS, not aspirations: they encode what the engine does TODAY,
//! including E-01. Where a number is only correct because a defect is live, the
//! assertion says so and names the defect, so fixing the defect makes this file go
//! red and forces the fix to be deliberate.

use money_safety::documented;
use money_safety::invariants::*;
use money_safety::scenario::{play_one_hand, recompute_rank, seat_players};
use money_safety::table_api::*;
use money_safety::world::*;
use money_safety::{assert_holds, assert_no_new_violations};
use std::time::Duration;

const ICP: u64 = 100_000_000;

/// Two funded players in seats 0 and 1. `play_one_hand` deals as `actors[0]`, which
/// is the same principal seated at 0, so the hand starts the way a real client does.
fn heads_up_world() -> World {
    let world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);
    seat_players(&world, &["alice", "bob"], 8 * ICP);
    world
}

// ---------------------------------------------------------------------------
// (a) money in == money out, over a hand with pre-flop AND post-flop betting
// ---------------------------------------------------------------------------

/// Kills mutations 1, 2, 3 and 4 (`calculate_side_pots` no-op / `state.pot / 2` /
/// pots into a throwaway `Vec` / `&[]` contributions).
///
/// The identity asserted is the full accounting of one hand:
///
/// ```text
///   collected   = preflop_pot + post_flop_in          (what players put in)
///   awarded     = collected                           (all of it is paid out)
///   chips_after = chips_before                        (nothing leaves the table)
/// ```
///
/// THIS TEST WAS REWRITTEN WHEN E-01 WAS FIXED, exactly as its previous version
/// instructed. It used to assert `basis == preflop_pot` and
/// `unpaid == post_flop_in`: the engine paid out of the side-pot breakdown frozen at
/// the flop, so every chip wagered on the flop, turn and river was debited from
/// stacks and credited to nobody, permanently. Those two assertions were correct
/// ONLY because the defect was live, and they said so.
///
/// The `payout_basis` figure is deliberately no longer asserted against
/// `preflop_pot`: it is the last breakdown observed while the hand was live, and
/// since the hand can now settle inside the betting loop that observation may be a
/// street behind. `awarded == collected` is the stronger statement and needs no
/// observation of intermediate state at all.
#[test]
fn seam_a_showdown_with_post_flop_betting_accounts_for_every_e8() {
    let mut world = heads_up_world();
    let out = play_one_hand(&mut world, ICP);

    assert_eq!(
        out.final_phase,
        GamePhase::HandComplete,
        "the hand must have settled; got {:?}",
        out.final_phase
    );
    assert!(
        out.preflop_pot > 0,
        "there must be pre-flop money: preflop_pot={}",
        out.preflop_pot
    );
    assert!(
        out.post_flop_in > 0,
        "there must be POST-FLOP money, otherwise this test is the passive case: \
         collected={} preflop_pot={}",
        out.collected,
        out.preflop_pot
    );

    // 1. Everything that went in is accounted for.
    assert_eq!(
        out.collected,
        out.preflop_pot + out.post_flop_in,
        "the hand collected {} but the pre-flop pot was {} and {} went in post-flop",
        out.collected,
        out.preflop_pot,
        out.post_flop_in
    );

    // 2. Every chip collected was paid out. This is the assertion a house rake
    //    cannot pass, and it is the one E-01 broke: the winner was paid the pre-flop
    //    pot and the rest was credited to nobody.
    let rake = check_awarded_equals_payout_basis(
        out.hand_number,
        out.collected,
        out.awarded,
        &out.final_phase,
    );
    assert_holds(&rake, Invariant::M3NoRake, "one hand, post-flop betting");
    assert_eq!(
        out.awarded, out.collected,
        "PINNED FIX (E-01): the winner must be paid every e8 collected, including \
         everything wagered after the flop. collected={} awarded={} (unpaid {}), \
         pre-flop pot was {} and {} went in post-flop. Before the fix `awarded` was \
         exactly the pre-flop pot and the difference was destroyed.",
        out.collected,
        out.awarded,
        out.unpaid(),
        out.preflop_pot,
        out.post_flop_in
    );
    assert_eq!(out.unpaid(), 0, "nothing may go unpaid");

    // 3. The engine still builds a side-pot breakdown while the hand is live: that
    //    is what mutations 1, 3 and 4 kill. It is display-only state now, but it
    //    must exist and it must be a breakdown of real money.
    assert!(
        out.used_side_pot_breakdown,
        "the engine must have written a side-pot breakdown into state.side_pots while \
         the hand was live; it did not, so calculate_side_pots did not run or did not \
         write there (docs/DEFECTS.md H-04 mutations 1, 3 and 4). side_pots={:?}",
        out.basis_side_pots
    );
    assert!(
        out.payout_basis >= out.preflop_pot && out.payout_basis <= out.collected,
        "the observed breakdown ({}) must be between the pre-flop pot ({}) and \
         everything collected ({}): below that means the split maths was handed \
         something other than the contributions (H-04 mutation 2), above it means the \
         breakdown is inventing money. side_pots={:?}",
        out.payout_basis,
        out.preflop_pot,
        out.collected,
        out.basis_side_pots
    );

    // 4. So the chips at the table are exactly conserved across the hand.
    assert_eq!(
        out.chips_after, out.chips_before,
        "the chips at the table must be unchanged across a hand with no external \
         money movement: before={} after={} (post_flop_in={})",
        out.chips_before,
        out.chips_after,
        out.post_flop_in
    );

    // 5. And the ledger-anchored invariants agree: nothing is stranded.
    let vs = check_world(&world);
    assert_no_new_violations(&vs, "after one post-flop-betting hand");
    assert!(
        !vs.iter().any(|v| v.invariant == Invariant::M1Conservation),
        "E-01 stranded the post-flop money on the ledger owned by nobody, and M1 saw \
         it. Nothing may be stranded now: {vs:?}"
    );
    println!(
        "seam(a): hand {} collected={} awarded={} basis_observed={} post_flop_in={} \
         unpaid={}",
        out.hand_number,
        out.collected,
        out.awarded,
        out.payout_basis,
        out.post_flop_in,
        out.unpaid()
    );
}

/// The control case: a hand with NO post-flop money must pay out everything it
/// collected. `awarded == collected`, no exceptions, no pinned defect.
#[test]
fn seam_a_control_a_passive_hand_pays_out_everything_it_collected() {
    let mut world = heads_up_world();
    let out = play_one_hand(&mut world, 0);

    assert_eq!(out.final_phase, GamePhase::HandComplete);
    assert!(out.collected > 0, "the blinds alone are money");
    assert_eq!(
        out.post_flop_in, 0,
        "this is the control case: nothing may go in post-flop"
    );
    assert_eq!(
        out.awarded, out.collected,
        "with no post-flop money the winner must be paid every e8 collected: \
         collected={} awarded={} basis={}",
        out.collected, out.awarded, out.payout_basis
    );
    assert_eq!(
        out.chips_after, out.chips_before,
        "and the chips at the table must be exactly conserved"
    );
    let rake = check_awarded_equals_payout_basis(
        out.hand_number,
        out.payout_basis,
        out.awarded,
        &out.final_phase,
    );
    assert_holds(&rake, Invariant::M3NoRake, "a passive hand");
}

// ---------------------------------------------------------------------------
// (b) side_pots is a breakdown of pot at the moment it is built
// ---------------------------------------------------------------------------

/// Kills mutations 1, 2, 3 and 4 directly, at the point the breakdown is written.
///
/// `calculate_side_pots` runs on the PreFlop -> Flop transition. At that instant --
/// before any post-flop money exists -- `state.side_pots` must be non-empty and must
/// sum to `state.pot` exactly. A no-op, a `state.pot / 2`, a throwaway `Vec` and an
/// empty contributions slice each break one of those two clauses.
#[test]
fn seam_b_side_pots_sum_to_pot_at_the_moment_calculate_side_pots_runs() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    seat_players(&world, &["alice", "bob", "carol"], 8 * ICP);
    world
        .start_new_hand(world.actor("alice"))
        .expect("start_new_hand");

    // Level the pre-flop bets, which is what triggers advance_to_next_street.
    for _ in 0..24 {
        let t = world.table_state();
        if t.phase != GamePhase::PreFlop {
            break;
        }
        let who = t
            .players
            .get(t.action_on as usize)
            .and_then(|p| p.as_ref())
            .map(|p| p.principal)
            .expect("the clock seat is occupied");
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        world
            .player_action(who, PlayerAction::Check)
            .expect("check must be legal once the pre-flop bets are level");
    }

    let flop = world.table_state();
    assert_eq!(
        flop.phase,
        GamePhase::Flop,
        "the hand must have reached the flop"
    );
    assert!(flop.pot > 0, "the blinds are in the pot");
    assert!(
        !flop.side_pots.is_empty(),
        "calculate_side_pots must have written a breakdown into state.side_pots on the \
         PreFlop -> Flop transition. It is empty, so either the routine did not run \
         (mutation 1), or its output went somewhere other than state.side_pots \
         (mutation 3), or it was handed no contributions and took the early return \
         (mutation 4). pot={}",
        flop.pot
    );
    assert_eq!(
        flop.side_pots_total(),
        flop.pot,
        "side_pots is a BREAKDOWN of pot, so at the instant it is built it must sum to \
         pot exactly. sum={} pot={} side_pots={:?} -- a fraction of the pot here means \
         the pot passed to the split maths was not state.pot (mutation 2).",
        flop.side_pots_total(),
        flop.pot,
        flop.side_pots
    );

    // Every seat with money in must be eligible for the main pot: an empty
    // contributions slice would produce a breakdown nobody can win.
    let contributors: Vec<u8> = flop
        .seated()
        .filter(|p| p.total_bet_this_hand > 0)
        .map(|p| p.seat)
        .collect();
    let main_pot = &flop.side_pots[0];
    for seat in &contributors {
        assert!(
            main_pot.eligible_players.contains(seat),
            "seat {seat} put money in this hand but is not eligible for the main pot \
             {main_pot:?}; the contributions the split was given did not describe this \
             table (mutation 4)"
        );
    }

    // The invariant function the fuzzer uses must agree.
    let vs = check_pot_breakdown(&world.snapshot());
    assert!(
        vs.iter().all(|v| v.check != "side_pots_sum_to_pot"),
        "M1b must not flag the breakdown at the instant it is built: {vs:?}"
    );
}

// ---------------------------------------------------------------------------
// (c) the deck actually varies between hands
// ---------------------------------------------------------------------------

/// Kills mutation 7 (`shuffle_deck` shadowed so every hand is dealt from the
/// constant seed `b"CONSTANT"`).
///
/// Note what is NOT sufficient: `shuffle_proof.seed_hash` varies per hand even under
/// that mutation, because the hash is taken over the real random bytes while the
/// SHUFFLE ignores them. The only witness is the dealt deck itself.
#[test]
fn seam_c_the_deck_is_different_every_hand() {
    let mut world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);
    seat_players(&world, &["alice", "bob"], 8 * ICP);

    let mut decks: Vec<Vec<Card>> = Vec::new();
    let mut seed_hashes: Vec<String> = Vec::new();
    for _ in 0..3 {
        let out = play_one_hand(&mut world, 0);
        assert_eq!(out.deck.len(), 52, "a hand is dealt from a full 52-card deck");
        decks.push(out.deck);
        if let Some(p) = world.table_state().shuffle_proof {
            seed_hashes.push(p.seed_hash);
        }
        world.advance(Duration::from_secs(4));
    }
    assert_eq!(decks.len(), 3, "three hands must have been dealt");

    let rendered: Vec<String> = decks.iter().map(|d| format!("{d:?}")).collect();
    let distinct: std::collections::BTreeSet<&String> = rendered.iter().collect();
    assert!(
        distinct.len() >= 2,
        "all {} hands were dealt from an IDENTICAL deck. The shuffle is not consuming \
         the per-hand seed, so every hand of poker this canister ever deals is the same \
         hand (docs/DEFECTS.md H-04 mutation 7). Per-hand seed hashes were {:?} -- note \
         that those varying proves nothing, because the seed hash is taken over the \
         random bytes even when the shuffle ignores them.",
        decks.len(),
        seed_hashes
    );

    // Stronger: the first physical card dealt must not be the same every hand.
    let firsts: std::collections::BTreeSet<String> = decks
        .iter()
        .filter_map(|d| d.first().map(|c| format!("{c:?}")))
        .collect();
    assert!(
        firsts.len() >= 2,
        "the deck differs between hands but the top card never does: {firsts:?}"
    );
}

// ---------------------------------------------------------------------------
// (d) evaluate_hand is really consulted
// ---------------------------------------------------------------------------

/// Kills mutation 6 (`evaluate_hand` shadowed by a stub returning `RoyalFlush` for
/// every player).
///
/// The oracle is the engine's own evaluator, re-run on the cards the CANISTER
/// revealed: for every player who reached the showdown, the rank the canister
/// recorded must equal `poker_core::evaluate_hand(hole, community)`, and the seats it
/// paid must be exactly the seats holding the best of those ranks.
///
/// Wave 1 proved the evaluator itself correct exhaustively (all 2,598,960 five-card
/// hands against three independent lineages). What was never checked is whether the
/// canister ASKS it. This is that check.
#[test]
fn seam_d_the_recorded_showdown_ranks_are_the_ranks_evaluate_hand_returns() {
    let mut world = World::new(TableConfig::heads_up_icp(), &["alice", "bob"]);
    seat_players(&world, &["alice", "bob"], 8 * ICP);

    let mut showdowns = 0;
    for _ in 0..4 {
        let out = play_one_hand(&mut world, 0);
        world.advance(Duration::from_secs(4));
        if !out.reached_showdown() {
            continue;
        }
        assert_eq!(
            out.community.len(),
            5,
            "a showdown is reached with a five-card board"
        );
        showdowns += 1;

        for record in &out.showdown {
            let recomputed = recompute_rank(record, &out.community).unwrap_or_else(|| {
                panic!(
                    "hand {}: the canister recorded seat {} at the showdown with NO hole \
                     cards, so its verdict cannot be checked against the evaluator",
                    out.hand_number, record.seat
                )
            });
            let recorded = record.hand_rank.clone().unwrap_or_else(|| {
                panic!(
                    "hand {}: seat {} reached the showdown with no recorded hand_rank",
                    out.hand_number, record.seat
                )
            });
            assert_eq!(
                recorded,
                recomputed,
                "hand {}: the canister says seat {} held {:?}, but running the engine's own \
                 evaluate_hand over the cards the canister itself revealed \
                 (hole={:?} board={:?}) gives {:?}. The showdown is not going through the \
                 extracted evaluator (docs/DEFECTS.md H-04 mutation 6).",
                out.hand_number,
                record.seat,
                recorded,
                record.cards,
                out.community,
                recomputed
            );
        }

        // And the money followed the best hand.
        let best = out
            .showdown
            .iter()
            .filter_map(|r| recompute_rank(r, &out.community))
            .max()
            .expect("at least one showdown hand");
        let should_be_paid: Vec<u8> = out
            .showdown
            .iter()
            .filter(|r| recompute_rank(r, &out.community).as_ref() == Some(&best))
            .map(|r| r.seat)
            .collect();
        let were_paid: Vec<u8> = out
            .showdown
            .iter()
            .filter(|r| r.amount_won > 0)
            .map(|r| r.seat)
            .collect();
        assert_eq!(
            were_paid, should_be_paid,
            "hand {}: the best hand at the showdown was {:?}, held by seat(s) {:?}, but the \
             seats that were PAID were {:?}. Showdown: {:?}, board {:?}",
            out.hand_number, best, should_be_paid, were_paid, out.showdown, out.community
        );
    }

    assert!(
        showdowns > 0,
        "four passive hands produced no showdown at all, so this test checked nothing"
    );
    println!("seam(d): checked {showdowns} showdown(s) against poker_core::evaluate_hand");
}

// ---------------------------------------------------------------------------
// the canister must not report its own failures
// ---------------------------------------------------------------------------

/// A `BUG:` or `CRITICAL:` line in the canister log is the engine's own testimony
/// that its accounting is wrong. Honest play must produce none.
///
/// This is the live half of the H-02 fix: the classifier can now block on such a
/// line, and this test states that in normal play there is nothing to block on, so a
/// future failure here is a real change rather than pre-existing noise.
#[test]
fn seam_honest_play_produces_no_self_reported_failure() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    seat_players(&world, &["alice", "bob", "carol"], 8 * ICP);

    let mut all_logs: Vec<String> = Vec::new();
    for _ in 0..3 {
        let out = play_one_hand(&mut world, ICP / 2);
        all_logs.extend(out.logs.clone());
        world.advance(Duration::from_secs(4));
    }
    let vs = check_self_reported_inconsistency(&all_logs, &GamePhase::HandComplete);
    let blocking: Vec<&Violation> = vs.iter().filter(|v| !documented::is_documented(v)).collect();
    assert!(
        blocking.is_empty(),
        "the canister reported {} failure(s) of its own during honest play:\n{}",
        blocking.len(),
        blocking
            .iter()
            .map(|v| format!("  {} -- {}", v.check, v.detail))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
