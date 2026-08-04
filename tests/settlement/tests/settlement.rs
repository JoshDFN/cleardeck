//! The comparison harness: drive the REAL table canister through complete hands
//! and compare the actual per-seat chip deltas against the oracle.
//!
//! Every test here installs a table canister wasm that this run built from the
//! checked-out source, checked for freshness against every file that feeds it, and
//! whose sha256 is asserted to equal the module hash the replica reports. See
//! `src/wasms.rs` for why that is three checks and not one.
//!
//! Run it with:
//! ```text
//!   cd tests/settlement && cargo test --test settlement -- --test-threads=1 --nocapture
//! ```
//! `--test-threads=1` because each test owns a PocketIC instance, i.e. a replica
//! process.

use settlement_oracle::table_api::*;
use settlement_oracle::{assert_all_agree, drive, suite, Bench, World};

// ===========================================================================
// smoke: the machinery itself
// ===========================================================================

/// Before any finding is believed, the plumbing has to be shown to work: exact
/// stacks are settable, the deck prediction matches the board the engine deals,
/// and a snapshot restores the position exactly.
#[test]
fn the_harness_measures_what_it_claims_to_measure() {
    let mut bench = Bench::ladder();
    println!(
        "table_canister wasm under test: {}",
        bench.world.table_wasm_sha256
    );

    let before: Vec<(u8, u64)> = bench
        .world
        .table_state()
        .seated()
        .map(|p| (p.seat, p.chips))
        .collect();
    assert_eq!(
        before,
        vec![(0, 20), (1, 40), (2, 80), (3, 160), (4, 400)],
        "exact stacks must be settable, or no all-in ladder is deliberate"
    );

    // One hand, everybody shoves. `HandComparison::build` asserts the predicted
    // board equals the dealt board, so this also proves src/deal.rs.
    let cmp = bench
        .hand_with_policy("smoke_all_in", drive::everyone_all_in())
        .expect("an untargeted hand always runs");
    println!("{}", cmp.report());
    assert_eq!(
        cmp.record.board.len(),
        5,
        "an all-in hand must run the board out"
    );
    assert!(
        cmp.record.predicted_board.is_some(),
        "the deal must have been read"
    );

    // The snapshot must put the stacks back.
    bench.rewind();
    let after_rewind: Vec<(u8, u64)> = bench
        .world
        .table_state()
        .seated()
        .map(|p| (p.seat, p.chips))
        .collect();
    assert_eq!(
        before, after_rewind,
        "rewind must restore the exact position, or every scenario runs against a \
         different table than the one it was written for"
    );
}

// ===========================================================================
// the deliberate scenarios
// ===========================================================================

/// The main event. Every class the task names, reached on purpose, with the
/// coverage counts printed for a reader.
#[test]
fn the_engine_settles_every_class_of_hand_the_way_the_rules_of_poker_do() {
    let mut bench = Bench::ladder();
    println!(
        "\ntable_canister wasm under test: {}\n",
        bench.world.table_wasm_sha256
    );

    suite::run_all(&mut bench);

    println!("{}", bench.coverage.report());
    println!(
        "rewind-and-re-deal attempts spent searching for named deals: {}",
        bench.deal_attempts
    );
    if !bench.unhit_targets.is_empty() {
        println!("targets NOT reached within their attempt budget:");
        for (label, attempts) in &bench.unhit_targets {
            println!("  {label} (after {attempts} deals)");
        }
    }

    println!("\n=== DISAGREEMENTS ({}) ===", bench.disagreements.len());
    for c in &bench.disagreements {
        println!("{}", c.report());
    }
    println!("=== HANDS THE ENGINE SETTLED CORRECTLY ===");
    for c in bench.comparisons.iter().filter(|c| c.agrees) {
        println!("  ok  {}", c.label);
    }

    // These classes are the reason the crate exists. A zero here means the harness
    // is not measuring what it claims, and that must fail loudly rather than be
    // reported as a clean run.
    let must_reach = [
        "3+ players to showdown",
        "4+ players to showdown",
        "2 simultaneous all-ins",
        "3 simultaneous all-ins",
        "4+ simultaneous all-ins",
        "all-ins at 2+ different depths",
        "all-ins at 3+ different depths",
        "2+ pot layers (a real side pot)",
        "3+ pot layers",
        "exact tie (a pot chopped)",
        "3-way chop",
        "odd chip had to be placed",
        "uncalled bet returned",
        "folded money above a short all-in",
        "money wagered after the flop",
        "seat vacated mid-hand",
        "fold-out, no showdown",
    ];
    let missing: Vec<&str> = must_reach
        .iter()
        .copied()
        .filter(|c| bench.coverage.per_class.get(c).copied().unwrap_or(0) == 0)
        .collect();
    assert!(
        missing.is_empty(),
        "the harness never reached these classes, so it cannot claim to measure them: {missing:?}"
    );

    // THE ACCEPTANCE CRITERION. Every hand -- every class above, reached on purpose
    // -- must be settled exactly the way the rules of poker say, seat by seat.
    //
    // This assertion was inverted when E-01, E-03, E-05 and the odd-chip rule were
    // fixed in wave 2. Before that it read `disagreements > 0`, because the
    // instrument was expected to be loud and the value of it was that it REPORTED
    // the defects rather than agreeing with them. It is now quiet, and quiet is what
    // has to be enforced: `Bench::gate` fails each hand where it runs, and this
    // states the same thing over the whole suite so a reader sees one number.
    assert_all_agree(&bench.comparisons, "the deliberate suite");
    assert_eq!(
        bench.coverage.disagreements, 0,
        "the oracle disagreed with {} of {} hands. Every one of them is printed above \
         with the cards, the money and the per-seat delta.",
        bench.coverage.disagreements,
        bench.comparisons.len()
    );
    // Not one chip may be collected and paid to nobody, and not one may reach a seat
    // that was not owed it. These are the two failure modes E-01 and E-05 were.
    assert_eq!(
        bench.coverage.destroyed_total, 0,
        "chips were collected and paid to nobody"
    );
    assert_eq!(
        bench.coverage.misdirected_total, 0,
        "chips reached a seat that was not owed them"
    );
    assert_eq!(
        bench.coverage.input_suspect, 0,
        "the engine's two accounts of the pot disagreed, so the oracle was measuring \
         against incomplete input"
    );
    // And the instrument is still connected: it must have measured real money.
    assert!(
        bench.comparisons.iter().any(|c| c.facts.total_collected() > 0),
        "no hand collected anything, so nothing was actually measured"
    );
}

/// Same shapes at real ICP magnitudes, to show nothing depends on the one-e8s chip.
#[test]
fn the_findings_are_not_an_artefact_of_the_micro_chip_unit() {
    let mut bench = Bench::new(
        TableConfig::realistic_six_max(),
        &[
            ("alice", 0, 200_000_000),
            ("bob", 1, 400_000_000),
            ("carol", 2, 700_000_000),
            ("dave", 3, 1_000_000_000),
        ],
        3_000_000_000,
    );
    bench.hand_with_policy(
        "realistic_four_way_all_in",
        drive::all_in_only(vec![0, 1, 2, 3]),
    );
    bench.hand_with_policy(
        "realistic_post_flop_betting",
        drive::bet_every_street(20_000_000),
    );
    println!("{}", bench.coverage.report());
    for c in &bench.disagreements {
        println!("{}", c.report());
    }
    assert_eq!(
        bench.coverage.hands, 2,
        "both realistic-scale hands must have settled"
    );
    // The post-flop-betting hand used to destroy every e8 wagered after the flop at
    // these magnitudes too, and the four-way all-in used to misplace odd chips. Both
    // must now be exact: the payout arithmetic is integer-only, so nothing about it
    // may depend on the size of a chip.
    assert_all_agree(&bench.comparisons, "at real ICP magnitudes");
    let post_flop = bench
        .comparisons
        .iter()
        .find(|c| c.label == "realistic_post_flop_betting")
        .expect("the post-flop hand ran");
    assert_eq!(
        post_flop.destroyed, 0,
        "this shape destroyed the whole post-flop pot before the E-01 fix"
    );
    println!(
        "at ICP magnitudes: {} e8s collected, {} destroyed, {} misdirected",
        post_flop.facts.total_collected(),
        post_flop.destroyed,
        post_flop.misdirected()
    );
}

// ===========================================================================
// a randomised sweep, to catch what the deliberate scenarios did not think of
// ===========================================================================

#[test]
fn randomised_hands_settle_the_way_the_rules_of_poker_do() {
    let hands: usize = std::env::var("SETTLEMENT_RANDOM_HANDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);

    let mut bench = Bench::ladder();
    for i in 0..hands {
        // Vary the betting shape deterministically, so the sweep is reproducible.
        let label = format!("random_{i:03}");
        let policy = match i % 4 {
            0 => drive::everyone_all_in(),
            1 => drive::passive(),
            2 => drive::bet_every_street(6),
            _ => drive::all_in_only(vec![1, 3]),
        };
        bench.hand_with_policy(&label, policy);
        if i % 6 == 5 {
            // Move the button, so position-dependent rules are not only ever tested
            // from one button seat.
            bench.advance_button();
        }
    }
    println!("{}", bench.coverage.report());
    println!("\n=== DISAGREEMENTS ({}) ===", bench.disagreements.len());
    for c in &bench.disagreements {
        println!("{}", c.report());
    }
    assert_eq!(
        bench.coverage.hands, hands,
        "every hand in the sweep must have settled"
    );
    // `Bench::gate` already failed at the first disagreeing hand. This states the
    // property over the whole sweep, because the critic of the first version of this
    // crate was right that a suite which only RECORDS disagreements lets a
    // conserving wrong-seat payout through.
    assert_all_agree(&bench.comparisons, "the randomised sweep");
    assert_eq!(bench.coverage.destroyed_total, 0);
    assert_eq!(bench.coverage.misdirected_total, 0);
}

// ===========================================================================
// the wrong-seat probe
// ===========================================================================

/// Eight multi-way all-in showdowns with real side-pot ladders, and NO post-flop
/// betting, so the engine's own E-01 defect is not in play and every chip the
/// canister collects is paid out.
///
/// In the tree as it stands this test is a CONTROL: the engine settles all of these
/// correctly (bar the one-chip odd-chip rule, which is allowed for here), so a run
/// that reports a large disagreement means a payout change has gone wrong.
///
/// With `$SETTLEMENT_EXPECT_WRONG_SEAT=1` the polarity flips: the run is expected to
/// find a large disagreement. That is how the planted-bug proof is executed. Copy the
/// repo into a scratch directory, plant a payout bug that conserves every chip, and
/// run:
///
/// ```text
///   SETTLEMENT_EXPECT_WRONG_SEAT=1 cargo test --test settlement -- \
///       the_oracle_convicts_a_conserving_wrong_seat_payout --nocapture --test-threads=1
/// ```
///
/// See README.md for the two plants that were run and what the oracle said.
#[test]
fn the_oracle_convicts_a_conserving_wrong_seat_payout() {
    let expect_wrong = std::env::var("SETTLEMENT_EXPECT_WRONG_SEAT").as_deref() == Ok("1");

    let mut bench = Bench::ladder();
    // With a bug deliberately planted, the disagreements are the POINT, so the
    // per-hand gate has to record them instead of failing at the first one.
    bench.record_disagreements_without_failing = expect_wrong;
    println!(
        "\ntable_canister wasm under test: {}\n",
        bench.world.table_wasm_sha256
    );
    suite::run_wrong_seat_probe(&mut bench);
    println!("{}", bench.coverage.report());

    // Every probe hand must CONSERVE. That is the whole premise: the defect being
    // hunted is invisible to conservation, so if conservation breaks, the plant was
    // the wrong kind of plant and the proof would be about the wrong thing.
    for c in &bench.comparisons {
        assert_eq!(
            c.destroyed, 0,
            "probe scenario `{}` destroyed {} chips. These scenarios have no post-flop \
             betting precisely so that they conserve; a non-zero figure here means the \
             hand is not the control it is meant to be.\n{}",
            c.label,
            c.destroyed,
            c.report()
        );
    }

    // The one-chip odd-chip rule difference (D-04) is present in the clean tree and
    // can show up here, so the polarity check looks for a difference LARGER than one
    // chip rather than any difference at all.
    let big: Vec<_> = bench
        .comparisons
        .iter()
        .filter(|c| c.worst_diff() > 1)
        .collect();
    for c in &big {
        println!("{}", c.report());
    }

    if expect_wrong {
        assert!(
            !big.is_empty(),
            "SETTLEMENT_EXPECT_WRONG_SEAT=1 but the oracle agreed with every one of the \
             {} probe hands. Either the plant is not on the settlement path, or the \
             oracle cannot see a wrong-seat payout -- and the second would make this \
             whole crate worthless.",
            bench.comparisons.len()
        );
        let worst = big.iter().map(|c| c.worst_diff()).max().unwrap_or(0);
        println!(
            "CONVICTED: {} of {} conserving hands were paid to the wrong seat; \
             largest per-seat error {worst} chips",
            big.len(),
            bench.comparisons.len()
        );
    } else {
        assert!(
            big.is_empty(),
            "{} of {} all-in showdowns were settled more than one chip away from the \
             rules of poker. In the tree as reviewed these hands are correct, so this is \
             a REGRESSION in the payout path.",
            big.len(),
            bench.comparisons.len()
        );
    }
}

// ===========================================================================
// what the instrument must be able to see
// ===========================================================================

/// The comparator must notice money going to the wrong seat even when nothing is
/// created or destroyed. Proven here on a synthetic record so the claim does not
/// depend on any engine defect existing; the engine-side proof is the planted-bug
/// run recorded in README.md.
#[test]
fn the_comparator_convicts_a_wrong_seat_payout_that_conserves_totals() {
    use settlement_oracle::cards::{board, hole};
    use settlement_oracle::oracle::{settle, HandFacts, SeatStake};

    let facts = HandFacts {
        num_seats: 2,
        dealer_seat: 1,
        board: board("Ks Kd 7h 3c 2s"),
        stakes: vec![
            SeatStake::contender(0, 100, hole("Ah 7c")), // two pair
            SeatStake::contender(1, 100, hole("Kh 4d")), // trip kings
        ],
    };
    let s = settle(&facts);
    let owed = s.net_deltas(&facts);
    assert_eq!(owed[&0], -100);
    assert_eq!(owed[&1], 100);

    // A payout that hands the pot to the WRONG seat. Totals conserve perfectly.
    let wrong: std::collections::BTreeMap<u8, i128> =
        [(0u8, 100i128), (1u8, -100i128)].into_iter().collect();
    assert_eq!(
        wrong.values().sum::<i128>(),
        0,
        "the wrong payout conserves every chip"
    );

    let diffs: Vec<i128> = [0u8, 1u8].iter().map(|s| wrong[s] - owed[s]).collect();
    assert_eq!(diffs, vec![200, -200]);
}

/// A run against a stale wasm must be impossible. The guard trap 1 demands,
/// exercised.
#[test]
fn a_stale_wasm_is_refused() {
    use settlement_oracle::wasms;
    let dir = std::env::temp_dir().join("settlement-oracle-staleness-check");
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("ancient.wasm");
    std::fs::write(&fake, b"\0asm").unwrap();
    // Backdate it well before any source file in the tree.
    let ancient = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1);
    let times = std::fs::FileTimes::new().set_modified(ancient);
    std::fs::File::options()
        .write(true)
        .open(&fake)
        .unwrap()
        .set_times(times)
        .unwrap();

    let refused = std::panic::catch_unwind(|| wasms::assert_fresh(&fake)).is_err();
    assert!(
        refused,
        "assert_fresh accepted a wasm dated 1970. The freshness guard is not working, \
         which is exactly how an earlier harness reported 10/10 green while a \
         fund-theft bug sat in lib.rs."
    );
    let _ = std::fs::remove_file(&fake);
}

/// The sha256 the harness prints is the module hash the replica reports for the
/// installed canister. `World::new` asserts it; this states it as its own claim.
#[test]
fn the_installed_module_is_the_one_the_harness_built() {
    let world = World::new(TableConfig::micro_six_max(), &["alice", "bob"]);
    let status = world
        .pic
        .canister_status(world.table, Some(world.controller))
        .expect("canister_status");
    assert_eq!(
        status.module_hash.map(hex::encode),
        Some(world.table_wasm_sha256.clone())
    );
    println!(
        "installed module hash == harness build: {}",
        world.table_wasm_sha256
    );
}
