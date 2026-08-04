//! Side-pot construction.
//!
//! PORTED BUG-FOR-BUG out of `calculate_side_pots(state: &mut TableState)` in
//! `src/table_canister/src/lib.rs` (commit ceacc37). This wave establishes
//! ground truth, so NOTHING here is fixed: the reconciliation branches that can
//! inflate or shrink the pot relative to actual player contributions are kept
//! exactly as they were, including the `f64` proportional cap. See
//! `docs/SECURITY-FINDINGS.md` and `tests/golden_vectors.rs`.

use candid::{CandidType, Deserialize};

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct SidePot {
    pub amount: u64,
    pub eligible_players: Vec<u8>,
}

/// One player's stake in the hand being settled.
///
/// `seat` is the index of the player in `TableState::players`, which is what the
/// original routine used (it enumerated the seat vector rather than reading
/// `Player::seat`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contribution {
    pub seat: u8,
    pub total_bet_this_hand: u64,
    pub has_folded: bool,
}

impl Contribution {
    pub fn new(seat: u8, total_bet_this_hand: u64, has_folded: bool) -> Self {
        Self {
            seat,
            total_bet_this_hand,
            has_folded,
        }
    }
}

/// Build the side pots for a hand.
///
/// `contributions` must already exclude players who bet nothing this hand: the
/// original routine filtered on `total_bet_this_hand > 0` while collecting, and
/// that filter is part of the observable behaviour.
///
/// `total_pot` is the canister's `TableState::pot`.
pub fn build_side_pots(contributions: &[Contribution], total_pot: u64) -> Vec<SidePot> {
    build_side_pots_logged(contributions, total_pot).0
}

/// Same as [`build_side_pots`], but also returns the diagnostics the original
/// routine wrote with `ic_cdk::println!`, so the canister can still log them
/// without `poker_core` depending on `ic-cdk`.
pub fn build_side_pots_logged(
    contributions: &[Contribution],
    total_pot: u64,
) -> (Vec<SidePot>, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();
    let mut side_pots: Vec<SidePot> = Vec::new();

    // Get unique bet levels, sorted ascending
    let mut bet_levels: Vec<u64> = contributions
        .iter()
        .map(|c| c.total_bet_this_hand)
        .collect();
    bet_levels.sort();
    bet_levels.dedup();

    let mut processed_amount = 0u64;

    for level in bet_levels {
        let contribution_per_player = level.saturating_sub(processed_amount);

        if contribution_per_player == 0 {
            continue;
        }

        // Calculate pot amount from all players who contributed at least up to this level
        let pot_amount: u64 = contributions
            .iter()
            .filter(|c| c.total_bet_this_hand >= level)
            .map(|_| contribution_per_player)
            .fold(0u64, |acc, x| acc.saturating_add(x));

        // Add contributions from players who bet less than this level but more than processed
        let partial_contributions: u64 = contributions
            .iter()
            .filter(|c| c.total_bet_this_hand > processed_amount && c.total_bet_this_hand < level)
            .map(|c| c.total_bet_this_hand.saturating_sub(processed_amount))
            .fold(0u64, |acc, x| acc.saturating_add(x));

        let level_pot = pot_amount.saturating_add(partial_contributions);

        // Eligible players are only those who haven't folded and bet at least this level
        let eligible_players: Vec<u8> = contributions
            .iter()
            .filter(|c| !c.has_folded && c.total_bet_this_hand >= level)
            .map(|c| c.seat)
            .collect();

        if level_pot > 0 && !eligible_players.is_empty() {
            side_pots.push(SidePot {
                amount: level_pot,
                eligible_players,
            });
        } else if level_pot > 0 && eligible_players.is_empty() {
            // Edge case: all eligible players folded - money goes to last pot
            // If no last pot exists, we need to find any player still in the hand
            if let Some(last_pot) = side_pots.last_mut() {
                last_pot.amount = last_pot.amount.saturating_add(level_pot);
            } else {
                // No existing side pot - find any non-folded player to create a pot for
                let any_eligible: Vec<u8> = contributions
                    .iter()
                    .filter(|c| !c.has_folded)
                    .map(|c| c.seat)
                    .collect();
                if !any_eligible.is_empty() {
                    side_pots.push(SidePot {
                        amount: level_pot,
                        eligible_players: any_eligible,
                    });
                }
                // If truly no one is eligible (everyone folded), pot is dead - this shouldn't happen
            }
        }

        processed_amount = level;
    }

    // Verify total matches total_pot - if not, adjust last pot
    let total_side_pots: u64 = side_pots
        .iter()
        .map(|sp| sp.amount)
        .fold(0u64, |acc, x| acc.saturating_add(x));

    if total_side_pots < total_pot {
        // Add remaining pot to last pot (or first eligible pot)
        let remaining = total_pot.saturating_sub(total_side_pots);
        if let Some(last_pot) = side_pots.last_mut() {
            last_pot.amount = last_pot.amount.saturating_add(remaining);
        }
    } else if total_side_pots > total_pot {
        // SANITY CHECK: Side pots should never exceed total pot
        // This indicates a bug - log it and cap to prevent creating chips from nothing
        warnings.push(format!(
            "BUG: Side pots ({}) exceed total pot ({}). Capping to pot amount.",
            total_side_pots, total_pot
        ));
        // Proportionally reduce all side pots to match total pot
        if total_side_pots > 0 {
            let ratio = total_pot as f64 / total_side_pots as f64;
            let mut distributed: u64 = 0;
            let pot_count = side_pots.len();
            for (i, side_pot) in side_pots.iter_mut().enumerate() {
                if i == pot_count - 1 {
                    // Last pot gets remainder to avoid rounding errors
                    side_pot.amount = total_pot.saturating_sub(distributed);
                } else {
                    let adjusted = (side_pot.amount as f64 * ratio) as u64;
                    side_pot.amount = adjusted;
                    distributed = distributed.saturating_add(adjusted);
                }
            }
        }
    }

    (side_pots, warnings)
}

/// Replace `existing` with freshly-built side pots.
///
/// This is the exact contract the canister needs, including one subtlety that is
/// easy to lose in a refactor: when nobody has bet anything this hand the
/// original routine returned EARLY, **without** clearing the previous hand's side
/// pots. That is reproduced here.
///
/// Returns the diagnostics the caller should log.
pub fn apply_side_pots(
    existing: &mut Vec<SidePot>,
    contributions: &[Contribution],
    total_pot: u64,
) -> Vec<String> {
    if contributions.is_empty() {
        return Vec::new();
    }

    let (pots, warnings) = build_side_pots_logged(contributions, total_pot);
    *existing = pots;
    warnings
}
