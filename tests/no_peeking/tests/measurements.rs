//! # THE NUMBERS
//!
//! A trustworthy game nobody will wait for is not a product, and a no-rake room
//! cannot absorb a cost it did not measure. `docs/NO-PEEKING-FEASIBILITY.md` §7
//! priced the ALTERNATIVE designs carefully and then budgeted this one from
//! published rates: "one inter-canister call from table to dealer per deal … budget
//! at most one extra round and the published 260,000 cycles per cross-canister
//! request/response". This file measures it instead.
//!
//! ## What these numbers are, and what they are not
//!
//! They come from PocketIC, so:
//!
//! * **Cycle figures are real debits**, taken as the difference of
//!   `pic.cycle_balance()` before and after. Unlike the vetKD figures in the
//!   feasibility doc — which came from the subnet's own price oracle because the
//!   local replica does not debit chain-key calls — these are observed spending.
//! * **Wall-clock is expressed in ROUNDS, not seconds.** PocketIC's clock is not
//!   mainnet's clock. The round structure is platform-independent and is the thing
//!   that decides whether a deal feels instant; the feasibility doc reasoned the
//!   same way in §2.4 and it is the honest unit.
//! * The baseline for "what a hand costs today" is `open_hand` measured DIRECTLY:
//!   one message that draws `raw_rand`, runs `poker_core::shuffle_deck` over 52
//!   cards and stores the result. That is precisely the work the live engine's
//!   `start_new_hand` does at the same point, so the difference between it and the
//!   full `start_hand` path is the measured price of moving the cards out of the
//!   table.

use candid::Principal;
use dealer_types::DealtInSeat;
use no_peeking::say;
use no_peeking::world::*;

fn banner(title: &str) {
    say!("\n================================================================");
    say!("  {title}");
    say!("================================================================");
}

/// A cycle figure, in the units a reader can act on.
fn cycles(n: u128) -> String {
    let usd = (n as f64) / 1_000_000_000_000.0 * 1.37; // 1 T cycles ~ $1.37 (XDR peg)
    if n >= 1_000_000_000 {
        format!("{:>15} ({:.3} B, ${:.6})", group(n), n as f64 / 1e9, usd)
    } else {
        format!("{:>15} ({:.3} M, ${:.8})", group(n), n as f64 / 1e6, usd)
    }
}

fn group(n: u128) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

/// Measures the replica's block interval once, so every later figure can be quoted
/// in rounds instead of in PocketIC's private units.
fn nanos_per_round(w: &World) -> u64 {
    let t0 = w.pic.get_time().as_nanos_since_unix_epoch();
    w.pic.tick();
    let t1 = w.pic.get_time().as_nanos_since_unix_epoch();
    (t1 - t0).max(1)
}

struct Meter<'a> {
    w: &'a World,
    cycles0: u128,
    time0: u64,
}

impl<'a> Meter<'a> {
    fn start(w: &'a World) -> Self {
        Self {
            w,
            cycles0: w.pic.cycle_balance(w.table) + w.pic.cycle_balance(w.dealer),
            time0: w.pic.get_time().as_nanos_since_unix_epoch(),
        }
    }
    fn spent(&self) -> u128 {
        let now = self.w.pic.cycle_balance(self.w.table) + self.w.pic.cycle_balance(self.w.dealer);
        self.cycles0.saturating_sub(now)
    }
    fn elapsed_ns(&self) -> u64 {
        self.w
            .pic
            .get_time()
            .as_nanos_since_unix_epoch()
            .saturating_sub(self.time0)
    }
}

// ---------------------------------------------------------------------------

/// What one message of each kind costs, and how many rounds it takes.
#[test]
fn the_price_of_moving_the_cards_out_of_the_table() {
    banner("THE DEAL: what the split actually costs");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = (w.player(0), w.player(1), w.player(2));
    w.sit(alice, 0, 100_000).unwrap();
    w.sit(bob, 1, 100_000).unwrap();
    w.sit(carol, 2, 100_000).unwrap();

    let per_round = nanos_per_round(&w);
    say!("replica block interval: {per_round} ns per round\n");

    // (1) THE DEAL ITSELF, with the inter-canister hop.
    let m = Meter::start(&w);
    let hand_id = w.start_hand(alice).unwrap();
    let full_deal = m.spent();
    let full_rounds = m.elapsed_ns() / per_round;
    say!("start_hand (table -> dealer -> raw_rand -> shuffle -> reply)");
    say!("   cycles {}", cycles(full_deal));
    say!("   rounds {full_rounds}");

    // Close this hand so the next measurement starts clean.
    for p in [alice, bob, carol] {
        w.stand_down(p, hand_id).unwrap();
    }
    w.act(alice, Action::Fold).unwrap();
    w.act(bob, Action::Fold).unwrap();
    w.try_advance(alice).unwrap();

    // (2) THE SAME DEAL WITHOUT THE HOP: `open_hand` called directly, which is one
    // message doing raw_rand + shuffle + store. That is the work the live engine's
    // `start_new_hand` does at the same point in the hand.
    let seats = vec![
        DealtInSeat {
            seat: 0,
            principal: alice,
        },
        DealtInSeat {
            seat: 1,
            principal: bob,
        },
        DealtInSeat {
            seat: 2,
            principal: carol,
        },
    ];
    let m = Meter::start(&w);
    let opened = w.open_hand_direct(w.table, seats).unwrap();
    let bare_deal = m.spent();
    let bare_rounds = m.elapsed_ns() / per_round;
    say!("\nopen_hand called directly (no hop) — the same work the live engine does");
    say!("   cycles {}", cycles(bare_deal));
    say!("   rounds {bare_rounds}");

    let added = full_deal.saturating_sub(bare_deal);
    say!("\nMEASURED PRICE OF THE SPLIT");
    say!("   added cycles per deal  {}", cycles(added));
    say!("   added rounds per deal  {}", full_rounds.saturating_sub(bare_rounds));
    say!(
        "   ratio                  {:.3}x the un-split deal",
        full_deal as f64 / bare_deal.max(1) as f64
    );
    say!(
        "\n   docs/NO-PEEKING-FEASIBILITY.md §7.5 budgeted ~1.05x and at most one extra\n   round. vetKD Design 1 for a 6-max hand was 156,923,076,918 cycles, ~31,000x."
    );

    // The split must not be an order of magnitude. If this ever fires, the design
    // has stopped being the cheap option and the recommendation has to be revisited.
    assert!(
        full_deal < bare_deal * 3,
        "the split costs {full_deal} against a bare deal of {bare_deal}: more than 3x is \
         not the design docs/NO-PEEKING-FEASIBILITY.md recommended"
    );
    assert!(
        full_rounds.saturating_sub(bare_rounds) <= 2,
        "the split added {} rounds; a deal is the most latency-sensitive moment of a hand",
        full_rounds.saturating_sub(bare_rounds)
    );

    // A player reading their own cards: a QUERY, so no round and no cycle fee to
    // the canister. It is faster than today's path, not slower.
    let m = Meter::start(&w);
    let _ = w.my_hole_cards(alice, opened.hand_id).unwrap();
    say!("\nmy_hole_cards (a query)");
    say!("   cycles {}", cycles(m.spent()));
    say!("   rounds {}", m.elapsed_ns() / per_round);
    assert_eq!(
        m.elapsed_ns() / per_round,
        0,
        "reading your own cards must not cost a consensus round"
    );
}

/// A whole hand, end to end, with every message counted.
#[test]
fn a_whole_hand_costs_this_much() {
    banner("A WHOLE HAND, six-handed, every message counted");
    let w = World::new(&["a", "b", "c", "d", "e", "f"]);
    let players: Vec<Principal> = (0..6).map(|i| w.player(i)).collect();
    for (i, p) in players.iter().enumerate() {
        w.sit(*p, i as u8, 100_000).unwrap();
    }

    let per_round = nanos_per_round(&w);
    let m = Meter::start(&w);
    let mut messages = 0usize;

    let hand_id = w.start_hand(players[0]).unwrap();
    messages += 1;

    // Preflop: everybody calls to the big blind.
    for _ in 0..12 {
        if w.query_round_closed() {
            break;
        }
        let Some(seat) = w.view().action_on else { break };
        if !w.act(players[seat as usize], Action::Call).is_ok() {
            break;
        }
        messages += 1;
    }

    // Three streets, each: every live seat acks, then the table advances.
    for _ in 0..3 {
        for p in &players {
            w.ack(*p, hand_id).unwrap();
            messages += 1;
        }
        w.try_advance(players[0]).unwrap();
        messages += 1;
        for _ in 0..12 {
            if w.query_round_closed() {
                break;
            }
            let Some(seat) = w.view().action_on else { break };
            if !w.act(players[seat as usize], Action::Check).is_ok() {
                break;
            }
            messages += 1;
        }
    }

    // Showdown.
    for p in &players {
        w.ack(*p, hand_id).unwrap();
        messages += 1;
    }
    w.try_advance(players[0]).unwrap();
    messages += 1;

    let spent = m.spent();
    let rounds = m.elapsed_ns() / per_round;
    let view = w.view();
    assert_eq!(view.phase, Phase::Complete, "the hand must have finished");
    assert!(!view.showdown.is_empty(), "it must have reached a showdown");

    say!("six-handed hand, dealt to showdown");
    say!("   update messages          {messages}");
    say!("   cycles (table + dealer)  {}", cycles(spent));
    say!("   rounds                   {rounds}");
    say!("   per-message average      {}", cycles(spent / messages as u128));
    say!(
        "\n   OF THOSE MESSAGES, {} are the ack protocol — the price of making an early\n   reveal impossible rather than merely visible. They are the players' own\n   heartbeats and the live engine already carries traffic of that shape\n   (`last_seen`, DISCONNECT_TIMEOUT_SECS = 90, 'Heartbeats: 2/second' in CLAUDE.md).",
        6 * 4
    );
    say!(
        "\n   FOR SCALE, from docs/NO-PEEKING-FEASIBILITY.md §7.2/§7.3:\n     vetKD Design 1, 6-max, key_1 : 156,923,076,918 cycles/hand  = $0.215/hand\n     that is ~$132,000/year for ONE running table, against a room that takes no rake."
    );
    say!(
        "     this hand                     : {} cycles",
        group(spent)
    );
    say!(
        "     ratio                         : {:.6}x the vetKD design",
        spent as f64 / 156_923_076_918f64
    );

    // The whole point of the recommendation over vetKD. If a hand ever costs within
    // an order of magnitude of the vetKD design, the cost argument in §7.5 is gone.
    assert!(
        spent < 156_923_076_918 / 10,
        "a whole hand cost {spent} cycles, which is no longer decisively cheaper than \
         the vetKD design's 156.9 B per deal"
    );
}

/// How long the dealer takes to answer, in rounds, for each thing a client waits
/// on during a hand.
#[test]
fn the_round_structure_of_a_hand() {
    banner("ROUNDS — what a player actually waits for");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = (w.player(0), w.player(1), w.player(2));
    for (i, p) in [alice, bob, carol].iter().enumerate() {
        w.sit(*p, i as u8, 100_000).unwrap();
    }
    let per_round = nanos_per_round(&w);
    let hand_id = w.start_hand(alice).unwrap();

    let mut rows: Vec<(&str, u64)> = Vec::new();

    let m = Meter::start(&w);
    let _ = w.my_hole_cards(alice, hand_id).unwrap();
    rows.push(("read my own two cards (query)", m.elapsed_ns() / per_round));

    let m = Meter::start(&w);
    let _ = w.community(alice, hand_id);
    rows.push(("read the board (query)", m.elapsed_ns() / per_round));

    let m = Meter::start(&w);
    w.ack(alice, hand_id).unwrap();
    rows.push(("ack / heartbeat (update)", m.elapsed_ns() / per_round));

    w.ack(bob, hand_id).unwrap();
    w.ack(carol, hand_id).unwrap();
    for _ in 0..6 {
        if w.query_round_closed() {
            break;
        }
        let Some(seat) = w.view().action_on else { break };
        let who = [alice, bob, carol][seat as usize];
        if !w.act(who, Action::Call).is_ok() {
            break;
        }
    }
    let m = Meter::start(&w);
    w.try_advance(alice).unwrap();
    rows.push((
        "reveal a street (table -> dealer)",
        m.elapsed_ns() / per_round,
    ));

    say!("{:<40} rounds", "step");
    for (label, r) in &rows {
        say!("{label:<40} {r}");
    }
    say!(
        "\n  A street reveal is one inter-canister hop. The feasibility doc measured a\n  same-subnet hop as not advancing the local block clock at all and budgeted at\n  most one extra round on mainnet; that is what this shows.\n\n  For contrast, §2.4: vetKD derivations issued SEQUENTIALLY are one round each\n  (17 derivations = 17 rounds = 4,239 ms locally), and DFINITY's own forum thread\n  records cross-subnet vetKD exceeding the 10-second synchronous-call ceiling in\n  production. Against a 30-45 s action clock that is a product problem."
    );

    assert!(
        rows.iter()
            .find(|(l, _)| l.contains("query"))
            .map(|(_, r)| *r == 0)
            .unwrap_or(false),
        "a query must not cost a round"
    );
    let reveal = rows.iter().find(|(l, _)| l.contains("street")).unwrap().1;
    assert!(
        reveal <= 2,
        "a street reveal took {reveal} rounds; a deal is the most latency-sensitive \
         moment of a hand"
    );
}

/// The dealer's own instruction count for the one thing every player does every
/// few seconds.
#[test]
fn reading_your_own_cards_is_cheap_enough_to_poll() {
    banner("INSTRUCTIONS — the cost of the hot path");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = (w.player(0), w.player(1), w.player(2));
    for (i, p) in [alice, bob, carol].iter().enumerate() {
        w.sit(*p, i as u8, 100_000).unwrap();
    }
    let hand_id = w.start_hand(alice).unwrap();

    let bytes = w
        .query_raw(
            w.dealer,
            alice,
            "instructions_to_read_my_cards",
            candid::encode_one(hand_id).unwrap(),
        )
        .expect("the probe is a public query");
    let instructions: u64 = candid::decode_one(&bytes).expect("decode");
    say!("my_hole_cards costs {} instructions", group(instructions as u128));
    say!(
        "  (it re-derives the whole 52-card deck from the seed every time, on purpose:\n   the dealer stores 32 bytes and never a deck, so there is no deck-shaped object\n   in its heap for anybody to find.)"
    );
    say!(
        "\n  For scale, docs/NO-PEEKING-FEASIBILITY.md §7.1 measured the live engine's\n  post-await deal slice at 27,792 instructions, and the IC bills 1 B cycles per\n  1 B instructions."
    );
    // The IC's per-message instruction limit for a query is 5 B. Polling this at
    // 1 Hz per player must not be anywhere near it.
    assert!(
        instructions < 5_000_000,
        "reading your own cards costs {instructions} instructions, which is too much to poll"
    );

    // And it does not grow with the number of hands the dealer has seen.
    let mut ids = vec![hand_id];
    for _ in 0..3 {
        for p in [alice, bob, carol] {
            let _ = w.stand_down(p, *ids.last().unwrap());
        }
        let _ = w.act(alice, Action::Fold);
        let _ = w.act(bob, Action::Fold);
        let _ = w.try_advance(alice);
        if let no_peeking::world::Outcome::Ok(id) = w.start_hand(alice) {
            ids.push(id);
        }
    }
    let bytes = w
        .query_raw(
            w.dealer,
            alice,
            "instructions_to_read_my_cards",
            candid::encode_one(*ids.last().unwrap()).unwrap(),
        )
        .expect("probe");
    let later: u64 = candid::decode_one(&bytes).expect("decode");
    say!(
        "\nafter {} hands the same read costs {} instructions",
        ids.len(),
        group(later as u128)
    );
    assert!(
        later < instructions * 2,
        "the cost of reading your own cards grew from {instructions} to {later} as hands \
         accumulated; the dealer is keeping something it should not"
    );

    // Runway, in the units an operator can act on.
    let health = w.dealer_health();
    say!(
        "\ndealer balance {} cycles after {} hands",
        group(health.cycle_balance),
        health.hands_dealt
    );
    let _ = w.advance_street_direct(w.table, 0); // no such hand; harmless
    say!("floor {} cycles", group(health.min_open_balance));
    assert!(health.hands_dealt >= 1);
}
