#!/usr/bin/env python3
"""Independent verifier for the ClearDeck provably-fair shuffle.

Written from docs/SHUFFLE-SPEC.md, NOT from src/poker_core/src/shuffle.rs. That is
the point: an independent reimplementation is the only thing that can catch a
specification which does not match the code. It shares no code with the canister,
imports nothing outside the Python standard library, and never talks to a network.

    # print the whole shuffled deck for a revealed seed
    python3 verify_shuffle.py <seed-hex>

    # print what a 2-player hand dealt from that seed would look like
    python3 verify_shuffle.py <seed-hex> --players 2

    # check a commitment at the same time
    python3 verify_shuffle.py <seed-hex> --seed-hash <hex>

    # replay ClearDeck's committed golden vectors ("S <seed> <symbols>" lines)
    python3 verify_shuffle.py --vectors ../golden_vectors.txt

Exit status is 0 only if everything asked for checked out.
"""

from __future__ import annotations

import argparse
import hashlib
import sys

# --- section 2 of the spec: deck construction ------------------------------

SUITS = "hdcs"            # Hearts, Diamonds, Clubs, Spades -- this order is normative
RANKS = "23456789TJQKA"   # 2 .. A -- rank_value = index + 2

TWO_POW_64 = 1 << 64


def build_deck() -> list[str]:
    """The 52 cards before shuffling, index 0 first: deck[13*suit + rank-2]."""
    return [rank + suit for suit in SUITS for rank in RANKS]


# --- section 3 of the spec: the shuffle ------------------------------------


def draw_bound(n: int) -> int:
    """floor(2^64 / n) * n -- the largest multiple of n that is <= 2^64."""
    return (TWO_POW_64 // n) * n


def shuffle(seed: bytes, deck_size: int = 52) -> tuple[list[str], int]:
    """Fisher-Yates driven by SHA-256, exactly as specified.

    Returns the shuffled deck and the number of rejected draws (expected: 0).
    """
    deck = build_deck()
    assert len(deck) == deck_size
    chain = seed
    rejections = 0

    for i in range(deck_size - 1, 0, -1):
        n = i + 1
        limit = draw_bound(n)
        while True:
            chain = hashlib.sha256(chain + bytes([i % 256])).digest()
            draw = int.from_bytes(chain[0:8], "little")
            if draw < limit:
                break
            rejections += 1          # never observed in practice; see spec 3.3
        j = draw % n
        deck[i], deck[j] = deck[j], deck[i]

    return deck, rejections


# --- section 4 of the spec: dealing order ---------------------------------


def deal(deck: list[str], players: int) -> dict:
    """Hole cards in ascending seat order, then burn/flop, burn/turn, burn/river."""
    if players < 2:
        raise ValueError("a hand needs at least 2 players dealt in")
    if 2 * players + 8 > len(deck):
        raise ValueError("not enough cards for that many players")

    holes = [(deck[2 * k], deck[2 * k + 1]) for k in range(players)]
    base = 2 * players
    return {
        "hole_cards": holes,
        "burn_before_flop": deck[base],
        "flop": [deck[base + 1], deck[base + 2], deck[base + 3]],
        "burn_before_turn": deck[base + 4],
        "turn": deck[base + 5],
        "burn_before_river": deck[base + 6],
        "river": deck[base + 7],
    }


# --- golden-vector symbol encoding (ClearDeck's compact test format) -------

ALPHA = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"


def card_symbol(card: str) -> str:
    rank, suit = card[0], card[1]
    return ALPHA[13 * SUITS.index(suit) + RANKS.index(rank)]


def deck_symbols(deck: list[str]) -> str:
    return "".join(card_symbol(c) for c in deck)


# --- CLI ------------------------------------------------------------------


def check_vectors(path: str) -> int:
    checked = failed = 0
    with open(path, "r", encoding="utf-8") as fh:
        for lineno, line in enumerate(fh, start=1):
            if not line.startswith("S "):
                continue
            _, seed_hex, expected = line.split()
            deck, _ = shuffle(bytes.fromhex(seed_hex))
            got = deck_symbols(deck)
            checked += 1
            if got != expected:
                failed += 1
                print(f"line {lineno}: MISMATCH\n  seed     {seed_hex}"
                      f"\n  expected {expected}\n  got      {got}")
    print(f"checked {checked} shuffle vectors, {failed} mismatched")
    return 0 if checked and not failed else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="Verify a ClearDeck shuffle from its revealed seed.")
    ap.add_argument("seed", nargs="?", help="revealed_seed, as lowercase hex")
    ap.add_argument("--seed-hash", help="the seed_hash the table committed to, as hex")
    ap.add_argument("--players", type=int, help="how many players were dealt in")
    ap.add_argument("--symbols", action="store_true", help="also print the compact symbol encoding")
    ap.add_argument("--vectors", help="replay a golden_vectors.txt instead of one seed")
    args = ap.parse_args()

    if args.vectors:
        return check_vectors(args.vectors)
    if not args.seed:
        ap.error("give a seed, or --vectors")

    try:
        seed = bytes.fromhex(args.seed)
    except ValueError:
        print("seed must be even-length hex", file=sys.stderr)
        return 2

    status = 0
    computed_hash = hashlib.sha256(seed).hexdigest()
    print(f"seed        {seed.hex()}  ({len(seed)} bytes)")
    print(f"SHA256(seed) {computed_hash}")
    if args.seed_hash:
        if computed_hash.lower() == args.seed_hash.lower().strip():
            print("commitment  OK: the revealed seed hashes to the committed seed_hash")
        else:
            print(f"commitment  MISMATCH: table committed to {args.seed_hash}")
            status = 1

    deck, rejections = shuffle(seed)
    print(f"rejections  {rejections}")
    print("deck")
    for row in range(4):
        cards = deck[row * 13:(row + 1) * 13]
        print(f"  {row * 13:2d}..{row * 13 + 12:2d}  " + " ".join(f"{c:>2}" for c in cards))
    if args.symbols:
        print(f"symbols     {deck_symbols(deck)}")

    if args.players:
        hand = deal(deck, args.players)
        print(f"\nhand with {args.players} players dealt in, seats in ascending order")
        for seat, (a, b) in enumerate(hand["hole_cards"]):
            print(f"  seat {seat}: {a} {b}")
        print(f"  flop:  {' '.join(hand['flop'])}   (burned {hand['burn_before_flop']})")
        print(f"  turn:  {hand['turn']}   (burned {hand['burn_before_turn']})")
        print(f"  river: {hand['river']}   (burned {hand['burn_before_river']})")

    return status


if __name__ == "__main__":
    sys.exit(main())
