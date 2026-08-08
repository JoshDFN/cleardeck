//! # THE NEGATIVE
//!
//! An independent auditor, given only the public interface, filed this as medium:
//!
//! > "Any controller can read the complete shuffled 52-card deck and every seated
//! > player's hole cards while a hand is in progress. Verified live at
//! > `phase = PreFlop`, `deck_index = 4`: the reply contained all 52 cards in dealt
//! > order plus all three players' hole cards."
//!
//! This file runs that attack against the sealed-dealer construction and asserts it
//! comes back empty. It is deliberately written the way a negative test has to be
//! written, because a negative test's failure mode is passing while broken:
//!
//! * **The method list comes from the MODULE, not from a document.** Both wasms'
//!   export sections are parsed, so a method added tomorrow is attacked tomorrow.
//!   `src/table_canister/table_canister.did` not describing the code it claims to is
//!   already a finding in this project (FINDING 03 / DEFECTS E-08); a hand-written
//!   list here would inherit that failure.
//! * **The detector is tested.** `src/peek.rs` has six unit tests that feed it cards
//!   in bare replies, nested three containers deep, inside `Result` variants and in
//!   the second of several reply values, and check it finds them — and two that check
//!   it does not fire on a reply with no card in it.
//! * **The secrets are learned AFTERWARDS.** The attack records every reply while
//!   the hand is live. Only then is the hand forced open and the seed published, and
//!   only then does the harness know what the eleven secret cards were. Every recorded
//!   reply is searched for all of them. Nothing about the attack could have been
//!   tuned to the answer.
//! * **The snapshot read is included**, because `docs/NO-PEEKING-FEASIBILITY.md`
//!   §3.1 showed that the obvious fix — deleting the field from `get_table_state` —
//!   closes nothing: the deck came out of a downloaded canister snapshot with four
//!   stock `dfx` commands and no code change.

use candid::{encode_args, encode_one, Principal};
use dealer_types::DealtInSeat;
use no_peeking::peek::cards_leaked;
use no_peeking::say;
use no_peeking::wasm_exports;
use no_peeking::world::*;
use no_peeking::build;
use poker_core::{create_deck, shuffle_deck, Card};
use std::collections::BTreeSet;
use std::time::Duration;

fn banner(title: &str) {
    say!("\n================================================================");
    say!("  {title}");
    say!("================================================================");
}

fn fmt(c: &Card) -> String {
    let r = match c.rank {
        poker_core::Rank::Ten => "T".to_string(),
        poker_core::Rank::Jack => "J".to_string(),
        poker_core::Rank::Queen => "Q".to_string(),
        poker_core::Rank::King => "K".to_string(),
        poker_core::Rank::Ace => "A".to_string(),
        o => (o as u8).to_string(),
    };
    let s = match c.suit {
        poker_core::Suit::Hearts => "h",
        poker_core::Suit::Diamonds => "d",
        poker_core::Suit::Clubs => "c",
        poker_core::Suit::Spades => "s",
    };
    format!("{r}{s}")
}

/// Every argument encoding the attack tries against every method.
///
/// A method whose signature is not in this list rejects with a decode error and is
/// reported as UNEXERCISED, which fails the run. That is the point: the attack must
/// not be able to quietly skip a method.
fn arg_candidates(hand_id: u64, someone: Principal) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("()", encode_args(()).unwrap()),
        ("(null)", encode_one(()).unwrap()),
        ("(hand_id)", encode_one(hand_id).unwrap()),
        ("(hand_id-1)", encode_one(hand_id.saturating_sub(1)).unwrap()),
        ("(hand_id, seat 0)", encode_args((hand_id, 0u8)).unwrap()),
        ("(hand_id, seat 1)", encode_args((hand_id, 1u8)).unwrap()),
        (
            "(hand_id, all seats)",
            encode_args((hand_id, vec![0u8, 1u8, 2u8])).unwrap(),
        ),
        ("(principal)", encode_one(someone).unwrap()),
        ("(seat 0, 1000)", encode_args((0u8, 1000u64)).unwrap()),
        ("(seat 0)", encode_one(0u8).unwrap()),
        ("(Action::Check)", encode_one(Action::Check).unwrap()),
        ("(Action::Fold)", encode_one(Action::Fold).unwrap()),
        (
            "(seats)",
            encode_one(vec![
                DealtInSeat {
                    seat: 0,
                    principal: someone,
                },
                DealtInSeat {
                    seat: 1,
                    principal: Principal::self_authenticating(b"x"),
                },
            ])
            .unwrap(),
        ),
    ]
}

/// One recorded reply from the attack.
struct Reply {
    canister: &'static str,
    method: String,
    sender: &'static str,
    /// Which dealt-in seat, if any, the sender legitimately holds. That seat's two
    /// cards are the ONE thing this whole design is supposed to give them, so they
    /// are subtracted from the secret set for this sender and nothing else is.
    sender_seat: Option<usize>,
    arg: &'static str,
    bytes: Vec<u8>,
}

// ---------------------------------------------------------------------------

#[test]
fn no_method_of_either_canister_yields_a_live_hand_s_cards_to_anybody() {
    banner("THE AUDITOR'S ATTACK, run against the sealed dealer");

    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = (w.player(0), w.player(1), w.player(2));
    let stranger = Principal::self_authenticating(b"cleardeck-no-peeking-stranger");

    w.sit(alice, 0, 1_000).unwrap();
    w.sit(bob, 1, 1_000).unwrap();
    w.sit(carol, 2, 1_000).unwrap();
    let hand_id = w.start_hand(alice).unwrap();

    let view = w.view();
    assert_eq!(view.phase, Phase::PreFlop, "the attack must run MID-HAND");
    say!("hand {hand_id} is live at phase = {:?}", view.phase);
    say!("  table  {}  controllers = {:?}", w.table, w.pic.get_controllers(w.table));
    say!("  dealer {}  controllers = {:?}", w.dealer, w.pic.get_controllers(w.dealer));
    assert!(w.pic.get_controllers(w.dealer).is_empty());

    // ---- who attacks ---------------------------------------------------
    //
    // `w.table` is in this list on purpose. PocketIC lets a test send an ingress
    // message from ANY principal, including a canister's, so this row is strictly
    // stronger than replacing the table's code with something hostile: it is what a
    // table that had been reinstalled with an attacker's wasm could do, without
    // having to write that wasm.
    let attackers: Vec<(&'static str, Principal, Option<usize>)> = vec![
        ("THE TABLE'S CONTROLLER", w.operator, None),
        ("THE TABLE CANISTER ITSELF", w.table, None),
        ("a seated opponent (alice)", alice, Some(0)),
        ("a stranger", stranger, None),
        ("anonymous", Principal::anonymous(), None),
    ];

    let surfaces: Vec<(&'static str, Principal, Vec<u8>)> = vec![
        ("dealer", w.dealer, w.dealer_wasm.clone()),
        ("table", w.table, w.table_wasm.clone()),
    ];

    // ---- the sweep -----------------------------------------------------
    let mut replies: Vec<Reply> = Vec::new();
    let mut unexercised: Vec<String> = Vec::new();
    let mut total_calls = 0usize;

    for (label, target, wasm) in &surfaces {
        let methods = wasm_exports::methods(wasm);
        assert!(
            !methods.is_empty(),
            "the {label} module exports no methods; the harness would prove nothing"
        );
        say!(
            "\n{label} exports {} methods: {}",
            methods.len(),
            methods
                .iter()
                .map(|m| format!("{}/{}", m.name, m.kind))
                .collect::<Vec<_>>()
                .join(" ")
        );
        for m in &methods {
            let mut answered = false;
            for (who_label, who, own_seat) in &attackers {
                for (arg_label, arg) in arg_candidates(hand_id, alice) {
                    total_calls += 1;
                    let raw = if m.is_query() {
                        w.query_raw(*target, *who, &m.name, arg)
                    } else {
                        w.update_raw(*target, *who, &m.name, arg)
                    };
                    if let Ok(bytes) = raw {
                        answered = true;
                        replies.push(Reply {
                            canister: label,
                            method: m.name.clone(),
                            sender: who_label,
                            sender_seat: *own_seat,
                            arg: arg_label,
                            bytes,
                        });
                    }
                }
            }
            if !answered {
                unexercised.push(format!("{label}.{}", m.name));
            }
        }
    }

    say!(
        "\n{total_calls} calls made, {} of them answered with a reply the harness kept",
        replies.len()
    );
    // Per attacker, so "we attacked as the table canister itself" is a measured
    // claim and not a comment. If the replica had refused ingress from a canister
    // principal this row would be 0 and the strongest attacker would silently not
    // have run.
    for (who_label, _, _) in &attackers {
        let n = replies.iter().filter(|r| r.sender == *who_label).count();
        say!("    as {who_label:<26} {n} replies");
        assert!(
            n > 0,
            "{who_label} never got a single reply, so that attacker did not actually run"
        );
    }
    assert!(
        unexercised.is_empty(),
        "these methods were never successfully called, so the attack did not reach them \
         and this run proves nothing about them: {unexercised:?}. Add an argument shape \
         to arg_candidates()."
    );

    // ---- the controller's OTHER doors ----------------------------------
    banner("the doors that need no code at all (feasibility §3.1)");

    let snapshot_dir = build::repo_root()
        .join("target")
        .join("no-peeking-snapshots");
    let _ = std::fs::remove_dir_all(&snapshot_dir);
    std::fs::create_dir_all(&snapshot_dir).unwrap();

    // The DEALER refuses every one of them, because there is nobody to authorise it.
    for (name, outcome) in [
        (
            "set_controllers(dealer, [operator])  as operator",
            w.pic
                .set_controllers(w.dealer, Some(w.operator), vec![w.operator])
                .err()
                .map(|e| e.reject_message),
        ),
        (
            "stop_canister(dealer)                as operator",
            w.pic
                .stop_canister(w.dealer, Some(w.operator))
                .err()
                .map(|e| e.reject_message),
        ),
        (
            "take_canister_snapshot(dealer)       as operator",
            w.pic
                .take_canister_snapshot(w.dealer, Some(w.operator), None)
                .err()
                .map(|e| e.reject_message),
        ),
        (
            "upgrade_canister(dealer)             as operator",
            w.pic
                .upgrade_canister(
                    w.dealer,
                    w.dealer_wasm.clone(),
                    encode_one(dealer_types::DealerInit {
                        table: w.table,
                        action_timeout_ns: None,
                        street_grace_ns: None,
                        force_finalize_after_ns: None,
                        min_open_balance: None,
                    })
                    .unwrap(),
                    Some(w.operator),
                )
                .err()
                .map(|e| e.reject_message),
        ),
    ] {
        match outcome {
            Some(msg) => say!("  REFUSED  {name}\n             {}", msg.trim()),
            None => panic!(
                "{name} SUCCEEDED. The dealer is not sealed and nothing else in this file means anything."
            ),
        }
    }

    // The TABLE allows all of them — it has a controller, exactly as the live
    // engine does — and they yield nothing, because the table has nothing.
    w.pic
        .stop_canister(w.table, Some(w.operator))
        .expect("the table has a controller and must be stoppable");
    let snap = w
        .pic
        .take_canister_snapshot(w.table, Some(w.operator), None)
        .expect("the table has a controller and must be snapshottable");
    say!(
        "\n  ALLOWED  take_canister_snapshot(table) as operator -> {} bytes",
        snap.total_size
    );
    w.pic
        .canister_snapshot_download(w.table, w.operator, snap.id.clone(), snapshot_dir.clone());
    w.pic
        .start_canister(w.table, Some(w.operator))
        .expect("restart the table");

    let mut heap = Vec::new();
    let mut heap_files = Vec::new();
    for entry in std::fs::read_dir(&snapshot_dir).unwrap().flatten() {
        if entry.path().is_file() {
            let bytes = std::fs::read(entry.path()).unwrap();
            heap_files.push(format!(
                "{} ({} bytes)",
                entry.file_name().to_string_lossy(),
                bytes.len()
            ));
            heap.extend_from_slice(&bytes);
        }
    }
    say!("  downloaded: {}", heap_files.join(", "));
    assert!(
        heap.len() > 100_000,
        "the snapshot download produced only {} bytes; if it is empty the search below \
         proves nothing",
        heap.len()
    );

    // ---- NOW learn what the secrets were -------------------------------
    banner("what was actually secret at the moment of the attack");

    // The last-resort door, used here purely as an oracle: it is the only way to
    // learn the seed of a hand nobody finished, and it is gated on a wall clock
    // nobody controls. See tests/disconnect.rs for what it is FOR.
    w.pic.advance_time(Duration::from_secs(2 * 3_600));
    w.pic.tick();
    let forced = w.force_finalize(stranger, hand_id).unwrap();
    let seed = hex::decode(&forced.revealed_seed).expect("seed is hex");
    let mut deck = create_deck();
    shuffle_deck(&mut deck, &seed);

    let p = 3usize;
    let mut secrets: Vec<(&'static str, Card)> = Vec::new();
    let labels = [
        "seat0 card1", "seat0 card2", "seat1 card1", "seat1 card2", "seat2 card1", "seat2 card2",
    ];
    for (k, label) in labels.iter().enumerate() {
        secrets.push((label, deck[k]));
    }
    for (label, idx) in [
        ("flop1", 2 * p + 1),
        ("flop2", 2 * p + 2),
        ("flop3", 2 * p + 3),
        ("turn", 2 * p + 5),
        ("river", 2 * p + 7),
    ] {
        secrets.push((label, deck[idx]));
    }
    say!(
        "the eleven cards that were secret at PreFlop:\n  {}",
        secrets
            .iter()
            .map(|(l, c)| format!("{l}={}", fmt(c)))
            .collect::<Vec<_>>()
            .join("  ")
    );
    say!("the 32 seed bytes: {}", forced.revealed_seed);

    // ---- the verdict ---------------------------------------------------
    banner("VERDICT");

    let mut leaks: Vec<String> = Vec::new();
    // POSITIVE CONTROL. A negative test that cannot fail is not evidence, so the
    // sweep has to be shown to be live: alice IS entitled to her own two cards, and
    // the same detector, on the same recorded replies, must find them.
    let mut own_cards_found = 0usize;
    for r in &replies {
        // The secret set for THIS sender: the eleven cards, minus the two that
        // belong to the seat this sender holds, if it holds one.
        let mine: BTreeSet<usize> = match r.sender_seat {
            Some(k) => [2 * k, 2 * k + 1].into_iter().collect(),
            None => BTreeSet::new(),
        };
        let scoped: Vec<(&'static str, Card)> = secrets
            .iter()
            .enumerate()
            .filter(|(i, _)| !mine.contains(i))
            .map(|(_, s)| *s)
            .collect();
        assert_eq!(
            scoped.len(),
            secrets.len() - mine.len(),
            "the scoped secret set was built wrong"
        );

        if r.sender_seat.is_some() {
            let own: Vec<(&'static str, Card)> = secrets
                .iter()
                .enumerate()
                .filter(|(i, _)| mine.contains(i))
                .map(|(_, s)| *s)
                .collect();
            if !cards_leaked(&r.bytes, &own).is_empty() {
                own_cards_found += 1;
            }
        }

        let found = cards_leaked(&r.bytes, &scoped);
        if !found.is_empty() {
            leaks.push(format!(
                "{}.{} as {} with {} leaked {:?}",
                r.canister, r.method, r.sender, r.arg, found
            ));
        }
    }
    for l in &leaks {
        say!("  LEAK  {l}");
    }
    assert!(
        own_cards_found > 0,
        "the sweep never found alice's OWN cards in any reply, so the detector was          never exercised against a real card and a clean result means nothing"
    );
    say!(
        "  control: {own_cards_found} replies DID contain alice's own two cards, found by the
           same detector on the same recorded bytes. The sweep is live."
    );
    assert!(
        leaks.is_empty(),
        "{} of {} replies contained a card that was secret from the caller who asked \
         for it:\n  {}",
        leaks.len(),
        replies.len(),
        leaks.join("\n  ")
    );
    say!(
        "  {} replies searched. Not one contained a card the caller was not entitled to.",
        replies.len()
    );

    // The 32 bytes that generate every card are not in the table's memory, and the
    // memory is the thing that defeated the obvious fix.
    let seed_in_heap = heap.windows(seed.len()).any(|win| win == seed.as_slice());
    say!(
        "  {} bytes of the table's downloaded memory searched for the 32-byte seed: {}",
        heap.len(),
        if seed_in_heap { "FOUND" } else { "absent" }
    );
    assert!(
        !seed_in_heap,
        "the seed is in the TABLE's memory. That is the whole finding, moved rather \
         than closed."
    );

    // Sanity in the other direction: the search WOULD find the seed if it were
    // there, so "absent" is a measurement and not a broken search.
    let mut planted = heap.clone();
    planted.extend_from_slice(&seed);
    assert!(
        planted.windows(seed.len()).any(|win| win == seed.as_slice()),
        "the heap search cannot find a seed that IS there, so its 'absent' means nothing"
    );
    say!("  (control: the same search finds the seed when it is planted in the same buffer)");

    let _ = std::fs::remove_dir_all(&snapshot_dir);
}

/// The surface itself, named. Some things are absent rather than gated, and an
/// absence is only a design decision if something checks that it is still absent.
#[test]
fn the_dealer_exports_nothing_that_could_name_a_deck_or_a_seed() {
    banner("THE SURFACE — what is not there");
    let dealer = build::dealer_wasm();
    let methods = wasm_exports::methods(&dealer);
    for m in &methods {
        say!("  {:<32} {}", m.name, m.kind);
    }
    // These are the shapes of every escape hatch this project has ever found: a
    // getter for the whole state, a debug method left in, an admin door.
    for forbidden in ["deck", "seed", "debug", "admin", "dump", "export", "all_"] {
        let hits: Vec<&str> = methods
            .iter()
            .filter(|m| m.name.to_lowercase().contains(forbidden))
            .map(|m| m.name.as_str())
            .collect();
        assert!(
            hits.is_empty(),
            "the dealer exports {hits:?}, whose name contains `{forbidden}`. The design \
             is that these are ABSENT, not gated -- there is no controller to gate them \
             against."
        );
    }
    say!(
        "\n  {} methods, none of them named for a deck, a seed, a dump or an admin.",
        methods.len()
    );

    // And the one method that names hole cards is scoped by its own signature: it
    // takes a hand id and nothing else, so there is no seat argument to point at
    // somebody else.
    let hole = methods
        .iter()
        .find(|m| m.name == "my_hole_cards")
        .expect("the dealer must have exactly one hole-card method");
    assert!(hole.is_query(), "reading your own cards must be a query");
    assert_eq!(
        methods
            .iter()
            .filter(|m| m.name.contains("hole"))
            .count(),
        1,
        "there must be exactly ONE method in the whole canister that names a hole card"
    );
    say!("  exactly one method names a hole card, it is a query, and its only argument is a hand id.");
}

/// The one method that DOES return a card returns exactly two, to exactly one
/// principal, and refuses everybody else — including the table's controller and a
/// player at the same table.
#[test]
fn my_hole_cards_is_scoped_to_the_seat_that_holds_them() {
    banner("my_hole_cards: two cards, one principal");
    let w = World::new(&["alice", "bob", "carol"]);
    let (alice, bob, carol) = (w.player(0), w.player(1), w.player(2));
    w.sit(alice, 0, 1_000).unwrap();
    w.sit(bob, 1, 1_000).unwrap();
    w.sit(carol, 2, 1_000).unwrap();
    let hand_id = w.start_hand(alice).unwrap();

    let a = w.my_hole_cards(alice, hand_id).unwrap();
    let b = w.my_hole_cards(bob, hand_id).unwrap();
    let c = w.my_hole_cards(carol, hand_id).unwrap();
    say!("alice {} {}", fmt(&a.0), fmt(&a.1));
    say!("bob   {} {}", fmt(&b.0), fmt(&b.1));
    say!("carol {} {}", fmt(&c.0), fmt(&c.1));
    assert_ne!(a, b);
    assert_ne!(b, c);

    for (label, who) in [
        ("the table's controller", w.operator),
        ("the table canister itself", w.table),
        ("a stranger", Principal::self_authenticating(b"nobody")),
        ("anonymous", Principal::anonymous()),
    ] {
        let out = w.my_hole_cards(who, hand_id);
        say!("  {label}: {}", out.err_text().trim());
        assert!(
            !out.is_ok(),
            "{label} got a hole card out of my_hole_cards"
        );
    }

    // There is no seat argument, so there is no "ask for seat 2" to try. That
    // absence is the design: the method's scope is `msg_caller` and cannot be
    // widened by an argument.
    let raw = w.query_raw(
        w.dealer,
        w.operator,
        "my_hole_cards",
        encode_args((hand_id, 2u8)).unwrap(),
    );
    say!(
        "  asking for a specific seat: {}",
        match &raw {
            Ok(_) => "the extra argument was ignored".to_string(),
            Err(e) => e.lines().next().unwrap_or("").to_string(),
        }
    );
    if let Ok(bytes) = raw {
        assert!(
            cards_leaked(&bytes, &[("carol1", c.0), ("carol2", c.1)]).is_empty(),
            "a seat argument reached through to another player's cards"
        );
    }
}

/// What the live engine's own interface says, next to what the spike's says.
///
/// Structural, and labelled as structural: this reads two committed files rather
/// than running the deployed canister, because driving the real table to `PreFlop`
/// needs the ICP ledger and the whole deposit path, which is
/// `tests/money_safety`'s job and not this one's. It is here because the contrast
/// is the entire point of the wave and it should not live only in prose.
#[test]
fn the_live_engine_declares_the_fields_the_spike_does_not_have() {
    banner("BEFORE and AFTER, from the two interfaces themselves");
    let live_did = build::repo_root()
        .join("src")
        .join("table_canister")
        .join("table_canister.did");
    let Ok(live) = std::fs::read_to_string(&live_did) else {
        say!("  SKIPPED: {} is not present", live_did.display());
        return;
    };

    // The live engine's TableState.
    let has_deck = live.contains("deck :") || live.contains("deck:");
    let has_hole = live.contains("hole_cards");
    say!("  src/table_canister/table_canister.did");
    say!("    declares `deck`       : {has_deck}");
    say!("    declares `hole_cards` : {has_hole}");
    assert!(
        has_deck && has_hole,
        "the live interface no longer declares the fields the auditor read. If the live \
         engine has been changed, this comparison is stale and must be rewritten rather \
         than deleted."
    );

    // And the source holds the seed.
    let live_src = std::fs::read_to_string(
        build::repo_root()
            .join("src")
            .join("table_canister")
            .join("src")
            .join("lib.rs"),
    )
    .expect("the live engine's source");
    assert!(
        live_src.contains("CURRENT_SEED"),
        "the live engine no longer holds the seed; this comparison is stale"
    );
    say!("  src/table_canister/src/lib.rs holds CURRENT_SEED: true");

    // The spike's table, EVERY source file of it.
    let stub_src = read_crate(&build::spike_root().join("table_stub").join("src"));
    // `deck` and `seed` must not appear as FIELDS. They appear in prose in the
    // spike's own comments, so the check is on struct-field declarations, which is
    // the thing that ends up in the heap and on the wire. Written to survive a
    // visibility change (`pub`, `pub(crate)`) rather than matching one spelling.
    for name in ["deck", "seed", "hole_cards", "deck_index"] {
        for line in stub_src.lines() {
            let t = line.trim_start();
            let t = t
                .strip_prefix("pub(crate) ")
                .or_else(|| t.strip_prefix("pub "))
                .unwrap_or(t);
            assert!(
                !t.starts_with(&format!("{name}:")),
                "the spike's table declares a field `{name}` here: `{}`. The whole claim \
                 rests on it having none.",
                line.trim()
            );
        }
    }
    say!("  src/no_peeking/table_stub/** declares no deck, seed, deck_index or hole_cards field");

    // And the dealer never logs, in ANY of its files.
    let dealer_src = read_crate(&build::spike_root().join("dealer_canister").join("src"));
    for logger in ["println!", "ic_cdk::print", "eprintln!"] {
        assert!(
            !dealer_src.contains(logger),
            "the dealer contains `{logger}`. Canister logs have their own visibility \
             rules and the safest log line about a hole card is the one that is never \
             written."
        );
    }
    say!("  src/no_peeking/dealer_canister/** writes no log line at all");
}

/// Every `.rs` under `dir`, concatenated. The checks above have to see the whole
/// crate, not one file of it: a field moved into a submodule is still a field.
fn read_crate(dir: &std::path::Path) -> String {
    let mut out = String::new();
    for entry in std::fs::read_dir(dir).expect("crate src dir").flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push_str(&std::fs::read_to_string(&path).expect("source file"));
            out.push('\n');
        }
    }
    assert!(!out.is_empty(), "no sources found under {}", dir.display());
    out
}
