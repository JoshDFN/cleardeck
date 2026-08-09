# FINDING 42 — the deposit address derived from a substituted canister id

Produced by `node tools/shots/repro-finding42.mjs` against the LOCAL replica.
Nothing here touches mainnet, and nothing here touches the deployed local stack:
the hostile canisters are new, and the substituted bundle is served off local
disk through the harness's `SHOTS_SERVE_DIST` path while every `/api` call still
goes to the real replica.

The PNGs are not tracked (see `.gitignore`, same policy as `artifacts/screens`).
This file and the two `evidence.json` files are the record; re-run the command to
regenerate the pictures.

    exit 1  the substitution reached the player   (the unfixed build)
    exit 0  the deposit path refused it           (this build)

## The fixture

| | |
|---|---|
| substituted lobby | `5iptu-f3777-77775-aaaga-cai` — a second lobby canister running the real lobby wasm, whose `get_tables()` names a table this build was never published with |
| substituted table | `55icz-et777-77775-aaafq-cai` — a table canister running the real table wasm. Same code, different id; that is the whole point |
| the table this build pins | `4caro-hl777-77775-aaaba-cai` (`table_1`) |
| victim | dev player 4, `4tgka-ghyhm-4ebbe-3xqm4-alsxm-onnix-mi73y-5bcjg-ecekb-coffw-zqe` |

A lobby that lies and a replica that lies put the same bytes on the wire.
`get_tables : () -> (vec TableInfo) query` is answered by one replica out of its
own memory, and this client verifies no certificate over it.

## pre-fix/ — revision `4e08c6d`, exit 1

| | |
|---|---|
| deposit address at the PINNED table | `8ad30532f9cfef73db25a4ecac2060573b19eb24191006cd8700a83fc9c4e531` |
| deposit address at the SUBSTITUTED canister | `aa86e03ef768732ea696ba66eaef1c78b3f6abdce41d2c9dac2af183bd94a05e` |
| **rendered to the player** | **`aa86e03ef768732ea696ba66eaef1c78b3f6abdce41d2c9dac2af183bd94a05e`** |
| warning beside it | none |
| trust attribute on the dialog | absent (there was no such decision) |
| Claim Deposit button | enabled |
| pinned table ids found in the bundle | none — the build carried no trust root at all |
| console errors | none |

Rendered under the heading **"YOUR DEPOSIT ADDRESS — Yours alone, at this table.
Send ICP here from any wallet or exchange, then Claim:"**, with a Copy Address
button. Every check on the screen reads green, because the canister the modal
cross-examines is the substituted one, and it answers consistently about itself.

The account is inside a canister whose controller can pay its entire ledger
balance to itself with one `install_code` — the capability
[FINDING 23](../../docs/SECURITY-FINDINGS.md#finding-23) executed for
39.99990000 ICP.

## post-fix/ — exit 0

| | |
|---|---|
| rendered address | none |
| trust attribute on the dialog | `refused` |
| pinned table ids found in the bundle | 3 of 3 |
| console errors | none |

> **This is not one of this build's tables. Nothing can be sent here.** Nothing
> can be deposited to 55icz-et777-77775-aaafq-cai: this table's canister id was
> sent to your browser by the lobby, over an ordinary query that carries no
> proof, and it is NOT one of the canister ids this build was published with.
> Your deposit address is computed FROM that id, so if the id is wrong the
> address belongs to whoever owns that canister and the money is theirs the
> moment you send it. The tables this build knows about are: … (the local
> canister ids this bundle was built against).

The address panel says the same thing in its own words ("No address is shown, on
purpose: …"), the Deposit and Claim buttons are disabled, and `handleDeposit()`
refuses before the ICRC-2 approval or the OISY transfer can be signed.

## The same claim, without a browser

`cd tests/money_safety && cargo test --test deposit_trust_root` — 7 tests, no
replica, ~0.1 s. Verified `0 passed; 7 failed` on the pre-fix tree and 7 passed
here; two plausible wrong fixes were also driven and each was caught by exactly
one test. See [FINDING 42](../../docs/SECURITY-FINDINGS.md#finding-42).
