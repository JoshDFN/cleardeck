//! M7 — THE UPGRADE GATE. State written by the PREVIOUS RELEASE must survive an
//! upgrade to the module under test, exactly.
//!
//! # Why this file exists (docs/DEFECTS.md E-38, SECURITY-FINDINGS.md FINDING 14)
//!
//! Every other "survives an upgrade" assertion in this harness upgrades the module
//! under test **to itself**: `World::upgrade` reuses `self.table_wasm`. The same
//! Candid type is on both sides of the wire, so a record-field addition is never
//! tested as an addition — and adding a field to a persisted record is the one
//! upgrade change that can destroy funds without an error.
//!
//! Two wave-2 agents, editing different regions of `lib.rs`, each added a persisted
//! field that Candid cannot read from older state:
//!
//!   * `PersistentState::deposit_watermark: u64` — top level, so `stable_restore`
//!     fails and the upgrade was REJECTED. Loud, and therefore safe.
//!   * `TableState::departed_stakes: Vec<DepartedStake>` — nested inside
//!     `opt TableState`. Candid decodes an `opt` it cannot read as **null**, so this
//!     one made the whole table arrive as `None`, after which `post_upgrade`
//!     re-inited an empty table and every seated player's chips were gone.
//!
//! `#[serde(default)]` does not help; only `opt` is a backward-compatible addition.
//! Both fields are `opt` as of the change this file gates.
//!
//! # What M7 demands, and why it is stated this way
//!
//! **The upgrade must SUCCEED and every e8 must still be there.** Nothing weaker.
//!
//! An earlier version of this test accepted a REJECTED upgrade as a pass, on the
//! grounds that a refusal loses nothing. That was the right shape while the tree
//! could not be upgraded at all, and it is the wrong shape now: the deposit fix —
//! the fix for the only proven fund theft in this project — can only reach a
//! deployed canister BY upgrade, so "the upgrade is refused" is a failure, not a
//! safe outcome. A refusal is reported with the rejection text so the reader knows
//! which field regressed.
//!
//! # This test goes RED if either `opt` regresses, by two different routes
//!
//!   * `deposit_watermark` back to `u64` → `stable_restore` fails outright →
//!     `post_upgrade` panics → the upgrade is REJECTED → `expect` below fails.
//!   * `departed_stakes` back to `Vec` → `opt TableState` decodes as null →
//!     `post_upgrade` re-inits an empty table → the seated-chips and pot assertions
//!     fail. Note that the `table_was_present` digest guard CANNOT save this case:
//!     `801aa79` never wrote the digest, so it restores as `None` and the guard
//!     stays silent. That is exactly why this test, and not the guard, is the gate.
//!
//! Both routes were executed against this file; see the report in
//! docs/SECURITY-FINDINGS.md FINDING 14.

use candid::Principal;
use money_safety::table_api::*;
use money_safety::wasms;
use money_safety::world::*;
use std::collections::BTreeMap;
use std::time::Duration;

const ICP: u64 = 100_000_000;

/// Everything about the table that an upgrade must not change, read through the
/// public/controller API only.
#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    escrow: BTreeMap<Principal, u64>,
    escrow_total: u64,
    /// `(seat, principal, chips, total_bet_this_hand, has_folded, hole_cards)`.
    seats: Vec<(u8, Principal, u64, u64, bool, Option<(Card, Card)>)>,
    chips_total: u64,
    pot: u64,
    hand_number: u64,
    phase: GamePhase,
    dealer_seat: u8,
    small_blind_seat: u8,
    big_blind_seat: u8,
    action_on: u8,
    current_bet: u64,
    min_raise: u64,
    community: Vec<Card>,
    deck: Vec<Card>,
    deck_index: u64,
    side_pots: Vec<(u64, Vec<u8>)>,
    seed_hash: Option<String>,
    departed: Vec<(u64, u8, Principal, u64)>,
}

fn fingerprint(world: &World) -> Fingerprint {
    let (escrow_total, escrow) = world.escrow_all();
    let t = world.table_state();
    Fingerprint {
        escrow,
        escrow_total,
        seats: t
            .seated()
            .map(|p| {
                (
                    p.seat,
                    p.principal,
                    p.chips,
                    p.total_bet_this_hand,
                    p.has_folded,
                    p.hole_cards,
                )
            })
            .collect(),
        chips_total: world.chips_total(),
        pot: t.pot,
        hand_number: t.hand_number,
        phase: t.phase.clone(),
        dealer_seat: t.dealer_seat,
        small_blind_seat: t.small_blind_seat,
        big_blind_seat: t.big_blind_seat,
        action_on: t.action_on,
        current_bet: t.current_bet,
        min_raise: t.min_raise,
        community: t.community_cards.clone(),
        deck: t.deck.clone(),
        deck_index: t.deck_index,
        side_pots: t
            .side_pots
            .iter()
            .map(|sp| (sp.amount, sp.eligible_players.clone()))
            .collect(),
        seed_hash: t.shuffle_proof.as_ref().map(|p| p.seed_hash.clone()),
        departed: t
            .departed()
            .iter()
            .map(|d| (d.hand_number, d.seat, d.principal, d.contributed))
            .collect(),
    }
}

/// Drive the hand until the flop is on the table and then STOP, so the upgrade
/// happens with a live hand: hole cards dealt, a board, money in the pot, a player
/// on the clock and a shuffle commitment outstanding.
fn deal_and_stop_on_the_flop(world: &World) {
    world
        .start_new_hand(world.actors[0].principal)
        .expect("start_new_hand must succeed with three seated players");
    for _ in 0..24 {
        let t = world.table_state();
        if t.phase == GamePhase::Flop || !t.phase.hand_in_progress() {
            return;
        }
        let Some(who) = money_safety::scenario::on_clock(&t) else {
            world.advance(Duration::from_secs(1));
            continue;
        };
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        world.advance(Duration::from_secs(1));
    }
}

/// THE GATE. Real state, written by `801aa79`'s own `pre_upgrade`, walked into the
/// module under test with nothing lost and nothing changed.
#[test]
fn m7_state_written_by_the_previous_release_survives_the_upgrade_exactly() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);

    // Install the PREVIOUS release and build real state with it, so the stable
    // memory this test upgrades from was written by the old code's own
    // `pre_upgrade`, with the old record shape. This is the whole point: state
    // written by the module under test cannot exercise a field addition.
    let old = wasms::previous_release_table_canister();
    world
        .reinstall_module(old.bytes.clone())
        .expect("the previous release must install");

    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let carol = world.actor("carol");

    // --- escrow, through the real ICRC-2 deposit path -----------------------
    for who in [alice, bob, carol] {
        world.fund_escrow(who, 5 * ICP).expect("deposit");
    }

    // --- money on the ledger waiting to be claimed --------------------------
    //
    // Transferred BEFORE the upgrade, claimed AFTER it, which is how the
    // verified-deposit anti-replay record gets exercised across a version boundary.
    // It cannot be recorded on the OLD build: `801aa79`'s `notify_deposit` declares
    // the ledger's `AccountIdentifier` as a record with a `hash` field where the
    // real ledger returns a bare `blob`, so its decode of `query_blocks` fails
    // outright (docs/SECURITY-FINDINGS.md FINDING 06, fixed in the module under
    // test). The attempt is made anyway and its outcome printed, because "the old
    // release cannot claim a real deposit at all" is part of what this fixture is
    // saying.
    let block = world
        .raw_transfer_to_canister(alice, 2 * ICP)
        .expect("a raw transfer into the canister's main account");
    match world.notify_deposit(alice, block) {
        Ok(v) => println!("M7: the previous release credited block {block}, balance now {v}"),
        Err(e) => println!(
            "M7: the previous release CANNOT claim block {block} (FINDING 06, its ledger reply \
             shapes are wrong): {e:?}. It is claimed after the upgrade instead."
        ),
    }

    // --- seats with chips, and a hand in progress ---------------------------
    world.join_table(alice, 0).expect("alice sits");
    world.join_table(bob, 1).expect("bob sits");
    world.join_table(carol, 2).expect("carol sits");
    world.advance(Duration::from_secs(4));
    deal_and_stop_on_the_flop(&world);

    let before = fingerprint(&world);
    assert!(
        before.chips_total > 0,
        "the fixture must put chips in seats or this test proves nothing: {}",
        before.chips_total
    );
    assert!(
        before.pot > 0,
        "the fixture must leave money in the pot: {}",
        before.pot
    );
    assert_eq!(
        before.community.len(),
        3,
        "the fixture must leave a live hand with a flop on the table, phase was {:?}",
        before.phase
    );
    assert!(
        before.seats.iter().all(|s| s.5.is_some()),
        "every seated player must be holding hole cards: {:?}",
        before.seats
    );
    println!(
        "M7: on {} -> escrow {} across {} principals, seated chips {}, pot {}, hand {} in \
         phase {:?} with board {:?}",
        &old.sha256[..12],
        before.escrow_total,
        before.escrow.len(),
        before.chips_total,
        before.pot,
        before.hand_number,
        before.phase,
        before.community.len()
    );

    // --- the real thing: install_code --mode upgrade ------------------------
    world.upgrade_to_module_under_test().unwrap_or_else(|e| {
        panic!(
            "the cross-version upgrade was REJECTED, so the module under test CANNOT BE \
             DEPLOYED over existing state. A rejection loses nothing, but it also means no \
             fix in this build can ever reach a live canister -- including the deposit \
             fix. The usual cause is a persisted field that is not `opt`: see \
             docs/SECURITY-FINDINGS.md FINDING 14 and check `PersistentState` and \
             `TableState` for any addition that is not wrapped in `Option`.\n\
             Rejection was:\n  {e}"
        )
    });

    let after = fingerprint(&world);

    // Stated as individual assertions rather than one struct comparison, because a
    // reader of a failure needs to know WHICH thing was lost.
    assert_eq!(
        after.escrow, before.escrow,
        "an accepted upgrade changed per-principal ESCROW"
    );
    assert_eq!(after.escrow_total, before.escrow_total, "escrow total moved");
    assert_eq!(
        after.seats, before.seats,
        "an accepted upgrade changed the SEATS: principals, chip stacks, this hand's \
         contributions, fold flags or HOLE CARDS. This is SECURITY-FINDINGS.md FINDING 14: a \
         non-`opt` field added to TableState makes `opt TableState` decode as null, and \
         post_upgrade then re-inits an EMPTY table over the top of real money."
    );
    assert_eq!(
        after.chips_total, before.chips_total,
        "an accepted upgrade DESTROYED SEATED CHIPS: {} -> {}",
        before.chips_total, after.chips_total
    );
    assert_eq!(
        after.pot, before.pot,
        "an accepted upgrade changed the live POT: {} -> {}",
        before.pot, after.pot
    );
    assert_eq!(
        (
            after.hand_number,
            after.phase.clone(),
            after.dealer_seat,
            after.small_blind_seat,
            after.big_blind_seat,
            after.action_on,
            after.current_bet,
            after.min_raise
        ),
        (
            before.hand_number,
            before.phase.clone(),
            before.dealer_seat,
            before.small_blind_seat,
            before.big_blind_seat,
            before.action_on,
            before.current_bet,
            before.min_raise
        ),
        "the in-progress hand did not survive: the betting state changed"
    );
    assert_eq!(
        (after.community.clone(), after.deck.clone(), after.deck_index),
        (before.community.clone(), before.deck.clone(), before.deck_index),
        "the DEAL did not survive: board, deck or deck cursor changed, which would deal \
         different cards for the rest of the hand than the committed shuffle promised"
    );
    assert_eq!(
        after.side_pots, before.side_pots,
        "the side-pot breakdown changed across the upgrade"
    );
    assert_eq!(
        after.seed_hash, before.seed_hash,
        "the shuffle commitment did not survive, so the revealed seed could never be checked \
         against it and the hand stops being verifiable"
    );

    // --- the anti-replay record, across a SECOND upgrade --------------------
    //
    // Claim the pre-upgrade transfer now, then upgrade again and require the replay
    // to still be refused. If `verified_deposits` / `deposit_watermark` did not
    // survive, the same ledger block could be credited a second time -- E-02, the
    // only proven fund theft in this project, reached through the deploy path.
    world
        .notify_deposit(alice, block)
        .expect("the module under test must be able to claim the pre-upgrade transfer");
    world.note_raw_deposit_credited(2 * ICP);
    assert!(
        world.notify_deposit(alice, block).is_err(),
        "block {block} was credited twice in a row, before any upgrade"
    );
    let escrow_with_claim = world.escrow_all();
    world
        .upgrade()
        .expect("a same-version upgrade must always be accepted");
    assert_eq!(
        world.escrow_all(),
        escrow_with_claim,
        "the second upgrade changed escrow"
    );
    assert!(
        world.notify_deposit(alice, block).is_err(),
        "AFTER AN UPGRADE, ledger block {block} could be credited a SECOND time. The \
         verified-deposit record did not survive, which turns the deploy path into the E-02 \
         double-credit primitive."
    );

    // --- and the restored hand is playable, not merely readable -------------
    let internal_before = world.snapshot().internal_total();
    money_safety::scenario::play_out_passively(&world, 60);
    let end = world.snapshot();
    assert!(
        !end.table.phase.hand_in_progress(),
        "the hand restored by post_upgrade must be able to finish; it stalled in {:?}",
        end.table.phase
    );
    assert_eq!(
        end.internal_total(),
        internal_before,
        "settling the hand that survived the upgrade minted or destroyed money: {} -> {}",
        internal_before,
        end.internal_total()
    );

    println!(
        "M7: cross-version upgrade {} -> {} SUCCEEDED with every e8, every stack, every card \
         and the anti-replay record intact, and the restored hand settled cleanly.",
        &old.sha256[..12],
        &wasms::table_canister_sha256()[..12]
    );
}

/// The other half of the coupling: the field the FINDING 13 fix DEPENDS on has to
/// survive an upgrade too.
///
/// The cross-version test above cannot cover this, because `801aa79` has no
/// `departed_stakes` to write. So this one creates a departed stake on the module
/// under test, upgrades to the same module, and checks that the stake — and above
/// all its OWNER — comes back byte for byte. Without it, an upgrade taken while a
/// player has left mid-hand would forget who the money in the pot belongs to, and
/// settlement would then have no owner to pay.
#[test]
fn m7b_a_departed_stake_and_its_owner_survive_an_upgrade() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");
    let carol = world.actor("carol");
    for who in [alice, bob, carol] {
        world.fund_escrow(who, 5 * ICP).expect("deposit");
    }
    world.join_table(alice, 0).expect("alice sits");
    world.join_table(bob, 1).expect("bob sits");
    world.join_table(carol, 2).expect("carol sits");
    world.advance(Duration::from_secs(4));
    deal_and_stop_on_the_flop(&world);

    // Whoever has money in the pot and is not on the clock can leave cleanly.
    let live = world.table_state();
    let leaver = live
        .seated()
        .find(|p| p.total_bet_this_hand > 0 && p.seat != live.action_on)
        .map(|p| (p.seat, p.principal, p.total_bet_this_hand))
        .expect("some seat other than the one on the clock has money in the pot");
    world
        .leave_table(leaver.1)
        .expect("leaving mid-hand must succeed");

    let before = fingerprint(&world);
    assert_eq!(
        before.departed,
        vec![(before.hand_number, leaver.0, leaver.1, leaver.2)],
        "the departure must have recorded exactly one stake, owned by the player who left"
    );

    world
        .upgrade()
        .expect("a same-version upgrade must always be accepted");

    let after = fingerprint(&world);
    assert_eq!(
        after.departed, before.departed,
        "the departed stake did not survive the upgrade. The OWNER travelling with the money \
         is what makes docs/SECURITY-FINDINGS.md FINDING 13's fix work; losing it at upgrade \
         time leaves money in the pot with nobody to pay it to."
    );
    assert_eq!(after.pot, before.pot, "the pot changed across the upgrade");
    assert_eq!(
        after.chips_total, before.chips_total,
        "seated chips changed across the upgrade"
    );
    assert_eq!(after.escrow, before.escrow, "escrow changed across the upgrade");

    // And the stake still reaches its owner after the upgrade.
    let escrow_before = world.escrow_all().1.get(&leaver.1).copied().unwrap_or(0);
    let internal_before = world.snapshot().internal_total();
    money_safety::scenario::play_out_passively(&world, 60);
    let end = world.snapshot();
    assert!(!end.table.phase.hand_in_progress());
    assert_eq!(
        end.internal_total(),
        internal_before,
        "settling after the upgrade minted or destroyed money"
    );
    let escrow_after = end.escrow.get(&leaver.1).copied().unwrap_or(0);
    assert!(
        escrow_after >= escrow_before,
        "the departed player's escrow went DOWN across the settlement that followed the \
         upgrade: {escrow_before} -> {escrow_after}"
    );
    println!(
        "M7b: departed stake (seat {}, owner {}, {} e8s) survived an upgrade and settled to \
         its owner.",
        leaver.0, leaver.1, leaver.2
    );
}

/// `pre_upgrade` must REFUSE the upgrade when it cannot save, not log and proceed.
///
/// # Why this is a source assertion and not an executed one
///
/// There is no way to make `stable_save` fail from outside the canister on
/// PocketIC, so the behaviour cannot be driven. What can be pinned is that the
/// decision has not been quietly reverted: the shipped code used to print
/// `CRITICAL: Failed to save state to stable memory` and fall through, with a
/// comment arguing a trap "could brick the canister".
///
/// That argument is backwards on a fund canister. A trap in `pre_upgrade` aborts
/// the UPGRADE and leaves the old code running with its heap intact — nothing is
/// bricked and nothing is lost. Proceeding has two outcomes and both are worse: if
/// stable memory is empty, `post_upgrade` panics anyway and the upgrade is rejected
/// with a less accurate reason; if stable memory still holds an OLDER snapshot from
/// a previous upgrade, `stable_restore` SUCCEEDS and the canister silently rolls
/// back to it — every balance and chip since that snapshot gone, and
/// `verified_deposits` / `deposit_watermark` rolled back with them, which re-opens
/// the E-02 replay window on blocks that were already credited.
#[test]
fn pre_upgrade_refuses_rather_than_proceeding_after_a_failed_save() {
    let src = std::fs::read_to_string(
        wasms::repo_root().join("src/table_canister/src/lib.rs"),
    )
    .expect("the engine source must be readable");
    let start = src
        .find("fn pre_upgrade()")
        .expect("pre_upgrade must exist in lib.rs");
    let body = &src[start..];
    let end = body
        .find("fn post_upgrade()")
        .expect("post_upgrade must follow pre_upgrade");
    let pre = &body[..end];

    assert!(
        pre.contains("stable_save"),
        "pre_upgrade no longer saves anything"
    );
    assert!(
        pre.contains("ic_cdk::trap("),
        "pre_upgrade does not trap on a failed `stable_save`. An upgrade that proceeds after \
         failing to save either gets rejected by post_upgrade anyway or silently restores an \
         OLDER snapshot, rolling back balances, chips AND the deposit anti-replay record. See \
         docs/SECURITY-FINDINGS.md FINDING 14."
    );
    assert!(
        !pre.contains("allow upgrade to proceed"),
        "the `Log but don't panic - allow upgrade to proceed` branch is back in pre_upgrade"
    );
}
