//! The machine-readable report.
//!
//! Shape is deliberately "grouped by root cause, one minimal repro each" rather
//! than a dump of every offending row: a 2.6M-hand sweep that disagrees on a
//! whole hand category would otherwise emit hundreds of thousands of
//! indistinguishable rows and bury the two or three real defects.

use serde::Serialize;

use crate::category::Category;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Severity {
    /// Wrong player is paid, or a pot is split/awarded against the rules.
    /// Directly moves money.
    FundImpacting,
    /// Wrong hand description shown to the player; pot still correct.
    Cosmetic,
    /// A misuse-shaped defect: an API that returns a plausible-looking wrong
    /// answer instead of refusing, reachable only from a caller bug today.
    Latent,
    /// The two references disagree with each other, so nothing can be concluded
    /// about ClearDeck from this hand.
    Inconclusive,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReferenceInfo {
    pub name: String,
    pub provenance: String,
}

/// One *class* of disagreement, with a single minimal reproducer.
#[derive(Clone, Debug, Serialize)]
pub struct DisagreementClass {
    /// Stable slug, e.g. `ordering/inversion/flush-vs-flush`.
    pub id: String,
    pub severity: Severity,
    /// Which check produced it.
    pub check: String,
    /// Which reference disagreed (or `both`).
    pub reference: String,
    /// How many rows in the run fell into this class.
    pub occurrences: u64,
    /// Exact cards, in `"Ah Kd .."` notation. Two entries for an ordering
    /// disagreement (the pair), one for a pointwise disagreement.
    pub minimal_repro: Vec<String>,
    /// What our engine said, verbatim `HandRank` debug.
    pub ours: Vec<String>,
    /// What the reference said.
    pub reference_says: Vec<String>,
    /// One line: what our engine gets wrong.
    pub explanation: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct OrderingRunSummary {
    pub reference: String,
    pub ours_class_count: usize,
    pub reference_class_count: usize,
    pub rows_compared: u64,
    pub false_ties: usize,
    pub false_splits: usize,
    pub inversions: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct CategoryCount {
    pub category: Category,
    pub ours: u64,
    pub expected: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ExhaustiveFiveSummary {
    pub hands_enumerated: u64,
    pub expected_hands: u64,
    pub category_mismatches_vs_rs_poker: u64,
    pub category_mismatches_vs_poker: u64,
    pub detail_mismatches_vs_poker: u64,
    pub royal_flush_alias_mismatches: u64,
    pub our_royal_flush_count: u64,
    pub our_straight_flush_count: u64,
    pub category_counts: Vec<CategoryCount>,
    pub category_counts_match_combinatorics: bool,
    pub ordering: Vec<OrderingRunSummary>,
    /// `rs_poker` vs `poker`: any nonzero entry here makes the affected hands
    /// inconclusive rather than evidence against ClearDeck.
    pub reference_cross_check: Option<OrderingRunSummary>,
    pub reference_cross_check_category_mismatches: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PairsSummary {
    pub seed: u64,
    pub pairs_five_vs_five: u64,
    pub pairs_showdown_shared_board: u64,
    pub sign_mismatches_vs_rs_poker: u64,
    pub sign_mismatches_vs_poker: u64,
    pub reference_sign_disagreements: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SevensSummary {
    pub seed: u64,
    pub hands_evaluated: u64,
    pub distinct_our_classes: usize,
    pub category_mismatches_vs_rs_poker: u64,
    pub category_mismatches_vs_poker: u64,
    pub detail_mismatches_vs_poker: u64,
    pub ordering: Vec<OrderingRunSummary>,
    pub reference_cross_check: Option<OrderingRunSummary>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ExhaustiveSevenSummary {
    pub hands_enumerated: u64,
    pub expected_hands: u64,
    /// `evaluate_hand` produced a `HandRank` no five-card hand can produce, so the
    /// proven class map has nothing to compare it against.
    pub unmapped_hand_ranks: u64,
    pub strength_mismatches_vs_rs_poker: u64,
    pub strength_mismatches_vs_poker: u64,
    /// Distinct `HandRank` values `evaluate_hand` produced. Over the full space
    /// this must be exactly 4824, a library-free combinatorial fact.
    pub distinct_our_classes: usize,
    pub expected_distinct_classes: Option<usize>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DegenerateSummary {
    pub probes_run: u64,
    pub probes_with_findings: u64,
}

/// The third reference run over the WHOLE five-card space, not just a sample.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ThirdReferenceSummary {
    pub available: bool,
    pub detail: String,
    pub hands_checked: u64,
    pub third_reference_class_count: usize,
    pub ordering: Vec<OrderingRunSummary>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AdjudicatorSummary {
    pub available: bool,
    pub detail: String,
    pub hands_checked: u64,
    pub disagreements_with_rs_poker: u64,
    pub disagreements_with_poker: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub harness: &'static str,
    pub harness_version: &'static str,
    pub engine_under_test: &'static str,
    pub references: Vec<ReferenceInfo>,
    pub adjudicator: AdjudicatorSummary,
    pub third_reference_exhaustive_five_card: ThirdReferenceSummary,
    pub exhaustive_five_card: Option<ExhaustiveFiveSummary>,
    pub exhaustive_seven_card: Option<ExhaustiveSevenSummary>,
    pub random_pairs: Option<PairsSummary>,
    pub random_seven_card: Option<SevensSummary>,
    pub degenerate_inputs: DegenerateSummary,
    /// Grouped by root cause. Empty means the engine agreed with both references
    /// everywhere the run looked.
    pub disagreement_classes: Vec<DisagreementClass>,
    /// Hands where the two references disagree with each other.
    pub inconclusive: Vec<DisagreementClass>,
    pub verdict: &'static str,
    pub notes: Vec<String>,
}

impl Report {
    pub fn fund_impacting(&self) -> usize {
        self.disagreement_classes
            .iter()
            .filter(|d| d.severity == Severity::FundImpacting)
            .count()
    }
}
