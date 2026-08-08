//! The dealer's private state, its frozen configuration, and the arithmetic that
//! decides which card is whose.
//!
//! Split out of `lib.rs` so that file is only the CANISTER SURFACE: what this
//! canister will do when asked, and by whom. Everything that decides *what a card
//! is* lives here, and nothing here is reachable from outside the crate.
//!
//! The one thing to notice is what the [`Hand`] struct does NOT contain: a deck.
//! It holds 32 bytes of seed and re-derives, so there is no deck-shaped object in
//! this canister's heap for anybody to find, and no field for a future refactor to
//! accidentally expose.

use candid::Principal;
use poker_core::{create_deck, shuffle_deck, Card};
use std::cell::RefCell;
use std::collections::BTreeMap;

pub use dealer_types::{
    community_indices, next_street, AckState, DealerHealth, DealerIdentity,
    DealerInit, DealtInSeat, FinalizeKind, ForcedFinalize, HandOpened, HandPublic, Readiness,
    RevealRecord, SeatCards, SeatReadiness, Street, MAX_SEATS,
};

// ---------------------------------------------------------------------------
// constants — all frozen at install, because nothing here can ever be changed
// ---------------------------------------------------------------------------

/// Default action clock, in nanoseconds. Matches `DEFAULT_ACTION_TIMEOUT_SECS` in
/// the live table canister, deliberately: this canister's timeout must never be
/// *shorter* than the table's, or the table could stand a seat down here while
/// that seat still had time to act there.
pub const DEFAULT_ACTION_TIMEOUT_NS: u64 = 60 * 1_000_000_000;

/// The SECOND clock, and the design would be broken without it.
///
/// The readiness gate would be a griefing weapon if the only way past it were a
/// seat going silent: a player who keeps heartbeating but never says it is ready
/// would freeze the hand until [`force_finalize`] an hour later, and one such
/// player could freeze a table indefinitely by rejoining. So the table may also
/// stand a seat down once the STREET has been open for this long, whatever that
/// seat is doing.
///
/// The number is a trade, stated so it can be argued with. It is the table's own
/// `STUCK_HAND_GRACE_NS` (300 s), which is:
///
/// * **five times** the action clock, so it can never fire during ordinary play;
/// * far outside the timescale of a real hand, so a table using it to reveal a
///   street early has to sit visibly still for five minutes first, with every
///   player's client watching `ack_status`;
/// * short enough that a griefer costs a table minutes rather than an hour.
///
/// A reveal that used it is recorded as `Readiness::GraceExpired`, a different
/// variant from `TimedOutByTable`, so the two are told apart on the public record.
pub const DEFAULT_STREET_GRACE_NS: u64 = 300 * 1_000_000_000;

/// How old a hand must be before ANY principal may force it open.
///
/// This is the last-resort door, and its only safety property is the clock, so the
/// number has to be far beyond any hand that could still be live. The table's own
/// `STUCK_HAND_GRACE_NS` is 300 s and its abandon door opens after that; one hour
/// is twelve times that and beyond the worst case of `MAX_SEATS` players each
/// burning a 60 s clock plus a 30 s time bank on four streets.
pub const DEFAULT_FORCE_FINALIZE_AFTER_NS: u64 = 3_600 * 1_000_000_000;

/// The dealer refuses to OPEN a hand it might not be able to FINISH.
///
/// `docs/NO-PEEKING-FEASIBILITY.md` §8 left this open: "what happens to a hand in
/// flight if the dealer freezes, which is unrecoverable in a way the table's freeze
/// is not, because `update_settings` on a zero-controller canister is refused
/// forever and the freezing threshold can never be raised after handover."
///
/// The answer is to make it unreachable rather than to recover from it. A hand
/// costs a bounded number of messages, so a balance floor checked at
/// [`open_hand`] means the dealer can only ever run dry BETWEEN hands, where the
/// consequence is "no new hands start", not "a live pot is unopenable". 2 T cycles
/// is roughly 400,000 update messages at the 5 M base rate — four orders of
/// magnitude more than one hand needs.
pub const DEFAULT_MIN_OPEN_BALANCE: u128 = 2_000_000_000_000;


// ---------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------

/// A live hand.
///
/// `seed` is the only secret in this canister and it is 32 bytes. Everything else
/// in poker follows from it deterministically, which is why no design that leaves
/// the seed inside the fund-holding table can be called no-peeking whatever else
/// it does to `get_table_state`.
pub struct Hand {
    pub hand_id: u64,
    pub seed: Vec<u8>,
    pub seed_hash: String,
    pub dealt_in: Vec<DealtInSeat>,
    pub street: Street,
    pub opened_at_ns: u64,
    pub street_opened_at_ns: u64,
    /// Seat -> how it became ready ON THE CURRENT STREET. `StoodDownSelf` and
    /// `TimedOutByTable` persist across streets; `Acked` is cleared when a street
    /// opens.
    pub ready: BTreeMap<u8, Readiness>,
    /// Last time each seat proved it was alive, by this canister's clock. Only
    /// [`ack`] writes it, and only the seat's own principal can call [`ack`].
    pub last_ack_ns: BTreeMap<u8, u64>,
    pub reveals: Vec<RevealRecord>,
    pub showdown: Vec<SeatCards>,
    pub revealed_seed: Option<String>,
    pub finalized_by: Option<FinalizeKind>,
}

pub struct Config {
    pub table: Principal,
    pub action_timeout_ns: u64,
    pub street_grace_ns: u64,
    pub force_finalize_after_ns: u64,
    pub min_open_balance: u128,
}

thread_local! {
    pub static CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
    pub static HANDS: RefCell<BTreeMap<u64, Hand>> = const { RefCell::new(BTreeMap::new()) };
    pub static NEXT_HAND_ID: RefCell<u64> = const { RefCell::new(1) };
}

/// Changed on purpose whenever this file is rebuilt for an experiment, so a test
/// can prove which build answered it. The live engine's equivalent trick is how
/// `docs/NO-PEEKING-FEASIBILITY.md` §3 proved a genuine upgrade had happened
/// between two vetKD derivations.
pub const BUILD: &str = "cleardeck-sealed-dealer-spike-1";

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

pub fn config() -> Result<Config, String> {
    CONFIG.with(|c| {
        c.borrow()
            .as_ref()
            .map(|c| Config {
                table: c.table,
                action_timeout_ns: c.action_timeout_ns,
                street_grace_ns: c.street_grace_ns,
                force_finalize_after_ns: c.force_finalize_after_ns,
                min_open_balance: c.min_open_balance,
            })
            .ok_or_else(|| "dealer not initialised".to_string())
    })
}

pub fn require_table(caller: Principal) -> Result<Config, String> {
    let cfg = config()?;
    if caller != cfg.table {
        return Err(format!(
            "only the registered table may call this; caller {caller} is not {}",
            cfg.table
        ));
    }
    Ok(cfg)
}

/// The seat a principal holds in a hand, or an error naming nothing it did not
/// already know.
pub fn seat_of(hand: &Hand, who: Principal) -> Option<u8> {
    hand.dealt_in
        .iter()
        .find(|d| d.principal == who)
        .map(|d| d.seat)
}

/// Re-derive the deck from the seed. Deliberately a LOCAL, transient value: this
/// canister never stores a deck, so there is no deck-shaped object in its heap to
/// find, and the one function that can produce one is private to this file.
pub fn deck_of(hand: &Hand) -> Vec<Card> {
    let mut deck = create_deck();
    shuffle_deck(&mut deck, &hand.seed);
    deck
}

pub fn community_now(hand: &Hand) -> Vec<Card> {
    let deck = deck_of(hand);
    community_indices(hand.dealt_in.len(), hand.street)
        .into_iter()
        .filter_map(|i| deck.get(i).copied())
        .collect()
}

pub fn hole_of(hand: &Hand, seat: u8) -> Option<(Card, Card)> {
    let k = hand.dealt_in.iter().position(|d| d.seat == seat)?;
    let deck = deck_of(hand);
    Some((*deck.get(2 * k)?, *deck.get(2 * k + 1)?))
}

pub fn readiness_vec(hand: &Hand) -> Vec<SeatReadiness> {
    hand.dealt_in
        .iter()
        .filter_map(|d| {
            hand.ready.get(&d.seat).map(|how| SeatReadiness {
                seat: d.seat,
                how: *how,
            })
        })
        .collect()
}

pub fn waiting_on(hand: &Hand) -> Vec<u8> {
    hand.dealt_in
        .iter()
        .filter(|d| !hand.ready.contains_key(&d.seat))
        .map(|d| d.seat)
        .collect()
}

pub fn ack_state(hand: &Hand) -> AckState {
    let waiting = waiting_on(hand);
    AckState {
        street: hand.street,
        ready: readiness_vec(hand),
        can_advance: waiting.is_empty() && next_street(hand.street).is_some(),
        waiting_on: waiting,
    }
}

/// Which kinds of readiness survive a street change.
///
/// `StoodDownSelf` and `TimedOutByTable` do: a seat that folded, or that has gone
/// silent, is not coming back to decide anything, and re-asking every street would
/// stall the hand for a player who is not there.
///
/// `Acked` and **`GraceExpired`** do NOT. `Acked` is obvious. `GraceExpired` is the
/// load-bearing one: it is the escape hatch for a present-but-unready seat, and if
/// it persisted, a table that burned the grace period ONCE would own that seat for
/// the rest of the hand and could then run the remaining streets out instantly.
/// Clearing it means a table stalling on purpose has to sit visibly still for the
/// whole grace period on EVERY street, which is the property that makes the hatch
/// safe to have.
pub fn survives_a_street(how: Readiness) -> bool {
    matches!(
        how,
        Readiness::StoodDownSelf | Readiness::TimedOutByTable
    )
}

pub fn open_street(hand: &mut Hand, street: Street, now: u64) {
    hand.street = street;
    hand.street_opened_at_ns = now;
    hand.ready.retain(|_, how| survives_a_street(*how));
}

pub fn live_hand_id() -> Option<u64> {
    HANDS.with(|h| {
        h.borrow()
            .values()
            .find(|hand| hand.finalized_by.is_none())
            .map(|hand| hand.hand_id)
    })
}

// ---------------------------------------------------------------------------
// host-side unit tests: the arithmetic that decides which card is whose
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn community_indices_match_the_shuffle_spec_table() {
        // docs/SHUFFLE-SPEC.md §4: flop = 2P+1..2P+3, turn = 2P+5, river = 2P+7.
        // Heads-up (P=2) is written out in the spec: hole cards deck[0..4], flop
        // deck[5..8], turn deck[9], river deck[11].
        assert_eq!(community_indices(2, Street::Flop), vec![5, 6, 7]);
        assert_eq!(community_indices(2, Street::Turn), vec![5, 6, 7, 9]);
        assert_eq!(community_indices(2, Street::River), vec![5, 6, 7, 9, 11]);
        // Six-handed.
        assert_eq!(community_indices(6, Street::River), vec![13, 14, 15, 17, 19]);
        assert_eq!(community_indices(0, Street::Dealt), Vec::<usize>::new());
    }

    #[test]
    fn a_full_table_still_fits_in_fifty_two_cards() {
        let last = *community_indices(MAX_SEATS, Street::River).last().unwrap();
        assert!(last < 52, "2P+7 must stay inside the deck for P={MAX_SEATS}");
    }

    #[test]
    fn streets_only_go_forwards() {
        assert_eq!(next_street(Street::Dealt), Some(Street::Flop));
        assert_eq!(next_street(Street::Flop), Some(Street::Turn));
        assert_eq!(next_street(Street::Turn), Some(Street::River));
        assert_eq!(next_street(Street::River), None);
        assert_eq!(next_street(Street::Complete), None);
    }

    fn hand_with(p: usize) -> Hand {
        Hand {
            hand_id: 1,
            seed: b"cleardeck-sealed-dealer-test-seed".to_vec(),
            seed_hash: String::new(),
            dealt_in: (0..p as u8)
                .map(|s| DealtInSeat {
                    seat: s,
                    principal: Principal::self_authenticating(format!("seat-{s}").as_bytes()),
                })
                .collect(),
            street: Street::Dealt,
            opened_at_ns: 0,
            street_opened_at_ns: 0,
            ready: BTreeMap::new(),
            last_ack_ns: BTreeMap::new(),
            reveals: Vec::new(),
            showdown: Vec::new(),
            revealed_seed: None,
            finalized_by: None,
        }
    }

    /// The whole hand must be a set of DISTINCT cards drawn from the one shuffled
    /// deck: no seat may be dealt a card that is also on the board or in another
    /// seat's hand. This is the property a re-derived deck could silently break.
    #[test]
    fn no_card_is_ever_dealt_twice() {
        for p in 2..=MAX_SEATS {
            let mut hand = hand_with(p);
            hand.street = Street::River;
            let mut seen: Vec<Card> = Vec::new();
            for d in &hand.dealt_in {
                let (a, b) = hole_of(&hand, d.seat).expect("hole cards");
                seen.push(a);
                seen.push(b);
            }
            seen.extend(community_now(&hand));
            let n = seen.len();
            for i in 0..n {
                for j in i + 1..n {
                    assert_ne!(seen[i], seen[j], "duplicate card with P={p}");
                }
            }
            assert_eq!(n, 2 * p + 5);
        }
    }

    /// The dealer must reproduce the deck the SPEC describes, from the seed alone,
    /// with no state of its own. If this ever fails, a hand this dealer dealt is no
    /// longer verifiable by a stranger and the spike has broken the one property
    /// five auditors already checked.
    #[test]
    fn cards_are_reproducible_from_the_seed_by_the_published_recipe() {
        let hand = hand_with(3);
        let mut spec_deck = create_deck();
        shuffle_deck(&mut spec_deck, &hand.seed);

        for (k, d) in hand.dealt_in.iter().enumerate() {
            assert_eq!(
                hole_of(&hand, d.seat).unwrap(),
                (spec_deck[2 * k], spec_deck[2 * k + 1]),
                "hole cards of dealt_in[{k}] must be deck[2k], deck[2k+1]"
            );
        }
        let mut river = hand;
        river.street = Street::River;
        let p = river.dealt_in.len();
        assert_eq!(
            community_now(&river),
            vec![
                spec_deck[2 * p + 1],
                spec_deck[2 * p + 2],
                spec_deck[2 * p + 3],
                spec_deck[2 * p + 5],
                spec_deck[2 * p + 7],
            ]
        );
    }

    /// Revealing a street must never change a card that was already on the board.
    #[test]
    fn the_board_only_grows() {
        let mut hand = hand_with(4);
        let mut previous: Vec<Card> = Vec::new();
        for street in [Street::Flop, Street::Turn, Street::River] {
            hand.street = street;
            let now = community_now(&hand);
            assert!(now.len() > previous.len());
            assert_eq!(&now[..previous.len()], &previous[..], "the board changed");
            previous = now;
        }
        assert_eq!(previous.len(), 5);
    }

    /// A seat that stood down stays ready across a street change; a seat that only
    /// acked has to ack again. This is what makes an all-in player stop being asked
    /// and an active player keep having a say.
    #[test]
    fn standing_down_survives_a_street_and_an_ack_does_not() {
        let mut hand = hand_with(4);
        hand.ready.insert(0, Readiness::Acked);
        hand.ready.insert(1, Readiness::StoodDownSelf);
        hand.ready.insert(2, Readiness::TimedOutByTable);
        hand.ready.insert(3, Readiness::GraceExpired);
        open_street(&mut hand, Street::Flop, 1_000);
        assert_eq!(hand.ready.get(&0), None, "an ack is per-street");
        assert_eq!(hand.ready.get(&1), Some(&Readiness::StoodDownSelf));
        assert_eq!(hand.ready.get(&2), Some(&Readiness::TimedOutByTable));
        assert_eq!(
            hand.ready.get(&3),
            None,
            "the grace hatch must be re-earned on every street, or a table that burned \
             it once would own that seat for the rest of the hand"
        );
        assert_eq!(waiting_on(&hand), vec![0, 3]);
    }
}
