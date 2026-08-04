//! ASKING the references about an input, instead of claiming to know their answer.
//!
//! docs/DEFECTS.md H-05: the degenerate-input probes used to attach hard-coded
//! `reference_says` strings -- "both references reject a hand containing the same
//! card twice", "no reference will evaluate fewer than five cards" -- to findings
//! where no reference was ever called. All of them were wrong about reference A.
//! Five of the twelve findings were pushed unconditionally, with no reference call
//! at all, so the harness was manufacturing its own corroboration.
//!
//! This module calls both in-process references on whatever cards it is given and
//! reports what came back, including "it panicked" and "it returned an error",
//! which are the two ways a library can refuse. Nothing here decides anything; it
//! only measures.
//!
//! # Why the panics have to be caught
//!
//! `rs_poker` carries internal `debug_assert!`s, so a call that returns a rank in
//! `--release` can abort in a `cargo test` (debug) build. A probe that crashes the
//! harness on a degenerate input teaches nobody anything, and "the reference
//! refuses this input" is exactly the fact under measurement -- so a panic is
//! captured as [`Verdict::Refused`] rather than allowed to kill the run.

use std::fmt;

use poker::Evaluator;
use rs_poker::core::Rankable;

use crate::cards::{slice_to_string, to_poker_crate, to_rs_poker, CardIdx};

/// What one reference did with one input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The library produced a rank. The string is its own rendering of it.
    Ranked(String),
    /// The library declined: an `Err`, or a panic/assertion. The string is why.
    Refused(String),
}

impl Verdict {
    pub fn ranked(&self) -> Option<&str> {
        match self {
            Verdict::Ranked(s) => Some(s),
            Verdict::Refused(_) => None,
        }
    }

    pub fn is_refusal(&self) -> bool {
        matches!(self, Verdict::Refused(_))
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Ranked(s) => write!(f, "ranked it {s}"),
            Verdict::Refused(why) => write!(f, "refused it ({why})"),
        }
    }
}

/// Both in-process references' measured answers for one input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceVerdicts {
    pub rs_poker: Verdict,
    pub poker_crate: Verdict,
}

impl ReferenceVerdicts {
    /// True when NEITHER reference refused, i.e. the claim "the references reject
    /// this" is false and any finding must not make it.
    pub fn none_refused(&self) -> bool {
        !self.rs_poker.is_refusal() && !self.poker_crate.is_refusal()
    }

    pub fn any_refused(&self) -> bool {
        self.rs_poker.is_refusal() || self.poker_crate.is_refusal()
    }

    /// The line that goes into the report's `reference_says`. Measured, quoted.
    pub fn describe(&self) -> String {
        format!(
            "rs_poker {}; poker 0.7.0 {}",
            self.rs_poker, self.poker_crate
        )
    }
}

/// Run `f`, turning a panic into a [`Verdict::Refused`] and keeping the panic
/// message off the terminal so a passing run stays readable.
fn quietly<T, F: FnOnce() -> T + std::panic::UnwindSafe>(f: F) -> Result<T, String> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let out = std::panic::catch_unwind(f);
    std::panic::set_hook(previous);
    out.map_err(|e| {
        let msg = e
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| e.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panicked".to_string());
        format!("panicked: {}", msg.lines().next().unwrap_or("panicked").trim())
    })
}

/// Reference A, `rs_poker`: its native rank for any card slice, whatever length,
/// duplicates included.
pub fn ask_rs_poker(hand: &[CardIdx]) -> Verdict {
    let converted: Vec<rs_poker::core::Card> = hand.iter().map(|&c| to_rs_poker(c)).collect();
    match quietly(move || format!("{:?}", converted.rank())) {
        Ok(rendered) => Verdict::Ranked(rendered),
        Err(why) => Verdict::Refused(why),
    }
}

/// Reference B, `poker` 0.7.0: `evaluate_five`, which validates both the card count
/// and distinctness and returns an `Err` for either.
pub fn ask_poker_crate(hand: &[CardIdx]) -> Verdict {
    let converted: Vec<poker::Card> = hand.iter().map(|&c| to_poker_crate(c)).collect();
    match quietly(move || {
        Evaluator::new()
            .evaluate_five(converted.as_slice())
            .map(|e| format!("{:?}", e.classify()))
            .map_err(|e| e.to_string())
    }) {
        Ok(Ok(rendered)) => Verdict::Ranked(rendered),
        Ok(Err(why)) => Verdict::Refused(why),
        Err(why) => Verdict::Refused(why),
    }
}

pub fn ask_both(hand: &[CardIdx]) -> ReferenceVerdicts {
    ReferenceVerdicts {
        rs_poker: ask_rs_poker(hand),
        poker_crate: ask_poker_crate(hand),
    }
}

/// The optional third reference, `phevaluator`, asked one hand at a time.
///
/// Only consulted when `$CLEARDECK_PHE_PYTHON` is set (`make diff-full` sets it).
/// One process per hand, deliberately: the batch protocol aborts the whole chunk on
/// the first unsupported input, and unsupported inputs are the entire point here.
///
/// `phevaluator` scores 1 (best) .. 7462 (worst), so a score of `0` is OUT OF RANGE:
/// it is the sentinel the C tables return for an input they cannot represent, handed
/// back as if it were a rank rather than raised as an error. That is the third
/// lineage failing to validate its input too, and it is recorded as `Ranked`,
/// because that is exactly what it does.
pub fn ask_phevaluator(hand: &[CardIdx]) -> Option<Verdict> {
    let python = std::env::var(crate::adjudicator::PYTHON_ENV_VAR).ok()?;
    let rendered = slice_to_string(hand);
    Some(
        match crate::adjudicator::rank_hands(&python, std::slice::from_ref(&rendered)) {
            Ok(scores) => match scores.first().copied() {
                Some(0) => Verdict::Ranked(
                    "0 -- the OUT-OF-RANGE sentinel (valid ranks are 1..7462), returned as a \
                     rank rather than raised"
                        .to_string(),
                ),
                Some(s) => Verdict::Ranked(format!("rank {s} of 7462")),
                None => Verdict::Refused("returned no score".to_string()),
            },
            Err(why) => Verdict::Refused(why),
        },
    )
}

/// Appends the third reference's measured answer to a description, when available.
pub fn describe_with_third(hand: &[CardIdx], verdicts: &ReferenceVerdicts) -> String {
    match ask_phevaluator(hand) {
        Some(v) => format!("{}; phevaluator {}", verdicts.describe(), v),
        None => format!(
            "{}; phevaluator not consulted (CLEARDECK_PHE_PYTHON unset)",
            verdicts.describe()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::parse_hand;

    fn cards(s: &str) -> Vec<CardIdx> {
        parse_hand(s).expect("literal cards")
    }

    /// The measured facts docs/DEFECTS.md H-05 records, pinned here so nobody has
    /// to take them on trust again. Every one of these was previously asserted by a
    /// hard-coded string that said the opposite.
    #[test]
    fn no_reference_in_this_harness_validates_its_own_input() {
        // Five copies of the same card: rs_poker calls it a straight flush.
        let five_aces = ask_both(&cards("Ah Ah Ah Ah Ah"));
        assert_eq!(
            five_aces.rs_poker,
            Verdict::Ranked("StraightFlush(0)".to_string()),
            "rs_poker's measured answer for Ah Ah Ah Ah Ah changed"
        );
        assert!(
            five_aces.poker_crate.is_refusal(),
            "poker 0.7.0 must reject a duplicate: {}",
            five_aces.poker_crate
        );

        // Two pairs made of duplicated cards: rs_poker calls it quads.
        let dup_quads = ask_both(&cards("Kh Kh Kd Kd Qs"));
        assert_eq!(
            dup_quads.rs_poker,
            Verdict::Ranked("FourOfAKind(155)".to_string()),
            "rs_poker's measured answer for Kh Kh Kd Kd Qs changed"
        );
        assert!(dup_quads.poker_crate.is_refusal());

        // Two cards: rs_poker ranks them anyway.
        let two = ask_both(&cards("Ah Kd"));
        assert_eq!(
            two.rs_poker,
            Verdict::Ranked("HighCard(2140)".to_string()),
            "rs_poker's measured answer for the two-card hand Ah Kd changed"
        );
        assert!(
            two.poker_crate.is_refusal(),
            "poker 0.7.0 must reject a two-card hand: {}",
            two.poker_crate
        );

        // Seven cards with a duplicate: rs_poker ranks that too.
        let seven_dup = ask_both(&cards("Ah Ah Kh Qh Jh 2c 3d"));
        assert!(
            seven_dup.rs_poker.ranked().is_some(),
            "rs_poker must have ranked the seven-card duplicate: {}",
            seven_dup.rs_poker
        );
    }

    /// The measurement must still be able to say "ranked" for an ordinary hand, or
    /// the refusal signal above means nothing.
    #[test]
    fn a_legal_five_card_hand_is_ranked_by_both() {
        let v = ask_both(&cards("As Ks Qs Js Ts"));
        assert!(v.none_refused(), "a royal flush is a legal hand: {v:?}");
        assert!(v.rs_poker.ranked().unwrap().contains("StraightFlush"));
        assert!(v.poker_crate.ranked().unwrap().contains("StraightFlush"));
    }
}
