//! FAULT INJECTION AT THE LEDGER BOUNDARY, and the invariant that reads it.
//!
//! docs/SECURITY-FINDINGS.md FINDING 29. `deposit()`, `claim_external_deposit()`
//! and `withdraw()` all perform an IRREVERSIBLE LEDGER MOVEMENT and only then
//! settle the canister's own books, in the post-await continuation. The finding
//! says: if that continuation does not run, the movement stands and the book
//! entry does not. Nobody had been able to drive it.
//!
//! # What a trap actually destroys, and how this file reproduces it exactly
//!
//! An IC message is atomic only *up to each await*. Concretely, `deposit()` runs
//! as two separate executions:
//!
//! ```text
//!   execution A   entry .. ic0.call_perform(icrc2_transfer_from) .. return Pending
//!                 ^ every state change A made is COMMITTED here
//!   -- the ledger executes; real money moves --
//!   execution B   the reply callback: claim the block, credit BALANCES, reply
//!                 ^ if B traps, exactly B's changes are discarded. A's stand.
//! ```
//!
//! So the state a trapped tail leaves behind is, by definition, **the state as of
//! the await point, with the ledger movement standing**. That is a state this
//! harness can construct exactly, with no instrumentation of the canister and no
//! mock ledger, using a real IC feature:
//!
//! 1. `submit_call` the money-moving method and `tick()` it forward to its LAST
//!    await point -- last, not first: `claim_external_deposit` awaits a balance
//!    query before it awaits the sweep, and only the second one moves anything.
//! 2. `take_canister_snapshot` there. That is the committed state a trapping B
//!    would be rolled back to.
//! 3. Let the call finish, so the LEDGER really performs the movement.
//! 4. `load_canister_snapshot` -- discard exactly execution B's writes.
//!
//! The ledger is untouched by steps 2 and 4: it is a different canister. What is
//! left is money that moved with no book entry for it, which is FINDING 29's end
//! state, produced without asking anyone to believe a mock.
//!
//! **This is a faithful reproduction of the CONSEQUENCE, not of the CAUSE.** It
//! does not prove a trap is reachable on mainnet; it proves what happens if one
//! is. What was tried in order to force a real trap, and why each failed, is
//! recorded in `docs/SECURITY-FINDINGS.md` under FINDING 29 and in
//! [`TRAP_FORCING_ATTEMPTS`] below, so the next reader does not repeat it.

use candid::{decode_one, encode_one, CandidType, Decode, Encode, Principal};
use serde::Deserialize;

use crate::invariants::{Invariant, Severity, Violation};
use crate::ledger;
use crate::world::{OpError, Outcome, World};

/// Every mechanism tried for forcing a genuine trap in the post-await tail, and
/// the measured reason it did not work. Printed by the proof test so the negative
/// result is on the record rather than in one agent's head.
pub const TRAP_FORCING_ATTEMPTS: &[(&str, &str)] = &[
    (
        "wasm_memory_limit squeeze",
        "update_canister_settings{wasm_memory_limit} set to the canister's exact reported \
         memory_size (6_946_875 bytes), and to +1 and +64KiB of it. All three deposits \
         completed normally and credited. The limit only bites on memory.grow, and neither \
         half of deposit() grows the heap: the allocator already holds enough free pages for \
         a reply decode and two map inserts.",
    ),
    (
        "out-of-cycles in the callback",
        "Measured, freezing_threshold=0: a whole deposit costs 12_565_638 cycles NET, but \
         making the outbound call reserves 42_109_265_417 cycles for the response and that \
         reservation is what the callback then executes out of. To starve the callback the \
         canister would have to hold less than the reservation, in which case call_perform \
         fails and no money moves at all. There is no balance that lets execution A run and \
         starves execution B.",
    ),
    (
        "upgrade inside the window",
        "install_code on a canister with an open call context does drop the callback, but the \
         window is not addressable from the outside: measured round by round, the ledger \
         movement and the credit both land in the SAME tick (tick2), and the module is \
         2_489_885 bytes so install_code cannot even be submitted as one ingress -- it needs \
         a chunk upload that costs more rounds than the window is wide.",
    ),
    (
        "a fault-injecting ledger stub that returns Ok and then panics",
        "A callee cannot make its caller's callback trap. The nearest thing it can do is \
         reply with something undecodable, and ic-cdk 0.19's ic_cdk::call turns a decode \
         failure into Err((CanisterError, ..)), not a trap (api/call.rs decoder_error_to_reject). \
         That reaches the SAME end state through deposit()'s ordinary Err branch -- see \
         FINDING 29's 'the trap is not the only trigger' note -- but it is not a trap.",
    ),
];

/// **WHERE THIS GATE CANNOT SEE.** Written down because the standing lesson of
/// this project is that the defect lives where the gate is blind, and a list of
/// blind spots somebody has to rediscover is not a list.
pub const NOT_COVERED: &[(&str, &str)] = &[
    (
        "IntentOutcome::Unknown",
        "The branch taken when the ledger CALL fails rather than the ledger refusing -- the one          that leaves the entry open because nobody knows whether the money moved. It cannot be          reached from this harness for the same reason a trap cannot: the real ledger neither          rejects nor replies undecodably. Verified by reading only. If it ever retired the entry          instead of releasing the lease, the money would be orphaned and no test here would say          so.",
    ),
    (
        "a payout the ledger definitively refuses",
        "`settle_intent`'s refund path for a `Payout`. The canister's ledger balance always          covers its escrow (that is M2 holding), so a well-formed payout is never refused in          this harness. Verified by reading only.",
    ),
    (
        "two resolvers in the SAME round",
        "Exactly-once is tested sequentially (resolve, then resolve again). A truly concurrent          pair would exercise `take_ledger_intent`'s atomicity rather than its ordering. The          property does not depend on the lease -- the removal is an atomic map operation and the          ledger deduplicates the movement -- but that argument is not measured here.",
    ),
    (
        "deposit subaccounts of principals that are not harness actors",
        "`Snapshot::ledger_deposit_subaccounts` sums over `World::actors`. A subaccount is a pure          function of a principal, so the full set is unenumerable; this is the same limit the          FINDING 21 / FINDING 28 work documents, inherited here.",
    ),
    (
        "the trap itself",
        "This file reproduces the CONSEQUENCE of a discarded continuation, faithfully. It does          not prove a trap is reachable on mainnet. See TRAP_FORCING_ATTEMPTS.",
    ),
];

// ---------------------------------------------------------------------------
// what the canister owns on the ledger, versus what it has written down
// ---------------------------------------------------------------------------

/// The two-sided reading FINDING 29 needs: everything the canister CONTROLS on
/// the ledger, and everything it has WRITTEN DOWN.
///
/// Deliberately not `Snapshot`: `Snapshot::internal_total` is about the books
/// alone, and the whole point here is the gap between the books and the chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Custody {
    /// `icrc1_balance_of(table, None)`.
    pub ledger_main: u64,
    /// Sum over every actor of the table's per-player deposit subaccount.
    pub ledger_subaccounts: u64,
    /// escrow + chips + pot, as the canister accounts for it.
    pub books: u64,
    /// Raw transfers the harness itself pushed in that no `notify_deposit` has
    /// claimed yet. Legitimately on the ledger and legitimately not in the books.
    pub uncredited_raw: u64,
    /// Money named by an OPEN journal entry: the canister has written down that it
    /// is mid-flight and whose it is. Legitimately on the ledger and legitimately
    /// not yet in the books -- but only because it is recoverable.
    pub journalled: u64,
}

impl Custody {
    pub fn ledger_total(&self) -> u64 {
        self.ledger_main.saturating_add(self.ledger_subaccounts)
    }

    /// Money the canister holds that NOTHING accounts for. This is the number
    /// FINDING 29 is about, and it must be zero in every state.
    pub fn orphaned(&self) -> u64 {
        self.ledger_total()
            .saturating_sub(self.books)
            .saturating_sub(self.uncredited_raw)
            .saturating_sub(self.journalled)
    }

    /// Money the books promise that the canister does not hold. M2's question,
    /// re-asked here because a trapped WITHDRAW tail produces this side, not the
    /// other one.
    pub fn short(&self) -> u64 {
        self.books
            .saturating_add(self.uncredited_raw)
            .saturating_sub(self.ledger_total())
    }
}

/// Read [`Custody`] off the running instance.
///
/// `journalled` is read from `get_ledger_intents`, and a build that does not
/// export that method reports zero -- deliberately, so this file compiles and
/// convicts against the UNFIXED canister as well as the fixed one. A probe that
/// only works against the fix cannot prove the defect.
pub fn custody(w: &World) -> Custody {
    let snap = w.snapshot();
    Custody {
        ledger_main: snap.ledger_main,
        ledger_subaccounts: snap.ledger_deposit_subaccounts,
        books: snap.internal_total(),
        uncredited_raw: w.uncredited_raw_deposits,
        journalled: journalled_total(w),
    }
}

/// One entry of the canister's ledger-intent journal, as the canister reports it.
///
/// Mirrors `LedgerIntentView` in `src/table_canister/src/lib.rs`. Kept as a
/// separate declaration on purpose: if the canister's shape drifts, the decode
/// fails loudly instead of the harness agreeing with the canister by sharing a
/// type.
#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LedgerIntentView {
    pub id: u64,
    pub who: Principal,
    /// "pull" | "sweep" | "payout"
    pub kind: String,
    pub amount: u64,
    pub opened_at_ns: u64,
    pub attempts: u32,
    pub leased_until_ns: Option<u64>,
    pub retry_deadline_ns: u64,
}

/// Every open journal entry, or an empty list on a build that has no journal.
pub fn ledger_intents(w: &World) -> Vec<LedgerIntentView> {
    match w.query_as(
        w.controller,
        "get_all_ledger_intents",
        Encode!().expect("no-arg encode"),
    ) {
        Ok(bytes) => decode_one::<Vec<LedgerIntentView>>(&bytes).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Money named by open journal entries that is on the ledger and not yet in the
/// books: a `pull` (arriving from outside) or a `sweep` (moving between two
/// accounts the canister owns).
///
/// A sweep is counted here and NOT in the canister's deposit-custody figure,
/// which nets open sweeps out: exactly one of the two terms carries it at any
/// moment, so the total does not change while the movement is in flight. A
/// `payout` is in neither -- it is money on its way OUT, already debited from
/// escrow.
pub fn journalled_total(w: &World) -> u64 {
    ledger_intents(w)
        .iter()
        .filter(|i| i.kind == "pull" || i.kind == "sweep")
        .fold(0u64, |acc, i| acc.saturating_add(i.amount))
}

/// The owner-driven resume path. Absent on an unfixed build, which reports as
/// `OpError::Trap`, and that is exactly the observation the proof test makes.
pub fn resolve_my_ledger_intents(w: &World, who: Principal) -> Outcome<Vec<String>> {
    let bytes = w
        .pic
        .update_call(
            w.table,
            who,
            "resolve_my_ledger_intents",
            Encode!().expect("no-arg encode"),
        )
        .map_err(|r| OpError::Trap(format!("{r:?}")))?;
    match decode_one::<Result<Vec<String>, String>>(&bytes) {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(m)) => Err(OpError::Err(m)),
        Err(e) => Err(OpError::Trap(format!("reply decode: {e}"))),
    }
}

// ---------------------------------------------------------------------------
// M14 LEDGER/BOOKS COHERENCE, as an invariant the fuzzer can run every step
// ---------------------------------------------------------------------------

/// M14: money that has moved on the ledger is money the books must HOLD or NAME.
///
/// Takes the snapshot the caller already has, so on the fuzzer's hot path this
/// costs exactly one extra query (`get_all_ledger_intents`) per step.
///
/// Two directions, and they are different failures:
///
/// * `orphaned > 0` -- the canister holds money nothing accounts for. This is
///   FINDING 29's own signature and it is `FundDestruction`: there is no admin
///   crediting path, so an orphan is unreachable by construction.
/// * `short > 0` -- the books promise more than the canister holds. This is M2's
///   direction and it is `FundCreation`, and it is reachable here in a way M2's
///   own check would miss: a discarded WITHDRAW continuation pays the player and
///   rolls the debit back.
pub fn check_ledger_books_coherence(w: &World, snap: &crate::world::Snapshot) -> Vec<Violation> {
    let c = Custody {
        ledger_main: snap.ledger_main,
        ledger_subaccounts: snap.ledger_deposit_subaccounts,
        books: snap.internal_total(),
        uncredited_raw: w.uncredited_raw_deposits,
        journalled: journalled_total(w),
    };
    let mut out = Vec::new();
    if c.orphaned() > 0 {
        out.push(Violation::new(
            Invariant::M14LedgerBooksCoherence,
            "no_orphaned_ledger_money",
            Severity::FundDestruction,
            c.orphaned() as i128,
            &snap.table.phase,
            format!(
                "{} e8s sit in an account this canister controls and NOTHING in the canister \
                 accounts for them: not escrow, not chips, not the pot, and not an open \
                 ledger-intent naming whose they are. reading: {c:?}. \
                 docs/SECURITY-FINDINGS.md FINDING 29.",
                c.orphaned()
            ),
        ));
    }
    if c.short() > 0 {
        out.push(Violation::new(
            Invariant::M14LedgerBooksCoherence,
            "books_not_backed_by_the_ledger",
            Severity::FundCreation,
            -(c.short() as i128),
            &snap.table.phase,
            format!(
                "the books promise {} e8s more than this canister holds on the ledger. \
                 reading: {c:?}. docs/SECURITY-FINDINGS.md FINDING 29: a discarded WITHDRAW \
                 continuation pays the player and rolls the escrow debit back.",
                c.short()
            ),
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// the injector
// ---------------------------------------------------------------------------

/// What a single fault injection did.
#[derive(Clone, Debug)]
pub struct Injection {
    pub method: String,
    pub who: Principal,
    /// Custody before the call.
    pub before: Custody,
    /// Custody after the call ran to completion, before the rollback.
    pub completed: Custody,
    /// Custody after execution B was discarded. THE STATE UNDER TEST.
    pub after: Custody,
    /// What the call replied before its tail was discarded. Recorded because a
    /// reply the caller acted on, describing state that no longer exists, is part
    /// of the harm.
    pub reply: Outcome<u64>,
    /// The ledger block index range the call wrote into, so the recovery sweep can
    /// quote a real index at `notify_deposit`.
    pub blocks_written: std::ops::Range<u64>,
}

impl Injection {
    /// Did the ledger move while the books did not? The one-line verdict.
    pub fn money_moved_books_did_not(&self) -> bool {
        self.after.ledger_total() != self.before.ledger_total()
            && self.after.books == self.before.books
    }
}

/// Run `method` on the table canister and discard exactly its post-await
/// continuation, leaving whatever the ledger did standing.
///
/// Returns `Err` with an explanation if the method never reached an await (in
/// which case there is nothing to inject into and no money moved).
pub fn trap_the_tail(
    w: &mut World,
    who: Principal,
    method: &str,
    arg: Vec<u8>,
    pre_ticks: u32,
) -> Result<Injection, String> {
    let before = custody(w);
    let blocks_before = chain_length(w);

    let msg = w
        .pic
        .submit_call(w.table, who, method, arg)
        .map_err(|r| format!("submit_call({method}) rejected: {r:?}"))?;

    // Walk the message to its LAST await point and snapshot there.
    //
    // `pre_ticks` is how many rounds that takes, and it is passed in rather than
    // discovered because `take_canister_snapshot` is itself a management-canister
    // update: PocketIC executes rounds to complete it, so a loop that snapshots
    // speculatively also advances the message it is trying to freeze.
    //
    // Measured round accounting on this two-subnet instance: round 1 runs the
    // method's first segment and emits its first request; every round after that
    // executes one callee AND runs the segment that its reply resumes. So a
    // method with `n` awaits settles in `n + 1` rounds and its LAST await point
    // is reached after exactly `n`. `deposit` and `withdraw` await once
    // (`pre_ticks = 1`); `claim_external_deposit` awaits a balance query and then
    // the sweep (`pre_ticks = 2`), and only the second one moves anything --
    // snapshotting after the first would roll back the intent the canister writes
    // before the sweep, and the harness would be testing a canister that never
    // got to write it.
    //
    // The `still in flight` check below is what makes this safe to state as a
    // constant: if PocketIC's round accounting ever differs, the injector fails
    // loudly instead of quietly measuring the wrong instant.
    for _ in 0..pre_ticks {
        w.pic.tick();
    }
    if w.pic.ingress_status(msg.clone()).is_some() {
        return Err(format!(
            "{method} had already settled after {pre_ticks} round(s), so the snapshot would \
             not be at an await point and nothing would be injected. Round accounting has \
             changed; re-derive pre_ticks."
        ));
    }

    // The committed state AT THE LAST AWAIT POINT: exactly what a trapping
    // continuation is rolled back to.
    let snapshot = w
        .pic
        .take_canister_snapshot(w.table, Some(w.controller), None)
        .map_err(|r| format!("take_canister_snapshot rejected: {r:?}"))?;

    // Let the ledger do the irreversible thing and the tail run to completion, so
    // the ledger's side is real and settled before anything is discarded.
    let raw = w.pic.await_call(msg);
    let reply = match raw {
        Ok(bytes) => match decode_one::<Result<u64, String>>(&bytes) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(m)) => Err(OpError::Err(m)),
            Err(e) => Err(OpError::Trap(format!("reply decode: {e}"))),
        },
        Err(r) => Err(OpError::Trap(format!("{r:?}"))),
    };
    let completed = custody(w);

    // Discard exactly execution B.
    w.pic
        .load_canister_snapshot(w.table, Some(w.controller), snapshot.id.clone())
        .map_err(|r| format!("load_canister_snapshot rejected: {r:?}"))?;

    let after = custody(w);
    let blocks_after = chain_length(w);

    Ok(Injection {
        method: method.to_string(),
        who,
        before,
        completed,
        after,
        reply,
        blocks_written: blocks_before..blocks_after,
    })
}

/// One await (the ICRC-2 pull).
pub fn trap_deposit_tail(w: &mut World, who: Principal, amount: u64) -> Result<Injection, String> {
    trap_the_tail(w, who, "deposit", encode_one(amount).expect("encode"), 1)
}

/// TWO awaits: a balance query, which moves nothing, and then the sweep, which is
/// the irreversible one. The snapshot has to be at the second.
pub fn trap_claim_external_tail(w: &mut World, who: Principal) -> Result<Injection, String> {
    trap_the_tail(
        w,
        who,
        "claim_external_deposit",
        Encode!().expect("no-arg encode"),
        2,
    )
}

/// One await (the payout transfer).
pub fn trap_withdraw_tail(w: &mut World, who: Principal, amount: u64) -> Result<Injection, String> {
    trap_the_tail(w, who, "withdraw", encode_one(amount).expect("encode"), 1)
}

// ---------------------------------------------------------------------------
// the fuzzer's fault injection
// ---------------------------------------------------------------------------

/// One run in this many injects a discarded continuation.
///
/// Not every run, for two reasons that are both about the gate staying useful: an
/// injection costs a canister snapshot plus a restore, which is the most
/// expensive thing this harness does; and a fuzzer whose every run is abnormal
/// stops being a fuzzer of ordinary play. One in four keeps both.
pub const FAULT_INJECTION_SEED_MODULUS: u64 = 4;

pub fn run_is_fault_injecting(seed: u64) -> bool {
    seed % FAULT_INJECTION_SEED_MODULUS == 0
}

/// Longer than the canister's `INTENT_LEASE_NS`. A discarded continuation leaves
/// its lease behind and the lease has to expire before anyone can take the entry
/// over; that expiry is the property, so this waits for it rather than reaching
/// around it.
const PAST_THE_LEASE_SECS: u64 = 45;

/// What one fuzz-run injection did.
#[derive(Clone, Debug)]
pub struct InjectionRun {
    pub description: String,
    /// M12 evaluated in the injected state -- the whole point.
    pub during: Vec<Violation>,
    /// M12 evaluated after the victim drove the recovery themselves.
    pub after_recovery: Vec<Violation>,
    /// What the owner's own `resolve_my_ledger_intents()` said.
    pub resolution: String,
}

/// Inject a discarded continuation at one of the three money doors, check M12 in
/// the resulting state, then let the OWNER recover it with a player-only call and
/// check M12 again.
///
/// Recovery happens inside this function on purpose: the fuzzer's other
/// invariants (M1 above all) state `ledger == owed`, which is deliberately
/// stricter than M12 and would fire for as long as an intent is open. Leaving the
/// world incoherent would turn every fault run into an M1 failure and bury the
/// thing being measured.
pub fn inject_and_recover(w: &mut World, pick: u64) -> InjectionRun {
    let actor = w.actors[(pick as usize) % w.actors.len()].clone();
    let door = pick % 3;
    let amount = 100_000_000u64 + (pick % 7) * 25_000_000;

    let injected = match door {
        0 => {
            let _ = w.approve(actor.principal, amount + ledger::TRANSFER_FEE);
            trap_deposit_tail(w, actor.principal, amount)
        }
        1 => {
            let _ = w.transfer_to_deposit_subaccount(actor.principal, amount);
            trap_claim_external_tail(w, actor.principal)
        }
        _ => {
            // A payout needs escrow to pay out of. Fund it the ordinary way first;
            // if that fails (allowance, an in-progress withdrawal) the injection
            // simply does not happen, which is reported rather than hidden.
            let _ = w.fund_escrow(actor.principal, amount.saturating_mul(2));
            trap_withdraw_tail(w, actor.principal, amount)
        }
    };

    let inj = match injected {
        Ok(i) => i,
        Err(why) => {
            return InjectionRun {
                description: format!("no injection at door {door} for {}: {why}", actor.name),
                during: Vec::new(),
                after_recovery: Vec::new(),
                resolution: String::new(),
            }
        }
    };

    let snap = w.snapshot();
    let during = check_ledger_books_coherence(w, &snap);

    // The owner, with a call any player may make.
    w.advance(std::time::Duration::from_secs(PAST_THE_LEASE_SECS));
    let resolution = format!("{:?}", resolve_my_ledger_intents(w, actor.principal));

    let snap = w.snapshot();
    let after_recovery = check_ledger_books_coherence(w, &snap);

    InjectionRun {
        description: format!(
            "door {door} ({}) for {}: ledger {} -> {}, books {} -> {}, journalled {}",
            inj.method,
            actor.name,
            inj.before.ledger_total(),
            inj.after.ledger_total(),
            inj.before.books,
            inj.after.books,
            inj.after.journalled
        ),
        during,
        after_recovery,
        resolution,
    }
}

/// Every actor finishes whatever the canister started for them. Called just
/// before the M9 drain so the drain measures money the player can actually reach,
/// not money that only a second wave of calls would reach.
pub fn resolve_everyones_intents(w: &mut World) -> Vec<String> {
    if ledger_intents(w).is_empty() {
        return Vec::new();
    }
    w.advance(std::time::Duration::from_secs(PAST_THE_LEASE_SECS));
    let actors: Vec<Principal> = w.actors.iter().map(|a| a.principal).collect();
    actors
        .into_iter()
        .map(|p| format!("{:?}", resolve_my_ledger_intents(w, p)))
        .collect()
}

// ---------------------------------------------------------------------------
// ledger chain length, so a recovery attempt can quote a REAL block index
// ---------------------------------------------------------------------------

#[derive(CandidType, Deserialize, Debug)]
struct GetBlocksArgs {
    start: u64,
    length: u64,
}

#[derive(CandidType, Deserialize, Debug)]
struct ChainLengthOnly {
    chain_length: u64,
    certificate: Option<Vec<u8>>,
    blocks: Vec<candid::Reserved>,
    first_block_index: u64,
    archived_blocks: candid::Reserved,
}

/// Number of blocks on the ICP ledger, i.e. the index the NEXT block will get.
pub fn chain_length(w: &World) -> u64 {
    let arg = Encode!(&GetBlocksArgs {
        start: 0,
        length: 0
    })
    .expect("query_blocks arg encode");
    let bytes = w
        .pic
        .query_call(w.ledger, Principal::anonymous(), "query_blocks", arg)
        .expect("query_blocks must not be rejected");
    Decode!(&bytes, ChainLengthOnly)
        .expect("query_blocks reply decode")
        .chain_length
}

// ---------------------------------------------------------------------------
// the recovery sweep
// ---------------------------------------------------------------------------

/// One attempted route out of the injected state.
#[derive(Clone, Debug)]
pub struct RecoveryAttempt {
    pub caller: &'static str,
    pub method: String,
    pub reply: String,
    /// Escrow credited to the victim by this call.
    pub credited: u64,
    /// Money that reached the victim's own ledger wallet because of this call.
    pub paid_out: u64,
}

/// Try EVERY door out of the state `injection` left, and report what each one
/// said and what each one moved.
///
/// The list is deliberately exhaustive over the money surface rather than
/// selective: FINDING 29's claim is "there is no recovery", and a claim of that
/// shape is only worth anything if the search that failed was complete. The set
/// is derived from the canister's own Candid interface, and
/// `assert_recovery_sweep_is_complete` fails if a money-moving method exists that
/// this sweep does not call.
pub fn recovery_sweep(w: &mut World, victim: Principal, blocks: std::ops::Range<u64>) -> Vec<RecoveryAttempt> {
    let mut out = Vec::new();
    let controller = w.controller;

    let mut record = |w: &mut World, caller: &'static str, method: String, reply: String| {
        let credited = w.get_balance(victim);
        let wallet = w.ledger_balance(victim, None);
        out.push(RecoveryAttempt {
            caller,
            method,
            reply,
            credited,
            paid_out: wallet,
        });
    };

    // --- the player's own doors -------------------------------------------
    let r = w.claim_external_deposit(victim);
    record(w, "victim", "claim_external_deposit()".into(), format!("{r:?}"));

    // Every block this call could possibly have written, quoted back at the one
    // method whose whole job is "I sent money, credit me".
    for block in blocks.clone() {
        let r = w.notify_deposit(victim, block);
        record(w, "victim", format!("notify_deposit({block})"), format!("{r:?}"));
        // notify_deposit is rate limited to 5/minute; keep the sweep honest by
        // paying the wait rather than by stopping early.
        w.advance_time_only(std::time::Duration::from_secs(61));
    }

    let r = w.withdraw(victim, 100_000_000);
    record(w, "victim", "withdraw(1 ICP)".into(), format!("{r:?}"));

    let r = w.cash_out(victim);
    record(w, "victim", "cash_out()".into(), format!("{r:?}"));

    let r = w.leave_table(victim);
    record(w, "victim", "leave_table()".into(), format!("{r:?}"));

    let r = resolve_my_ledger_intents(w, victim);
    record(
        w,
        "victim",
        "resolve_my_ledger_intents()".into(),
        format!("{r:?}"),
    );

    // --- the controller's doors -------------------------------------------
    for method in [
        "admin_return_all_chips_to_escrow",
        "admin_get_all_balances",
        "admin_get_table_chips",
    ] {
        let r = w
            .pic
            .update_call(w.table, controller, method, Encode!().unwrap())
            .map(|_| "reply".to_string())
            .unwrap_or_else(|e| format!("{e:?}"));
        record(w, "controller", format!("{method}()"), r);
    }

    // The one method that was deliberately deleted. Calling it by name is the
    // point: the report has to show that the last in-band repair is gone, not
    // assert it from a comment.
    let r = w
        .pic
        .update_call(
            w.table,
            controller,
            "admin_restore_balance",
            Encode!(&victim, &100_000_000u64).unwrap(),
        )
        .map(|_| "reply".to_string())
        .unwrap_or_else(|e| format!("{e:?}"));
    record(w, "controller", "admin_restore_balance(victim, 1 ICP)".into(), r);

    let r = w
        .pic
        .update_call(
            w.table,
            controller,
            "resolve_ledger_intent",
            encode_one(0u64).unwrap(),
        )
        .map(|_| "reply".to_string())
        .unwrap_or_else(|e| format!("{e:?}"));
    record(w, "controller", "resolve_ledger_intent(0)".into(), r);

    out
}

/// The victim's money is back if their escrow plus their wallet is whole again.
pub fn recovered(w: &World, victim: Principal, wallet_before_everything: u64) -> bool {
    // A recovery is only a recovery if the money is REACHABLE, so escrow counts
    // and so does the wallet. Fees are not recoverable and are not counted
    // against the canister.
    let escrow = w.get_balance(victim);
    let wallet = w.ledger_balance(victim, None);
    escrow.saturating_add(wallet).saturating_add(3 * ledger::TRANSFER_FEE) >= wallet_before_everything
}
