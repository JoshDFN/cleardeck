//! THE PERMANENT RECORD -- docs/SECURITY-FINDINGS.md FINDING 30.
//!
//! ClearDeck's central claim is a record a stranger can check. These tests drive
//! the real table canister and the real archive canister on PocketIC, play the
//! exact sequence the third auditor found -- somebody leaves mid-hand, somebody
//! else takes the empty chair -- and then read the archive as an outsider.
//!
//! Three properties, and every one of them was false before this file existed:
//!
//!   1. **Everyone who put money in the hand is named.** The player list is built
//!      from `hand_stakes`, the settlement basis, so a departed stake keeps its
//!      OWNER. Before: built from `state.players` at settlement, so a player who
//!      left was simply absent from the permanent record of the hand they paid for.
//!   2. **Nobody who did not play is named.** A principal who bought into the
//!      empty chair after the flop was recorded as a participant -- with a
//!      position, and with the DEPARTED player's starting stack.
//!   3. **The record states how many players were dealt in, and in what order.**
//!      docs/SHUFFLE-SPEC.md section 4 offsets the board by `P`, and told a
//!      verifier to count `P` from the record itself. On a hand somebody left,
//!      that count was wrong and the verifier reproduced the WRONG BOARD.
//!
//! None of this is a money defect and that is the point: every conservation
//! invariant in this harness is green on the sequence below, in both directions.

use candid::Principal;
use money_safety::archive::{install_archive, HandHistoryRecord};
use money_safety::invariants::*;
use money_safety::scenario::{on_clock, seat_players};
use money_safety::table_api::*;
use money_safety::world::*;
use std::collections::BTreeSet;
use std::time::Duration;

const ICP: u64 = 100_000_000;

/// Everything the shuffle spec needs to lay out a hand, derived from a seed the
/// same way `docs/SHUFFLE-SPEC.md` section 3 says, with `poker_core`'s own shuffle.
///
/// The spec is cross-checked against two independent outsider verifiers by
/// `poker_core`'s golden vectors; what this reproduces is section 4, the DEALING
/// ORDER, which is the part FINDING 30 makes unusable.
fn board_from_seed(seed_hex: &str, players: usize) -> (Vec<Card>, Card, Card) {
    let seed = hex_bytes(seed_hex);
    let mut deck = poker_core::create_deck();
    poker_core::shuffle_deck(&mut deck, &seed);
    let p = players;
    let flop: Vec<Card> = (2 * p + 1..=2 * p + 3).map(|i| engine_card(deck[i])).collect();
    (flop, engine_card(deck[2 * p + 5]), engine_card(deck[2 * p + 7]))
}

fn engine_card(c: poker_core::Card) -> Card {
    Card {
        suit: match c.suit {
            poker_core::Suit::Hearts => Suit::Hearts,
            poker_core::Suit::Diamonds => Suit::Diamonds,
            poker_core::Suit::Clubs => Suit::Clubs,
            poker_core::Suit::Spades => Suit::Spades,
        },
        rank: match c.rank {
            poker_core::Rank::Two => Rank::Two,
            poker_core::Rank::Three => Rank::Three,
            poker_core::Rank::Four => Rank::Four,
            poker_core::Rank::Five => Rank::Five,
            poker_core::Rank::Six => Rank::Six,
            poker_core::Rank::Seven => Rank::Seven,
            poker_core::Rank::Eight => Rank::Eight,
            poker_core::Rank::Nine => Rank::Nine,
            poker_core::Rank::Ten => Rank::Ten,
            poker_core::Rank::Jack => Rank::Jack,
            poker_core::Rank::Queen => Rank::Queen,
            poker_core::Rank::King => Rank::King,
            poker_core::Rank::Ace => Rank::Ace,
        },
    }
}

fn hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn show(c: &Card) -> String {
    let r = match c.rank {
        Rank::Two => "2", Rank::Three => "3", Rank::Four => "4", Rank::Five => "5",
        Rank::Six => "6", Rank::Seven => "7", Rank::Eight => "8", Rank::Nine => "9",
        Rank::Ten => "T", Rank::Jack => "J", Rank::Queen => "Q", Rank::King => "K",
        Rank::Ace => "A",
    };
    let s = match c.suit {
        Suit::Hearts => "h", Suit::Diamonds => "d", Suit::Clubs => "c", Suit::Spades => "s",
    };
    format!("{r}{s}")
}

fn dump(label: &str, rec: &HandHistoryRecord) {
    println!("\n--- {label}: archived hand_id {} (table hand {}) ---", rec.hand_id, rec.hand_number);
    println!("  total_pot {}  rake {}  showdown {}", rec.total_pot, rec.rake, rec.went_to_showdown);
    println!("  seed_hash     {}", rec.shuffle_proof.seed_hash);
    println!("  revealed_seed {}", rec.shuffle_proof.revealed_seed);
    match &rec.dealt_in {
        Some(d) => {
            println!("  dealt in ({} players, deal order):", d.len());
            for (k, s) in d.iter().enumerate() {
                println!("      k={k} seat {} {}", s.seat, s.principal);
            }
        }
        None => println!("  dealt in: THE RECORD DOES NOT SAY (no such field)"),
    }
    println!("  players named by the record:");
    for p in &rec.players {
        println!(
            "      seat {} {} pos={} start={} end={} won={} contributed={:?} dealt_in={:?} left={:?}",
            p.seat, p.principal, p.position, p.starting_chips, p.ending_chips, p.amount_won,
            p.contributed, p.dealt_in, p.left_mid_hand
        );
    }
    for w in &rec.winners {
        println!("      WINNER seat {} {} {}", w.seat, w.principal, w.amount);
    }
    let flop = rec.flop.as_ref().map(|(a, b, c)| format!("{} {} {}", show(a), show(b), show(c)));
    println!(
        "  board: flop {:?} turn {:?} river {:?}",
        flop,
        rec.turn.as_ref().map(show),
        rec.river.as_ref().map(show)
    );
}

/// Set up: four seats dealt in, then one player leaves mid-hand.
///
/// `join_after` optionally buys the departed player's empty chair before the hand
/// settles. Returns (world, archive, dealt-in principals, the leaver).
struct Played {
    world: World,
    archive: money_safety::archive::Archive,
    dealt_in: Vec<Principal>,
    leaver: Principal,
    /// What the LIVE TABLE said the leaver had in the pot, read before she left
    /// and before any of this settled. An anchor from outside the record.
    leaver_stake: u64,
    intruder: Option<Principal>,
    hand_number: u64,
}

fn play_a_hand_somebody_left(join_after: bool) -> Played {
    let world = World::new(
        TableConfig::six_max_icp(),
        &["alice", "bob", "carol", "dave", "attacker"],
    );
    let archive = install_archive(&world);

    seat_players(&world, &["alice", "bob", "carol", "dave"], 6 * ICP);
    let alice = world.actor("alice");
    let attacker = world.actor("attacker");

    world.start_new_hand(alice).expect("deal");
    let t = world.table_state();
    let dealt_in: Vec<Principal> = t
        .players
        .iter()
        .flatten()
        .filter(|p| p.hole_cards.is_some())
        .map(|p| p.principal)
        .collect();
    assert_eq!(dealt_in.len(), 4, "four seats must be dealt in");
    println!("dealt in: {dealt_in:?}");

    // Get real money from every seat into the pot: one orbit of calls.
    for _ in 0..4 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(who) = on_clock(&t) else { break };
        if world.player_action(who, PlayerAction::Call).is_err() {
            let _ = world.player_action(who, PlayerAction::Check);
        }
    }

    // THE MOVE. Alice walks away from a hand she has money in. Her stake stays in
    // the pot; `departed_stakes` keeps the record of whose it is.
    let before = world.table_state();
    assert!(before.phase.hand_in_progress(), "the hand must still be live");
    let alice_stake = before
        .players
        .iter()
        .flatten()
        .find(|p| p.principal == alice)
        .map(|p| p.total_bet_this_hand)
        .unwrap_or(0);
    assert!(alice_stake > 0, "alice must have money in the pot before she leaves");
    world.leave_table(alice).expect("alice leaves mid-hand");
    let after_leave = world.table_state();
    assert!(
        after_leave.players[0].is_none(),
        "alice's chair must be empty"
    );
    println!("alice left with {alice_stake} e8s of hers in the pot");

    let intruder = if join_after {
        // Somebody who has never seen a card in this hand buys the empty chair.
        world.fund_escrow(attacker, 6 * ICP).expect("attacker deposit");
        world.join_table(attacker, 0).expect("attacker takes the chair");
        println!("attacker bought seat 0 mid-hand");
        Some(attacker)
    } else {
        None
    };

    // Play it out.
    for _ in 0..40 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(who) = on_clock(&t) else {
            world.advance(Duration::from_secs(1));
            continue;
        };
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Fold).is_ok() {
            continue;
        }
        world.advance(Duration::from_secs(1));
    }
    let end = world.table_state();
    assert_eq!(end.phase, GamePhase::HandComplete, "the hand must settle");
    // The archive call is an inter-canister call made from settlement; let it land.
    world.advance(Duration::from_secs(1));

    Played {
        world,
        archive,
        dealt_in,
        leaver: alice,
        leaver_stake: alice_stake,
        intruder,
        hand_number: end.hand_number,
    }
}

// ---------------------------------------------------------------------------
// M12 -- the three properties
// ---------------------------------------------------------------------------

/// FINDING 30, the "omits" half: the player who left is in the permanent record.
#[test]
fn the_archive_names_the_player_who_left_mid_hand() {
    let p = play_a_hand_somebody_left(false);
    let rec = p
        .archive
        .hand_numbered(&p.world, p.hand_number)
        .expect("the hand must reach the archive");
    dump("leaver, chair left empty", &rec);

    let named = rec.named_principals();
    assert!(
        named.contains(&p.leaver),
        "FALSE RECORD: {} put money into hand {} and then left, and the permanent record does \
         not name her at all. The archive is the artifact 'provably fair' rests on; a hand whose \
         participants are wrong cannot be verified by anybody. Record names: {:?}",
        p.leaver,
        rec.hand_number,
        named
    );

    // And with what she put in -- checked against the figure read off the LIVE
    // TABLE before she left, which is a source outside the record entirely.
    let contributed = rec.contributed_by_principal();
    assert_eq!(
        contributed.get(&p.leaver).copied().unwrap_or(0),
        p.leaver_stake,
        "the record must say she put in the {} e8s the live table said she had in the pot \
         before she left, not a figure derived from the seats afterwards",
        p.leaver_stake
    );
}

/// The blind spot this file would otherwise have.
///
/// The fuzzer's M12 gate reads the TABLE's copy of the record, because installing
/// an archive in every fuzz world would cost a canister per run. That is only
/// sound if the table's copy and the archive's copy are the same list -- and
/// "these two lists are built the same way" is exactly the kind of claim that has
/// been false four times in this project. `record_hand_to_history` builds it ONCE
/// and clones it into both, and this test is what makes that checkable rather than
/// a comment: it reads both copies, off the wire, and compares them field by field.
#[test]
fn the_tables_copy_of_the_record_and_the_archives_copy_are_the_same_list() {
    let p = play_a_hand_somebody_left(true);
    let archived = p
        .archive
        .hand_numbered(&p.world, p.hand_number)
        .expect("the hand must reach the archive");
    let local = p
        .world
        .hand_history(p.hand_number)
        .expect("the table must keep its own copy");

    let local_players = local
        .participants
        .as_ref()
        .expect("the table's own record must name the participants too -- FINDING 30 says \
                get_hand_history had the identical omission, so the client showed the same \
                false record");

    let mine: Vec<(u8, Principal, u64, u64)> = local_players
        .iter()
        .map(|x| (x.seat, x.principal, x.contributed.unwrap_or(0), x.amount_won))
        .collect();
    let theirs: Vec<(u8, Principal, u64, u64)> = archived
        .players
        .iter()
        .map(|x| (x.seat, x.principal, x.contributed.unwrap_or(0), x.amount_won))
        .collect();
    assert_eq!(
        mine, theirs,
        "the table's copy of hand {} and the archive's copy name different people or different \
         amounts. Every gate that reads only one of them is blind to the other.",
        p.hand_number
    );

    let mine_dealt: Vec<(u8, Principal)> = local
        .dealt_in
        .as_ref()
        .expect("the table must state the deal too")
        .iter()
        .map(|d| (d.seat, d.principal))
        .collect();
    let theirs_dealt: Vec<(u8, Principal)> = archived
        .dealt_in
        .as_ref()
        .expect("the archive must state the deal")
        .iter()
        .map(|d| (d.seat, d.principal))
        .collect();
    assert_eq!(
        mine_dealt, theirs_dealt,
        "the two copies disagree about who was dealt in, so they would give a verifier two \
         different boards"
    );
}

/// FINDING 30, the "invents" half: a mid-hand arrival is NOT in the record.
#[test]
fn the_archive_does_not_name_a_player_who_bought_the_chair_mid_hand() {
    let p = play_a_hand_somebody_left(true);
    let intruder = p.intruder.expect("this arm seats an intruder");
    let rec = p
        .archive
        .hand_numbered(&p.world, p.hand_number)
        .expect("the hand must reach the archive");
    dump("leaver, chair re-occupied", &rec);

    assert!(
        !rec.named_principals().contains(&intruder),
        "FALSE RECORD: {} bought seat 0 AFTER the deal and was never dealt a card, and the \
         permanent record of hand {} names him as a player. A confident false record is worse \
         than no record.",
        intruder,
        rec.hand_number
    );
    assert!(
        rec.named_principals().contains(&p.leaver),
        "the departed player must still be named: {:?}",
        rec.named_principals()
    );
    for seat in rec.dealt_in.as_ref().expect("the record must state who was dealt in") {
        assert_ne!(
            seat.principal, intruder,
            "the dealt-in list must not contain a principal who was never dealt a card"
        );
    }
}

/// FINDING 30, the verifier half: the record states `P`, and `P` reproduces the
/// board that was actually dealt.
///
/// This is the whole product claim, executed. Everything used below comes out of
/// the archived record and out of docs/SHUFFLE-SPEC.md; nothing comes from the
/// canister's live state.
#[test]
fn the_board_can_be_reproduced_from_the_archived_record_alone() {
    let p = play_a_hand_somebody_left(false);
    let rec = p
        .archive
        .hand_numbered(&p.world, p.hand_number)
        .expect("the hand must reach the archive");
    dump("board reproduction", &rec);

    let seed = &rec.shuffle_proof.revealed_seed;
    assert!(!seed.is_empty(), "a settled hand must archive its revealed seed");

    let dealt = rec
        .players_dealt_in()
        .expect("SHUFFLE-SPEC section 4 needs P, and the record must state it");
    assert_eq!(
        dealt,
        p.dealt_in.len(),
        "the record says {dealt} players were dealt in; {} were",
        p.dealt_in.len()
    );

    let (flop, turn, river) = board_from_seed(seed, dealt);
    let archived_flop = rec.flop.as_ref().expect("a hand that saw a flop archives it");
    assert_eq!(
        vec![archived_flop.0, archived_flop.1, archived_flop.2],
        flop,
        "the flop reproduced from the archived seed with P={dealt} is not the flop in the record"
    );
    if let Some(t) = rec.turn {
        assert_eq!(t, turn, "turn mismatch at P={dealt}");
    }
    if let Some(r) = rec.river {
        assert_eq!(r, river, "river mismatch at P={dealt}");
    }

    // And the count the OLD record implied -- the seats still occupied at
    // settlement -- reproduces a different board, which is what made the wrong
    // participant list a fairness defect rather than a cosmetic one.
    let occupied_at_settlement = rec
        .players
        .iter()
        .filter(|pl| pl.left_mid_hand == Some(false))
        .map(|pl| pl.principal)
        .collect::<BTreeSet<_>>()
        .len();
    if occupied_at_settlement != dealt {
        let (wrong_flop, _, _) = board_from_seed(seed, occupied_at_settlement);
        assert_ne!(
            wrong_flop, flop,
            "for this test to mean anything, P={occupied_at_settlement} must give a DIFFERENT \
             board from P={dealt}"
        );
        println!(
            "  P={dealt} (correct) -> {}\n  P={occupied_at_settlement} (seats at settlement) -> {}",
            flop.iter().map(show).collect::<Vec<_>>().join(" "),
            wrong_flop.iter().map(show).collect::<Vec<_>>().join(" ")
        );
    }
}

/// The record must balance: what it says people put in, what it says they were
/// paid, and the pot must be one arithmetic.
#[test]
fn the_archived_record_balances() {
    for join_after in [false, true] {
        let p = play_a_hand_somebody_left(join_after);
        let rec = p
            .archive
            .hand_numbered(&p.world, p.hand_number)
            .expect("the hand must reach the archive");
        dump(&format!("balance, chair re-occupied={join_after}"), &rec);

        let contributed: u64 = rec
            .players
            .iter()
            .fold(0u64, |a, pl| a.saturating_add(pl.contributed.unwrap_or(0)));
        let won: u64 = rec
            .players
            .iter()
            .fold(0u64, |a, pl| a.saturating_add(pl.amount_won));
        assert_eq!(
            contributed, rec.total_pot,
            "the record's own contributions must add up to the pot it records"
        );
        assert_eq!(
            won, rec.total_pot,
            "the record's own winnings must add up to the pot it records"
        );
        assert_eq!(
            rec.awarded_total(),
            rec.total_pot,
            "the winner list must add up to the pot"
        );
        let delta: i128 = rec.players.iter().fold(0i128, |a, pl| {
            a + pl.ending_chips as i128 - pl.starting_chips as i128
        });
        assert_eq!(
            delta, 0,
            "sum(ending - starting) over every participant must be zero: the house takes nothing"
        );
    }
}

/// The whole point of building the record from the settlement basis: the archive
/// and the money agree about who was in the hand, on the same hand.
#[test]
fn the_archive_agrees_with_the_settlement_basis_and_the_money_is_untouched() {
    let p = play_a_hand_somebody_left(true);
    let rec = p
        .archive
        .hand_numbered(&p.world, p.hand_number)
        .expect("the hand must reach the archive");

    // Every conservation invariant, on the same state. They were green before the
    // fix too -- that is why FINDING 30 survived three auditors' money tests.
    let vs = check_world(&p.world);
    assert_no_new_violations(&vs, "after a hand somebody left and somebody joined");

    let dealt: BTreeSet<Principal> = rec
        .dealt_in
        .as_ref()
        .expect("the record must state who was dealt in")
        .iter()
        .map(|d| d.principal)
        .collect();
    assert_eq!(
        dealt,
        p.dealt_in.iter().copied().collect::<BTreeSet<_>>(),
        "the record's dealt-in set must be the set the shuffle actually consumed"
    );
}

/// An upgrade in the middle of a hand must not cost that hand its verifiability.
///
/// `DEALT_IN` is written by the deal loop into a `thread_local`, and thread-locals
/// are heap: they do not survive an upgrade unless somebody persists them. If it
/// were dropped, the hand would settle, archive normally, balance perfectly, name
/// the right people -- and carry `dealt_in = null`, so its board could never be
/// reproduced. Nothing else in this file would notice, because everything else
/// about that record is true.
///
/// So this drives the exact sequence: deal, upgrade to the same module (which runs
/// `pre_upgrade` and `post_upgrade` for real), finish the hand, and reproduce the
/// board from the archived record alone.
#[test]
fn an_upgrade_in_the_middle_of_a_hand_does_not_lose_the_deal_record() {
    let mut world = World::new(
        TableConfig::six_max_icp(),
        &["alice", "bob", "carol", "dave"],
    );
    let archive = install_archive(&world);
    seat_players(&world, &["alice", "bob", "carol"], 6 * ICP);

    world.start_new_hand(world.actor("alice")).expect("deal");
    let dealt = world
        .table_state()
        .players
        .iter()
        .flatten()
        .filter(|p| p.hole_cards.is_some())
        .count();
    assert_eq!(dealt, 3);

    world.upgrade().expect("a mid-hand upgrade must succeed");

    for _ in 0..40 {
        let t = world.table_state();
        if !t.phase.hand_in_progress() {
            break;
        }
        let Some(who) = on_clock(&t) else {
            world.advance(Duration::from_secs(1));
            continue;
        };
        if world.player_action(who, PlayerAction::Check).is_ok() {
            continue;
        }
        if world.player_action(who, PlayerAction::Call).is_ok() {
            continue;
        }
        let _ = world.player_action(who, PlayerAction::Fold);
    }
    let end = world.table_state();
    assert_eq!(end.phase, GamePhase::HandComplete);
    world.advance(Duration::from_secs(1));

    let rec = archive
        .hand_numbered(&world, end.hand_number)
        .expect("the hand must reach the archive");
    dump("upgraded mid-hand", &rec);

    let p = rec.players_dealt_in().expect(
        "an upgrade must not cost the hand its P. Without it the record still balances and still \
         names the right people, and the board can never be reproduced -- the quietest possible \
         way for a hand to stop being verifiable.",
    );
    assert_eq!(p, dealt, "the deal record must survive the upgrade intact");

    if let Some(flop) = rec.flop.as_ref() {
        let (expected, _, _) = board_from_seed(&rec.shuffle_proof.revealed_seed, p);
        assert_eq!(
            vec![flop.0, flop.1, flop.2],
            expected,
            "the board must still reproduce from the archived record after an upgrade"
        );
    }
}

use money_safety::assert_no_new_violations;
