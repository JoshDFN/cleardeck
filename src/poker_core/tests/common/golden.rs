//! The golden-vector runner, shared by the NATIVE test target
//! (`tests/golden_vectors.rs`) and the WASM32 harness
//! (`tests/wasm32_harness`, driven by `tests/wasm32_golden.rs`).
//!
//! It exists as a shared module for one reason: the two targets must check the
//! *same* vectors with the *same* code. `docs/FINDING-02-shuffle-not-verifiable.md`
//! is a defect that only appears on wasm32 and that a native-only golden test can
//! never see, so "the native suite is green" is not evidence about the canister.
//! If this logic were copied into the wasm harness the copies would drift, which
//! is exactly the failure mode `poker_core` was extracted to end.
//!
//! Nothing here uses `std::fs`, `std::process`, threads or the clock: it must
//! compile and run unchanged inside a `wasm32-unknown-unknown` module with no
//! imports.

#![allow(dead_code)]

use poker_core::{
    apply_side_pots, check_straight, create_deck, detect_straight, evaluate_five_cards,
    evaluate_hand, get_straight_high, shuffle_deck, Card, Contribution, Rank, SidePot, Suit,
};

/// The vector fixture, embedded so the wasm module is self-contained.
pub const GOLDEN: &str = include_str!("../golden_vectors.txt");

pub const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

const SUITS: [Suit; 4] = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];
const RANKS: [Rank; 13] = [
    Rank::Two,
    Rank::Three,
    Rank::Four,
    Rank::Five,
    Rank::Six,
    Rank::Seven,
    Rank::Eight,
    Rank::Nine,
    Rank::Ten,
    Rank::Jack,
    Rank::Queen,
    Rank::King,
    Rank::Ace,
];

/// Expected vector counts. A truncated or silently-emptied fixture must fail.
pub const EXPECTED_SHUFFLE: usize = 2000;
pub const EXPECTED_SEVEN_CARD: usize = 3000;
pub const EXPECTED_FIVE_CARD: usize = 3000;
pub const EXPECTED_STRAIGHT: usize = 6188;
pub const EXPECTED_SIDE_POTS: usize = 1500;

/// Cards are encoded ABSOLUTELY: suit ordinal (H,D,C,S) * 13 + (rank - 2).
///
/// Deliberately NOT "position in `create_deck()`". An encoding relative to
/// `create_deck()` cancels out on both sides, so reordering the deck would slip
/// past every vector -- verified by mutation testing. `create_deck`'s own order is
/// pinned separately by `create_deck_order_is_pinned`.
pub fn card_from_index(idx: usize) -> Card {
    Card {
        suit: SUITS[idx / 13],
        rank: RANKS[idx % 13],
    }
}

pub fn card_index(c: &Card) -> usize {
    let suit_ord = match c.suit {
        Suit::Hearts => 0usize,
        Suit::Diamonds => 1,
        Suit::Clubs => 2,
        Suit::Spades => 3,
    };
    suit_ord * 13 + (c.rank.value() as usize - 2)
}

pub fn decode_cards(symbols: &str) -> Vec<Card> {
    symbols
        .bytes()
        .map(|b| {
            let idx = ALPHA
                .iter()
                .position(|&a| a == b)
                .unwrap_or_else(|| panic!("bad card symbol {:?}", b as char));
            card_from_index(idx)
        })
        .collect()
}

pub fn encode_cards(cards: &[Card]) -> String {
    cards
        .iter()
        .map(|c| ALPHA[card_index(c)] as char)
        .collect()
}

pub fn decode_hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd-length hex: {}", s);
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("bad hex"))
        .collect()
}

/// Must match the generator's encoding exactly.
pub fn encode_pots(pots: &[SidePot]) -> String {
    if pots.is_empty() {
        return "-".to_string();
    }
    pots.iter()
        .map(|p| {
            let elig: Vec<String> = p.eligible_players.iter().map(|s| s.to_string()).collect();
            format!("{}#{}", p.amount, elig.join(","))
        })
        .collect::<Vec<_>>()
        .join(";")
}

pub fn parse_contributions(field: &str) -> Vec<Contribution> {
    if field == "-" {
        return Vec::new();
    }
    field
        .split(';')
        .map(|entry| {
            let mut it = entry.split(':');
            let seat: u8 = it.next().expect("seat").parse().expect("seat");
            let bet: u64 = it.next().expect("bet").parse().expect("bet");
            let folded: u8 = it.next().expect("folded").parse().expect("folded");
            Contribution::new(seat, bet, folded == 1)
        })
        .collect()
}

/// Exactly what `table_canister::calculate_side_pots` does, minus the logging.
/// The generator drove the ORIGINAL `calculate_side_pots(&mut TableState)` with
/// the same pre-seeded sentinel, so this pins the adapter contract as well as the
/// arithmetic.
pub fn replay_side_pots(contributions: &[Contribution], total_pot: u64) -> (Vec<SidePot>, Vec<String>) {
    let mut pots = vec![SidePot {
        amount: 999,
        eligible_players: vec![7],
    }];
    // The original routine dropped players who bet nothing while collecting, so
    // an all-zero table yields no contributions and the early return applies.
    let live: Vec<Contribution> = contributions
        .iter()
        .filter(|c| c.total_bet_this_hand > 0)
        .copied()
        .collect();
    let warnings = apply_side_pots(&mut pots, &live, total_pot);
    (pots, warnings)
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Counts {
    pub shuffle: usize,
    pub seven_card: usize,
    pub five_card: usize,
    pub straight: usize,
    pub side_pots: usize,
}

#[derive(Default)]
pub struct Outcome {
    pub counts: Counts,
    /// One entry per disagreeing vector, in file order.
    pub failures: Vec<String>,
}

impl Outcome {
    /// Human-readable, and identical on both targets so a wasm failure reads the
    /// same as a native one.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "counts: S={} E={} F={} D={} P={}\n",
            self.counts.shuffle,
            self.counts.seven_card,
            self.counts.five_card,
            self.counts.straight,
            self.counts.side_pots
        ));
        for f in &self.failures {
            out.push_str(f);
            out.push('\n');
        }
        out
    }

    /// Returns `Err(message)` unless every vector matched AND every family has its
    /// full expected population.
    pub fn verify(&self) -> Result<(), String> {
        let mut problems: Vec<String> = self
            .failures
            .iter()
            .take(20)
            .cloned()
            .collect();
        if self.failures.len() > 20 {
            problems.push(format!("... and {} more", self.failures.len() - 20));
        }
        let mut check = |got: usize, want: usize, name: &str| {
            if got != want {
                problems.push(format!("expected {} {} vectors, saw {}", want, name, got));
            }
        };
        check(self.counts.shuffle, EXPECTED_SHUFFLE, "shuffle");
        check(self.counts.seven_card, EXPECTED_SEVEN_CARD, "evaluate_hand");
        check(self.counts.five_card, EXPECTED_FIVE_CARD, "evaluate_five_cards");
        check(self.counts.straight, EXPECTED_STRAIGHT, "detect_straight");
        check(self.counts.side_pots, EXPECTED_SIDE_POTS, "side-pot");
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("\n"))
        }
    }
}

/// Replays every vector in `golden`, collecting disagreements instead of
/// panicking: a wasm module cannot report a panic message, and "how many vectors
/// diverged, and which" is the diagnostic that matters.
pub fn run_all(golden: &str) -> Outcome {
    let mut out = Outcome::default();

    for (lineno, line) in golden.lines().enumerate() {
        let lineno = lineno + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let kind = &line[..1];
        let rest = &line[2..];

        match kind {
            "S" => {
                let mut it = rest.splitn(2, ' ');
                let seed = decode_hex(it.next().expect("seed"));
                let expected = it.next().expect("deck");
                let mut deck = create_deck();
                shuffle_deck(&mut deck, &seed);
                let got = encode_cards(&deck);
                if got != expected {
                    out.failures.push(format!(
                        "line {}: shuffle_deck diverged\n  expected {}\n  got      {}",
                        lineno, expected, got
                    ));
                }
                out.counts.shuffle += 1;
            }
            "E" => {
                let mut it = rest.splitn(2, ' ');
                let cards = decode_cards(it.next().expect("cards"));
                let expected = it.next().expect("rank");
                assert_eq!(cards.len(), 7, "line {}", lineno);
                let hole = (cards[0], cards[1]);
                let got = format!("{:?}", evaluate_hand(&hole, &cards[2..7]));
                if got != expected {
                    out.failures.push(format!(
                        "line {}: evaluate_hand diverged\n  expected {}\n  got      {}",
                        lineno, expected, got
                    ));
                }
                out.counts.seven_card += 1;
            }
            "F" => {
                let mut it = rest.splitn(2, ' ');
                let cards = decode_cards(it.next().expect("cards"));
                let expected = it.next().expect("rank");
                assert_eq!(cards.len(), 5, "line {}", lineno);
                let got = format!("{:?}", evaluate_five_cards(&cards));
                if got != expected {
                    out.failures.push(format!(
                        "line {}: evaluate_five_cards diverged\n  expected {}\n  got      {}",
                        lineno, expected, got
                    ));
                }
                out.counts.five_card += 1;
            }
            "D" => {
                let f: Vec<&str> = rest.split(' ').collect();
                assert_eq!(f.len(), 4, "line {}", lineno);
                let ranks: Vec<u8> = f[0].split(',').map(|r| r.parse().expect("rank")).collect();
                let got_detect = format!("{:?}", detect_straight(&ranks));
                let got_check = check_straight(&ranks).to_string();
                let got_high = get_straight_high(&ranks).to_string();
                if got_detect != f[1] || got_check != f[2] || got_high != f[3] {
                    out.failures.push(format!(
                        "line {}: straight detection diverged\n  expected {} {} {}\n  got      {} {} {}",
                        lineno, f[1], f[2], f[3], got_detect, got_check, got_high
                    ));
                }
                out.counts.straight += 1;
            }
            "P" => {
                let f: Vec<&str> = rest.splitn(4, ' ').collect();
                assert_eq!(f.len(), 4, "line {}", lineno);
                let contributions = parse_contributions(f[0]);
                let total_pot: u64 = f[1].parse().expect("pot");
                let (pots, warnings) = replay_side_pots(&contributions, total_pot);
                let got_pots = encode_pots(&pots);
                let got_warn = if warnings.is_empty() {
                    "-".to_string()
                } else {
                    warnings.join("~")
                };
                if got_pots != f[2] || got_warn != f[3] {
                    out.failures.push(format!(
                        "line {}: side pots diverged\n  input    {} pot={}\n  expected {} | {}\n  got      {} | {}",
                        lineno, f[0], total_pot, f[2], f[3], got_pots, got_warn
                    ));
                }
                out.counts.side_pots += 1;
            }
            other => panic!("unknown vector kind {:?} at line {}", other, lineno),
        }
    }

    out
}

/// Recomputes the `S` (shuffle) family from the seeds already in `golden` and
/// emits replacement lines, in file order, one per line, `S <seed_hex> <deck>`.
///
/// Called from the WASM32 harness so the committed vectors are produced by the
/// target that actually runs the shuffle. See `docs/SHUFFLE-SPEC.md`.
pub fn regenerate_shuffle_lines(golden: &str) -> String {
    let mut out = String::new();
    for line in golden.lines() {
        if !line.starts_with("S ") {
            continue;
        }
        let seed_hex = line[2..].split(' ').next().expect("seed");
        let seed = decode_hex(seed_hex);
        let mut deck = create_deck();
        shuffle_deck(&mut deck, &seed);
        out.push_str(&format!("S {} {}\n", seed_hex, encode_cards(&deck)));
    }
    out
}

/// `create_deck()` order, encoded. Pinned by both targets.
pub fn deck_order_symbols() -> String {
    encode_cards(&create_deck())
}
