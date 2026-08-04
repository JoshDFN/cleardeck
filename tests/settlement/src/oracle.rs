//! THE SETTLEMENT ORACLE.
//!
//! Given the facts of a finished hand -- who put how much in, who gave up their
//! claim, who holds which two cards, what the board is, where the button is --
//! this module derives **what each seat is owed** from the rules of poker.
//!
//! # Why this file exists
//!
//! Wave 1 proved two things and left the gap between them uncovered:
//!
//! * `poker_core`'s hand ranking is correct over all 2,598,960 five-card hands.
//! * chips are conserved: nothing is minted.
//!
//! Neither of those says the RIGHT SEAT GOT PAID. A payout that hands the main
//! pot to the worst hand at the table conserves every total and satisfies every
//! M1..M6 money invariant. `determine_winners` -- per-side-pot eligibility, tie
//! detection, chopping, odd-chip assignment -- is the code that decides that, and
//! it is covered by nothing.
//!
//! # Independence: the rule this file lives by
//!
//! **Nothing here is derived from `determine_winners`, `calculate_side_pots`, or
//! `poker_core::side_pots`.** A port of the engine's logic would be a mirror, not
//! an oracle: it would agree with every bug. The construction below is written
//! from the rules of the game:
//!
//! 1. an uncalled bet is returned to the player who made it (it was never in
//!    play, because nobody covered it);
//! 2. what remains is split into layered pots, one per distinct all-in depth;
//! 3. a player is eligible for a layer only if they covered it in full;
//! 4. among the eligible players who did not give up their claim, the best
//!    five-card hand wins the layer; equal hands chop it;
//! 5. chips that do not divide are given out one at a time, clockwise from the
//!    button.
//!
//! The ONE thing borrowed from the engine's own crate is
//! [`poker_core::evaluate_hand`], for step 4's "best five-card hand". That is
//! deliberate and safe: it is the part wave 1 exhaustively proved, and a critic
//! could not construct a `poker_core` ranking bug the differential harness failed
//! to convict. Ranking is settled. Payment is not.
//!
//! # The odd-chip rule, stated and justified
//!
//! When a pot does not divide evenly among the players chopping it, the leftover
//! chips are awarded **one each, walking clockwise from the button** -- so the
//! first eligible winner to the button's left gets the first odd chip, the next
//! one gets the second, and so on.
//!
//! That is what real rooms do. Robert's Rules of Poker (high-hand split, flop
//! games) gives the odd chip to the first player clockwise from the button, and
//! the TDA rules put odd chips with the player(s) in the **earliest position**,
//! distributed one at a time when there is more than one. The rule matters here
//! because ClearDeck's chips are e8s: a three-way chop of a pot that is not a
//! multiple of three leaves real, withdrawable chips to assign.
//!
//! Note for the reader comparing against the engine: the engine gives the WHOLE
//! remainder to a single seat (`pot_share + remainder`). For a two-way chop the
//! remainder is at most one chip and the two rules coincide. For a three-way chop
//! the remainder can be two chips, and then they differ by one chip. The oracle
//! reports it; it does not paper over it.
//!
//! # Impossible inputs
//!
//! Real betting cannot produce a pot layer that nobody is eligible for, or a
//! showdown contender with no cards. Engine defects can. Rather than guess, the
//! oracle picks the least-arbitrary answer (a layer nobody can win is returned to
//! the players who put it there) and records an [`Anomaly`] so the harness can
//! report those hands in their own bucket instead of quietly counting them as
//! payout defects.

use poker_core::{evaluate_hand, Card, HandRank};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// inputs
// ---------------------------------------------------------------------------

/// One seat's stake in the hand being settled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeatStake {
    /// Index into the table's seat vector.
    pub seat: u8,
    /// Every chip this seat put into the pot over the whole hand: blinds, antes,
    /// bets, calls and raises on every street.
    pub contributed: u64,
    /// True if this seat has given up its claim on the pot. Folding does that.
    /// So does vacating the seat mid-hand: a player who is no longer at the table
    /// cannot win a pot, and the money they had already committed stays in it.
    pub relinquished: bool,
    /// The two cards this seat holds. `None` for a seat that never got cards, or
    /// whose cards are unknown because it left the table.
    pub hole: Option<(Card, Card)>,
}

impl SeatStake {
    pub fn contender(seat: u8, contributed: u64, hole: (Card, Card)) -> Self {
        Self {
            seat,
            contributed,
            relinquished: false,
            hole: Some(hole),
        }
    }

    pub fn folded(seat: u8, contributed: u64) -> Self {
        Self {
            seat,
            contributed,
            relinquished: true,
            hole: None,
        }
    }
}

/// Everything the oracle needs about a finished hand. Nothing else.
///
/// Deliberately absent: `state.pot`, `state.side_pots`, the engine's winner list,
/// and anything else the engine computed. The oracle is handed the FACTS and
/// works out the answer itself.
#[derive(Clone, Debug)]
pub struct HandFacts {
    /// Length of the table's seat vector, i.e. the modulus for "clockwise".
    pub num_seats: usize,
    /// The button.
    pub dealer_seat: u8,
    /// The community cards. Five of them in any hand that reached a showdown.
    pub board: Vec<Card>,
    /// One entry per seat that had money in the pot. Seats that contributed
    /// nothing may be omitted; including them with `contributed: 0` is harmless.
    pub stakes: Vec<SeatStake>,
}

impl HandFacts {
    pub fn total_collected(&self) -> u64 {
        self.stakes
            .iter()
            .fold(0u64, |a, s| a.saturating_add(s.contributed))
    }

    fn stake(&self, seat: u8) -> Option<&SeatStake> {
        self.stakes.iter().find(|s| s.seat == seat)
    }
}

// ---------------------------------------------------------------------------
// outputs
// ---------------------------------------------------------------------------

/// Why a seat is owed a particular amount. Carried so a disagreement report can
/// say *which* pot was misassigned, not just that the total was wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AwardReason {
    /// A bet no other player covered. It was never in play; it goes back.
    UncalledBet,
    /// A share of pot layer `layer`, won at showdown.
    PotShare { layer: usize },
    /// An odd chip from pot layer `layer` that did not divide.
    OddChip { layer: usize },
    /// A layer no remaining player could win, returned to the seats that put the
    /// money there. See the module docs: real betting cannot produce this.
    OrphanReturn { layer: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Award {
    pub seat: u8,
    pub amount: u64,
    pub reason: AwardReason,
}

/// One layered pot: the slice of everybody's stake between two all-in depths.
#[derive(Clone, Debug)]
pub struct PotLayer {
    pub index: usize,
    /// Exclusive lower bound of the layer, in chips-per-player.
    pub lo: u64,
    /// Inclusive upper bound of the layer, in chips-per-player.
    pub hi: u64,
    /// Total chips in this layer.
    pub amount: u64,
    /// Every seat that put money into this layer, including seats that folded.
    pub contributors: Vec<u8>,
    /// Seats that covered the layer in full AND still have a claim on the pot.
    pub eligible: Vec<u8>,
    /// The eligible seats holding the best hand. Empty only for an orphan layer.
    pub winners: Vec<u8>,
    pub best: Option<HandRank>,
}

/// An input the rules of poker do not define an answer for. Always a symptom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Anomaly {
    /// A pot layer no remaining player is eligible for.
    OrphanLayer { layer: usize, amount: u64 },
    /// A seat still contesting the pot but holding no cards.
    ContenderWithoutCards { seat: u8 },
    /// A seat still contesting the pot whose seven cards cannot be ranked:
    /// duplicates, or a board that is not three to five cards.
    UnrankableHand { seat: u8, why: String },
    /// Two or more seats went to a showdown on a board that is not five cards.
    ShortBoardAtShowdown { board_len: usize, contenders: usize },
    /// Money in the pot and nobody left with a claim on it at all.
    NoClaimants { amount: u64 },
}

/// What the oracle says the hand owes.
#[derive(Clone, Debug)]
pub struct Settlement {
    /// Chips each seat is owed OUT of the pot. This is gross, not net: a seat
    /// that put 100 in and wins a 300 pot appears here as 300.
    pub owed: BTreeMap<u8, u64>,
    pub awards: Vec<Award>,
    pub layers: Vec<PotLayer>,
    /// `(seat, amount)` of the returned uncalled bet, if there was one.
    pub uncalled: Option<(u8, u64)>,
    pub total_collected: u64,
    pub anomalies: Vec<Anomaly>,
}

impl Settlement {
    pub fn owed_to(&self, seat: u8) -> u64 {
        self.owed.get(&seat).copied().unwrap_or(0)
    }

    pub fn total_owed(&self) -> u64 {
        self.owed.values().fold(0u64, |a, v| a.saturating_add(*v))
    }

    /// The NET change the oracle says each seat's stack should show across the
    /// hand: what it is owed out of the pot minus what it put in. This is the
    /// quantity that is directly comparable to an observed chip delta.
    pub fn net_deltas(&self, facts: &HandFacts) -> BTreeMap<u8, i128> {
        let mut out: BTreeMap<u8, i128> = BTreeMap::new();
        for s in &facts.stakes {
            out.insert(s.seat, self.owed_to(s.seat) as i128 - s.contributed as i128);
        }
        for (seat, owed) in &self.owed {
            out.entry(*seat).or_insert(*owed as i128);
        }
        out
    }

    pub fn is_clean(&self) -> bool {
        self.anomalies.is_empty()
    }
}

// ---------------------------------------------------------------------------
// the oracle
// ---------------------------------------------------------------------------

/// Derive what each seat is owed.
///
/// Panics only if its own arithmetic fails to conserve, which would be a bug in
/// the oracle itself. That self-check is the point: an instrument that silently
/// loses chips cannot convict an engine of losing chips.
pub fn settle(facts: &HandFacts) -> Settlement {
    let total_collected = facts.total_collected();
    let mut anomalies: Vec<Anomaly> = Vec::new();
    let mut awards: Vec<Award> = Vec::new();

    // --- step 1: return the uncalled bet -----------------------------------
    //
    // A player only ever risks what somebody else is willing to match. If one
    // seat put in strictly more than every other seat, the excess was never
    // contested and comes straight back. This is the rule that makes "bet 300,
    // everybody folds" return 200 of a 300 bet against a 100 caller, and the rule
    // that returns the over-bet part of a big all-in against a short stack.
    let mut effective: BTreeMap<u8, u64> = facts
        .stakes
        .iter()
        .filter(|s| s.contributed > 0)
        .map(|s| (s.seat, s.contributed))
        .collect();

    let uncalled = uncalled_excess(&effective);
    if let Some((seat, excess)) = uncalled {
        if let Some(e) = effective.get_mut(&seat) {
            *e = e.saturating_sub(excess);
        }
        awards.push(Award {
            seat,
            amount: excess,
            reason: AwardReason::UncalledBet,
        });
    }

    // --- step 2: who still has a claim -------------------------------------
    //
    // Every seat that neither folded nor left. Note that ranking is NOT done here:
    // the rules only need a hand shown when two or more players still want the
    // pot. A player left alone by everybody else's folds wins without showing, and
    // in that case there is usually no five-card board to rank against anyway.
    let live: BTreeSet<u8> = facts
        .stakes
        .iter()
        .filter(|s| !s.relinquished)
        .map(|s| s.seat)
        .collect();

    let mut ranks: BTreeMap<u8, HandRank> = BTreeMap::new();
    if live.len() >= 2 {
        // A real showdown. Now every claimant has to be rankable.
        for seat in &live {
            let stake = facts.stake(*seat).expect("live seat has a stake");
            match stake.hole {
                Some(hole) => match rank_seven(&hole, &facts.board) {
                    Ok(rank) => {
                        ranks.insert(*seat, rank);
                    }
                    Err(why) => anomalies.push(Anomaly::UnrankableHand { seat: *seat, why }),
                },
                None => anomalies.push(Anomaly::ContenderWithoutCards { seat: *seat }),
            }
        }
        if facts.board.len() != 5 {
            anomalies.push(Anomaly::ShortBoardAtShowdown {
                board_len: facts.board.len(),
                contenders: live.len(),
            });
        }
    }

    // --- step 3: layer the remaining money by all-in depth ------------------
    let mut levels: Vec<u64> = effective.values().copied().filter(|v| *v > 0).collect();
    levels.sort_unstable();
    levels.dedup();

    let mut layers: Vec<PotLayer> = Vec::with_capacity(levels.len());
    let mut lo = 0u64;
    for (index, hi) in levels.iter().copied().enumerate() {
        // Every seat's slice of THIS layer: the part of its stake that sits
        // between `lo` and `hi`.
        let mut amount = 0u64;
        let mut contributors: Vec<u8> = Vec::new();
        for (seat, eff) in &effective {
            let slice = eff.min(&hi).saturating_sub(lo);
            if slice > 0 {
                amount = amount.saturating_add(slice);
                contributors.push(*seat);
            }
        }
        // Eligible: covered the layer in full and still has a claim.
        let eligible: Vec<u8> = effective
            .iter()
            .filter(|(seat, eff)| {
                **eff >= hi
                    && facts
                        .stake(**seat)
                        .map(|s| !s.relinquished)
                        .unwrap_or(false)
            })
            .map(|(seat, _)| *seat)
            .collect();

        layers.push(PotLayer {
            index,
            lo,
            hi,
            amount,
            contributors,
            eligible,
            winners: Vec::new(),
            best: None,
        });
        lo = hi;
    }

    // --- step 4: award each layer ------------------------------------------
    for layer in layers.iter_mut() {
        if layer.amount == 0 {
            continue;
        }
        let mut in_the_running: Vec<u8> = layer
            .eligible
            .iter()
            .copied()
            .filter(|s| live.contains(s))
            .collect();
        in_the_running.sort_unstable();

        // A single claimant takes the layer without showing a hand: everybody else
        // gave up their claim. This is the rule that pays the last player standing
        // when a hand ends before any board is dealt.
        if in_the_running.len() >= 2 {
            in_the_running.retain(|s| ranks.contains_key(s));
        }

        if in_the_running.is_empty() {
            // Nobody can win this money. Real betting cannot get here (the top
            // layer always has at least two seats in it after the uncalled bet
            // is returned, and a seat only folds facing a bet it has not
            // matched). Return it to the seats that put it there and shout.
            anomalies.push(Anomaly::OrphanLayer {
                layer: layer.index,
                amount: layer.amount,
            });
            if layer.contributors.is_empty() {
                anomalies.push(Anomaly::NoClaimants {
                    amount: layer.amount,
                });
                continue;
            }
            let split = split_evenly(
                layer.amount,
                &layer.contributors,
                facts.dealer_seat,
                facts.num_seats,
            );
            for (seat, amount, _is_odd_chip) in split {
                awards.push(Award {
                    seat,
                    amount,
                    reason: AwardReason::OrphanReturn { layer: layer.index },
                });
            }
            continue;
        }

        let (winners, best) = if in_the_running.len() == 1 {
            (in_the_running.clone(), ranks.get(&in_the_running[0]).cloned())
        } else {
            let best = in_the_running
                .iter()
                .map(|s| ranks.get(s).expect("multi-way claimants are ranked").clone())
                .max()
                .expect("non-empty");
            let winners: Vec<u8> = in_the_running
                .iter()
                .copied()
                .filter(|s| ranks.get(s) == Some(&best))
                .collect();
            (winners, Some(best))
        };

        for (seat, amount, is_odd_chip) in
            split_evenly(layer.amount, &winners, facts.dealer_seat, facts.num_seats)
        {
            awards.push(Award {
                seat,
                amount,
                reason: if is_odd_chip {
                    AwardReason::OddChip { layer: layer.index }
                } else {
                    AwardReason::PotShare { layer: layer.index }
                },
            });
        }

        layer.winners = winners;
        layer.best = best;
    }

    // --- step 5: fold the awards into per-seat totals -----------------------
    let mut owed: BTreeMap<u8, u64> = BTreeMap::new();
    for a in &awards {
        if a.amount == 0 {
            continue;
        }
        *owed.entry(a.seat).or_insert(0) = owed
            .get(&a.seat)
            .copied()
            .unwrap_or(0)
            .saturating_add(a.amount);
    }

    let settlement = Settlement {
        owed,
        awards,
        layers,
        uncalled,
        total_collected,
        anomalies,
    };

    // The oracle's own conservation check. Every chip collected is owed to
    // somebody: won, chopped, returned as an uncalled bet, or returned out of an
    // orphan layer. If this ever fires, the instrument is broken, not the engine.
    let owed_total = settlement.total_owed();
    assert_eq!(
        owed_total, total_collected,
        "ORACLE BUG: it owes {owed_total} out of {total_collected} collected. \
         facts={facts:?} settlement={settlement:?}"
    );

    settlement
}

// ---------------------------------------------------------------------------
// the two rules that need their own function
// ---------------------------------------------------------------------------

/// Rank two hole cards against a board, validating the input FIRST.
///
/// Two reasons this is not a bare `evaluate_hand` call:
///
/// * The oracle must never panic on data it is measuring. `poker_core`'s
///   `evaluate_hand` traps on an invalid seven-card set (that is the E-09 fix),
///   and an instrument that aborts the run cannot report a finding.
/// * The validation is the oracle's own, so the oracle's notion of "these are
///   not 7 real distinct cards" cannot be softened by a change on the engine
///   side. It also keeps this crate compiling against `poker_core` before and
///   after the E-09 fix lands, since only `evaluate_hand` is used.
fn rank_seven(hole: &(Card, Card), board: &[Card]) -> Result<HandRank, String> {
    if !(3..=5).contains(&board.len()) {
        return Err(format!(
            "a hand cannot be ranked against a {}-card board",
            board.len()
        ));
    }
    let mut seen: BTreeSet<(u8, u8)> = BTreeSet::new();
    for c in [hole.0, hole.1].iter().chain(board.iter()) {
        let key = (c.suit as u8, c.rank as u8);
        if !seen.insert(key) {
            return Err(format!("duplicate card {:?} in the seven-card set", c));
        }
    }
    Ok(evaluate_hand(hole, board))
}

/// The uncalled part of the largest bet.
///
/// If exactly one seat put in strictly more than every other seat, the difference
/// between it and the next-largest stake was never covered by anybody and is
/// returned. If two or more seats tie for the largest stake, everything was
/// matched and nothing comes back.
///
/// Note this does NOT care whether the big bettor folded. Legal betting cannot
/// make the unique largest contributor fold -- to fold you must be facing a bet
/// you have not matched, which would make somebody else the largest -- but if the
/// engine produces that state anyway, the money still was not covered, so it
/// still goes back.
fn uncalled_excess(effective: &BTreeMap<u8, u64>) -> Option<(u8, u64)> {
    if effective.is_empty() {
        return None;
    }
    let mut by_amount: Vec<(u8, u64)> = effective.iter().map(|(s, v)| (*s, *v)).collect();
    // Sort descending by amount; seat order breaks ties deterministically.
    by_amount.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let (top_seat, top) = by_amount[0];
    let second = by_amount.get(1).map(|(_, v)| *v).unwrap_or(0);
    if top > second {
        Some((top_seat, top - second))
    } else {
        None
    }
}

/// Split `amount` among `seats`, giving the chips that do not divide out one at a
/// time clockwise from the button.
///
/// Returns `(seat, amount, is_odd_chip)` entries. Odd chips come back as their
/// own one-chip entries so a report can point at exactly which chip went where.
fn split_evenly(
    amount: u64,
    seats: &[u8],
    dealer_seat: u8,
    num_seats: usize,
) -> Vec<(u8, u64, bool)> {
    let mut out: Vec<(u8, u64, bool)> = Vec::new();
    if seats.is_empty() {
        return out;
    }
    let n = seats.len() as u64;
    let share = amount / n;
    let remainder = amount % n;

    for seat in seats {
        out.push((*seat, share, false));
    }
    // One odd chip each, starting with the first of them clockwise from the
    // button. `remainder < n`, so nobody gets two.
    for seat in clockwise_from_button(dealer_seat, num_seats, seats)
        .into_iter()
        .take(remainder as usize)
    {
        out.push((seat, 1, true));
    }
    out
}

/// `seats`, reordered starting from the first seat clockwise of the button.
///
/// Clockwise from the button is the small-blind side of the table, i.e. the
/// earliest position, which is where the odd chip goes.
pub fn clockwise_from_button(dealer_seat: u8, num_seats: usize, seats: &[u8]) -> Vec<u8> {
    let modulus = num_seats.max(1) as u64;
    let dealer = dealer_seat as u64 % modulus;
    let mut ordered: Vec<u8> = seats.to_vec();
    ordered.sort_by_key(|s| {
        // distance walking clockwise from the seat AFTER the button
        let s = *s as u64 % modulus;
        (s + modulus - dealer - 1) % modulus
    });
    ordered
}
