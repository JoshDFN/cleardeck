# ClearDeck shuffle specification v1

**Normative.** This document defines, exactly, how a ClearDeck hand is dealt from a seed. It is
written so that a stranger with a SHA-256 implementation and 64-bit integers can reproduce any
hand's cards from the revealed seed, in any language, without reading the Rust.

If this document and `src/poker_core/src/shuffle.rs` ever disagree, that is a bug in ClearDeck,
not in your verifier. Report it.

> **Status of the code this describes:** unaudited alpha software. Verifying a shuffle proves the
> deal was not tampered with after the commitment; it proves nothing about the rest of the
> engine, which has known defects listed in [DEFECTS.md](DEFECTS.md).

---

## 0. What is being proven, and what is not

| | |
|---|---|
| **Proven** | The 52-card order was fixed before any card was seen, and the cards you were dealt follow from the seed the table committed to. |
| **Proven** | Nobody chose the deck after seeing hole cards: the commitment `SHA256(seed)` is published before the deal and the seed is revealed only when the hand ends. |
| **NOT proven** | That the seed was unpredictable. The seed comes from the Internet Computer's `raw_rand`, and you are trusting the subnet's randomness, not arithmetic. |
| **NOT proven** | That the money went to the right player. That is settlement, a separate question. |

A verifier that reproduces the cards has checked the shuffle. It has **not** audited ClearDeck.

## 1. Inputs

For each hand, the table canister publishes a `ShuffleProof`:

| field | type | meaning |
|---|---|---|
| `seed_hash` | lowercase hex string, 64 chars | `SHA256(seed)`, published **before** the deal |
| `revealed_seed` | lowercase hex string, or absent | the `seed` bytes, published **after** the hand ends |
| `timestamp` | nanoseconds since the Unix epoch | when the hand started |

`seed` is 32 bytes obtained from the Internet Computer management canister's `raw_rand`. The
algorithm below accepts a seed of **any** byte length, and the test vectors deliberately include
short seeds, but a live hand always uses 32 bytes.

**Step 0 of any verification:** check `SHA256(revealed_seed) == seed_hash`, byte for byte. If that
fails, stop: the table revealed a seed it did not commit to. (This check alone is *not* a
verification of the shuffle. It is what the canister's own `verify_shuffle` query does, and it only
proves the canister can hash.)

## 2. Deck construction

The deck before shuffling is always the same 52 cards in the same order. Index 0 first:

```
suits, in this order:   Hearts, Diamonds, Clubs, Spades
ranks, in this order:   2 3 4 5 6 7 8 9 10 J Q K A

deck[13 * suit_ordinal + (rank_value - 2)]
```

so `deck[0] = 2h`, `deck[12] = Ah`, `deck[13] = 2d`, `deck[26] = 2c`, `deck[39] = 2s`,
`deck[51] = As`. Getting this order wrong changes every card, so it is pinned by its own test
(`create_deck_order_is_pinned`) on both the host and wasm32.

## 3. The shuffle

A single Fisher-Yates pass from the top of the deck down, with every swap index drawn from a
SHA-256 hash chain rooted at the seed.

### 3.1 The hash chain

`chain` is a byte string. It starts as the raw `seed` bytes (**not** the hex text of the seed) and
is replaced by a 32-byte digest at every step:

```
chain <- SHA256(chain || counter_byte)
```

`||` is byte concatenation and `counter_byte` is a single byte, `i mod 256`, where `i` is the
current deck position. For a 52-card deck `i` runs 51 down to 1, so `counter_byte` is just `i`.

Every digest produced advances the chain, **including digests whose draw is rejected** (see 3.3).
There is one chain per hand, threaded through all 51 steps in order.

### 3.2 The draw

From each new `chain` value, take the **first 8 bytes** and read them as a **little-endian**
unsigned 64-bit integer:

```
draw = chain[0] + chain[1]*2^8 + chain[2]*2^16 + ... + chain[7]*2^56
```

The remaining 24 bytes of the digest are not used for the draw; they matter only because the whole
digest becomes the next `chain`.

### 3.3 The rejection rule (do not skip this)

Reducing a 64-bit draw modulo `n` is uniform only if the draw came from a range whose size is an
exact multiple of `n`. `2^64` is not, unless `n` is a power of two, so the values in the final,
short bucket are **rejected** rather than reduced:

```
bound(n) = floor(2^64 / n) * n          # the largest multiple of n that is <= 2^64

accept draw  <=>  draw < bound(n)
```

On rejection, hash again with the **same** `counter_byte` (which yields a different digest, because
the chain has advanced) and test the new draw. Repeat until a draw is accepted.

Notes for implementers:

- `bound(n)` equals exactly `2^64` when `n` is a power of two, so nothing is ever rejected for
  `n` in {2, 4, 8, 16, 32}. In languages with big integers (Python, JavaScript `BigInt`) write it
  literally as `(2**64 // n) * n`. In fixed-width 64-bit arithmetic, `2^64` does not fit: compute
  `r = 2^64 mod n` as `((2^64 - 1) mod n + 1) mod n` and reject iff `r != 0 and draw >= 2^64 - r`.
  ClearDeck's Rust does the comparison in 128-bit arithmetic, which is 128 bits on every target.
- The rejection probability per step is `(2^64 mod n) / 2^64`, which over `n` in `2..=52` peaks at
  `41/2^64 ≈ 2.2e-18` (at `n = 43`). **You will almost certainly never see a rejection.** It is
  specified because "almost certainly" is not "provably", and a verifier that omits the rule will
  one day disagree with the table and be unable to tell that from cheating.
- Concrete bounds: `bound(52) = 18446744073709551600` (16 values rejected),
  `bound(51) = 18446744073709551615` (1), `bound(43) = 18446744073709551575` (41, the worst case),
  `bound(32) = 2^64` (0).

### 3.4 The swap

```
j = draw mod n
swap deck[i] and deck[j]        # j may equal i, which is a no-op, and that is correct
```

### 3.5 Complete pseudocode

```
function shuffle(seed_bytes):
    deck  = the 52 cards of section 2, in order
    chain = seed_bytes
    for i = 51 down to 1:
        n     = i + 1
        limit = (2^64 // n) * n
        repeat:
            chain = SHA256(chain || byte(i mod 256))
            draw  = little_endian_u64(chain[0:8])
        until draw < limit
        j = draw mod n
        swap deck[i], deck[j]
    return deck
```

That is the whole shuffle. 51 iterations, one SHA-256 each (plus a re-hash on the astronomically
unlikely rejection).

### 3.6 Width independence is part of the specification

`draw` is a 64-bit value and must stay 64-bit until after the modulo. ClearDeck's canister runs on
`wasm32-unknown-unknown`, where a Rust `usize` is **32 bits**; routing the draw through `usize`
truncates it and silently produces a different deck from the same seed. That defect shipped, and it
is the reason this document exists — see
[FINDING-02-shuffle-not-verifiable.md](FINDING-02-shuffle-not-verifiable.md). The golden vectors
are therefore generated by executing the shuffle **inside a wasm32 module**, and CI replays them on
wasm32 as well as on the host.

## 4. Dealing order

The shuffled deck is consumed strictly front to back. `deck_index` starts at 0.

1. **Hole cards.** Walk the seats in **ascending seat index** (0, 1, 2, ... — *not* starting left of
   the dealer button, which is the usual casino convention but is not what this engine does). Every
   seat whose player is `Active` at the moment the hand starts takes the **next two** cards, in
   order: `(deck[deck_index], deck[deck_index + 1])`, and `deck_index` advances by 2. Seats that
   are empty, sitting out, or busted to zero chips take no cards and consume nothing.
2. **Flop.** Burn one card (`deck_index += 1`), then the flop is the next three cards in order.
3. **Turn.** Burn one card, then the turn is the next card.
4. **River.** Burn one card, then the river is the next card.

Burn cards are never revealed and never used. So with `P` players dealt in:

| | index into the shuffled deck |
|---|---|
| hole cards of the `k`-th dealt-in seat (`k` from 0) | `2k`, `2k+1` |
| flop | `2P+1`, `2P+2`, `2P+3` |
| turn | `2P+5` |
| river | `2P+7` |

A heads-up hand (`P = 2`) therefore uses `deck[0..4]` for hole cards, `deck[5..8]` for the flop,
`deck[9]` for the turn and `deck[11]` for the river.

`P` is the number of seats that were **dealt in**, which is not necessarily the number of seats
occupied: a player who was sitting out or had no chips when the hand started took no cards. Count it
from the hand's own record — every seat the history shows with cards, plus any that folded — or read
it from your own client's view of the table at the moment the hand started. If you use the wrong
`P`, your hole cards will still match (they come from the front of the deck) but the board will be
offset, which is the one mistake a verifier is likely to make.

## 5. Worked example

A seed you can regenerate yourself, so it is visibly not cherry-picked:

```
seed = SHA256("ClearDeck shuffle spec v1 worked example")     # ASCII, no trailing newline
     = bdfa6f039a2d44dc8a330c5435891ec693bc5a5f4cffd94454d3205c0f4873dc
```

Those 32 **bytes** are the seed. Check your SHA-256 wiring against the commitment a table would
publish for it:

```
SHA256(seed bytes) = c1d4f9e294d69f17f206846cc7b96462901816938b7cd10c36624d3f6af31643
```

The 52 cards of the shuffled deck, index 0 first. Rejected draws: **0**.

```
 0..12   Jd Kc Td 7d 8d 4h 7c 9s Qc Qd Qs 6c 6s
13..25   5s 3c Th 4d As 5h Tc 5d Jh 5c 2c Kd Js
26..38   Qh Ad 4c 2d 7s 2h 6h Ts 8h 9c 6d 3s Ah
39..51   Jc 9d 3d 4s 8s 3h 7h Ac 8c 9h Kh Ks 2s
```

Compact form, the encoding ClearDeck's test vectors use (`A`-`Z` then `a`-`z`, indexed by
`13 * suit_ordinal + rank_value - 2`, the same absolute index as section 2):

```
WlVSTCfukXxerqbIPzDiQJdaYwKZcNsAEvGhRoMjUOptBFmgHLyn
```

Dealt as a heads-up hand (`P = 2`), by section 4:

| | |
|---|---|
| seat 0 hole cards | `Jd Kc` |
| seat 1 hole cards | `Td 7d` |
| flop | `4h 7c 9s` (`8d` burned) |
| turn | `Qd` (`Qc` burned) |
| river | `6c` (`Qs` burned) |

Every line above was produced three independent ways and cross-checked: by the canister's own
Rust compiled to `wasm32-unknown-unknown` and run in a wasm module, by the Python verifier, and by
the JavaScript verifier of section 6.

## 6. Reference verifiers

Two independent implementations written from **this document**, in two languages, neither sharing a
line with the Rust or with each other:

- `src/poker_core/tests/verify/verify_shuffle.py` — Python 3, standard library only.
- `src/poker_core/tests/verify/verify_shuffle.mjs` — Node, standard library only.

Both print the deck for a revealed seed, optionally check it against the committed hash, and
optionally lay out the hand for a given number of players:

```
python3 src/poker_core/tests/verify/verify_shuffle.py <seed-hex> --players 2 --seed-hash <hex>
node    src/poker_core/tests/verify/verify_shuffle.mjs <seed-hex> --players 2 --seed-hash <hex>
```

Both can also replay ClearDeck's committed vectors end to end, which is a 2,000-case cross-language
differential against the canister's own shuffle:

```
python3 src/poker_core/tests/verify/verify_shuffle.py --vectors src/poker_core/tests/golden_vectors.txt
node    src/poker_core/tests/verify/verify_shuffle.mjs --vectors src/poker_core/tests/golden_vectors.txt
```

They are deliberately *not* built from `poker_core`: an independent reimplementation is the only
thing that can catch a specification which does not match the code.

## 7. How this is tested

| test | what it pins |
|---|---|
| `poker_core::shuffle::tests` (host) | `bound(n)` is the largest multiple of `n` below `2^64`; the rejection branch redraws instead of reducing a rejected value; the chain advances on every digest; the old 32-bit-truncating shuffle no longer reproduces our output |
| `poker_core/tests/golden_vectors.rs` (host) | 2,000 seed/deck pairs, plus the pre-shuffle deck order |
| `poker_core/tests/wasm32_golden.rs` (**wasm32**) | the same 2,000 pairs, replayed inside a `wasm32-unknown-unknown` module, which is the target the canister runs on. This is the test that would have caught FINDING-02 |
| `.github/workflows/ci.yml` job `wasm32-tests` | runs the wasm32 replay on every push and pull request |
| `src/poker_core/tests/verify/*`, driven by `wasm32_golden.rs::outsider_reimplementations_reproduce_the_vectors` | the Python and JavaScript verifiers replaying all 2,000 vectors, so THIS DOCUMENT drifting from the code is a test failure |
| the proof of the fix (2026-08-04) | two hands dealt on the real canister wasm under PocketIC, then reproduced — hole cards and full board — from the revealed seed alone by both outsider verifiers. The pre-fix arithmetic reproduces 0 of 9 and 0 of 11 of those cards |

## 8. Version history

| version | date | change |
|---|---|---|
| v1 | 2026-08-04 | First specification. Width-independent draw plus the rejection rule; golden vectors regenerated by wasm32 execution. Supersedes the undocumented pre-`ceacc37` behaviour, which truncated the draw to 32 bits on-chain and could not be reproduced by anyone. No hand had ever been dealt on mainnet, so no history was invalidated. |
