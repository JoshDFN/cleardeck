//! The CI subset: a few seconds, fixed seeds, plus one explicit regression case
//! for every minimal repro the full run turned up.
//!
//! The exhaustive `C(52,5)` sweep and the multi-million seven-card sweep stay a
//! manual `cargo run --release`. What runs here is:
//!
//! * the card conversions into all three card representations, proven bijective;
//! * the explicit cross-library category mapping, one known hand per category;
//! * a seeded sample of five-card hands through the *same* sweep code the
//!   exhaustive run uses, asserted to produce zero disagreements;
//! * a seeded sample of seven-card hands through `evaluate_hand`, likewise;
//! * the literal `sign(our_cmp) == sign(ref_cmp)` pair check, both draw modes;
//! * one `characterises_*` test per known defect, pinning TODAY'S answer so the
//!   behaviour cannot change silently, paired with an `#[ignore]`d
//!   `defect_*_is_fixed` test spelling out the answer the fix wave owes.
//!
//! # Reading the `characterises_*` tests
//!
//! They are NOT a specification. Each one asserts an answer the harness has
//! already shown to be WRONG, and carries a `FIX WAVE:` note saying what it must
//! become. Never copy an expectation out of one of them.

use cleardeck_differential::cards::{
    face_value, from_cleardeck, from_str, parse_hand, rank_index, suit_index, to_cleardeck,
    to_poker_crate, to_rs_poker, CardIdx, NUM_CARDS,
};
use cleardeck_differential::category::{
    category_of_cleardeck, category_of_poker_crate, category_of_rs_poker, Category,
};
use cleardeck_differential::checks::{degenerate, exhaustive, pairs, sevens, sevens_exhaustive};
use cleardeck_differential::engine::{ours_cmp, ours_five, ours_five_cards_raw, ours_hand, ours_seven};
use cleardeck_differential::oracles::poker_oracle::PokerCrateOracle;
use cleardeck_differential::oracles::rs_poker_oracle::RsPokerOracle;
use cleardeck_differential::oracles::Oracle;
use cleardeck_differential::report::Severity;
use cleardeck_differential::{SEED_FAST_FIVE, SEED_PAIRS, SEED_SEVENS};
use poker_core::{detect_straight, HandRank};

/// Sizes tuned so the whole file finishes in a few seconds in a DEBUG build,
/// which is what `cargo test` gives CI by default.
const FAST_FIVE_CARD_HANDS: usize = 150_000;
const FAST_SEVEN_CARD_HANDS: u64 = 15_000;
const FAST_PAIRS_PER_MODE: u64 = 10_000;

fn five(s: &str) -> [CardIdx; 5] {
    let v = parse_hand(s).expect("literal cards");
    assert_eq!(v.len(), 5, "{s:?} is not five cards");
    [v[0], v[1], v[2], v[3], v[4]]
}

fn seven(s: &str) -> [CardIdx; 7] {
    let v = parse_hand(s).expect("literal cards");
    assert_eq!(v.len(), 7, "{s:?} is not seven cards");
    [v[0], v[1], v[2], v[3], v[4], v[5], v[6]]
}

// ---------------------------------------------------------------------------
// Representation: nothing downstream means anything if the cards are wrong
// ---------------------------------------------------------------------------

#[test]
fn card_index_round_trips_through_cleardeck() {
    for c in 0..NUM_CARDS as CardIdx {
        assert_eq!(from_cleardeck(&to_cleardeck(c)), c, "card {c}");
        assert_eq!(to_cleardeck(c).rank.value(), face_value(c));
    }
}

#[test]
fn card_index_round_trips_through_its_own_notation() {
    for c in 0..NUM_CARDS as CardIdx {
        let s = cleardeck_differential::cards::to_str(c);
        assert_eq!(from_str(&s).unwrap(), c, "{s}");
    }
}

#[test]
fn conversions_into_both_references_are_injective_and_rank_preserving() {
    let mut rs_seen = std::collections::BTreeSet::new();
    let mut pk_seen = std::collections::BTreeSet::new();
    for c in 0..NUM_CARDS as CardIdx {
        let rs = to_rs_poker(c);
        // rs_poker's `Value` is face value minus two, same as our rank index.
        assert_eq!(rs.value as u8, rank_index(c), "rs_poker rank for card {c}");
        assert!(rs_seen.insert(u8::from(rs)), "rs_poker collision on card {c}");

        let pk = to_poker_crate(c);
        assert_eq!(
            cleardeck_differential::cards::poker_rank_face_value(pk.rank()),
            face_value(c),
            "poker rank for card {c}"
        );
        assert!(
            pk_seen.insert(pk.unique_integer()),
            "poker collision on card {c}"
        );
    }
    assert_eq!(rs_seen.len(), NUM_CARDS);
    assert_eq!(pk_seen.len(), NUM_CARDS);
}

#[test]
fn suits_are_distinct_and_flush_relevant_only() {
    // Four cards of one rank must produce four distinct suits in every library.
    for ri in 0..13u8 {
        let mut cd = std::collections::BTreeSet::new();
        for si in 0..4u8 {
            let c = ri * 4 + si;
            assert_eq!(suit_index(c), si);
            cd.insert(format!("{:?}", to_cleardeck(c).suit));
        }
        assert_eq!(cd.len(), 4, "rank index {ri} did not yield 4 distinct suits");
    }
}

// ---------------------------------------------------------------------------
// The explicit category mapping, one known hand per category
// ---------------------------------------------------------------------------

#[test]
fn category_mapping_agrees_on_one_known_hand_per_category() {
    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();
    let cases: [(&str, Category); 10] = [
        ("Ac Kd Qh Js 9c", Category::HighCard),
        ("Ac Ad Qh Js 9c", Category::OnePair),
        ("Ac Ad Qh Qs 9c", Category::TwoPair),
        ("Ac Ad Ah Qs 9c", Category::ThreeOfAKind),
        ("9c 8d 7h 6s 5c", Category::Straight),
        ("Ac Qc 9c 6c 3c", Category::Flush),
        ("Ac Ad Ah Qs Qc", Category::FullHouse),
        ("Ac Ad Ah As Qc", Category::FourOfAKind),
        ("9c 8c 7c 6c 5c", Category::StraightFlush),
        // The ace-high straight flush: ClearDeck has its own variant for it,
        // both references call it the top straight flush.
        ("Ac Kc Qc Jc Tc", Category::StraightFlush),
    ];

    for (spec, expected) in cases {
        let hand = five(spec);
        let ours = category_of_cleardeck(&ours_five(&hand));
        let a = rs.eval5(&hand).category;
        let b = pk.eval5(&hand).category;
        assert_eq!(ours, expected, "ClearDeck category for {spec}");
        assert_eq!(a, expected, "rs_poker category for {spec}");
        assert_eq!(b, expected, "poker category for {spec}");
    }

    // The wheel is a straight, and the steel wheel a straight flush, in all three.
    let wheel = five("5h 4d 3c 2s Ah");
    assert_eq!(category_of_cleardeck(&ours_five(&wheel)), Category::Straight);
    assert_eq!(rs.eval5(&wheel).category, Category::Straight);
    assert_eq!(pk.eval5(&wheel).category, Category::Straight);
}

/// One hand per category, hitting all nine arms of each of the three mapping
/// functions. Guards against a future library version adding a variant that
/// silently falls into the wrong arm, and against a mapping arm being dead.
#[test]
fn every_category_variant_is_covered_by_the_mapping_functions() {
    use poker::Evaluator;
    use rs_poker::core::Rankable;

    let evaluator = Evaluator::new();
    let specs = [
        "Ac Kd Qh Js 9c",
        "Ac Ad Qh Js 9c",
        "Ac Ad Qh Qs 9c",
        "Ac Ad Ah Qs 9c",
        "9c 8d 7h 6s 5c",
        "Ac Qc 9c 6c 3c",
        "Ac Ad Ah Qs Qc",
        "Ac Ad Ah As Qc",
        "9c 8c 7c 6c 5c",
    ];
    let mut from_rs = std::collections::BTreeSet::new();
    let mut from_pk = std::collections::BTreeSet::new();
    let mut from_us = std::collections::BTreeSet::new();

    for spec in specs {
        let hand = five(spec);

        // Reference A, straight from the library.
        let rs_cards: Vec<rs_poker::core::Card> = hand.iter().map(|&c| to_rs_poker(c)).collect();
        from_rs.insert(category_of_rs_poker(rs_cards.rank().category()));

        // Reference B, straight from the library.
        let pk_cards: Vec<poker::Card> = hand.iter().map(|&c| to_poker_crate(c)).collect();
        let class = evaluator
            .evaluate_five(pk_cards.as_slice())
            .expect("five distinct cards")
            .classify();
        from_pk.insert(category_of_poker_crate(class));

        from_us.insert(category_of_cleardeck(&ours_five(&hand)));
    }

    assert_eq!(from_rs.len(), 9, "rs_poker mapping missed a category");
    assert_eq!(from_pk.len(), 9, "poker mapping missed a category");
    assert_eq!(from_us.len(), 9, "ClearDeck mapping missed a category");
    assert_eq!(from_rs, from_pk);
    assert_eq!(from_rs, from_us);
}

// ---------------------------------------------------------------------------
// Seeded samples through the real check code
// ---------------------------------------------------------------------------

#[test]
fn sampled_five_card_hands_agree_with_both_references() {
    let out = exhaustive::sweep_five(
        exhaustive::sampled_five_card_hands(SEED_FAST_FIVE, FAST_FIVE_CARD_HANDS),
        false,
    );
    let fund: Vec<_> = out
        .disagreements
        .iter()
        .filter(|d| d.severity == Severity::FundImpacting || d.severity == Severity::Cosmetic)
        .collect();
    assert!(
        fund.is_empty(),
        "five-card sample disagreed with a reference: {fund:#?}"
    );
    for o in &out.summary.ordering {
        assert_eq!(o.false_ties, 0, "false ties vs {}", o.reference);
        assert_eq!(o.false_splits, 0, "false splits vs {}", o.reference);
        assert_eq!(o.inversions, 0, "inversions vs {}", o.reference);
    }
    assert_eq!(out.summary.royal_flush_alias_mismatches, 0);
    assert_eq!(out.summary.detail_mismatches_vs_poker, 0);
    assert!(
        out.inconclusive.is_empty(),
        "references disagreed with each other: {:#?}",
        out.inconclusive
    );
}

#[test]
fn sampled_seven_card_hands_agree_with_both_references() {
    let out = sevens::run(SEED_SEVENS, FAST_SEVEN_CARD_HANDS);
    assert!(
        out.disagreements.is_empty(),
        "seven-card sample disagreed: {:#?}",
        out.disagreements
    );
    for o in &out.summary.ordering {
        assert_eq!(o.false_ties, 0, "false ties vs {}", o.reference);
        assert_eq!(o.false_splits, 0, "false splits vs {}", o.reference);
        assert_eq!(o.inversions, 0, "inversions vs {}", o.reference);
    }
    assert!(out.inconclusive.is_empty(), "{:#?}", out.inconclusive);
}

#[test]
fn pairwise_sign_agrees_with_both_references() {
    let out = pairs::run(SEED_PAIRS, FAST_PAIRS_PER_MODE);
    assert_eq!(out.summary.sign_mismatches_vs_rs_poker, 0, "{:#?}", out.disagreements);
    assert_eq!(out.summary.sign_mismatches_vs_poker, 0, "{:#?}", out.disagreements);
    assert_eq!(
        out.summary.reference_sign_disagreements, 0,
        "the two references disagreed with each other: {:#?}",
        out.inconclusive
    );
}

/// The degenerate probe set must report EXACTLY the defects that are still live --
/// no more (a new id means a real path regressed) and no fewer (a missing id means a
/// probe stopped probing, which is how a harness quietly becomes decoration).
///
/// Since the E-09 validation landed, each probe emits a finding only while the engine
/// still ACCEPTS the input it is testing, so a fixed leg makes its id DISAPPEAR. That
/// is what makes this an equality assertion and not a subset one.
#[test]
fn degenerate_probe_set_reports_exactly_the_defects_that_are_still_live() {
    let out = degenerate::run();
    let ids: Vec<&str> = out.findings.iter().map(|f| f.id.as_str()).collect();

    // Still live: detect_straight returns the WEAKEST straight when handed more than
    // five ranks. Unreachable from evaluate_hand today, so it is not a live money
    // bug, but it is not fixed either.
    const STILL_LIVE: &[&str] = &["degenerate/detect_straight-prefers-the-wheel"];

    // Fixed 2026-08-04 by the input-validation boundary in poker_core. If any of
    // these comes back, the validation regressed and duplicate or short-board inputs
    // are being ranked again.
    const FIXED: &[&str] = &[
        "degenerate/fewer-than-five-cards-returns-empty-highcard",
        "degenerate/evaluate_five_cards-fabricates-a-hand-from-more-than-five-cards",
        "degenerate/duplicate-cards-are-silently-ranked",
        "degenerate/duplicate-card-reaches-evaluate_hand",
    ];

    for id in FIXED {
        assert!(
            !ids.contains(id),
            "{id} is back. poker_core is ranking an input it had started refusing, so a \
             duplicate or short-board hand can decide a pot again (docs/DEFECTS.md E-09). \
             All findings: {ids:?}"
        );
    }
    for id in STILL_LIVE {
        assert!(
            ids.contains(id),
            "{id} produced NO finding. Either the defect was fixed -- in which case move it to \
             FIXED here and update docs/DEFECTS.md -- or the probe stopped probing. Findings: \
             {ids:?}"
        );
    }
    assert_eq!(
        ids, STILL_LIVE,
        "the degenerate probe set reported something unexpected. Findings: {ids:?}"
    );

    // The flop (5-card) and turn (6-card) probes are legitimate boards and must NEVER
    // produce a finding.
    assert!(
        !ids.contains(&"degenerate/flop-only-five-cards"),
        "evaluate_hand broke on a 5-card board"
    );
    assert!(
        !ids.contains(&"degenerate/turn-only-six-cards"),
        "evaluate_hand broke on a 6-card board"
    );

    // And every finding must quote a MEASURED reference answer, not a claim about
    // one (docs/DEFECTS.md H-05). The probe module renders every verdict as
    // "rs_poker <verb> ...; poker 0.7.0 <verb> ...".
    for f in &out.findings {
        for says in &f.reference_says {
            assert!(
                says.contains("rs_poker") && says.contains("poker 0.7.0")
                    || says.starts_with("Some("),
                "finding {} carries a reference claim that names no reference call: {says:?}",
                f.id
            );
        }
    }
}

/// The `--exhaustive-sevens` path, over a slice of the seven-card space.
///
/// Ignored by default because building the class map means the full 2,598,960-hand
/// five-card sweep, which is ~4s in release and far too slow for a debug CI run.
/// Run it with `cargo test --release -- --ignored --nocapture`.
#[test]
#[ignore = "needs the full five-card sweep; run with `cargo test --release -- --ignored`"]
fn exhaustive_seven_card_slice_agrees_via_the_proven_class_map() {
    let five_out = exhaustive::sweep_five(exhaustive::all_five_card_hands(), true);
    let map = five_out.class_map.expect("full sweep yields a class map");
    assert!(
        map.trustworthy,
        "the five-card sweep was not clean, so the class map cannot be used as an oracle"
    );
    assert_eq!(map.rs.len(), 7462);
    assert_eq!(map.pk.len(), 7462);

    // Every seven-card hand whose two lowest cards are the two lowest in the deck:
    // C(50,5) = 2,118,760 hands.
    let slice = sevens_exhaustive::all_seven_card_hands()
        .take_while(|h| h[0] == 0 && h[1] == 1);
    let out = sevens_exhaustive::run_over(&map, slice, 2_118_760);
    assert_eq!(out.summary.hands_enumerated, 2_118_760);
    assert_eq!(out.summary.unmapped_hand_ranks, 0);
    assert_eq!(out.summary.strength_mismatches_vs_rs_poker, 0);
    assert_eq!(out.summary.strength_mismatches_vs_poker, 0);
    assert!(out.disagreements.is_empty(), "{:#?}", out.disagreements);
}

// ---------------------------------------------------------------------------
// Regression cases: the exact hands the full run reported
// ---------------------------------------------------------------------------

/// The one structural naming difference, pinned in both directions.
#[test]
fn regression_royal_flush_variant_is_exactly_the_ace_high_straight_flush() {
    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();
    for suit in ['c', 'd', 'h', 's'] {
        let spec = format!("A{suit} K{suit} Q{suit} J{suit} T{suit}");
        let hand = five(&spec);
        assert_eq!(ours_five(&hand), HandRank::RoyalFlush, "{spec}");
        assert!(RsPokerOracle::is_royal_flush(rs.eval5(&hand).strength), "{spec}");
        assert!(PokerCrateOracle::is_royal_flush(pk.eval5(&hand).strength), "{spec}");

        let king_high = five(&format!("K{suit} Q{suit} J{suit} T{suit} 9{suit}"));
        assert_eq!(ours_five(&king_high), HandRank::StraightFlush(13), "{spec}");
        assert!(!RsPokerOracle::is_royal_flush(rs.eval5(&king_high).strength));
        assert!(!PokerCrateOracle::is_royal_flush(pk.eval5(&king_high).strength));
    }
}

/// The full ladder, in ClearDeck's own `Ord`, must be strictly increasing and
/// must be strictly increasing in both references too.
#[test]
fn regression_category_ladder_is_strictly_increasing_everywhere() {
    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();
    let ladder = [
        "7c 5d 4h 3s 2c", // worst possible hand
        "Ac Kd Qh Js 9c",
        "2c 2d 3h 4s 5c",
        "Ac Ad Kh Qs Jc",
        "2c 2d 3h 3s 5c",
        "Ac Ad Kh Ks Qc",
        "2c 2d 2h 3s 5c",
        "Ac Ad Ah Ks Qc",
        "5h 4d 3c 2s Ah", // the wheel: weakest straight
        "Ac Kd Qh Js Tc",
        "7c 5c 4c 3c 2c",
        "Ac Kc Qc Jc 9c",
        "2c 2d 2h 3s 3c",
        "Ac Ad Ah Ks Kc",
        "2c 2d 2h 2s 3c",
        "Ac Ad Ah As Kc",
        "5c 4c 3c 2c Ac", // steel wheel: weakest straight flush
        "Kc Qc Jc Tc 9c",
        "Ac Kc Qc Jc Tc", // royal
    ];
    for w in ladder.windows(2) {
        let a = five(w[0]);
        let b = five(w[1]);
        assert!(
            ours_cmp(&ours_five(&a), &ours_five(&b)).is_lt(),
            "ClearDeck: {} should be weaker than {}",
            w[0],
            w[1]
        );
        assert!(
            rs.eval5(&a).strength < rs.eval5(&b).strength,
            "rs_poker: {} should be weaker than {}",
            w[0],
            w[1]
        );
        assert!(
            pk.eval5(&a).strength < pk.eval5(&b).strength,
            "poker: {} should be weaker than {}",
            w[0],
            w[1]
        );
    }
}

/// Two players sharing a board, chopping a straight that uses only the board.
#[test]
fn regression_shared_board_chop_is_a_chop_in_all_three() {
    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();
    // Board 9-8-7-6-5 rainbow-ish; both players' hole cards are irrelevant.
    let a = seven("2c 3d 9h 8s 7c 6d 5h");
    let b = seven("2h 3s 9h 8s 7c 6d 5h");
    assert_eq!(ours_cmp(&ours_seven(&a), &ours_seven(&b)), std::cmp::Ordering::Equal);
    assert_eq!(rs.eval7(&a).strength, rs.eval7(&b).strength);
    assert_eq!(pk.eval7(&a).strength, pk.eval7(&b).strength);
}

// ---- E-09: the input-validation boundary --------------------------------
//
// 2026-08-04, wave 2: FOUR of the five E-09 legs are FIXED in `poker_core`.
// `evaluate_hand` and `evaluate_five_cards` now validate the card count AND
// distinctness and refuse (`try_*` return `HandInputError`; the panicking aliases
// panic with "IMPOSSIBLE HAND").
//
// Each leg below is a pair: a `characterises_*` test stating what the engine does
// NOW, and an `e09_*` test asserting the refusal. Both are ORDINARY GATES in
// `./scripts/dev.sh test`.
//
// The `e09_*` tests were `#[ignore]`d markers run by `make known-defects`, which is
// where a defect lives while it is still present. The wave-2 coherence pass
// un-`#[ignore]`d them and removed them from `DEFECT_MARKERS` in scripts/dev.sh,
// because a marker that has gone GREEN and stays in the list makes the target shout
// on every run until everybody ignores it. The lifecycle is:
// red-on-purpose marker -> defect fixed -> ordinary gate.
//
// The fifth leg, `detect_straight` preferring the wheel over a better straight
// (E-13), is still live, still `#[ignore]`d, and still the one thing
// `make known-defects` reports.

/// FIXED (was: `evaluate_hand` returned `HandRank::HighCard([])` below five cards,
/// which is `Ord`-EQUAL for every player and `Ord`-LESS than every real hand, so any
/// showdown reached before the flop chopped instead of erroring).
///
/// `evaluate_hand` must now REFUSE a board that cannot make a five-card hand.
#[test]
fn characterises_defect_short_board_returns_empty_high_card() {
    for spec in ["Ah Kd", "Ah Kd Qc", "Ah Kd Qc Js"] {
        let hand = parse_hand(spec).unwrap();
        let hole = (to_cleardeck(hand[0]), to_cleardeck(hand[1]));
        let board: Vec<poker_core::Card> = hand[2..].iter().map(|&c| to_cleardeck(c)).collect();
        let refused = poker_core::try_evaluate_hand(&hole, &board);
        assert!(
            refused.is_err(),
            "board {spec} has {} community card(s) and cannot make a five-card hand, but \
             try_evaluate_hand returned {refused:?}. A rank here compares between players and \
             decides a pot (docs/DEFECTS.md E-09).",
            board.len()
        );
        // And the panicking alias the canister calls must not paper over it.
        assert!(
            std::panic::catch_unwind(|| poker_core::evaluate_hand(&hole, &board)).is_err(),
            "evaluate_hand accepted the short board {spec}"
        );
    }
}

/// E-09 leg 1, now an ordinary gate: `evaluate_hand` must refuse a board that
/// cannot make a five-card hand. Asserts the FIXED behaviour with a call that does
/// not itself panic, because `ours_hand` panics on a short board.
#[test]
// Was a known-defects marker (red on purpose) until E-09 was fixed in wave 2. Now
// an ordinary gate in the fast subset, which is where a FIXED defect belongs:
// a marker that has gone green teaches everyone to ignore the marker list.
fn e09_evaluate_hand_refuses_a_short_board() {
    let hand = parse_hand("Ah Kd Qc").unwrap();
    let hole = (to_cleardeck(hand[0]), to_cleardeck(hand[1]));
    let board: Vec<poker_core::Card> = hand[2..].iter().map(|&c| to_cleardeck(c)).collect();
    assert!(
        poker_core::try_evaluate_hand(&hole, &board).is_err(),
        "a board that cannot make a five-card hand must be refused, not ranked"
    );
}

/// FIXED (was: `evaluate_five_cards` accepted more than five cards and tested the
/// flush on one suit and the straight on ALL ranks independently, so it reported
/// `StraightFlush(7)` for `2h 3h 4h 5h 9h 6c 7c`, a hand that contains no straight
/// flush at all).
///
/// `evaluate_five_cards` must now refuse anything that is not exactly five cards.
#[test]
fn characterises_defect_evaluate_five_cards_fabricates_a_straight_flush() {
    for spec in ["2h 3h 4h 5h 9h 6c 7c", "2h 3h 4h 5h 9h 6c"] {
        let hand = parse_hand(spec).unwrap();
        let converted: Vec<poker_core::Card> = hand.iter().map(|&c| to_cleardeck(c)).collect();
        let refused = poker_core::try_evaluate_five_cards(&converted);
        assert!(
            refused.is_err(),
            "evaluate_five_cards ranks exactly five cards, but was handed {} and returned \
             {refused:?} for {spec}",
            hand.len()
        );
    }

    // And the reason it mattered: the best REAL hand in those cards is a flush, so
    // the fabricated straight flush would have beaten every genuine hand at the
    // table. Both references still say flush, so the fix did not change the answer
    // for a legitimate seven-card evaluation.
    let seven_cards = parse_hand("2h 3h 4h 5h 9h 6c 7c").unwrap();
    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();
    let seven_arr: [CardIdx; 7] = seven_cards.clone().try_into().unwrap();
    assert_eq!(rs.eval7(&seven_arr).category, Category::Flush);
    assert_eq!(pk.eval7(&seven_arr).category, Category::Flush);
}

/// MARKER (see `defect_short_board_is_fixed`). GREEN means the defect is fixed.
#[test]
// Was a known-defects marker until E-09 was fixed in wave 2.
fn e09_evaluate_five_cards_refuses_more_than_five_cards() {
    let seven: Vec<poker_core::Card> = parse_hand("2h 3h 4h 5h 9h 6c 7c")
        .unwrap()
        .iter()
        .map(|&c| to_cleardeck(c))
        .collect();
    assert!(
        poker_core::try_evaluate_five_cards(&seven).is_err(),
        "evaluate_five_cards ranks exactly five cards; seven must be refused, not ranked \
         (it used to report StraightFlush(7), a hand not present in those cards)"
    );
}

/// FIXED (was: `evaluate_five_cards` ranked `Ah Ah Ah Ah Ah` as
/// `Flush([14,14,14,14,14])` and `Kh Kh Kd Kd Qs` as `FourOfAKind(13,12)`).
///
/// A hand containing the same physical card twice must be refused. Note from
/// docs/DEFECTS.md H-05 why this had to be fixed HERE and could not be delegated:
/// `rs_poker` ranks `Ah Ah Ah Ah Ah` as `StraightFlush(0)` and `phevaluator` returns
/// its out-of-range sentinel `0`, so no reference would have caught it either. That
/// claim is itself measured, in `checks::reference_probe`.
#[test]
fn characterises_defect_duplicate_cards_are_ranked_silently() {
    for spec in ["Ah Ah Ah Ah Ah", "Ah Ah Ah Ah Kh", "Kh Kh Kd Kd Qs"] {
        let hand = parse_hand(spec).unwrap();
        let converted: Vec<poker_core::Card> = hand.iter().map(|&c| to_cleardeck(c)).collect();
        let refused = poker_core::try_evaluate_five_cards(&converted);
        assert!(
            refused.is_err(),
            "{spec} is physically impossible but evaluate_five_cards returned {refused:?}"
        );
    }
}

/// MARKER (see `defect_short_board_is_fixed`). GREEN means the defect is fixed.
#[test]
// Was a known-defects marker until E-09 was fixed in wave 2.
fn e09_evaluate_five_cards_refuses_duplicate_cards() {
    let five: Vec<poker_core::Card> = parse_hand("Ah Ah Ah Ah Ah")
        .unwrap()
        .iter()
        .map(|&c| to_cleardeck(c))
        .collect();
    assert!(
        poker_core::try_evaluate_five_cards(&five).is_err(),
        "five copies of the ace of hearts must be refused, not ranked as a flush"
    );
}

/// FIXED (was: `evaluate_hand` -- the function `table_canister::determine_winners`
/// calls -- validated neither the card count nor distinctness, so hole cards
/// `Ah Ah` on the board `Kh Qh Jh 2c 3d` produced `Flush([14,14,13,12,11])`: a
/// five-card flush built from only FOUR physical cards).
#[test]
fn characterises_defect_duplicate_card_reaches_evaluate_hand() {
    for spec in ["Ah Ah Kh Qh Jh 2c 3d", "Ah Kh Ah 2c 3d 4s 5h"] {
        let hand = parse_hand(spec).unwrap();
        let hole = (to_cleardeck(hand[0]), to_cleardeck(hand[1]));
        let board: Vec<poker_core::Card> = hand[2..].iter().map(|&c| to_cleardeck(c)).collect();
        let refused = poker_core::try_evaluate_hand(&hole, &board);
        assert!(
            refused.is_err(),
            "{spec} repeats a physical card but try_evaluate_hand returned {refused:?}"
        );
    }
}

/// MARKER (see `defect_short_board_is_fixed`). GREEN means the defect is fixed.
#[test]
// Was a known-defects marker until E-09 was fixed in wave 2.
fn e09_evaluate_hand_refuses_duplicate_cards() {
    let hand = parse_hand("Ah Ah Kh Qh Jh 2c 3d").unwrap();
    let hole = (to_cleardeck(hand[0]), to_cleardeck(hand[1]));
    let board: Vec<poker_core::Card> = hand[2..].iter().map(|&c| to_cleardeck(c)).collect();
    assert!(
        poker_core::try_evaluate_hand(&hole, &board).is_err(),
        "a flush must use five distinct physical cards; a repeated card must be refused"
    );
}

/// FIX WAVE: `detect_straight` tests the wheel before scanning downward, so on
/// more than five ranks it returns the WEAKEST straight. Unreachable from
/// `evaluate_hand` today (which only ever passes five ranks) and therefore not a
/// live money bug, but it is the reason the misuse above is so easy to hit.
#[test]
fn characterises_defect_detect_straight_prefers_the_wheel() {
    assert_eq!(
        detect_straight(&[14, 6, 5, 4, 3, 2]),
        Some(5),
        "detect_straight no longer prefers the wheel: update the finding"
    );
    assert_eq!(detect_straight(&[14, 7, 6, 5, 4, 3, 2]), Some(5));
    // On exactly five ranks, which is the only way `evaluate_five_cards` calls it
    // today, it is correct.
    assert_eq!(detect_straight(&[14, 5, 4, 3, 2]), Some(5));
    assert_eq!(detect_straight(&[6, 5, 4, 3, 2]), Some(6));
    assert_eq!(detect_straight(&[14, 13, 12, 11, 10]), Some(14));
}

#[test]
#[ignore = "known defect: detect_straight prefers the wheel over a better straight on >5 ranks; un-ignore when the fix wave lands"]
fn defect_detect_straight_returns_the_best_straight() {
    assert_eq!(detect_straight(&[14, 6, 5, 4, 3, 2]), Some(6));
    assert_eq!(detect_straight(&[14, 7, 6, 5, 4, 3, 2]), Some(7));
}
