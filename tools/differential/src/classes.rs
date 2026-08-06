//! Equivalence classes and the exhaustive ordering proof.
//!
//! # Why this is the load-bearing check
//!
//! A poker evaluator is only ever used to answer "does A beat B, lose to B, or
//! chop with B". So the property that decides where money goes is not any
//! particular score but the *ordering* the scores induce. Two evaluators agree
//! iff there is a strictly monotonic bijection between their equivalence classes.
//!
//! Checking that by sampling pairs of hands only ever samples a vanishing
//! fraction of the `C(2598960, 2)` ≈ 3.4e12 pairs. [`compare_orderings`] instead
//! proves it for **every** pair in `O(n)` after an `O(n log k)` interning pass:
//!
//! * bucket every hand by our class, in our class order;
//! * within a bucket, every reference class id must be identical, otherwise we
//!   tie two hands the reference separates ([`ViolationKind::FalseTie`]: a chop
//!   where one player should have won the whole pot);
//! * across consecutive buckets, `max(ref ids of bucket i) < min(ref ids of
//!   bucket i+1)`, otherwise we either split a class the reference ties
//!   ([`ViolationKind::FalseSplit`]: a whole pot where there should be a chop) or
//!   we order two classes backwards ([`ViolationKind::Inversion`]: the wrong
//!   player is paid).
//!
//! Because our bucket order is a total order, those two local conditions imply
//! the global one, so a clean result is a proof over all pairs, not a sample.

use std::collections::BTreeMap;

/// Interns values of an ordered type and hands out dense ids in **rank order**,
/// so id 0 is the weakest observed class and `len()-1` the strongest.
pub struct RankOrderIndex<T: Ord + Clone> {
    /// Insertion-order id -> value. Populated during the streaming pass.
    by_insertion: Vec<T>,
    lookup: BTreeMap<T, u32>,
}

impl<T: Ord + Clone> Default for RankOrderIndex<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord + Clone> RankOrderIndex<T> {
    pub fn new() -> Self {
        Self {
            by_insertion: Vec::new(),
            lookup: BTreeMap::new(),
        }
    }

    /// Returns the *insertion* id for `value`, interning it if new. Insertion ids
    /// are cheap to record per row; [`finish`](Self::finish) converts them to
    /// rank-order ids afterwards.
    pub fn intern(&mut self, value: &T) -> u32 {
        if let Some(&id) = self.lookup.get(value) {
            return id;
        }
        let id = self.by_insertion.len() as u32;
        self.by_insertion.push(value.clone());
        self.lookup.insert(value.clone(), id);
        id
    }

    pub fn len(&self) -> usize {
        self.by_insertion.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_insertion.is_empty()
    }

    /// Consumes the index and returns
    /// `(insertion_id -> rank_order_id, rank_order_id -> value)`.
    pub fn finish(self) -> (Vec<u32>, Vec<T>) {
        let mut insertion_to_order = vec![0u32; self.by_insertion.len()];
        let mut values_in_order = Vec::with_capacity(self.by_insertion.len());
        // `BTreeMap` iterates in key order, i.e. weakest class first.
        for (order_id, (value, insertion_id)) in self.lookup.into_iter().enumerate() {
            insertion_to_order[insertion_id as usize] = order_id as u32;
            values_in_order.push(value);
        }
        (insertion_to_order, values_in_order)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViolationKind {
    /// We rate two hands EQUAL that the reference separates. In a real pot this
    /// is a chop where one player was entitled to the whole thing.
    FalseTie,
    /// We rate two hands DIFFERENT that the reference ties. In a real pot this
    /// awards the whole pot to one player where the rules require a chop.
    FalseSplit,
    /// We order two hands the OPPOSITE way to the reference. In a real pot the
    /// losing hand is paid.
    Inversion,
}

impl ViolationKind {
    pub fn name(self) -> &'static str {
        match self {
            ViolationKind::FalseTie => "FalseTie",
            ViolationKind::FalseSplit => "FalseSplit",
            ViolationKind::Inversion => "Inversion",
        }
    }

    pub fn money_impact(self) -> &'static str {
        match self {
            ViolationKind::FalseTie => "pot is chopped when one player should win it outright",
            ViolationKind::FalseSplit => "pot is awarded outright when it should be chopped",
            ViolationKind::Inversion => "pot is awarded to the losing hand",
        }
    }
}

/// A concrete two-hand witness for an ordering disagreement.
#[derive(Clone, Copy, Debug)]
pub struct Violation<W> {
    pub kind: ViolationKind,
    pub left: W,
    pub right: W,
    /// Our class ids for `left` / `right` (rank order).
    pub ours_left: u32,
    pub ours_right: u32,
    /// Reference class ids for `left` / `right` (rank order).
    pub ref_left: u32,
    pub ref_right: u32,
}

/// One bucket of hands sharing an identical class on our side.
#[derive(Clone, Copy, Debug)]
struct Bucket<W> {
    min_ref: u32,
    min_witness: W,
    max_ref: u32,
    max_witness: W,
    count: u64,
}

/// Outcome of an exhaustive ordering comparison.
#[derive(Clone, Debug)]
pub struct OrderingComparison<W> {
    pub ours_class_count: usize,
    pub reference_class_count: usize,
    pub rows_compared: u64,
    pub violations: Vec<Violation<W>>,
}

impl<W> OrderingComparison<W> {
    pub fn agrees(&self) -> bool {
        self.violations.is_empty() && self.ours_class_count == self.reference_class_count
    }
}

/// Proves (or refutes) that our class order and the reference class order are the
/// same order, over every pair of the supplied rows.
///
/// `rows` is `(our rank-order class id, reference rank-order class id, witness)`.
/// `ours_class_count` / `reference_class_count` are the total distinct class
/// counts, used only for reporting.
pub fn compare_orderings<W: Copy>(
    rows: impl Iterator<Item = (u32, u32, W)>,
    ours_class_count: usize,
    reference_class_count: usize,
) -> OrderingComparison<W> {
    let mut buckets: Vec<Option<Bucket<W>>> = vec![None; ours_class_count];
    let mut rows_compared = 0u64;
    let mut violations = Vec::new();

    for (ours_id, ref_id, witness) in rows {
        rows_compared += 1;
        let slot = &mut buckets[ours_id as usize];
        match slot {
            None => {
                *slot = Some(Bucket {
                    min_ref: ref_id,
                    min_witness: witness,
                    max_ref: ref_id,
                    max_witness: witness,
                    count: 1,
                })
            }
            Some(b) => {
                b.count += 1;
                if ref_id < b.min_ref {
                    b.min_ref = ref_id;
                    b.min_witness = witness;
                }
                if ref_id > b.max_ref {
                    b.max_ref = ref_id;
                    b.max_witness = witness;
                }
            }
        }
    }

    // (1) Within a bucket our engine says "equal"; the reference must agree.
    for (ours_id, bucket) in buckets.iter().enumerate() {
        let Some(b) = bucket else { continue };
        if b.min_ref != b.max_ref {
            violations.push(Violation {
                kind: ViolationKind::FalseTie,
                left: b.min_witness,
                right: b.max_witness,
                ours_left: ours_id as u32,
                ours_right: ours_id as u32,
                ref_left: b.min_ref,
                ref_right: b.max_ref,
            });
        }
    }

    // (2) Across buckets, in our rank order, the reference must strictly increase.
    let mut previous: Option<(usize, Bucket<W>)> = None;
    for (ours_id, bucket) in buckets.iter().enumerate() {
        let Some(current) = bucket else { continue };
        if let Some((prev_id, prev)) = previous {
            if current.min_ref == prev.max_ref {
                violations.push(Violation {
                    kind: ViolationKind::FalseSplit,
                    left: prev.max_witness,
                    right: current.min_witness,
                    ours_left: prev_id as u32,
                    ours_right: ours_id as u32,
                    ref_left: prev.max_ref,
                    ref_right: current.min_ref,
                });
            } else if current.min_ref < prev.max_ref {
                violations.push(Violation {
                    kind: ViolationKind::Inversion,
                    left: prev.max_witness,
                    right: current.min_witness,
                    ours_left: prev_id as u32,
                    ours_right: ours_id as u32,
                    ref_left: prev.max_ref,
                    ref_right: current.min_ref,
                });
            }
        }
        previous = Some((ours_id, *current));
    }

    OrderingComparison {
        ours_class_count,
        reference_class_count,
        rows_compared,
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_orderings_have_no_violations() {
        let rows = vec![(0u32, 0u32, 'a'), (0, 0, 'b'), (1, 1, 'c'), (2, 5, 'd')];
        let out = compare_orderings(rows.into_iter(), 3, 3);
        assert!(out.violations.is_empty(), "{:?}", out.violations);
        assert_eq!(out.rows_compared, 4);
    }

    #[test]
    fn detects_a_false_tie() {
        // One of our classes covers two different reference classes.
        let rows = vec![(0u32, 0u32, 'a'), (0, 1, 'b')];
        let out = compare_orderings(rows.into_iter(), 1, 2);
        assert_eq!(out.violations.len(), 1);
        assert_eq!(out.violations[0].kind, ViolationKind::FalseTie);
        assert_eq!((out.violations[0].left, out.violations[0].right), ('a', 'b'));
    }

    #[test]
    fn detects_a_false_split() {
        // Two of our classes map onto one reference class.
        let rows = vec![(0u32, 7u32, 'a'), (1, 7, 'b')];
        let out = compare_orderings(rows.into_iter(), 2, 1);
        assert_eq!(out.violations.len(), 1);
        assert_eq!(out.violations[0].kind, ViolationKind::FalseSplit);
    }

    #[test]
    fn detects_an_inversion() {
        let rows = vec![(0u32, 9u32, 'a'), (1, 4, 'b')];
        let out = compare_orderings(rows.into_iter(), 2, 2);
        assert_eq!(out.violations.len(), 1);
        assert_eq!(out.violations[0].kind, ViolationKind::Inversion);
    }

    #[test]
    fn rank_order_index_hands_out_ids_in_value_order() {
        let mut idx: RankOrderIndex<u32> = RankOrderIndex::new();
        let hi = idx.intern(&100);
        let lo = idx.intern(&1);
        let mid = idx.intern(&50);
        assert_eq!(idx.intern(&100), hi, "interning must be idempotent");
        let (to_order, values) = idx.finish();
        assert_eq!(values, vec![1, 50, 100]);
        assert_eq!(to_order[lo as usize], 0);
        assert_eq!(to_order[mid as usize], 1);
        assert_eq!(to_order[hi as usize], 2);
    }
}
