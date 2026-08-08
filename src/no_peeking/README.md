# The no-peeking spike

**A SPIKE. Nothing here is deployed, nothing here is in `icp.yaml`, and nothing in
`src/table_canister`, `src/poker_core` or the frontend was changed to build it.**

## What it is

`docs/NO-PEEKING-FEASIBILITY.md` answered "can a platform primitive stop a
controller reading the deck mid-hand, and at what price". Its answer was: **not
vetKD** (proven, three transport keys and a real upgrade returned the identical
secret), **not deleting the field** (proven, the deck came out of a downloaded
canister snapshot with four stock `dfx` commands), but **a separate canister with
an empty controller list**.

This directory is that canister, plus the smallest table that can exercise it.

| crate | what it is |
|---|---|
| `dealer_types` | The Candid wire types and the deal arithmetic from `docs/SHUFFLE-SPEC.md` §4. No `ic-cdk`, so it builds for the host and the arithmetic is unit-testable. Linked by everybody, so there is no second copy to drift. |
| `dealer_canister` | **The sealed dealer.** Holds the seed and the cards. Installed, then its controllers are dropped to `[]`. Has no `get_deck`, no `get_seed`, no admin method, no `post_upgrade`, and writes no log line. `lib.rs` is only the canister SURFACE; `state.rs` holds the state and the arithmetic that decides which card is whose. |
| `table_stub` | A minimal table: seats, blinds, betting, all-ins, side pots, showdown, settlement — and **no deck, no seed, no hole-card field**. It keeps a controller, on purpose, because that controller is the adversary. `settlement.rs` is the `poker_core` call sites, unchanged, taking the cards as an argument. |

The harness that attacks it is `tests/no_peeking`.

## The one sentence

**The secret of a ClearDeck hand is 32 bytes.** The shuffle is deterministic, so
whoever holds the seed holds every card. Today that is `CURRENT_SEED` inside the
fund-holding table canister, whose controller can read it three different ways
(`get_table_state`, a snapshot download, or an upgrade to code that prints it).
Here it is inside a canister that has no controller and exposes no method that
returns it.

## Build and test

```bash
./src/no_peeking/build.sh            # both modules, with their sha256
./src/no_peeking/build.sh --did      # and regenerate the two .did files
cd src/no_peeking && cargo test      # 11 host tests: the deal arithmetic

cd tests/no_peeking && cargo test -- --nocapture --test-threads=2
```

The harness needs a PocketIC server binary (`$POCKET_IC_BIN`, else
`$(dfx cache show)/pocket-ic`) and builds these modules itself, so what it attacks
is always the code in the tree.

## Why it is a detached workspace

Same rule as `src/guardian_canister` and `tests/money_safety`. The root
`Cargo.lock` is an input to `cargo build --locked` for the six modules that are
live on mainnet, and mainnet matches a reproducible build 6 of 6. A crate that
cannot change the root lockfile cannot change those hashes, so "the tree still
builds the deployed modules byte for byte" needs no argument. `git diff Cargo.lock`
after this spike was written is empty.

`poker_core` is pulled in **by path**, never vendored: the dealer must shuffle with
the same function the live engine shuffles with, or `docs/SHUFFLE-SPEC.md` and the
five independent reproductions of it stop applying to the hands it deals.

## What this is NOT

* It is **not** a migration. The live engine is untouched and every existing test
  is still green.
* It does **not** hold money. `table_stub` has no ledger and mints chips on
  request; the deposit/withdraw path is `tests/money_safety`'s subject and is
  deliberately out of scope.
* It does **not** deliver "nobody can peek". Node operators on a standard
  application subnet can read canister memory, and the dashboard reports
  `sev_enabled = false` on both the subnet ClearDeck's tables run on and the one
  holding vetKD `key_1`. The claim this construction supports, and the only one
  that survives an auditor, is:

  > **No principal can peek, and no controller key exists that could.**

* It does **not** hide a hand after it is over. The seed is published when the hand
  ends, on purpose, because that is what makes the shuffle verifiable — and the
  seed opens every card, including a folded player's. That is unchanged behaviour,
  it is stated in `docs/SHUFFLE-SPEC.md`, and the fix for it is Option C (vetKD for
  the archive) in a different canister.

## If you deploy this

The dealer is only what this claims to be once its controller list is **empty**.
Before dropping the controllers, and it is one-way:

1. Set the freezing threshold generously. `update_settings` on a zero-controller
   canister is refused forever, so this number can never be raised again.
2. Set `min_open_balance` so the dealer stops opening hands long before it could
   freeze in the middle of one.
3. Rehearse the handover on the local replica the way `docs/SECURITY-FINDINGS.md`
   FINDING 23 §6 describes.
4. Publish the fact that anyone may top it up with
   `dfx canister deposit-cycles <amount> <dealer>`, which needs no controller.
