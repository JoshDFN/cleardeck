//! The four checks.
//!
//! * [`exhaustive`]: every one of the `C(52,5) = 2,598,960` five-card hands, with
//!   a *complete* (not sampled) proof that our class ordering is the references'
//!   class ordering.
//! * [`sevens`]: millions of seeded random seven-card hands through
//!   `evaluate_hand`, the entry point `table_canister::showdown` actually calls.
//! * [`sevens_exhaustive`], opt-in via `--exhaustive-sevens`: all `C(52,7) =
//!   133,784,560` seven-card hands, checked in constant memory against the class
//!   correspondence [`exhaustive`] proved.
//! * [`pairs`]: the literal `sign(our_cmp(a,b)) == sign(ref_cmp(a,b))` check over
//!   random hand pairs, including the realistic case of two players sharing one
//!   board. Redundant with [`exhaustive`] by construction, and kept deliberately:
//!   it is an independent implementation of the same question, so it also guards
//!   the class-ordering machinery itself.
//! * [`third_exhaustive`]: the third reference (`phevaluator`, C-backed, ~900k
//!   hands/second) run over the WHOLE five-card space rather than a sample, so the
//!   headline claim is "three independently-implemented evaluators agree on every
//!   hand". Needs `CLEARDECK_PHE_PYTHON`; degrades to "unavailable" without it.
//! * [`degenerate`]: inputs a well-behaved evaluator should refuse (fewer than five
//!   cards, more than five cards into the five-card entry point, duplicate cards).
//!   Both references are ASKED what they do with each one via
//!   [`reference_probe`], because the measured answer is usually "it ranked it
//!   anyway" -- see docs/DEFECTS.md H-05.

pub mod degenerate;
pub mod exhaustive;
pub mod pairs;
pub mod reference_probe;
pub mod sevens;
pub mod sevens_exhaustive;
pub mod third_exhaustive;
