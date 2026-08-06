//! The literal pairwise sign check.
//!
//! `assert sign(our_cmp(a,b)) == sign(reference_cmp(a,b))`, written the obvious
//! naive way over randomly drawn pairs. [`crate::checks::exhaustive`] already
//! proves this for every five-card pair by a smarter route, so this check is
//! deliberately redundant: it is an independent implementation of the same
//! question and therefore also a test of the class-ordering machinery. If the two
//! ever disagree, the machinery is what is wrong.
//!
//! Two draw modes:
//!
//! * `five_vs_five`: two independent five-card hands. Covers pairs the
//!   exhaustive sweep also covers.
//! * `showdown_shared_board`: deal nine cards: a five-card board plus two hole
//!   cards each for two players, then compare `evaluate_hand` outputs. This is
//!   the *actual* comparison `table_canister` performs when it awards a pot, and
//!   it is the only mode where the two hands share five cards.

use std::cmp::Ordering;

use crate::cards::{slice_to_string, CardIdx};
use crate::engine::{ours_cmp, ours_five, ours_seven, render};
use crate::oracles::poker_oracle::PokerCrateOracle;
use crate::oracles::rs_poker_oracle::RsPokerOracle;
use crate::oracles::Oracle;
use crate::report::{DisagreementClass, PairsSummary, Severity};
use crate::rng::SplitMix64;

pub struct PairsOutcome {
    pub summary: PairsSummary,
    pub disagreements: Vec<DisagreementClass>,
    pub inconclusive: Vec<DisagreementClass>,
}

fn sign(o: Ordering) -> i8 {
    match o {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// Everything needed to render one sign disagreement. A struct rather than nine
/// positional arguments, because a transposed pair of `i8` signs here would be an
/// invisible reporting bug.
struct SignMismatch<'a> {
    check: &'a str,
    reference: &'a str,
    severity: Severity,
    left: &'a [CardIdx],
    right: &'a [CardIdx],
    ours_says: String,
    reference_says: String,
    ours_sign: i8,
    ref_sign: i8,
}

impl SignMismatch<'_> {
    fn into_class(self) -> DisagreementClass {
        DisagreementClass {
            id: format!(
                "pair-sign/{}/ours{}-ref{}",
                self.check, self.ours_sign, self.ref_sign
            ),
            severity: self.severity,
            check: self.check.to_string(),
            reference: self.reference.to_string(),
            occurrences: 1,
            minimal_repro: vec![
                format!("A = {}", slice_to_string(self.left)),
                format!("B = {}", slice_to_string(self.right)),
            ],
            ours: vec![self.ours_says],
            reference_says: vec![self.reference_says],
            explanation: format!(
                "sign(our_cmp(A,B)) = {} but sign({}_cmp(A,B)) = {}; the engine and the \
                 reference would pay different players.",
                self.ours_sign, self.reference, self.ref_sign
            ),
        }
    }
}

pub fn run(seed: u64, pairs_each_mode: u64) -> PairsOutcome {
    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();
    let mut rng = SplitMix64::new(seed);

    let mut disagreements: Vec<DisagreementClass> = Vec::new();
    let mut inconclusive: Vec<DisagreementClass> = Vec::new();
    let mut rs_mismatches = 0u64;
    let mut pk_mismatches = 0u64;
    let mut ref_disagreements = 0u64;

    // ---- mode 1: two independent five-card hands ----------------------------
    for _ in 0..pairs_each_mode {
        let a: [CardIdx; 5] = {
            let d = rng.deal(5);
            [d[0], d[1], d[2], d[3], d[4]]
        };
        let b: [CardIdx; 5] = {
            let d = rng.deal(5);
            [d[0], d[1], d[2], d[3], d[4]]
        };

        let our_a = ours_five(&a);
        let our_b = ours_five(&b);
        let ours = sign(ours_cmp(&our_a, &our_b));
        let s_rs = sign(rs.eval5(&a).strength.cmp(&rs.eval5(&b).strength));
        let s_pk = sign(pk.eval5(&a).strength.cmp(&pk.eval5(&b).strength));

        if s_rs != s_pk {
            ref_disagreements += 1;
            if inconclusive.len() < 32 {
                inconclusive.push(
                    SignMismatch {
                        check: "pairs_five_vs_five",
                        reference: "rs_poker-vs-poker",
                        severity: Severity::Inconclusive,
                        left: &a,
                        right: &b,
                        ours_says: format!("{} / {}", render(&our_a), render(&our_b)),
                        reference_says: format!("rs_poker sign {s_rs}, poker sign {s_pk}"),
                        ours_sign: s_rs,
                        ref_sign: s_pk,
                    }
                    .into_class(),
                );
            }
            continue;
        }

        if ours != s_rs {
            rs_mismatches += 1;
            if disagreements.len() < 32 {
                disagreements.push(
                    SignMismatch {
                        check: "pairs_five_vs_five",
                        reference: "rs_poker",
                        severity: Severity::FundImpacting,
                        left: &a,
                        right: &b,
                        ours_says: format!("{} / {}", render(&our_a), render(&our_b)),
                        reference_says: format!("rs_poker sign {s_rs}"),
                        ours_sign: ours,
                        ref_sign: s_rs,
                    }
                    .into_class(),
                );
            }
        }
        if ours != s_pk {
            pk_mismatches += 1;
            if disagreements.len() < 64 {
                disagreements.push(
                    SignMismatch {
                        check: "pairs_five_vs_five",
                        reference: "poker",
                        severity: Severity::FundImpacting,
                        left: &a,
                        right: &b,
                        ours_says: format!("{} / {}", render(&our_a), render(&our_b)),
                        reference_says: format!("poker sign {s_pk}"),
                        ours_sign: ours,
                        ref_sign: s_pk,
                    }
                    .into_class(),
                );
            }
        }
    }

    // ---- mode 2: real showdown, two players sharing one board ---------------
    for _ in 0..pairs_each_mode {
        let dealt = rng.deal(9);
        let board = [dealt[0], dealt[1], dealt[2], dealt[3], dealt[4]];
        let seat_a: [CardIdx; 7] = [
            dealt[5], dealt[6], board[0], board[1], board[2], board[3], board[4],
        ];
        let seat_b: [CardIdx; 7] = [
            dealt[7], dealt[8], board[0], board[1], board[2], board[3], board[4],
        ];

        let our_a = ours_seven(&seat_a);
        let our_b = ours_seven(&seat_b);
        let ours = sign(ours_cmp(&our_a, &our_b));
        let s_rs = sign(rs.eval7(&seat_a).strength.cmp(&rs.eval7(&seat_b).strength));
        let s_pk = sign(pk.eval7(&seat_a).strength.cmp(&pk.eval7(&seat_b).strength));

        if s_rs != s_pk {
            ref_disagreements += 1;
            if inconclusive.len() < 32 {
                inconclusive.push(
                    SignMismatch {
                        check: "pairs_showdown_shared_board",
                        reference: "rs_poker-vs-poker",
                        severity: Severity::Inconclusive,
                        left: &seat_a,
                        right: &seat_b,
                        ours_says: format!("{} / {}", render(&our_a), render(&our_b)),
                        reference_says: format!("rs_poker sign {s_rs}, poker sign {s_pk}"),
                        ours_sign: s_rs,
                        ref_sign: s_pk,
                    }
                    .into_class(),
                );
            }
            continue;
        }

        if ours != s_rs {
            rs_mismatches += 1;
            if disagreements.len() < 96 {
                disagreements.push(
                    SignMismatch {
                        check: "pairs_showdown_shared_board",
                        reference: "rs_poker",
                        severity: Severity::FundImpacting,
                        left: &seat_a,
                        right: &seat_b,
                        ours_says: format!("{} / {}", render(&our_a), render(&our_b)),
                        reference_says: format!("rs_poker sign {s_rs}"),
                        ours_sign: ours,
                        ref_sign: s_rs,
                    }
                    .into_class(),
                );
            }
        }
        if ours != s_pk {
            pk_mismatches += 1;
            if disagreements.len() < 128 {
                disagreements.push(
                    SignMismatch {
                        check: "pairs_showdown_shared_board",
                        reference: "poker",
                        severity: Severity::FundImpacting,
                        left: &seat_a,
                        right: &seat_b,
                        ours_says: format!("{} / {}", render(&our_a), render(&our_b)),
                        reference_says: format!("poker sign {s_pk}"),
                        ours_sign: ours,
                        ref_sign: s_pk,
                    }
                    .into_class(),
                );
            }
        }
    }

    PairsOutcome {
        summary: PairsSummary {
            seed,
            pairs_five_vs_five: pairs_each_mode,
            pairs_showdown_shared_board: pairs_each_mode,
            sign_mismatches_vs_rs_poker: rs_mismatches,
            sign_mismatches_vs_poker: pk_mismatches,
            reference_sign_disagreements: ref_disagreements,
        },
        disagreements,
        inconclusive,
    }
}
