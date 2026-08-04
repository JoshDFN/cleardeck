//! The third reference, run EXHAUSTIVELY over all 2,598,960 five-card hands.
//!
//! `phevaluator` ships a C extension and ranks ~900k hands/second, so the whole
//! five-card space costs about three seconds of Python plus the pipe. That makes
//! the third lineage affordable as a full reference rather than only as a sampled
//! adjudicator, and turns the headline claim from "two evaluators agree" into
//! "three independently-implemented evaluators agree, on every hand".
//!
//! Requires `CLEARDECK_PHE_PYTHON`; without it the check reports itself as
//! unavailable rather than failing, because `cargo run --release` must work on a
//! machine with no Python.
//!
//! # Deadlock note
//!
//! Hands are sent in bounded chunks and `oracle3/phe_oracle.py` buffers its whole
//! answer until stdin hits EOF. Writing 2.6M lines into a child that was
//! streaming answers back would deadlock as soon as its 64 KiB stdout pipe filled.

use crate::adjudicator::{rank_hands, PYTHON_ENV_VAR};
use crate::cards::{pack5, slice_to_string, CardIdx};
use crate::classes::{compare_orderings, OrderingComparison, RankOrderIndex, ViolationKind};
use crate::describe::{describe_ordering, FiveWitness};
use crate::engine::ours_five;
use crate::oracles::poker_oracle::{PokerCrateOracle, PokerStrength};
use crate::oracles::rs_poker_oracle::RsPokerOracle;
use crate::oracles::Oracle;
use crate::report::{DisagreementClass, OrderingRunSummary, Severity, ThirdReferenceSummary};

use super::exhaustive::all_five_card_hands;

/// Hands per child invocation. Bounds peak memory on both sides of the pipe.
const CHUNK: usize = 250_000;

pub struct ThirdExhaustiveOutcome {
    pub summary: ThirdReferenceSummary,
    pub disagreements: Vec<DisagreementClass>,
    pub inconclusive: Vec<DisagreementClass>,
}

fn summarise(reference: &str, cmp: &OrderingComparison<FiveWitness>) -> OrderingRunSummary {
    let count = |k: ViolationKind| cmp.violations.iter().filter(|v| v.kind == k).count();
    OrderingRunSummary {
        reference: reference.to_string(),
        ours_class_count: cmp.ours_class_count,
        reference_class_count: cmp.reference_class_count,
        rows_compared: cmp.rows_compared,
        false_ties: count(ViolationKind::FalseTie),
        false_splits: count(ViolationKind::FalseSplit),
        inversions: count(ViolationKind::Inversion),
    }
}

/// Interners plus one interned-id column per evaluator.
struct Corpus {
    ours_idx: RankOrderIndex<poker_core::HandRank>,
    rs_idx: RankOrderIndex<rs_poker::core::Rank>,
    pk_idx: RankOrderIndex<PokerStrength>,
    phe_idx: RankOrderIndex<i64>,
    packed: Vec<u32>,
    ours_ins: Vec<u32>,
    rs_ins: Vec<u32>,
    pk_ins: Vec<u32>,
    phe_ins: Vec<u32>,
}

impl Corpus {
    fn new() -> Self {
        Self {
            ours_idx: RankOrderIndex::new(),
            rs_idx: RankOrderIndex::new(),
            pk_idx: RankOrderIndex::new(),
            phe_idx: RankOrderIndex::new(),
            packed: Vec::new(),
            ours_ins: Vec::new(),
            rs_ins: Vec::new(),
            pk_ins: Vec::new(),
            phe_ins: Vec::new(),
        }
    }

    /// Ranks one chunk of hands with all four evaluators and appends the columns.
    fn absorb(
        &mut self,
        python: &str,
        rs: &RsPokerOracle,
        pk: &PokerCrateOracle,
        chunk: &mut Vec<[CardIdx; 5]>,
    ) -> Result<(), String> {
        if chunk.is_empty() {
            return Ok(());
        }
        let rendered: Vec<String> = chunk.iter().map(|h| slice_to_string(h)).collect();
        let scores = rank_hands(python, &rendered)?;
        for (i, hand) in chunk.iter().enumerate() {
            self.packed.push(pack5(hand));
            self.ours_ins.push(self.ours_idx.intern(&ours_five(hand)));
            self.rs_ins.push(self.rs_idx.intern(&rs.eval5(hand).strength));
            self.pk_ins.push(self.pk_idx.intern(&pk.eval5(hand).strength));
            // phevaluator: 1 is the best hand, so negate to make greater better.
            self.phe_ins.push(self.phe_idx.intern(&-scores[i]));
        }
        chunk.clear();
        Ok(())
    }
}

pub fn run() -> ThirdExhaustiveOutcome {
    let Ok(python) = std::env::var(PYTHON_ENV_VAR) else {
        return unavailable(format!(
            "{PYTHON_ENV_VAR} not set; the third reference (phevaluator) was not run. \
             See tools/differential/oracle3/phe_oracle.py for the one-line setup."
        ));
    };

    let rs = RsPokerOracle;
    let pk = PokerCrateOracle::new();
    let mut corpus = Corpus::new();
    let mut chunk: Vec<[CardIdx; 5]> = Vec::with_capacity(CHUNK);

    for hand in all_five_card_hands() {
        chunk.push(hand);
        if chunk.len() == CHUNK {
            if let Err(e) = corpus.absorb(&python, &rs, &pk, &mut chunk) {
                return unavailable(format!("third reference unusable: {e}"));
            }
        }
    }
    if let Err(e) = corpus.absorb(&python, &rs, &pk, &mut chunk) {
        return unavailable(format!("third reference unusable: {e}"));
    }

    let Corpus {
        ours_idx,
        rs_idx,
        pk_idx,
        phe_idx,
        packed,
        ours_ins,
        rs_ins,
        pk_ins,
        phe_ins,
    } = corpus;

    let ours_classes = ours_idx.len();
    let rs_classes = rs_idx.len();
    let pk_classes = pk_idx.len();
    let phe_classes = phe_idx.len();
    let (ours_order, _) = ours_idx.finish();
    let (rs_order, _) = rs_idx.finish();
    let (pk_order, _) = pk_idx.finish();
    let (phe_order, _) = phe_idx.finish();

    let ours_vs_phe = compare_orderings(
        rows(&packed, &ours_ins, &ours_order, &phe_ins, &phe_order),
        ours_classes,
        phe_classes,
    );
    let rs_vs_phe = compare_orderings(
        rows(&packed, &rs_ins, &rs_order, &phe_ins, &phe_order),
        rs_classes,
        phe_classes,
    );
    let pk_vs_phe = compare_orderings(
        rows(&packed, &pk_ins, &pk_order, &phe_ins, &phe_order),
        pk_classes,
        phe_classes,
    );

    let mut disagreements = describe_ordering(
        "exhaustive_five_card_third_reference",
        "phevaluator",
        &ours_vs_phe.violations,
    );
    // A reference-vs-reference conflict is not evidence against ClearDeck.
    let mut inconclusive = describe_ordering(
        "exhaustive_five_card_third_reference",
        "rs_poker-vs-phevaluator",
        &rs_vs_phe.violations,
    );
    inconclusive.extend(describe_ordering(
        "exhaustive_five_card_third_reference",
        "poker-vs-phevaluator",
        &pk_vs_phe.violations,
    ));
    for entry in inconclusive.iter_mut() {
        entry.severity = Severity::Inconclusive;
    }
    // If a reference conflicts with the third lineage, our own comparison against
    // it cannot convict us either.
    if !inconclusive.is_empty() {
        for entry in disagreements.iter_mut() {
            entry.severity = Severity::Inconclusive;
        }
        inconclusive.append(&mut disagreements);
    }

    ThirdExhaustiveOutcome {
        summary: ThirdReferenceSummary {
            available: true,
            detail: format!("phevaluator via {python}"),
            hands_checked: packed.len() as u64,
            third_reference_class_count: phe_classes,
            ordering: vec![
                summarise("ours-vs-phevaluator", &ours_vs_phe),
                summarise("rs_poker-vs-phevaluator", &rs_vs_phe),
                summarise("poker-vs-phevaluator", &pk_vs_phe),
            ],
        },
        disagreements,
        inconclusive,
    }
}

fn rows<'a>(
    packed: &'a [u32],
    left_ins: &'a [u32],
    left_order: &'a [u32],
    right_ins: &'a [u32],
    right_order: &'a [u32],
) -> impl Iterator<Item = (u32, u32, FiveWitness)> + 'a {
    (0..packed.len()).map(move |i| {
        (
            left_order[left_ins[i] as usize],
            right_order[right_ins[i] as usize],
            FiveWitness(packed[i]),
        )
    })
}

fn unavailable(detail: String) -> ThirdExhaustiveOutcome {
    ThirdExhaustiveOutcome {
        summary: ThirdReferenceSummary {
            available: false,
            detail,
            ..Default::default()
        },
        disagreements: Vec::new(),
        inconclusive: Vec::new(),
    }
}
