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
| **Proven** | **The whole 52-card order was fixed before the board was shown.** Record the commitment while a hand is in progress and only the flop exists, then predict the turn and the river from the seed revealed at the end: they come out right, and so do the hole cards of a seat that folded and never showed them. |
| **Proven** | The cards you were dealt follow from the seed the table committed to. Change any one card, at any position, and `SHA256(seed)` stops matching the commitment. |
| **NOT proven** | **That the commitment existed before the cards did.** `start_new_hand` commits and deals in a single message, so no outsider can observe the commitment before cards exist. The row above is what a stranger can actually check, and it is the honest version of the claim. |
| **NOT proven** | That the seed was unpredictable. The seed comes from the Internet Computer's `raw_rand`, and you are trusting the subnet's randomness, not arithmetic. |
| **NOT proven** | That the money went to the right player. That is settlement, a separate question. |

A verifier that reproduces the cards has checked the shuffle. It has **not** audited ClearDeck.

> **On the wording.** This section used to read *"the commitment `SHA256(seed)` is published before
> the deal"*. An independent auditor, working only from this document and a running canister,
> reproduced three hands exactly and then said that sentence claims more than anyone outside the
> canister can witness, because the commit and the deal are one message. It is correct.
> [DEFECTS.md D-06](DEFECTS.md#d-06). What the auditor *did* prove is the first row, and that is
> what this document now claims. You can witness the ordering yourself, without trusting us: copy
> the commitment off the screen while a hand is still running, and check it against the one shown
> after the seed is revealed.

## 1. Inputs

For each hand, the table canister publishes a `ShuffleProof`:

| field | type | meaning |
|---|---|---|
| `seed_hash` | lowercase hex string, 64 chars | `SHA256(seed)`, on screen from the moment the first card is dealt |
| `revealed_seed` | lowercase hex string, or absent | the `seed` bytes, published **after** the hand ends |
| `timestamp` | nanoseconds since the Unix epoch | when the hand started, according to the canister |

`seed` is 32 bytes obtained from the Internet Computer management canister's `raw_rand`. The
algorithm below accepts a seed of **any** byte length, and the test vectors deliberately include
short seeds, but a live hand always uses 32 bytes.

**Step 0 of any verification:** check `SHA256(revealed_seed) == seed_hash`, byte for byte, **on your
own machine**. If that fails, stop: the table revealed a seed it did not commit to.

This check alone is *not* a verification of the shuffle, and asking the canister to do it for you is
not a verification of anything at all. The table exposes
`check_shuffle_commitment : (record { seed_hash : text; revealed_seed : text }) -> (CommitmentCheck)`
and the archive exposes `check_recorded_hand : (nat64) -> (RecordedHandCheck)`; both re-hash the same
32 bytes and both say so in their own reply, in a `this_does_not_prove` field. They exist so a
mistake reads as a mistake — the answer names which field was malformed, or that the two values were
put in each other's fields, instead of returning a bare `false` that a player reads as "I was
cheated". They are not part of the verification, and nothing in sections 2 to 5 needs them.

> The table's old `verify_shuffle : (text, text) -> (bool)` returned `false` for a genuine proof
> passed in the natural reading order, with no parameter names on the wire to say which string was
> which ([DEFECTS.md E-44](DEFECTS.md#e-44)). It is still there, still deprecated, and now answers
> in either order so it cannot manufacture an accusation; prefer `check_shuffle_commitment`.

## 1a. Naming a hand — READ THIS BEFORE YOU CITE ONE

A verification you cannot address to a specific hand is not checkable by anyone else. So:

```
hand_uid = "<table canister id>:<seed_hash>"
```

That is the hand's name. It is derived from two fields every hand record already carries, it is
unique, and it never changes. Cite it.

**`hand_number` is not a name.** It restarts at 1 every time a table is reset or reinstalled, and
nothing stops the archive holding many hands under one number. Measured on the local archive on
2026-08-08, over 3,218 archived hands:

| | |
|---|---|
| distinct `(table, hand_number)` citations | 1,651 |
| citations that name **more than one** record | 947 |
| records living under such a citation | 2,514 — **78% of the archive** |
| citations whose records have **different pots** | 877 |
| worst case | **`table_2` "hand 1" answers to 70 records, with 6 different pots** |
| distinct `seed_hash` values over the same 3,218 records | **3,218** |

The commitment is already a perfect key over every hand this project has ever archived; the hand
number was never a key at all. [DEFECTS.md E-71](DEFECTS.md#e-71).

### Looking a hand up

```
icp canister call history get_hand_by_uid '("<table canister id>:<seed_hash>")'
icp canister call history get_hand_by_uid '("<seed_hash>")'      # bare, if it is unambiguous
```

Case and surrounding whitespace are forgiven, because you will paste this off a screen. Nothing
else is: a 63-character paste is refused by name rather than resolved to a hand. A bare commitment
that somehow matched records on two tables is refused too, and the reply tells you which tables.

### If you hold an older citation

Reports written before hands had names cite `(table, hand number)`. Nothing about them was
renumbered and nothing was orphaned — `hand_id` and `hand_number` are exactly what they were, and
`get_hand(hand_id)` still answers. To find out what such a citation covers:

```
icp canister call history resolve_hand_number '(principal "<table>", <n> : nat64)'
```

It returns **every** record that answers to it, oldest first, each with its permanent name, its pot
and its timestamp — plus `is_unique`, which tells you whether the citation identifies a hand at
all. The pot or the timestamp you quoted will pin the one you meant; then cite its `hand_uid`.

### Checking that the archive's names really are names

The archive states it about itself, recounted from its own records rather than from its index:

```
icp canister call history get_archive_integrity --query
```

`name_collisions` and `records_without_a_usable_commitment` must both be **0** and
`index_disagrees_with_records` must be **false**. `ambiguous_hand_number_citations` is expected to
be large and is not a fault: it is the count of old citations that name a set, and it is the reason
this section exists. A record whose commitment is not a SHA-256 digest cannot be named, cannot be
verified, and is **refused** by `record_hand` rather than stored, so `records_without_a_usable_commitment`
is 0 by construction on any archive written by this code.

The same call also answers the no-rake question over **every record ever archived**:
`records_with_a_nonzero_rake` and `rake_recorded_total` must both be **0**. That matters because
every other check of that property in this repository reads a handful of recent hands, so a rake on
an older record was checked by nothing at all.

None of these counts are read out of an index. They are recounted from the records themselves on
every call, because an index cannot be used to check itself.

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

### Where `P` comes from — READ IT, DO NOT COUNT IT

`P` is the number of seats that were **dealt in**, which is not necessarily the number of seats
occupied: a player who was sitting out or had no chips when the hand started took no cards, and a
player can leave or arrive while the hand is still running.

**The record states it.** Every hand record carries a `dealt_in` field: an ordered list of
`{ seat; principal }`, one entry per seat the deal fed, in the order the deck was consumed. So:

```
P                              = length of dealt_in
hole cards of dealt_in[k]      = deck[2k], deck[2k+1]
```

It is on both surfaces, and both are the same list:

```
icp canister call history  get_hand <hand_id>            # dealt_in
icp canister call table_1  get_hand_history <hand_number> # dealt_in, and participants
```

`check_recorded_hand(hand_id)` prints the exact verifier command for the hand, with its `P` already
filled in, in `verify_it_yourself`.

> **This section used to say "count it from the hand's own record — every seat the history shows
> with cards, plus any that folded".** That instruction was wrong, and following it produced the
> wrong board. The player list it told you to count was built from the seats **as they stood when
> the hand settled**, so on any hand somebody left it was short by one and on any hand somebody
> joined it was long by one. Measured, on a four-handed hand one player left after the flop:
> `P = 4` gives the flop `9d 4d 8c`, and `P = 3` — the seats still occupied at settlement — gives
> `6h 5h 9d`. Both are internally consistent; only one is the hand that was played. An independent
> auditor hit exactly this and reported the table had dealt a board that did not follow from its
> own seed. See [DEFECTS.md E-66](DEFECTS.md#e-66) and
> [SECURITY-FINDINGS.md FINDING 30](SECURITY-FINDINGS.md#finding-30).

If a record's `dealt_in` is **null**, it was written before the canister recorded this, and its
board cannot be reproduced. Do not guess `P`: the hole cards will still match at any `P` (they come
from the front of the deck) while the board silently comes out wrong, which is the one mistake a
verifier is likely to make and cannot detect. A record with a null `dealt_in` is a hand whose
shuffle you can check only as far as the hole cards.

Your own client's view of the table at the moment the hand started is an independent second source
and is worth using when you have it.

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

## 6a. How long the proof survives, and who can destroy it

A fairness guarantee you cannot re-check tomorrow is not a fairness guarantee. So, precisely:

| where the proof lives | how long | who can destroy it |
|---|---|---|
| the **table canister**, `get_hand_history(hand_number)` | the **last 100 hands**. `periodic_cleanup` drains the excess; heads-up that is under an hour of play | any **controller of the table**, in one call: `reset_table` erases all 100 at once. `admin_reinit_table` does the same |
| the **archive canister** (`history`), `get_hand(hand_id)` | **for the life of the canister.** No method deletes, prunes, edits or expires a record, and there is no cap. Re-sending a hand already stored returns the existing id instead of writing a second copy | any **controller of the archive**, by reinstalling or deleting the canister itself. No application code can prevent that. The archive's `admin` cannot remove a record, but can admit a new writer with `authorize_table` |
| **your own machine** | as long as you keep it | you |

Both canisters state this themselves, so the paragraph above is checkable rather than believable:

```
icp canister call table_1 get_fairness_retention   # the table's copy, and its cap
icp canister call history get_retention_policy     # the archive's copy, and its admin
icp canister call table_1 get_history_status       # is archiving actually working RIGHT NOW
```

`get_history_status` matters more than it looks. Through wave 5 the archive was deployed, authorised
for no tables and holding zero records; every `record_hand` was rejected and the only report was an
`ic_cdk::println!` nobody can read ([DEFECTS.md T-34](DEFECTS.md#t-34)). A table now counts both
outcomes, keeps the last error verbatim, and holds every un-archived hand for retry, which any
non-anonymous caller can push with `flush_unrecorded_hands`. If `history_canister` is `null`, or
`unrecorded_backlog` is not zero, **nothing durable is being written and you should copy the seed
hash off your own screen.**

## 7. How this is tested

| test | what it pins |
|---|---|
| `poker_core::shuffle::tests` (host) | `bound(n)` is the largest multiple of `n` below `2^64`; the rejection branch redraws instead of reducing a rejected value; the chain advances on every digest; the old 32-bit-truncating shuffle no longer reproduces our output |
| `poker_core/tests/golden_vectors.rs` (host) | 2,000 seed/deck pairs, plus the pre-shuffle deck order |
| `poker_core/tests/wasm32_golden.rs` (**wasm32**) | the same 2,000 pairs, replayed inside a `wasm32-unknown-unknown` module, which is the target the canister runs on. This is the test that would have caught FINDING-02 |
| `.github/workflows/ci.yml` job `wasm32-tests` | runs the wasm32 replay on every push and pull request |
| `src/poker_core/tests/verify/*`, driven by `wasm32_golden.rs::outsider_reimplementations_reproduce_the_vectors` | the Python and JavaScript verifiers replaying all 2,000 vectors, so THIS DOCUMENT drifting from the code is a test failure |
| the proof of the fix (2026-08-04) | two hands dealt on the real canister wasm under PocketIC, then reproduced — hole cards and full board — from the revealed seed alone by both outsider verifiers. The pre-fix arithmetic reproduces 0 of 9 and 0 of 11 of those cards |
| `table_canister::commitment_check_tests` (host) | that **no honest caller can get an accusation out of `check_shuffle_commitment` by making a mistake**: the documented placement, the transposed placement, upper case, surrounding whitespace, a truncated paste, a principal in the hash field, an empty field and a genuine cross-hand mismatch each get the arm that names what happened, and both matching arms still carry their own `this_does_not_prove` |
| `history_canister::retention_tests` (host) | that a re-sent hand is stored **once**, that a hand reusing a number after `reset_table` is **still stored** (the failure mode that silently destroys proofs, [DEFECTS.md E-49](DEFECTS.md#e-49)), and that 250 consecutive writes never shrink the archive |
| the durability run (2026-08-05) | 101 real hands on a local table: all 101 acknowledged by the archive, then the table's copy destroyed twice over — pruned to 100 by `periodic_cleanup` and then wiped entirely by `reset_table` — with hand 1 still readable from the archive and still verifying against `shasum` off-chain |
| `tests/money_safety/tests/invariants/archive.rs` (PocketIC, real table + real archive) | **section 4 end to end on a hand somebody left.** Four players dealt in, one leaves after putting money in the pot, another buys the empty chair; the hand settles and is archived. The record must name the player who left, must not name the one who arrived, must state `P`, and the board must come back out of `deck[2P+1..]` for the `P` the record states. It also asserts that the `P` the OLD record implied gives a *different* board, so the test cannot pass vacuously |
| M12 ARCHIVE FIDELITY in `tests/money_safety/src/invariants/record.rs`, on every hand the fuzzer settles | that the record says who played, states who was dealt in, adds up, names nobody who neither took a card nor put in a chip, and — on every hand the harness could measure independently — attributes to each person exactly what it watched them stake |

## 8. Version history

| version | date | change |
|---|---|---|
| v1 | 2026-08-04 | First specification. Width-independent draw plus the rejection rule; golden vectors regenerated by wasm32 execution. Supersedes the undocumented pre-`ceacc37` behaviour, which truncated the draw to 32 bits on-chain and could not be reproduced by anyone. No hand had ever been dealt on mainnet, so no history was invalidated. |
| v1 | 2026-08-05 | **No change to the algorithm**, so every vector and every hand already dealt is unaffected. Documentation only: section 0 now claims what an outsider can check ("the whole 52-card order was fixed before the board was shown") and files the ordering claim under NOT proven; section 1's step 0 describes `check_shuffle_commitment` and why asking the canister proves nothing; section 6a states how long a proof survives and who can destroy it. |
| v1 | 2026-08-06 | **No change to the algorithm.** Section 4's instruction for obtaining `P` was WRONG and is replaced: it told a verifier to count the players in the hand record, and that list was built from the seats at settlement, so on any hand somebody left or joined it gave the wrong `P` and therefore the wrong board from the right seed. The record now carries `dealt_in`, the deal's own ordered list, and section 4 says to read it. Records written before this carry `dealt_in = null` and their boards cannot be reproduced; section 4 says that too, rather than letting a verifier guess. [SECURITY-FINDINGS.md FINDING 30](SECURITY-FINDINGS.md#finding-30). |
| v1 | 2026-08-08 | **No change to the algorithm**, so every vector and every hand already dealt is unaffected, and no archived record was rewritten, renumbered or reindexed. New section 1a: a hand is NAMED `table_id:seed_hash`, because `(table, hand_number)` never was a key — measured over 3,218 archived hands, 947 of 1,651 citations named more than one record and `table_2` "hand 1" answered to 70 of them with 6 different pots. The name is derived from two fields every record already carried, so every record already stored acquired one the moment the code shipped; an older `(table, hand number)` citation still resolves, through `resolve_hand_number`, to the labelled set it always named. [DEFECTS.md E-71](DEFECTS.md#e-71). |
