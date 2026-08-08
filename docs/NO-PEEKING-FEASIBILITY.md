# NO-PEEKING FEASIBILITY: what the Internet Computer will actually let us ship

**Question this answers:** the auditor's finding is that any controller can read the whole
shuffled deck and every player's hole cards mid-hand
(`src/table_canister/src/lib.rs` `get_table_state`). Can a platform primitive close that, and
at what price?

**Status:** spike. Nothing in the live engine was changed. All measurements were taken on the
project's own local replica (PocketIC `v15.0.0-2026-07-17-04-19`, gateway `:8077`) against a
purpose-built probe canister. No mainnet call was made; the only mainnet contact is read-only
public HTTPS to `ic-api.internetcomputer.org` to learn which subnet the live tables sit on.

> **§1–§9 are the FEASIBILITY study: what the platform allows, and at what price. §10 is the
> BUILD, added afterwards.** §8 recommended a sealed dealer and said plainly that it was proven
> at the primitive level and not at the game level, leaving three questions open: the showdown
> split, how a street advances without giving the table's controller a private early peek, and
> what happens to a hand in flight if the dealer freezes. §10 answers all three by executing
> them, in `src/no_peeking` and `tests/no_peeking`, and reports one number that came out
> materially worse than §7.5 budgeted.

---

## 0. Verdict, in four sentences

1. **vetKD is real, live, and locally callable, and it does not solve this problem.** The
   derived key is a pure function of `(canister_id, context, input)`. Anyone who can run code
   in the table canister can re-derive any player's key with a transport key of their own
   choosing. This was executed, not reasoned about: three runs with three different transport
   keys, one of them after a genuine code upgrade, returned **the identical secret**.
2. **Deleting the deck from `get_table_state` does not close the finding either.** A controller
   can take a canister snapshot and download the raw wasm heap. The full deck and both players'
   hole cards were read out of a downloaded snapshot with stock `dfx` and `strings`, with no
   code change and no upgrade.
3. **The one construction that reaches the stated goal today is a separate dealer canister with
   an empty controller list.** Measured locally: it still serves calls, still calls vetKD, still
   accepts cycle top-ups from anybody. `install_code`, `stop_canister`,
   `take_canister_snapshot` and re-adding a controller are all refused with `IC0512`. Cost:
   roughly one inter-canister hop per hand, which is about 30,000× less than the vetKD design
   per deal, and strictly stronger against the controller.
4. **The residual, and it is not small: node operators on both relevant subnets can read
   canister memory.** IC docs say so plainly, and the dashboard confirms `sev_enabled = false`
   on the 13-node application subnet the tables run on *and* on the 34-node fiduciary subnet
   that holds vetKD `key_1`. A construction where **nobody** can read the deck requires the deck
   to never exist in plaintext on-chain, which means mental poker, which means the disconnect
   problem. See §6.

---

## 1. How to reproduce every number here

The probe canister lives outside this repo, in the wave's scratch directory:

```
$SCRATCH/vetkd_probe/          # cargo + dfx project, deploys to the ClearDeck local replica
  src/lib.rs                   # every probe method
  dfx.json                     # network "cd" -> http://localhost:8077 (the icp-cli gateway)
```

```bash
# The ClearDeck local network must already be up (./scripts/dev.sh local-up).
cd $SCRATCH/vetkd_probe
cargo build --release --target wasm32-unknown-unknown
candid-extractor target/wasm32-unknown-unknown/release/vetkd_probe.wasm > vetkd_probe.did
dfx deploy vetkd_probe --network cd

dfx canister call vetkd_probe probe_public_key '("test_key_1","cleardeck-holecards-v1")' --network cd
dfx canister call vetkd_probe probe_cost '("key_1")' --network cd
dfx canister call vetkd_probe probe_derive_concurrent '("test_key_1", 20 : nat32)' --network cd
dfx canister call vetkd_probe attack_rederive \
  '("test_key_1","cleardeck-holecards-v1","table_2:hand_41:seat_3","any-seed-you-like")' --network cd
```

The local network's subnet topology (which is how we know a threshold-key subnet exists locally)
comes from PocketIC's own REST API:

```bash
# find the pocket-ic process that owns :8077, then its API port
lsof -nP -a -p <pid> -iTCP -sTCP:LISTEN
curl -s http://127.0.0.1:<api-port>/read/topology
```

That reply contains a subnet of kind `TestThresholdKeys` with id
`fuqsr-in2lc-zbcjj-ydmcw-pzq7h-4xm2z-pto4i-dcyee-5z4rz-x63ji-nae`, alongside the `Application`
subnet the probe canister was actually installed on. So every vetKD call measured below is a
cross-subnet call, the same shape it would have on mainnet.

---

## 2. vetKD: availability, interface, cost, latency

### 2.1 It is live, and it is available locally

| Question | Answer | Evidence |
|---|---|---|
| Live on mainnet? | Yes. "The vetKD management canister API is live on mainnet." Production key `key_1`. | [ICP docs, VetKeys](https://docs.internetcomputer.org/concepts/vetkeys/) |
| Which subnet holds `key_1`? | `pzp6e-…`, a **34-node fiduciary** subnet. | dashboard: `subnet_specialization = fiduciary`, `total_nodes = 34` |
| Which subnet do ClearDeck's tables run on? | `jtdsg-3h6gi-hs7o5-z2soi-43w3z-soyl3-ajnp3-ekni5-sw553-5kw67-nqe`, a **13-node application** subnet, 33,770 running canisters. | `ic-api.internetcomputer.org/api/v3/canisters/<id>` |
| So is the call local to our subnet? | **No.** Every `vetkd_derive_key` from a ClearDeck table would be a cross-subnet call to `pzp6e`. The fee is set by the subnet where the key lives, not ours. | ICP docs, vetKD API |
| Available on the local replica? | **Yes, measured.** | below |
| Does the CDK we already use support it? | **Yes.** `ic-cdk 0.19`, already a `table_canister` dependency, exports `vetkd_public_key`, `vetkd_derive_key`, `cost_vetkd_derive_key`. No new dependency needed. | `ic-cdk-0.19.0/src/management_canister.rs:811,827,860` |

Local `vetkd_public_key` results, one call per key name:

| key name | result |
|---|---|
| `test_key_1` | **ok**, 96-byte public key `9333c5e551638b49…` |
| `key_1` | **ok**, 96-byte public key `b172a609318e4efa…` |
| `dfx_test_key` | **ok**, 96-byte public key `9679ac9b5afaab65…` |
| `insecure_test_key_1` | rejected: `ChainKeyError("Requested unknown threshold key: vetkd:Bls12_381_G2:insecure_test_key_1")`. That name belongs to the mainnet fake-key testing canister `vrqyr-saaaa-aaaan-qzn4q-cai`, not to a replica. |

`vetkd_derive_key` succeeded locally for both `test_key_1` and `key_1`, returning a **192-byte**
`encrypted_key`.

### 2.2 The exact interface

From the IC interface spec (management canister). This is the *current* shape. Note it is
`input` / `context` / `transport_public_key`, **not** the older `derivation_id` /
`derivation_path` / `encryption_public_key` that pre-2025 material describes:

```candid
type vetkd_curve = variant { bls12_381_g2 };

type vetkd_public_key_args = record {
    canister_id : opt canister_id;
    context : blob;
    key_id : record { curve : vetkd_curve; name : text };
};
type vetkd_public_key_result = record { public_key : blob };

type vetkd_derive_key_args = record {
    input : blob;
    context : blob;
    transport_public_key : blob;
    key_id : record { curve : vetkd_curve; name : text };
};
type vetkd_derive_key_result = record { encrypted_key : blob };
```

`vetkd_public_key` is a **bounded-wait** call and costs nothing. `vetkd_derive_key` is an
**unbounded-wait** call and must carry cycles; `ic-cdk`'s wrapper attaches them for you from
`cost_vetkd_derive_key`.

### 2.3 Cost, asked of the subnet itself rather than recalled

`ic0.cost_vetkd_derive_key` is a system call that reports the price the *serving* subnet will
charge. Queried through the probe on the local replica:

| key name | cycles per derivation | ≈ USD |
|---|---|---|
| `test_key_1` | `10,000,000,000` | $0.0137 |
| `key_1` | `26,153,846,153` | $0.0358 |

These match the published [cycle-costs table](https://docs.internetcomputer.org/references/cycle-costs/)
exactly, which is a useful cross-check: the local replica is not quoting a toy price.

> The local replica does **not** actually debit the canister. The probe's balance read
> `3,500,000,000,000` cycles before and after 40 derivations. So the cycle figures above come
> from the subnet's own price oracle and the published table, not from an observed debit. That
> is the honest provenance and it is the best available without calling mainnet.

### 2.4 Latency: the shape is what matters, not the local milliseconds

PocketIC's clock is not mainnet's clock, so the useful measurement is **round structure**, which
is platform-independent. Measured with `ic_cdk::api::time()` across the awaits:

| pattern | n | elapsed | interpretation |
|---|---|---|---|
| baseline `raw_rand` (what a hand costs today) | 1 | 232 ms | 1 round |
| `vetkd_derive_key`, sequential awaits | 1 | 229 ms | 1 round |
| " | 2 | 469 ms | 2 rounds |
| " | 13 | 3,072 ms | 13 rounds |
| " | 17 | 4,239 ms | 17 rounds |
| `vetkd_derive_key`, all issued then joined | 2 | 237 ms | **1 round** |
| " | 13 | 271 ms | **1 round** |
| " | 17 | 280 ms | **1 round** |

So: **sequential derivation is linear in the number of cards and is disqualifying. Batched
derivation collapses to a single round-trip.** Any design that uses vetKD must issue every
derivation of a deal concurrently. That is a hard constraint on the code, not a preference.

For mainnet seconds I have no measurement and will not invent one. The relevant published
evidence is a production report from a developer running 50+ canisters: `vetkd_derive_key`
regularly returned *"Message did not complete execution and certification within the replica
defined timeout"*, and DFINITY's answer was that the v3 synchronous call endpoint has a
**10-second** ceiling and cross-subnet vetKD derivation can exceed it
([forum thread, 2025-07-31 → 2025-08-04](https://forum.dfinity.org/t/threshold-key-derivation-vetkd-vetkey-message-did-not-complete-execution-and-certification-within-the-replica-defined-timeout/54257)).
A deal that occasionally blows past 10 seconds against a 30–45 s action clock is a product
problem, not a tuning problem.

### 2.5 The throughput ceiling nobody mentions: a 20-deep shared queue

Batching is free until it isn't. Bisected locally, one canister, one message:

| concurrent derivations | result |
|---|---|
| 20 | **ok** (3/3 repeats) |
| 21 | **fails** (3/3 repeats) |
| 52 | fails |

```
CallRejected { raw_reject_code: 4,
  reject_message: "vetkd_derive_key request failed: request queue for key
                   vetkd:Bls12_381_G2:test_key_1 is full." }
```

Then the question that decides whether a poker *room* can use this: is the queue per canister,
or shared? Two **different** canisters firing simultaneously:

| canister A | canister B | total | result |
|---|---|---|---|
| 10 | 10 | 20 | **both ok** |
| 11 | 11 | 22 | **both fail** |
| 15 | 15 | 30 | both fail |

**The queue is shared per key across the subnet.** This is not a PocketIC artefact: DFINITY
staff confirm the same constant for chain-key signing on mainnet: *"The limit of a queue of 20
is actually per signing subnet. This is not a hard limit and could be changed if there is
demand"*, from `max_queue_size: 20` in the subnet's chain-key config, and *"increasing the queue
size would also increase the max latency one may expect for signing"*
([forum](https://forum.dfinity.org/t/signature-queue-for-key-ecdsakey-1-is-full/33647)).

Two consequences:

- On mainnet, ClearDeck would be sharing a 20-deep queue for `key_1` with **every other dapp on
  `pzp6e`** (3,968 running canisters). We would not control our own deal availability.
- The reject code is `4` (CANISTER_REJECT), not a transient system reject. A caller must
  implement its own retry with backoff; nothing retries for you. A retry loop inside a deal is a
  new source of nondeterministic latency in the most timing-sensitive moment of the hand.

---

## 3. The finding that decides this: vetKD does not lock out the controller

vetKD's threat model is "no *node* sees the raw key". It is not "no *controller* sees the raw
key". The derived key is a deterministic function of `(canister_id, context, input)`; the
transport public key only wraps it for the trip home. So anybody who can cause the canister to
execute code chooses the transport key, and therefore gets the secret.

I did not want to assert that. `attack_rederive` derives a key, decrypts it with the transport
secret, verifies it against the canister's own derived public key, and reports the raw vetKey
and the symmetric key an implementation would actually use for a card.

Three runs, identical `context = "cleardeck-holecards-v1"` and
`input = "table_2:hand_41:seat_3"`:

| run | transport seed | canister code | `encrypted_key` (wire) | recovered vetKey | symmetric key |
|---|---|---|---|---|---|
| A | `alices-own-secret` | build-A | `…a5e99744df5c…` | `94532b7763412bdc…d772144b9e583dc159f3` | `d6d48efe…a705cdad` |
| B | `operator-chosen-key` | build-A | `…a5e9b8f48894…` | `94532b7763412bdc…d772144b9e583dc159f3` | `d6d48efe…a705cdad` |
| C | `attacker-seed-post-upgrade` | **build-B**, module hash `0x8bc9948652f2d540…` after a real `install_code --mode upgrade` | `…a5e98922a7e8…` | `94532b7763412bdc…d772144b9e583dc159f3` | `d6d48efe…a705cdad` |

The wire blob differs every time, and the transport wrapping works exactly as advertised. **The
secret underneath is byte-identical in all three.** Run C carries a different build marker
returned by the canister itself, so the code genuinely changed between B and C.

Read plainly: if hole cards were IBE-encrypted to players under a vetKey derived by the table
canister, the operator's recovery is *one upgrade away*, it works retroactively on every hand
ever dealt, and it leaves no trace in the application's own logs. That is the same power the
auditor found, moved behind a cryptographic curtain. It would be worse than the current state,
because the current state is at least legible: `get_table_state` is one obvious function with a
comment on it saying what it does.

### 3.1 And there is a second controller channel that needs no code at all

Even with `get_table_state` deleted, `take_canister_snapshot` / `read_canister_snapshot_data`
are management-canister methods gated only on controllership, and they return the raw wasm heap.
Executed locally against the probe, which had stashed a deck in a heap global exactly the way a
table canister holds one mid-hand:

```
$ dfx canister stop  vetkd_probe --network cd
$ dfx canister snapshot create   vetkd_probe --network cd
$ dfx canister snapshot download vetkd_probe <id> --dir ./snap --network cd
$ strings -a snap/wasm_memory.bin | grep CLEARDECK-PEEK-DEMO
CLEARDECK-PEEK-DEMO|DECK=2s9s3sTs4sJs5sQs6sKs7sAs8s3hTh4hJh5hQh6hKh7hAh8h2h9h4dJd5dQd6dKd7dAd8d2d9d3dTd5cQc6cKc7cAc8c2c9c3cTc4cJc|SEAT3_HOLE=AsKd|SEAT1_HOLE=7c2h|CLEARDECK-PEEK-DEMO
```

Four stock commands. No code change, no upgrade, canister restarted afterwards. **Any fix that
only edits the canister's public interface fixes nothing.** The fix has to change *who holds the
deck*.

---

## 4. Threshold ECDSA and Schnorr

Older, more certainly available, and, for this problem, beside the point.

Measured locally, `sign_with_schnorr` (BIP340 secp256k1):

| key name | result | signature | rounds (n=1) | rounds (n=13 sequential) |
|---|---|---|---|---|
| `dfx_test_key` | ok | 64 bytes | 1 (227 ms) | n/a |
| `test_key_1` | ok | 64 bytes | 1 (229 ms) | 13 (3,053 ms) |
| `key_1` | ok | 64 bytes | 1 (224 ms) | n/a |

Same interface shape, same cost tier (`10,000,000,000` / `26,153,846,153` cycles), same 20-deep
shared queue.

**They cannot be composed into what we need.** The management canister offers
`sign_with_ecdsa`, `ecdsa_public_key`, `sign_with_schnorr`, `schnorr_public_key`: four signing
and public-key operations. There is no threshold *decryption* method anywhere in the interface
except `vetkd_derive_key`. A signature does not hide a card from anybody. The only encryption
capability the platform exposes is vetKD/IBE, and §3 shows what its trust boundary actually is.

What threshold signatures *could* usefully do for ClearDeck, none of which is this wave's
problem: co-sign the pre-deal shuffle commitment so an off-chain or cross-chain verifier can
check it without trusting an IC gateway; act as a VRF (`ic-vetkeys` exposes `VrfOutput`), but
`raw_rand` already gives unbiased randomness and the shuffle spec already commits to it.

---

## 5. The shape of the problem, primitive by primitive

Poker needs four things. Here is what each available primitive actually delivers.

| Requirement | `raw_rand` + hash shuffle (today) | vetKD / IBE | t-ECDSA / t-Schnorr | Zero-controller canister | Mental poker (player keys) |
|---|---|---|---|---|---|
| **A shuffled deck nobody can read** | ✗ canister, controller, snapshot, nodes | ✗, the deriving canister is the gatekeeper and its controller is the canister (§3) | ✗ no decryption capability | **✗ for nodes, ✓ for every principal** | **✓**, deck exists only as shares across players |
| **Each player reads exactly their own two cards** | ✓ `get_table_view` already scopes by caller | ✓ (this is IBE's natural shape) | ✗ | ✓ per-caller method, authenticated query | ✓ |
| **Community cards public at the right moment, not before** | ✓ by phase gate | ~ no time-lock primitive exists; still a code gate | ✗ | ✓ by phase gate, and see §7.3 | ~ needs a joint-reveal round per street |
| **At showdown, prove the cards were what is claimed** | **✓ already**: seed commitment + `docs/SHUFFLE-SPEC.md`; five auditors reproduced it | ~ neutral; would need a new proof | ~ could co-sign the commitment | **✓ unchanged** | ✗ needs a new proof system |
| **A disconnected player does not stall the hand** | ✓ first-class today | ✓ player re-authenticates, re-derives, needs no stored key | n/a | ✓ | **✗ this is the failure mode** |

Two rows carry the whole answer.

**Row 1, column vetKD:** the reason the ✗ is there is §3, and it is structural, not a
configuration mistake. To deal, *something* must know the deck. If a canister shuffles, that
canister holds the deck, and its controller can read it. Making the deck derive from a vetKey
instead of `raw_rand` does not help: the derivation is deterministic and re-runnable at will
(proven), so it hides the deck from a *state snapshot* but not from live code. The only way to
make a canister-held deck unreadable is to make the canister's code unchangeable, at which
point vetKD is no longer doing the work; immutability is.

**Row 5, column mental poker:** this is the price of the only ✓ in row 1, and this engine
already treats disconnect as a first-class event (FINDING 12, FINDING 16, FINDING 19, the
30/45 s action clocks and the time bank). Adopting a scheme where a disconnect makes a live hand
mathematically unopenable is walking straight into the failure this codebase has already spent
three waves engineering around.

---

## 6. The honest alternatives

### Option A. Sealed dealer: split the deck into a canister with no controllers

Move deck generation and card custody out of the fund-holding table into a new `dealer`
canister whose controller list is **empty**. The table keeps the money, the betting, the side
pots, the showdown evaluation and every upgrade path it has today. It never receives a hole
card.

I checked every premise of this against the replica rather than assuming it.

```
$ dfx canister call aaaaa-aa update_settings \
    '(record { canister_id = principal "xgyck-…"; settings = record { controllers = opt vec {} } })'
()
$ dfx canister info dealer_immutable
Controllers:
Module hash: 0xf9549ecb…
```

| Premise | Result |
|---|---|
| Still serves update calls | **✓** `probe_public_key` → ok |
| Still serves queries | **✓** `probe_cost` → ok |
| Still allowed to call `vetkd_derive_key` | **✓** ok, key recovered |
| Can anyone `install_code --mode upgrade` it (with genuinely different code)? | **✗ refused**: `Only controllers of canister xgyck-… can call ic00 method install_code, error code Some("IC0512")`. The identical command against the sibling canister that *does* have controllers succeeded in the same minute. |
| Can anyone `stop_canister` it? | **✗ refused** (controller error) |
| Can anyone snapshot it? | **✗ refused**, and snapshotting requires stopping first, which is also refused |
| Can anyone re-add themselves as controller? | **✗ refused** |
| Can anyone keep it alive with cycles? | **✓** `dfx canister deposit-cycles 1000000000000` succeeded from a non-controller. `deposit_cycles` is open to all principals. |

**What this delivers against the wave's success criterion.** "A hand where the canister itself
cannot name a player's hole cards, and neither can anyone holding its controller key." The
*table* canister, the one with a controller key and the one holding real ICP and ckBTC, provably
cannot name them, because they are not in it and `get_table_state` has nothing left to leak. The
dealer can name them, but **no controller key for the dealer exists**, and its interface has no
method that returns another player's card. That is a real, checkable property, not a promise.

**What it does not deliver.** Node operators on `jtdsg` can read the dealer's memory. IC's own
security page is explicit: *"Node operators on standard application subnets can read canister
memory"*, with SEV-SNP named as the future mitigation and the current advice being not to store
secrets in canister state at all. The dashboard reports `sev_enabled = false` for `jtdsg`, and
also for `pzp6e`, the fiduciary subnet holding vetKD `key_1`, so moving to vetKD would not have
escaped this either. This is the honest ceiling of any on-chain-deck design today.

**Trade-offs to state out loud:**

- *Immutability is permanent.* A dealer bug can never be patched. Mitigation: keep the dealer
  tiny (a shuffle, a seat map, three read methods), give it no money, and make the recovery path
  "deploy dealer v2 and point new hands at it". Crucially, swapping dealers **cannot**
  retroactively open old hands: dealer v1 remains uncontrolled and its state is unreachable.
- *The freezing threshold is frozen forever*, because `update_settings` is refused. It must be
  set generously at install, before the controllers are dropped. See the existing runway work:
  a canister below ~30 days of idle burn freezes, and true zero uninstalls.
- *The handover is one-way and must be scripted and rehearsed*, exactly like the guardian
  handover described in `docs/SECURITY-FINDINGS.md` FINDING 23 §6.
- *This composes with the guardian rather than competing with it.* The guardian
  (`src/guardian_canister`, currently deliberately absent from `icp.yaml`) protects **money**
  against a controller and explicitly cannot prevent a malicious upgrade. The sealed dealer
  protects **cards** against a controller by removing the upgrade verb entirely. Different
  problems, different mechanisms, no overlap.

### Option B. Mental poker with player-held keys

The classical answer, and the only one where the deck exists in plaintext nowhere: players
jointly shuffle a commutatively-encrypted deck, each holds a private key, and cards are opened
by cooperative decryption.

- **What it uniquely buys:** the deck is unreadable by the canister, by every controller, *and*
  by every node operator. It is the only option that clears row 1 of §5 outright.
- **The failure mode, which is not theoretical here:** a player who disconnects mid-hand holds
  a decryption share nobody else has. Their opponents' cards may become unopenable, and the pot
  cannot be awarded. This engine already treats disconnect as a routine, expected event with a
  clock attached.
- **The standard mitigations, and why each is a real cost:**
  - *Pre-committed recovery shares* (each player pre-splits their key so a threshold of the
    others can open their cards on timeout). This hands colluding players a legitimate
    mechanism for opening a live opponent's cards. It re-creates the exact scandal, with the
    protocol's blessing.
  - *Force-fold on timeout, open only after the hand is dead*. Sound, and probably the right
    shape, but it means a disconnected player's hand is dead rather than checked-down, which is
    a rules change players will feel.
  - *Client-side liveness requirements*. Mental poker adds a client round-trip per street.
    Against FINDING 19 ("nothing on chain moves the game, so liveness is outsourced to whoever
    has a browser tab open"), this makes an already-known weakness materially worse.
- **Scale of work:** a new shuffle protocol, a new proof system to replace the seed-commitment
  story that five auditors have already validated, a new client-side crypto stack, and a rewrite
  of the disconnect handling that took three waves to get right. This is a programme, not a
  wave.

### Option C. vetKD for what it is actually good at: the archive, and key recovery

Do not use vetKD for the deal. Use it for the one place ClearDeck has a genuine
key-management problem: the permanent hand archive in the `history` canister.

Today a folded player's hole cards are reconstructible from the archive by anyone, and one auditor
demonstrated exactly that. IBE-encrypting a folded player's cards to their own principal in the
archive means: the record stays permanent and complete, only that player can open it, and they
can open it from any device, years later, with **no stored key**. They just authenticate to
Internet Identity and re-derive. That last property is the thing no client-held-key scheme can
match, and it is worth having.

Cost is bounded and off the critical path: one derivation per player per *archive read*, not per
deal, paid by the reader, with no deal-time latency and no contention with the deal.

Caveat, carried forward from §3: whoever controls the deriving canister can also derive. So the
deriving canister for this must be the sealed dealer (no controllers), not `history`.

### Option D. SEV-SNP confidential subnets

Not available. `sev_enabled = false` on both `jtdsg` (where the tables live) and `pzp6e` (where
`key_1` lives). Nothing to evaluate today; this is on the list of things that would have to
change (§8).

### Option E. Do nothing but remove the field from `get_table_state`

**Rejected, and it is important to say why loudly**, because it is the obvious cheap fix and it
is worse than nothing: §3.1 reads the deck out of a downloaded snapshot with four stock commands
and no code change. Shipping this and calling the finding closed would convert a documented,
legible hole into an undocumented one.

---

## 7. Cost and latency budget

### 7.1 What a hand costs today (measured)

The deal is one `raw_rand` (1 round, no cycle fee) plus an in-canister Fisher-Yates over 52
cards. The probe's baseline reports **27,792 instructions** in the post-`await` slice, which is
the slice that does the shuffle. At the published rate (1B cycles per 1B instructions) plus the
5,000,000-cycle update message base, the deal step costs on the order of **5.03M cycles ≈
$0.0000069**.

> Measurement caveat, so these numbers are read correctly: `performance_counter(0)` counts
> instructions **within the current message execution**, and an `await` starts a new slice. So
> every `instructions` figure in this document is the final slice, not the whole call. That is
> the right number for the shuffle (it happens after the await) and for the BLS decrypt in §3
> (likewise), and it is why the `instructions` column of the batched-derivation runs stays flat
> at ~50k regardless of `n`. Those are callback slices that do almost nothing.

### 7.2 What a hand costs with vetKD

Two designs, because the difference is large and the naive one is the one people reach for.

**Design 1: one vetKey per seated player per hand** (a player's two cards travel under one
key; community cards need no key at all because they become public anyway):

| table | derivations | cycles (`key_1`) | ≈ USD/hand |
|---|---|---|---|
| Heads-up (2) | 2 | 52.3 B | **$0.072** |
| 6-max (6) | 6 | 156.9 B | **$0.215** |
| 9-max (9) | 9 | 235.4 B | **$0.322** |

**Design 2: one vetKey per card** (2 per player + 5 community):

| table | cards | cycles (`key_1`) | ≈ USD/hand | fits the 20-deep queue? |
|---|---|---|---|---|
| Heads-up | 9 | 235.4 B | $0.322 | yes |
| 6-max | 17 | 444.6 B | $0.609 | yes, with 3 slots to spare |
| 9-max | **23** | 601.5 B | $0.824 | **no: 23 > 20, the deal fails outright** |

That last row is not a load problem. A 9-max hand under Design 2 cannot be dealt in one batch
**even on an empty subnet**, and dealing it sequentially costs 23 rounds (§2.4).

### 7.3 The number that decides it

The deal step goes from ~5.03M cycles to 156.9B cycles for a 6-max hand: **a factor of about
31,000**.

And ClearDeck takes **no rake**. There is no revenue line this comes out of. At a routine 70
hands per hour:

| | per hand | per hour, one table | per year, one table |
|---|---|---|---|
| 6-max, Design 1, `key_1` | $0.215 | **$15.05** | **~$132,000** |
| Heads-up, Design 1, `key_1` | $0.072 | $5.02 | ~$44,000 |

One 6-max table, running, costs the operator roughly $132,000 a year in key derivations alone.
That is the finding, and it decides the design on its own, before latency, before the queue,
before §3 showing it does not even close the hole.

### 7.4 Throughput, at room scale

Design 1 issues 6 derivations for a 6-max deal. Against a **shared** 20-deep queue:

- 3 ClearDeck tables dealing in the same round = 18 → fits.
- 4 tables = 24 → **all four deals fail**, with a non-transient reject that we must retry
  ourselves.
- And that assumes the entire `key_1` queue is ours. On `pzp6e` it is shared with 3,968 other
  running canisters.

A poker room whose deals fail when four of its own tables happen to deal at once is not a poker
room.

### 7.5 What Option A costs

One inter-canister call from table to dealer per deal, and an authenticated **query** per player
to fetch their own cards.

- Locally, a same-subnet inter-canister call did **not advance the block clock at all**
  (`elapsed_ns = 0` for 1 and for 6 sequential hops), while every chain-key call advanced it by
  one round. On mainnet, budget at most one extra round and the published 260,000 cycles per
  cross-canister request/response.
- The player's card fetch is a query: **no consensus round, no cycle fee to the canister**. It
  is faster than today's path, not slower. (Queries are not certified, so a malicious node could
  lie to you about your own cards, but the seed reveal at hand end makes any such lie
  detectable after the fact, which is exactly the guarantee the existing shuffle spec provides.)

| design | cycles per 6-max deal | vs today | cost *added* to the deal |
|---|---|---|---|
| today | ~5.03 M | 1× | n/a |
| Option A, sealed dealer | ~5.29 M (one extra hop) | ~1.05× | 260,000 cycles |
| Option D1, vetKD `key_1` | ~156.9 B | ~31,000× | 156,923,076,918 cycles |

**Option A's deal costs about 30,000× less than the vetKD design, and the confidentiality it
adds costs about 600,000× less, while being strictly stronger against the controller.** That is
the whole report in one line.

---

## 8. Recommendation

**Build Option A. Do not use vetKD to hide cards. Keep vetKD on the shelf for Option C.**

Concretely, for the next wave:

1. **New `dealer` canister, controllers dropped to `[]` at handover.** It draws the seed via
   `raw_rand`, shuffles with the existing `poker_core::shuffle_deck` so `docs/SHUFFLE-SPEC.md`
   and all five external verifications stay valid word for word, and holds the deck.
2. **Its interface has no method that returns a card the caller is not entitled to.** Not gated
   on a role, but *absent*, the way `src/guardian_canister` has no `install_code` verb. Three
   methods: `my_hole_cards()` scoped to `msg_caller`, `community(street)` which returns nothing
   before the table advances the street, `reveal_showdown()` which publishes the cards of
   players who reached showdown. No `get_deck`. No debug method. No controller escape hatch,
   because there is no controller.
3. **Every community-card reveal is public and simultaneous.** The dealer must not have a
   private-reveal path, or a table controller could advance a street early and read the flop
   before players act. If every reveal is visible to everybody at once, an early reveal is
   evidence rather than an advantage.
4. **The table canister loses `hole_cards` and `deck` from its state entirely**, which is what
   makes the auditor's finding un-reintroducible rather than merely fixed. `get_table_state` can
   then stay exactly as it is; it will have nothing to leak. Showdown evaluation takes revealed
   cards from the dealer as input.
5. **Set the dealer's freezing threshold generously before dropping controllers**, and document
   that anyone may top it up with `deposit_cycles` (verified above). Rehearse the handover on
   the local replica the way FINDING 23 §6 describes.
6. **Write down what this does not defend against**, in the product's own words, next to the
   existing unaudited-alpha disclaimer: node operators on the subnet can read canister memory,
   `sev_enabled = false`, and that is a property of the platform today and not of ClearDeck.
   Overstating this would be worse than the original finding.

**What we would be able to say truthfully afterwards, which no other poker room can say:** there
is no key on earth that opens a ClearDeck hole card. Not because we promise not to use it, but
because the management canister refuses `install_code` on the canister that holds the cards, and
you can check that yourself with one `dfx canister info`.

---

## 9. What would have to change for the ideal answer

The ideal is "no entity anywhere can read the deck, and a disconnect does not stall the hand".
Today the platform does not reach it. The specific things that would move the line:

| Blocker | What would have to change |
|---|---|
| Node operators can read canister memory (`sev_enabled = false` on `jtdsg` and `pzp6e`) | SEV-SNP confidential subnets generally available, and ClearDeck's tables placed on one. This is the single change that would make Option A cryptographically complete rather than merely controller-proof. |
| vetKD's gatekeeper is the calling canister, so a controller can re-derive (§3) | A vetKD variant where the derivation is bound to the *requesting principal* rather than the calling canister: the subnet enforces "only principal P may obtain the key for identity P", not "the canister decides". Nothing in the current interface expresses that. |
| 20-deep shared chain-key queue, non-transient reject | A per-canister reservation, or a queue deep enough that a room's deals cannot collide. DFINITY has said the constant is not hard and is raisable on demand, but raising it also raises worst-case signing latency, so it is a trade, not a free win. |
| $0.036 per derivation against a no-rake room | An order-of-magnitude price reduction, or a design that needs one derivation per *session* rather than per hand. |
| Cross-subnet vetKD can exceed the 10 s sync-call window | vetKD keys available on ordinary application subnets, removing the cross-subnet hop. |
| Mental poker's disconnect stall | Nothing platform-side fixes this; it is inherent to player-held keys. It needs a rules decision (force-fold and open only once the hand is dead), not a platform feature. |

Until at least the first row changes, **"nobody can peek" is not achievable on the IC**, and the
strongest true claim available is the one Option A buys: *no principal can peek, and there is no
controller key that could.* That is a large, checkable, honest improvement over both the current
state and over every room that asks you to trust an operator, and it is the claim this project
should make, precisely because it is the one that survives an auditor.

---

# 10. THE BUILD: what happened when Option A was actually made to deal a hand

Everything from here down was produced by running code, not by reasoning about the platform.
It lives in two detached workspaces that cannot touch the root `Cargo.lock`
(`git diff Cargo.lock` is empty, so mainnet's 6-of-6 reproducible build is untouched):

```
src/no_peeking/dealer_types/       the wire types + docs/SHUFFLE-SPEC.md §4 arithmetic (no ic-cdk)
src/no_peeking/dealer_canister/    THE SEALED DEALER   (lib.rs = surface, state.rs = the cards)
src/no_peeking/table_stub/         a table with no deck, no seed and no hole-card field
tests/no_peeking/                  the harness that plays a hand and then attacks it
```

```bash
./src/no_peeking/build.sh --did             # both modules + their .did
cd src/no_peeking   && cargo test           # 11 host tests (the deal arithmetic)
cd tests/no_peeking && cargo test -- --nocapture --test-threads=2   # 25 tests
```

**11 host tests + 25 harness tests green. `cargo test --workspace` at the repo root: unchanged,
still green, and `git diff Cargo.lock` is empty.**

## 10.1 The negative, executed

The auditor's finding was: *"any controller can read the complete shuffled 52-card deck and
every seated player's hole cards while a hand is in progress … verified live at
`phase = PreFlop`."* `tests/no_peeking/tests/no_peek.rs` runs that attack against the sealed
dealer, mid-hand, at `phase = PreFlop`:

| | |
|---|---|
| methods attacked | **26**, read out of the two wasm modules' **export sections**, not out of a `.did`. A method the harness cannot successfully call at all **fails the run** rather than being skipped. |
| attackers | **5**: the table's controller, **the table canister's own principal**, a seated opponent, a stranger, anonymous. |
| argument shapes per method | 13 |
| calls | **1,690**, of which **885** produced a reply the harness kept |
| management-canister doors | `update_settings`, `stop_canister`, `take_canister_snapshot`, `install_code` against the dealer — **all four refused**, `CanisterInvalidController` |
| snapshot | the **table** was stopped, snapshotted and **downloaded** (2,030,787 bytes: `wasm_memory.bin`, `wasm_module.bin`, `metadata.json`) |
| **cards leaked** | **0** |
| seed found in the table's downloaded memory | **absent** |

The sender list includes `w.table` on purpose. PocketIC lets a test send ingress from any
principal, including a canister's, so that row is **strictly stronger than replacing the
table's wasm with something hostile**: it is what an arbitrary attacker-controlled table could
do, without having to write one. It got 177 replies and leaked nothing.

Three things make the green result mean something, because a negative test's failure mode is
passing while broken:

1. **The detector is itself tested.** `src/peek.rs` decodes a reply generically
   (`IDLArgs::from_bytes`, using the message's own type table, so it works on a reply type the
   harness has never seen) and searches the value tree for a card. Seven unit tests feed it
   cards in a bare reply, three containers deep, inside a `Result` variant and in the second of
   several reply values, and two check it does **not** fire on a card-free reply.
2. **The secrets are learned afterwards.** The attack records every reply while the hand is
   live. Only then is the hand forced open, the seed published, and the eleven cards that were
   secret at `PreFlop` derived. Nothing about the sweep could have been tuned to the answer.
3. **There is a positive control that must fire.** Alice *is* entitled to her own two cards,
   and the same detector, on the same recorded bytes, finds them in exactly the 4 replies where
   she asked. If it ever finds 0, the run fails.

And a control on the memory search: the same search finds the seed when the seed is planted in
the same buffer, so "absent" is a measurement rather than a broken `windows()`.

The dealer's surface, extracted from the module and asserted against:

```
open_hand/update  my_hole_cards/query  ack/update  stand_down/update
table_stand_down/update  advance_street/update  community/query
reveal_showdown/update  finish_hand/update  force_finalize/update
hand_public/query  ack_status/query  dealer_health/query  dealer_identity/query
instructions_to_read_my_cards/query
```

Fifteen methods. No name contains `deck`, `seed`, `debug`, `admin`, `dump` or `export`; exactly
one names a hole card, it is a query, and its only argument is a hand id — so there is no seat
parameter to point at somebody else. These are **absent, not gated**, which is the only form of
protection that means anything on a canister with no controller to gate against.

## 10.2 Question (a) — the showdown split. Answered: there isn't one.

§8 flagged this as open because `evaluate_hand` and the side-pot code read hole cards directly.
The answer turned out to be the boring one, and boring is the right answer here: **they keep
reading hole cards directly. The cards arrive as an argument.**

`reveal_showdown(hand_id, seats)` returns the named seats' cards to the table after the river,
publishing them to everybody in the same message. The table then calls
`poker_core::try_evaluate_hand`, `build_side_pots_from_contributions` and `split_pot_clockwise`
exactly as `src/table_canister/src/lib.rs` calls them today. **`poker_core` is not touched**, so
the ~15,700 golden vectors, the settlement oracle and the money-safety invariants all still bind
to the same code.

Executed in `full_hand.rs`, three-handed, with a genuine side pot (two seats at 300, one all in
for 120):

```
side pots  [SidePot { amount: 360, eligible: [0, 1, 2] },
            SidePot { amount: 360, eligible: [0, 1] }]
WINNER seat 1 takes 360 + 360
chips: 700 / 1420 / 0  -> total 2120, conserved
```

and the shuffle survived, which was non-negotiable:

```
dealt_in[0] seat 0 : dealer gave 4c 7c, the SPEC gives 4c 7c
dealt_in[1] seat 1 : dealer gave Qh Js, the SPEC gives Qh Js
dealt_in[2] seat 2 : dealer gave 5d Th, the SPEC gives 5d Th
board              : dealer showed Kd 8h Jc 5h 7s, the SPEC gives Kd 8h Jc 5h 7s
```

The harness re-derives the deck itself from the revealed seed with `create_deck` +
`shuffle_deck`, after checking `SHA256(revealed_seed) == seed_hash` on its own machine — the
same two steps `docs/SHUFFLE-SPEC.md` §1 tells a stranger to take. The dealer never stores a
deck; it stores 32 bytes and re-derives.

## 10.3 Question (b) — the early reveal. The sketch was replaced with a mechanism.

§8 item 3 proposed *"every reveal is public and simultaneous, so a premature reveal is evidence
rather than an advantage"*. **Evidence is weaker than prevention, and it was not necessary to
settle for it.**

The dealer reveals a street only when **every dealt-in seat has said it is ready, in its own
name**:

* `ack(hand_id)` — player-signed, idempotent, and also a liveness heartbeat.
* `stand_down(hand_id)` — player-signed, permanent ("I folded / I am all in").
* `table_stand_down(hand_id, seat)` — the table's only route, and it runs on the dealer's own
  `time()`, never on the table's word.

**`table_stand_down` needed two clocks, and the second one is the thing the build found that the
study had not.** With only the first, the design has a griefing hole big enough to sink it:

* **Clock one, the action clock (60 s).** Measured against `max(street_opened_at, last_seen)`. A
  seat that has genuinely gone silent is stood down after one clock. A seat whose client is
  heartbeating **cannot be stood down under this clock at all**.
* **Clock two, the street grace (300 s — the table's own `STUCK_HAND_GRACE_NS`).** Measured
  against `street_opened_at` only, and it applies whatever the seat is doing. Without it, a
  single player who heartbeats forever and never gets ready freezes the hand until the one-hour
  last-resort door — and can do it again on every hand. `ack` has to double as the liveness
  heartbeat (otherwise a silent player is indistinguishable from a thinking one), and the moment
  it does, "ready" stops being forceable by silence alone. The second clock is the price of that.

The trade, stated so it can be argued with rather than buried: **within the timescale of a real
hand this is prevention; beyond it, it is bounded evidence.** A table that wants a street early
must sit visibly still for five minutes, with every client able to watch `ack_status`, and the
reveal is then recorded as `Readiness::GraceExpired` rather than `Acked`. And the hatch is
**cleared on every street** — a table that burns five minutes once does not then own that seat
for the rest of the hand, it has to burn five minutes again. Both halves are tested
(`a_heartbeating_griefer_cannot_freeze_the_hand_forever`).

Measured, clock one:

```
after two acks, still waiting on [2]
table_stand_down(seat 2) straight away -> seat 2 is not silent yet: 59999999997 ns remain
  at +30 s -> 29999999996 ns remain
  at +65 s -> stood down
```

and against an attentive seat, four minutes of trying at 30-second intervals with a 15-second
heartbeat: **8 attempts, all refused, board still 0 cards.** The controller, a stranger,
anonymous and another player are refused outright, not on a clock. And clock two, with the
griefer heartbeating throughout:

```
4.5 minutes of heartbeating: still refused
at +5.5 minutes -> [seat 0 Acked, seat 1 Acked, seat 2 GraceExpired]
after the flop opened, readiness carried forward = []      <- the hatch is not inherited
and on the new street the table is refused again
```

The reveal is also public and simultaneous *and* logged. After a coerced stand-down:

```
reveal record: requested_by = lqy7q-dh777-77777-aaaaq-cai   (the table)
  seat 0 : Acked
  seat 1 : Acked
  seat 2 : TimedOutByTable
```

readable by anybody through `hand_public`. So prevention against a live player, and a public
record naming who asked and how each seat became ready when the player was not live.

**The same gate had to be put on `finish_hand`**, and this was the sharpest thing the build
found that the study had not: publishing the seed publishes *every* card, so `finish_hand` is a
more dangerous method than `advance_street`. A hostile table with two colluding seats could
otherwise have stood those two down at `PreFlop`, claimed a fold-out, and had the seed published
while the victim still had to act. `finish_hand` is therefore refused unless a showdown was
published **or every seat has stood down** — each in its own name or on a full measured clock:

```
with seat 2 still live, finish_hand -> refusing to publish the seed: no showdown was
revealed and seats [2] have not stood down.
```

## 10.4 Question (c) — the dealer freezing. Made unreachable, and then given a door anyway.

§8 called this "unrecoverable in a way the table's freeze is not, because `update_settings` on a
zero-controller canister is refused forever and the freezing threshold can never be raised after
handover". Two answers, both executed:

**Make it unreachable.** A hand costs a bounded number of messages, so the dealer refuses to
*open* a hand it might not be able to *finish*. `open_hand` checks its own
`canister_cycle_balance()` against a floor fixed at install, and `dealer_health()` publishes the
balance, the floor and `can_open_hand` to anybody. The dealer can therefore only ever run dry
**between** hands, where the consequence is "no new hands start", not "a live pot is
unopenable".

```
dealer balance 99,996,531,777,023 vs floor 500,000,000,000,000 -> can_open_hand = false
start_hand -> dealer balance … is below the floor …. Refusing to OPEN a hand this canister
              might not be able to FINISH. Anyone can fix this:
              `dfx canister deposit-cycles <amount> <dealer>` needs no controller.
```

**And a last-resort door regardless.** `force_finalize(hand_id)` is callable **by any principal
on the internet** once the hand is older than `force_finalize_after_ns` (one hour by default,
twelve times the table's own `STUCK_HAND_GRACE_NS`). It publishes the seed, the full board and
every hole card, and the table's permissionless `rescue_hand` then settles the pot **to its real
winner**. Measured, with the table dead and 600 chips on the felt:

```
force_finalize at +1 minute    -> refused, the door opens 3539999999996 ns from now
force_finalize at +30 minutes  -> refused
force_finalize at +59 minutes  -> refused (and revealed_seed still null at every step)
force_finalize at +62 minutes  -> called by a passer-by, not the operator, not a player
    seat 0 Pair(7,…)   seat 1 Pair(11,…)   seat 2 Pair(5,…)
    the seed says seat 1 wins;  winners [(1, 600)];  chips [800, 1400, 800]
```

That is the point this project has already got wrong twice, closed in both directions at once:
**the pot is not stuck forever, and it is not refunded so the winner is robbed.** The test
asserts both — it recomputes the winner from the seed independently, and it fails if everybody
ends the hand with their starting stack.

The door is safe because its only gate is a wall clock no caller controls, set far beyond any
hand that could still be live. It is also the recovery for the freeze case: `deposit_cycles` is
open to all principals (measured in §6), so anyone can unfreeze the dealer and then call it.

## 10.5 The numbers, measured

Cycle figures are **real observed debits** (`pic.cycle_balance()` before and after), unlike the
vetKD figures in §2.3 which came from the subnet's price oracle. Rounds are PocketIC ticks; its
block interval is 1 ns per round, so the round counts are exact.

**The deal.** `open_hand` called directly is one message doing `raw_rand` + Fisher-Yates over 52
cards + store — the same work `start_new_hand` does at the same point. The difference between it
and the full `start_hand` path is the measured price of moving the cards out of the table:

| | cycles | rounds |
|---|---|---|
| `open_hand` direct (the un-split deal) | 12,399,992 | 3 |
| `start_hand` (table → dealer → `raw_rand` → shuffle → reply) | 23,196,375 | 3 |
| **added by the split** | **10,796,383** | **0** |
| `my_hole_cards` (a query) | 0 | 0 |

**§7.5 budgeted ~1.05×. The measured figure is 1.87×, and that correction belongs here rather
than in a footnote.** §7.5 was counting only the published 260,000-cycle cross-canister
request/response fee; what it missed is that the hop is also a whole extra *update message*, and
the 5,000,000-cycle message base dominates. The absolute number is still small — 10.8 M cycles,
about $0.0000148 — but the ratio in §7.5 was wrong and the table there should be read with this
row next to it.

**A whole hand**, six-handed, dealt to showdown:

| | |
|---|---|
| update messages | 53 |
| cycles (table + dealer) | **439,508,443** (~$0.0006) |
| rounds | 55 |
| per-message average | 8,292,612 |
| of which the ack protocol | 24 messages |

Against §7.2/§7.3's vetKD Design 1 for a 6-max hand — 156,923,076,918 cycles, $0.215/hand,
~$132,000/year for one running table against a room that takes **no rake**:

> **a whole hand here costs 0.0028× the vetKD design's DEAL STEP ALONE**, ~357× less, and unlike
> the vetKD design it actually closes the finding.

**Round structure**, which is what a player waits for:

| step | rounds |
|---|---|
| read my own two cards (query) | **0** |
| read the board (query) | **0** |
| ack / heartbeat (update) | 1 |
| reveal a street (table → dealer) | 1 |

For contrast, §2.4: vetKD derivations issued sequentially are one round each (17 = 4,239 ms
locally), and DFINITY's own forum thread records cross-subnet vetKD exceeding the 10-second
synchronous-call ceiling in production.

**The hot path.** `my_hole_cards` costs **750,843 instructions**, because it re-derives the whole
52-card deck from the seed on every call — deliberately, so no deck-shaped object exists in the
dealer's heap for anyone to find. That is ~27× the live engine's 27,792-instruction deal slice,
in absolute terms 0.75 M against a 5 B query limit, and it does **not** grow as hands accumulate
(750,843 → 750,988 after four hands).

## 10.6 What the ack protocol actually costs, stated plainly

24 of the 53 messages in a six-handed hand are acks. That is the price of turning §8's sketch
into prevention, and it should be argued for rather than buried:

* It is **not new traffic in kind.** The live engine already heartbeats (`last_seen`,
  `DISCONNECT_TIMEOUT_SECS = 90`, and `CLAUDE.md`'s own rate limit line "Heartbeats: 2/second").
* It adds **no wall-clock**, because a client fires `dealer.ack` and `table.act` in parallel.
* It is **~2×** on message count, against vetKD's ~31,000× on cycles.
* It is what makes "the table cannot show the flop early" a property rather than a promise.

The residual cost is real and belongs in the migration estimate:

* every client has to heartbeat to a second canister and to `stand_down` when it folds or goes
  all in;
* a player who folds without telling the dealer costs the table one action clock per street;
* and a player who is present but will not get ready costs the table the **five-minute grace
  period** per street (§10.3). That is the worst case of the whole design and it is worth
  restating: one determined griefer can slow a table to roughly one street per five minutes,
  visibly, and cannot do worse than that.

## 10.7 What this build did NOT change, and does not claim

* **The live engine is untouched.** `src/table_canister`, `src/poker_core` and the frontend
  were not edited. `cargo test --workspace` is green and the root `Cargo.lock` is byte-identical,
  so the 6-of-6 reproducible build still reproduces.
* **`table_stub` holds no money.** Chips are plain numbers; there is no ledger, no deposit and
  no withdrawal. Proving the money path under this split is the next wave's job and belongs in
  `tests/money_safety`, against the real ledger.
* **Post-hand privacy is unchanged and still zero.** `finish_hand` publishes the seed, and the
  seed opens every card including a folded player's. That is deliberate and documented
  (`docs/SHUFFLE-SPEC.md`), it is what one auditor already demonstrated, and the fix for it is
  Option C — vetKD for the archive — in a different canister.
* **Node operators can still read the dealer's memory.** `sev_enabled = false` on both relevant
  subnets. §0 item 4 stands unchanged. The claim this build supports is the one §9 named, and no
  more:

  > **No principal can peek, and no controller key exists that could.**

## 10.8 What a real migration would cost, now that the shape is known

Ordered by risk, not by effort:

1. **Move `deck`, `deck_index`, `hole_cards` and `CURRENT_SEED` out of `TableState`.** These are
   in the canister's stable-memory serialisation and in its `.did`. `TableState` is persisted
   nested inside `opt TableState`, and FINDING 14 records what happens when a field's optionality
   is got wrong there: the whole record silently decodes as `null`. This is the dangerous step
   and it is a state-migration problem, not a poker problem.
2. **Re-point showdown, `get_table_view` and `record_hand_to_history`** at the revealed-cards
   argument. Mechanical; `poker_core` does not change. The history canister needs `dealt_in` from
   the dealer, which it already has a field for.
3. **Client work: the ack protocol.** Every seated client heartbeats to the dealer and calls
   `stand_down` when it folds or goes all in. Without this, every street costs an action clock.
4. **The handover, rehearsed.** Freezing threshold and `min_open_balance` set generously
   *before* controllers are dropped, because neither can ever be raised again. Follow
   `docs/SECURITY-FINDINGS.md` FINDING 23 §6.
5. **A dealer-v2 story on paper before v1 ships.** A sealed dealer can never be patched. The
   recovery is "deploy v2, point new hands at it", and swapping dealers cannot retroactively
   open old hands because v1 stays uncontrolled and unreachable. Write that down before it is
   needed.
6. **Extend `tests/money_safety`** to run its M1–M6 invariants against the split table. That is
   the gate that decides whether this is shippable, and it is the one thing this spike
   deliberately did not attempt.
