//! What was actually reached.
//!
//! Coverage here is derived from the OBSERVED hand, never from the scenario's
//! intent. A scenario called `three_way_allin_ladder` that in fact produced a
//! two-way pot must not be allowed to count as three-way coverage; a reader has to
//! be able to see the difference between what the harness aimed at and what the
//! canister did.

use std::collections::{BTreeMap, BTreeSet};

use crate::observe::HandComparison;
use crate::oracle::AwardReason;

/// The interesting properties of a settled hand, all read back off the record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Features {
    /// Seats that reached the end of the hand without folding or leaving.
    pub contenders: usize,
    /// Seats that put their entire starting stack in and did not fold.
    pub all_ins: usize,
    /// Distinct all-in depths among those seats. Two or more means a real ladder.
    pub distinct_all_in_depths: usize,
    /// Non-empty pot layers the oracle built.
    pub layers: usize,
    /// Layers with more than one winner.
    pub chopped_layers: usize,
    /// Largest number of seats sharing a single layer.
    pub max_chop: usize,
    /// Odd chips the oracle had to place.
    pub odd_chips: usize,
    /// A bet nobody covered was returned.
    pub uncalled_bet: bool,
    /// A seat that folded had contributed more than the shortest all-in: dead
    /// money sitting above a short stack's reach.
    pub dead_money_above_short_all_in: bool,
    /// Money went into the pot after the flop was dealt.
    pub post_flop_money: bool,
    /// A seat that started the hand was empty when it ended.
    pub vacated_seat: bool,
    /// The hand ended without a showdown.
    pub fold_out: bool,
    /// The engine and the oracle disagreed about at least one seat.
    pub disagreement: bool,
    /// Chips the hand collected and paid to nobody.
    pub destroyed: i128,
}

impl Features {
    pub fn of(c: &HandComparison) -> Self {
        let r = &c.record;
        let contenders = r
            .seats
            .iter()
            .filter(|s| !s.folded && !s.vacated)
            .count();

        let all_in_seats: Vec<&crate::observe::SeatTrace> = r
            .seats
            .iter()
            .filter(|s| !s.folded && !s.vacated && s.chips_before > 0 && s.contributed >= s.chips_before)
            .collect();
        let depths: BTreeSet<u64> = all_in_seats.iter().map(|s| s.contributed).collect();

        let shortest_all_in = depths.iter().next().copied();
        let dead_money_above_short_all_in = match shortest_all_in {
            Some(short) => r
                .seats
                .iter()
                .any(|s| (s.folded || s.vacated) && s.contributed > short),
            None => false,
        };

        let live_layers: Vec<&crate::oracle::PotLayer> =
            c.settlement.layers.iter().filter(|l| l.amount > 0).collect();
        let chopped_layers = live_layers.iter().filter(|l| l.winners.len() > 1).count();
        let max_chop = live_layers
            .iter()
            .map(|l| l.winners.len())
            .max()
            .unwrap_or(0);
        let odd_chips = c
            .settlement
            .awards
            .iter()
            .filter(|a| matches!(a.reason, AwardReason::OddChip { .. }))
            .count();

        // Money went in after the flop if the hand collected more than the seats had
        // put in at the moment a flop first appeared.
        //
        // This used to compare against `side_pots_first_seen`, the breakdown the
        // engine froze when the flop was dealt. That reading only worked while the
        // breakdown was frozen -- which was E-01. The engine now refreshes it after
        // every action, so a lagging observation would count as post-flop money and
        // the class would over-report.
        let collected = c.facts.total_collected();
        let post_flop_money = r
            .wagered_at_flop
            .map(|at_flop| collected > at_flop)
            .unwrap_or(false);

        Self {
            contenders,
            all_ins: all_in_seats.len(),
            distinct_all_in_depths: depths.len(),
            layers: live_layers.len(),
            chopped_layers,
            max_chop,
            odd_chips,
            uncalled_bet: c.settlement.uncalled.is_some(),
            dead_money_above_short_all_in,
            post_flop_money,
            vacated_seat: r.seats.iter().any(|s| s.vacated),
            fold_out: contenders <= 1,
            disagreement: !c.agrees,
            destroyed: c.destroyed,
        }
    }
}

/// Named scenario classes the task asks for a count of. Each is a predicate over
/// [`Features`], so the count is of hands that DEMONSTRABLY had the property.
pub const CLASSES: &[(&str, fn(&Features) -> bool)] = &[
    ("hands settled", |_| true),
    ("showdown reached", |f| f.contenders >= 2),
    ("3+ players to showdown", |f| f.contenders >= 3),
    ("4+ players to showdown", |f| f.contenders >= 4),
    ("2 simultaneous all-ins", |f| f.all_ins >= 2),
    ("3 simultaneous all-ins", |f| f.all_ins >= 3),
    ("4+ simultaneous all-ins", |f| f.all_ins >= 4),
    ("all-ins at 2+ different depths", |f| {
        f.distinct_all_in_depths >= 2
    }),
    ("all-ins at 3+ different depths", |f| {
        f.distinct_all_in_depths >= 3
    }),
    ("2+ pot layers (a real side pot)", |f| f.layers >= 2),
    ("3+ pot layers", |f| f.layers >= 3),
    ("exact tie (a pot chopped)", |f| f.chopped_layers >= 1),
    ("3-way chop", |f| f.max_chop >= 3),
    ("odd chip had to be placed", |f| f.odd_chips >= 1),
    ("uncalled bet returned", |f| f.uncalled_bet),
    ("folded money above a short all-in", |f| {
        f.dead_money_above_short_all_in
    }),
    ("money wagered after the flop", |f| f.post_flop_money),
    ("seat vacated mid-hand", |f| f.vacated_seat),
    ("fold-out, no showdown", |f| f.fold_out),
];

/// Counts per class, plus the disagreement tally.
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    pub per_class: BTreeMap<&'static str, usize>,
    pub hands: usize,
    pub disagreements: usize,
    pub anomalous: usize,
    pub destroyed_total: i128,
    pub misdirected_total: u64,
    /// Hands where `state.pot` was seen above the sum of every seat's
    /// `total_bet_this_hand`, i.e. where the engine's own two accounts of the pot
    /// disagreed and the oracle's input was therefore incomplete.
    pub input_suspect: usize,
}

impl Coverage {
    pub fn record(&mut self, c: &HandComparison) {
        let f = Features::of(c);
        self.hands += 1;
        for (name, pred) in CLASSES {
            if pred(&f) {
                *self.per_class.entry(name).or_insert(0) += 1;
            }
        }
        if !c.agrees {
            self.disagreements += 1;
            self.misdirected_total = self.misdirected_total.saturating_add(c.misdirected());
        }
        if !c.anomalies.is_empty() {
            self.anomalous += 1;
        }
        if c.engine_pot_exceeded_observed_contributions() {
            self.input_suspect += 1;
        }
        self.destroyed_total += c.destroyed;
    }

    pub fn report(&self) -> String {
        let mut out = String::new();
        out.push_str("\n=== SETTLEMENT ORACLE: what was actually reached ===\n");
        for (name, _) in CLASSES {
            let n = self.per_class.get(name).copied().unwrap_or(0);
            let bar = if n == 0 {
                "  NOT REACHED".to_string()
            } else {
                String::new()
            };
            out.push_str(&format!("  {n:>5}  {name}{bar}\n"));
        }
        out.push_str(&format!(
            "\n  {:>5}  hands where the engine and the oracle DISAGREED\n",
            self.disagreements
        ));
        out.push_str(&format!(
            "  {:>5}  hands the oracle flagged as not cleanly rule-defined (own bucket)\n",
            self.anomalous
        ));
        out.push_str(&format!(
            "  {:>5}  chips collected and paid to nobody, summed over all hands\n",
            self.destroyed_total
        ));
        out.push_str(&format!(
            "  {:>5}  chips paid to a seat that was not owed them, summed over all hands\n",
            self.misdirected_total
        ));
        out.push_str(&format!(
            "  {:>5}  hands where the engine's own two accounts of the pot disagreed, so\n\
             \x20        the oracle's INPUT was incomplete (see the WARNING lines above)\n",
            self.input_suspect
        ));
        out
    }

    /// Classes with a zero count. A reader should see these, because a coverage
    /// report that only lists what was hit is not a coverage report.
    pub fn unreached(&self) -> Vec<&'static str> {
        CLASSES
            .iter()
            .filter(|(name, _)| self.per_class.get(name).copied().unwrap_or(0) == 0)
            .map(|(name, _)| *name)
            .collect()
    }
}
