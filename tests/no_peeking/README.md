# The no-peeking harness

Runs a real ClearDeck hand against the **sealed dealer** (`src/no_peeking`) on
PocketIC, and then attacks it with the auditor's own attack.

```bash
cd tests/no_peeking
cargo test -- --nocapture --test-threads=2
```

Needs a PocketIC server binary: `$POCKET_IC_BIN`, else `$(dfx cache show)/pocket-ic`.
The harness builds the two spike modules itself, so what it attacks is always the
code in the tree. **Local only**: nothing in this crate knows a mainnet canister id.

| target | what a green run means |
|---|---|
| `--lib` | The leak DETECTOR works: it finds a card in a bare reply, three containers deep, inside a `Result`, and in the second of several reply values — and does not fire on a reply with no card. A negative test whose detector is broken passes while broken. |
| `--test no_peek` | **The negative.** 26 methods read out of the two wasm modules' export sections, 5 attackers (the table's controller, the table canister itself, a seated opponent, a stranger, anonymous), 13 argument shapes, 1,690 calls, 885 kept replies, 0 leaks. Plus `install_code` / `stop_canister` / `take_canister_snapshot` / `update_settings` against the dealer, all refused, and a real snapshot of the table downloaded and searched for the seed. |
| `--test full_hand` | A whole hand with a real side pot, settled from cards the table never held, and the shuffle still reproducible by a stranger from the revealed seed per `docs/SHUFFLE-SPEC.md`. |
| `--test disconnect` | A silent seat, an attentive seat the table cannot time out, a heartbeating griefer who is bounded rather than able to freeze the hand, a dead table whose pot still pays the real winner, and a dealer that refuses to open a hand it might not be able to finish. |
| `--test measurements` | Cycles and rounds, measured rather than budgeted. |

## Read `no_peek.rs` first

It is the file the wave exists for, and it is written the way a negative test has
to be written, because a negative test's failure mode is passing while broken:

* the method list comes from the **module**, not from a `.did`, so a method added
  tomorrow is attacked tomorrow — and a method the harness cannot call at all fails
  the run rather than being skipped;
* the secrets are learned **after** the attack, by forcing the hand open on the
  last-resort clock, so nothing about the sweep could have been tuned to the answer;
* there is a **positive control**: the same detector, on the same recorded bytes,
  does find alice's own two cards in the four replies where she asked for them. If
  it ever finds zero, the run fails.
