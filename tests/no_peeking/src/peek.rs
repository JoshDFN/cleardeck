//! Deciding whether a reply contains a card, without trusting a per-method
//! hand-written check.
//!
//! # Why this is not a string search
//!
//! A `Card` on the Candid wire is a record of two no-payload variants, i.e. two
//! small integers. Grepping a reply blob for two-byte patterns would find a card in
//! almost any reply and prove nothing. So the reply is DECODED generically —
//! `IDLArgs::from_bytes` reads the message's own type table, so it works on a reply
//! whose type the harness has never seen — and the resulting value tree is searched
//! for a subtree structurally equal to the encoding of the card we are looking for.
//!
//! That means a new method returning a card in a new wrapper type is caught with no
//! change here, which is the property `tests/no_peek.rs` needs: the test enumerates
//! the canister's exported surface from the MODULE, so a method that did not exist
//! when this file was written is still attacked and still checked.

use candid::types::value::IDLValue;
use candid::{encode_one, IDLArgs};
use poker_core::Card;

/// The needle: what `card` looks like once decoded without type information.
pub fn card_value(card: &Card) -> IDLValue {
    let bytes = encode_one(card).expect("a Card always encodes");
    let args = IDLArgs::from_bytes(&bytes).expect("a Card always decodes generically");
    args.args
        .into_iter()
        .next()
        .expect("encode_one produces one value")
}

/// Does `haystack` contain `needle` anywhere inside it?
pub fn contains(haystack: &IDLValue, needle: &IDLValue) -> bool {
    if haystack == needle {
        return true;
    }
    match haystack {
        IDLValue::Record(fields) => fields.iter().any(|f| contains(&f.val, needle)),
        IDLValue::Variant(v) => contains(&v.0.val, needle),
        IDLValue::Vec(items) => items.iter().any(|i| contains(i, needle)),
        IDLValue::Opt(inner) => contains(inner, needle),
        _ => false,
    }
}

/// Every card from `secrets` that appears anywhere in `reply`.
///
/// A reply the harness cannot decode contributes nothing, and that is correct: an
/// undecodable reply is one no client could read a card out of either. Replies that
/// never happened (the replica rejected the message) never reach here.
pub fn cards_leaked(reply: &[u8], secrets: &[(&'static str, Card)]) -> Vec<&'static str> {
    let Ok(args) = IDLArgs::from_bytes(reply) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (label, card) in secrets {
        let needle = card_value(card);
        if args.args.iter().any(|v| contains(v, &needle)) {
            found.push(*label);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use candid::encode_args;
    use poker_core::{Rank, Suit};

    fn ace_of_spades() -> Card {
        Card {
            suit: Suit::Spades,
            rank: Rank::Ace,
        }
    }
    fn two_of_hearts() -> Card {
        Card {
            suit: Suit::Hearts,
            rank: Rank::Two,
        }
    }

    /// THE DETECTOR MUST DETECT. If this fails, every green result in
    /// `tests/no_peek.rs` is worthless, which is exactly the failure mode a
    /// negative test has: it passes when it is broken.
    #[test]
    fn a_card_in_a_bare_reply_is_found() {
        let reply = encode_one(ace_of_spades()).unwrap();
        let found = cards_leaked(&reply, &[("As", ace_of_spades())]);
        assert_eq!(found, vec!["As"]);
    }

    #[test]
    fn a_card_nested_in_an_opt_vec_record_is_found() {
        #[derive(candid::CandidType)]
        struct Wrapper {
            noise: u64,
            deep: Option<Vec<(u8, Card)>>,
        }
        let reply = encode_one(Wrapper {
            noise: 7,
            deep: Some(vec![(3, ace_of_spades())]),
        })
        .unwrap();
        assert_eq!(
            cards_leaked(&reply, &[("As", ace_of_spades())]),
            vec!["As"],
            "a card hidden three containers deep must still be found"
        );
    }

    #[test]
    fn a_card_inside_a_result_variant_is_found() {
        let reply = encode_one(Ok::<(Card, Card), String>((
            ace_of_spades(),
            two_of_hearts(),
        )))
        .unwrap();
        let found = cards_leaked(
            &reply,
            &[("As", ace_of_spades()), ("2h", two_of_hearts())],
        );
        assert_eq!(found, vec!["As", "2h"]);
    }

    #[test]
    fn a_card_in_the_second_of_several_reply_values_is_found() {
        let reply = encode_args((99u64, two_of_hearts())).unwrap();
        assert_eq!(cards_leaked(&reply, &[("2h", two_of_hearts())]), vec!["2h"]);
    }

    /// And it must NOT fire on a reply that has no card in it, or the negative test
    /// would be unfalsifiable in the other direction.
    #[test]
    fn a_reply_with_no_card_yields_nothing() {
        let reply = encode_one("no cards here, just a long string with suits and ranks in it: Spades Ace").unwrap();
        assert!(cards_leaked(&reply, &[("As", ace_of_spades())]).is_empty());
    }

    /// A DIFFERENT card must not match: the detector distinguishes rank and suit,
    /// so "we found a card" cannot be an artefact of every card looking alike.
    #[test]
    fn a_different_card_does_not_match() {
        let reply = encode_one(two_of_hearts()).unwrap();
        assert!(cards_leaked(&reply, &[("As", ace_of_spades())]).is_empty());
        assert_eq!(cards_leaked(&reply, &[("2h", two_of_hearts())]), vec!["2h"]);
    }

    #[test]
    fn an_undecodable_reply_reports_no_leak_rather_than_panicking() {
        assert!(cards_leaked(&[0xde, 0xad, 0xbe, 0xef], &[("As", ace_of_spades())]).is_empty());
    }
}
