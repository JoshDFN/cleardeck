//! Turning raw violations into grouped, minimally-reproducible report entries.
//!
//! Requirement: "for every disagreement class, produce a MINIMAL reproducing hand
//! (exact cards) and a one-line explanation ... group by root cause; do not dump
//! 50,000 raw rows." Everything here exists to honour that: violations are keyed
//! by *shape* (what kind of ordering error, between which hand categories), the
//! first witness for each shape is kept, and the rest are counted.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use poker_core::HandRank;

use crate::cards::{slice_to_string, unpack5, CardIdx};
use crate::category::{category_of_cleardeck, Category};
use crate::classes::{Violation, ViolationKind};
use crate::engine::render;
use crate::report::{DisagreementClass, Severity};

/// A witness is either a 5-card hand (packed) or a 7-card hand.
pub trait Witness: Copy {
    fn cards(&self) -> Vec<CardIdx>;
    fn our_rank(&self) -> HandRank;
}

#[derive(Clone, Copy, Debug)]
pub struct FiveWitness(pub u32);

impl Witness for FiveWitness {
    fn cards(&self) -> Vec<CardIdx> {
        unpack5(self.0).to_vec()
    }
    fn our_rank(&self) -> HandRank {
        crate::engine::ours_five(&unpack5(self.0))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SevenWitness(pub [CardIdx; 7]);

impl Witness for SevenWitness {
    fn cards(&self) -> Vec<CardIdx> {
        self.0.to_vec()
    }
    fn our_rank(&self) -> HandRank {
        crate::engine::ours_seven(&self.0)
    }
}

fn ordering_word(kind: ViolationKind) -> (&'static str, &'static str) {
    match kind {
        ViolationKind::FalseTie => ("==", "!="),
        ViolationKind::FalseSplit => ("!=", "=="),
        ViolationKind::Inversion => (">", "<"),
    }
}

/// Groups ordering violations by root-cause shape and emits one entry each.
pub fn describe_ordering<W: Witness>(
    check: &str,
    reference: &str,
    violations: &[Violation<W>],
) -> Vec<DisagreementClass> {
    // Key: (kind, our category of left, our category of right).
    let mut grouped: BTreeMap<(u8, Category, Category), (Violation<W>, u64)> = BTreeMap::new();

    for v in violations {
        let lr = v.left.our_rank();
        let rr = v.right.our_rank();
        let key = (
            v.kind as u8,
            category_of_cleardeck(&lr),
            category_of_cleardeck(&rr),
        );
        grouped
            .entry(key)
            .and_modify(|(_, n)| *n += 1)
            .or_insert((*v, 1));
    }

    grouped
        .into_iter()
        .map(|((_, cat_l, cat_r), (v, occurrences))| {
            let left_cards = v.left.cards();
            let right_cards = v.right.cards();
            let left_rank = v.left.our_rank();
            let right_rank = v.right.our_rank();
            let (ours_op, ref_op) = ordering_word(v.kind);

            let mut explanation = String::new();
            let _ = write!(
                explanation,
                "ClearDeck orders hand A {ours_op} hand B, {reference} orders them A {ref_op} B \
                 ({cat_l} vs {cat_r}); at showdown this means the {impact}.",
                cat_l = cat_l.name(),
                cat_r = cat_r.name(),
                impact = v.kind.money_impact(),
            );

            DisagreementClass {
                id: format!(
                    "ordering/{}/{}-vs-{}",
                    v.kind.name().to_lowercase(),
                    cat_l.name(),
                    cat_r.name()
                ),
                severity: Severity::FundImpacting,
                check: check.to_string(),
                reference: reference.to_string(),
                occurrences,
                minimal_repro: vec![
                    format!("A = {}", slice_to_string(&left_cards)),
                    format!("B = {}", slice_to_string(&right_cards)),
                ],
                ours: vec![
                    format!("A -> {}", render(&left_rank)),
                    format!("B -> {}", render(&right_rank)),
                ],
                reference_says: vec![format!(
                    "A class id {} , B class id {} (rank order, higher is better)",
                    v.ref_left, v.ref_right
                )],
                explanation,
            }
        })
        .collect()
}

/// Applies the harness's central rule: **ClearDeck is only convicted when BOTH
/// references agree against it.**
///
/// Given every entry raised against `left_reference` and `right_reference`,
/// entries whose root-cause `id` was raised by both keep their severity and are
/// returned as convictions. An entry only one reference raised means the other
/// reference sided with ClearDeck, i.e. the two references disagree with each
/// other about that hand, so it is downgraded to
/// [`Severity::Inconclusive`] and returned in the second slot.
///
/// Returns `(convictions, inconclusive)`.
///
/// Not applicable to checks where only one reference can speak (the sub-rank
/// detail check: `rs_poker` exposes no kicker detail, so `poker` is the sole
/// authority there and unanimity is unattainable by construction).
pub fn require_unanimity(
    entries: Vec<DisagreementClass>,
    left_reference: &str,
    right_reference: &str,
) -> (Vec<DisagreementClass>, Vec<DisagreementClass>) {
    let ids_from = |reference: &str| -> std::collections::BTreeSet<String> {
        entries
            .iter()
            .filter(|e| e.reference == reference)
            .map(|e| e.id.clone())
            .collect()
    };
    let left = ids_from(left_reference);
    let right = ids_from(right_reference);

    let mut convictions = Vec::new();
    let mut inconclusive = Vec::new();
    for mut entry in entries {
        let unanimous = left.contains(&entry.id) && right.contains(&entry.id);
        if unanimous {
            convictions.push(entry);
        } else {
            let other = if entry.reference == left_reference {
                right_reference
            } else {
                left_reference
            };
            entry.severity = Severity::Inconclusive;
            entry.explanation = format!(
                "{} INCONCLUSIVE: only {} raised this; {} agreed with ClearDeck, so the two \
                 references disagree with each other about this hand and neither can convict.",
                entry.explanation, entry.reference, other
            );
            inconclusive.push(entry);
        }
    }
    (convictions, inconclusive)
}

#[cfg(test)]
mod unanimity_tests {
    use super::*;

    fn entry(id: &str, reference: &str) -> DisagreementClass {
        DisagreementClass {
            id: id.to_string(),
            severity: Severity::FundImpacting,
            check: "t".into(),
            reference: reference.to_string(),
            occurrences: 1,
            minimal_repro: vec![],
            ours: vec![],
            reference_says: vec![],
            explanation: "base".into(),
        }
    }

    #[test]
    fn both_references_agreeing_against_us_is_a_conviction() {
        let entries = vec![entry("ordering/x", "rs_poker"), entry("ordering/x", "poker")];
        let (convict, unproven) = require_unanimity(entries, "rs_poker", "poker");
        assert_eq!(convict.len(), 2);
        assert!(unproven.is_empty());
        assert!(convict.iter().all(|e| e.severity == Severity::FundImpacting));
    }

    #[test]
    fn one_reference_alone_cannot_convict() {
        let entries = vec![entry("ordering/x", "rs_poker")];
        let (convict, unproven) = require_unanimity(entries, "rs_poker", "poker");
        assert!(convict.is_empty(), "a lone reference must not convict");
        assert_eq!(unproven.len(), 1);
        assert_eq!(unproven[0].severity, Severity::Inconclusive);
        assert!(
            unproven[0].explanation.contains("poker agreed with ClearDeck"),
            "explanation must name the reference that sided with us: {}",
            unproven[0].explanation
        );
    }

    #[test]
    fn unrelated_ids_are_judged_independently() {
        let entries = vec![
            entry("ordering/a", "rs_poker"),
            entry("ordering/a", "poker"),
            entry("ordering/b", "poker"),
        ];
        let (convict, unproven) = require_unanimity(entries, "rs_poker", "poker");
        assert_eq!(convict.len(), 2);
        assert!(convict.iter().all(|e| e.id == "ordering/a"));
        assert_eq!(unproven.len(), 1);
        assert_eq!(unproven[0].id, "ordering/b");
    }
}

/// A pointwise (single-hand) disagreement: our label for a hand differs from the
/// reference's label for the same hand.
#[derive(Clone, Debug)]
pub struct PointwiseHit {
    pub cards: Vec<CardIdx>,
    pub ours: String,
    pub reference_says: String,
    /// Root-cause key. Two hits with the same key are the same defect.
    pub shape: String,
}

pub struct PointwiseCollector {
    check: String,
    reference: String,
    severity: Severity,
    id_prefix: String,
    hits: BTreeMap<String, (PointwiseHit, u64)>,
    /// Hard cap on distinct shapes retained, so a systematically broken engine
    /// cannot make the report unbounded.
    cap: usize,
    /// Hits whose shape arrived after the cap was reached. Counted, not retained.
    /// This is a count of HITS, not of distinct shapes, a hit with an already-
    /// dropped shape has no retained entry to increment, so it lands here too.
    dropped_hits: u64,
}

impl PointwiseCollector {
    pub fn new(
        check: &str,
        reference: &str,
        severity: Severity,
        id_prefix: &str,
    ) -> Self {
        Self {
            check: check.to_string(),
            reference: reference.to_string(),
            severity,
            id_prefix: id_prefix.to_string(),
            hits: BTreeMap::new(),
            cap: 64,
            dropped_hits: 0,
        }
    }

    pub fn record(&mut self, hit: PointwiseHit) {
        if let Some(entry) = self.hits.get_mut(&hit.shape) {
            entry.1 += 1;
            return;
        }
        if self.hits.len() >= self.cap {
            self.dropped_hits += 1;
            return;
        }
        self.hits.insert(hit.shape.clone(), (hit, 1));
    }

    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// Total hits recorded, retained or not.
    pub fn total(&self) -> u64 {
        self.hits.values().map(|(_, n)| *n).sum::<u64>() + self.dropped_hits
    }

    pub fn finish(self, explain: impl Fn(&PointwiseHit) -> String) -> Vec<DisagreementClass> {
        let mut out: Vec<DisagreementClass> = self
            .hits
            .values()
            .map(|(hit, n)| DisagreementClass {
                id: format!("{}/{}", self.id_prefix, hit.shape),
                severity: self.severity,
                check: self.check.clone(),
                reference: self.reference.clone(),
                occurrences: *n,
                minimal_repro: vec![slice_to_string(&hit.cards)],
                ours: vec![hit.ours.clone()],
                reference_says: vec![hit.reference_says.clone()],
                explanation: explain(hit),
            })
            .collect();
        if self.dropped_hits > 0 {
            out.push(DisagreementClass {
                id: format!("{}/TRUNCATED", self.id_prefix),
                severity: self.severity,
                check: self.check.clone(),
                reference: self.reference.clone(),
                occurrences: self.dropped_hits,
                minimal_repro: vec![],
                ours: vec![],
                reference_says: vec![],
                explanation: format!(
                    "{} further disagreement hits were counted but not retained: their shapes                      arrived after the {}-distinct-shape cap. Raise `PointwiseCollector::cap` to                      see them.",
                    self.dropped_hits, self.cap
                ),
            });
        }
        out
    }
}
