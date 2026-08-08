//! ClearDeck GUARDIAN — the controller of the controllers.
//!
//! # The defect this exists for
//!
//! docs/SECURITY-FINDINGS.md FINDING 23. Every ClearDeck canister has one
//! controller principal. A controller can call `install_code --mode reinstall` or
//! `uninstall_code` on a funded table. An auditor did exactly that on the local
//! replica: deposited 5 ICP as a player, ran ONE command as the controller with
//! the SAME wasm and no code change, and her balance read 0 while the ledger
//! still held her 5 ICP at the canister's account. Every player-callable recovery
//! said she had nothing.
//!
//! # The mechanism
//!
//! `install_code`, `uninstall_code`, `update_settings`, `delete_canister`,
//! `stop_canister` and `load_canister_snapshot` are methods of the MANAGEMENT
//! canister, and the management canister accepts them only from a controller of
//! the target. Controllership is a list of principals, and a principal may be a
//! CANISTER. So if a table's only controller is this canister, then the only way
//! any of those calls can reach that table is for THIS canister's code to make
//! them — and this canister's code makes exactly three of them and never the
//! rest.
//!
//! Controllership is NOT transitive. Whoever controls the guardian is not
//! thereby a controller of the tables, and the management canister rejects their
//! `install_code` on a table outright. That is measured, not assumed:
//! `tests/money_safety/tests/controller_custody.rs`,
//! `guardian_premise_a_controller_of_the_guardian_is_not_a_controller_of_the_table`.
//!
//! # What this canister can and cannot prevent — read this before believing it
//!
//! CAN prevent, absolutely, for as long as it holds the controller seat:
//!   * `install_code --mode reinstall` on a table  (the auditor's wipe)
//!   * `install_code --mode install` on a table
//!   * `uninstall_code` on a table
//!   * `delete_canister` on a table
//!   * `update_settings` on a table — so the controller list itself is frozen and
//!     nobody can be added back as a direct controller
//!   * `load_canister_snapshot` on a table — a state rollback that un-does a
//!     withdrawal or re-arms a spent deposit
//!   * `stop_canister` on a table — a withdrawal freeze
//! None of those seven verbs exists anywhere in this canister's interface, so
//! there is no argument, no role and no emergency in which it emits one.
//!
//! CANNOT prevent:
//!   * A MALICIOUS UPGRADE. `install_code --mode upgrade` preserves state, but the
//!     code that runs after it is new code, and new code can zero a balance in
//!     `post_upgrade`. The guardian does not make the operator honest. What it
//!     does is make every code change take the form of a PUBLIC PROPOSAL that
//!     sits, readable by anyone with no authentication, for a fixed delay before
//!     it can execute. The operator's power is not removed; its silence and its
//!     immediacy are.
//!   * The NNS. A subnet's replica software is chosen by NNS proposal, and no
//!     application canister can bind that.
//!   * CYCLE EXHAUSTION. A canister that reaches zero cycles is uninstalled by the
//!     protocol, which destroys exactly what `uninstall_code` destroys. Nobody's
//!     controller list can stop that. `deposit_cycles` is callable by ANY
//!     principal, so anybody can top the tables up; the freezing threshold is the
//!     real defence and must be set high at handover.
//!
//! # Lockout, and the escape
//!
//! A guardian with a bug is a permanent loss of upgradeability on canisters
//! holding money. Two things bound that:
//!
//! 1. `UpgradeSelf`. The guardian's own controller list is `[guardian]` — it
//!    controls itself, and nobody else controls it. Its code can therefore still
//!    be replaced, but only through its own timelocked proposal queue, so fixing
//!    a guardian bug costs `SELF_DELAY_SECS` of public notice and nothing else.
//! 2. **A bricked guardian is a loss of FIXABILITY, not a loss of FUNDS.** If this
//!    canister stopped working entirely, the tables would keep running exactly as
//!    they are: players still deposit, play and withdraw, because none of that
//!    goes through here. What would be lost is the ability to ever change the
//!    tables again. That is a real cost and it is the honest price of the
//!    property above, and it is why this file is deliberately small, has no
//!    timers, no inter-canister calls other than the three management calls it
//!    names, and no dependency that can fail at runtime.
//!
//! # Deliberate non-features
//!
//! No pause. No emergency mode. No "recovery" role. No settable timelock. Every
//! one of those is a second key with a shorter path to the same power, and this
//! project has already shipped a method called `admin_reinit_table` described in
//! its own Candid as "for recovery after upgrade issues" that destroyed 40 ICP.

use candid::{CandidType, Principal};
use ic_cdk::management_canister::{
    self, CanisterInstallMode, ChunkHash, ClearChunkStoreArgs, InstallChunkedCodeArgs,
    StoredChunksArgs, UploadChunkArgs,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// constants — none of these is settable at runtime, by anybody, ever
// ---------------------------------------------------------------------------

/// Notice period before a code change may be installed on a guarded canister.
///
/// It has one job: be longer than the time it takes a player who is paying
/// attention to withdraw. A ClearDeck withdrawal is one update call with a
/// 60-second cooldown, so the binding constraint is not the mechanics, it is
/// noticing. 72 hours is the shortest delay that covers a weekend.
pub const GUARDED_DELAY_SECS: u64 = 72 * 60 * 60;

/// Notice period before the GUARDIAN's own code may be replaced. Longer than
/// `GUARDED_DELAY_SECS` on purpose: replacing this code replaces all of the rules
/// above, so it is the single most consequential thing the operator can do, and
/// it must never be the quickest route to a table.
pub const SELF_DELAY_SECS: u64 = 7 * 24 * 60 * 60;

/// Same notice as a self-upgrade: handing the key to a different principal is
/// indistinguishable, from a player's seat, from handing it to an attacker.
pub const OPERATOR_DELAY_SECS: u64 = 7 * 24 * 60 * 60;

/// A cap so a runaway or hostile operator cannot make the queue unreadable, which
/// would defeat the entire point of the queue being public.
pub const MAX_PENDING: usize = 32;

/// Chunks per module. 64 x 1 MiB is comfortably more than any ClearDeck wasm.
pub const MAX_CHUNKS: usize = 64;

/// The management canister's own per-chunk limit.
pub const MAX_CHUNK_BYTES: usize = 1024 * 1024;

/// Upgrade args here are small Candid records. This is a sanity bound, not a
/// protocol limit.
pub const MAX_ARG_BYTES: usize = 64 * 1024;

/// How many finished proposals are retained for the public record.
pub const MAX_HISTORY: usize = 256;

/// How long a proposal may sit in `Executing` before it becomes retryable and
/// cancellable again.
///
/// `Executing` is the one status no ordinary path clears, and `UpgradeSelf` is
/// sent as a ONE-WAY call (see `self_upgrade`), so a self-upgrade that the
/// management canister refuses leaves no trace at all: no reply, no continuation,
/// no `post_upgrade`. Without this grace period that proposal would be wedged for
/// ever, and since `MAX_PENDING` is finite a hostile or clumsy operator could wedge
/// the whole queue. One hour is far longer than any install takes and far shorter
/// than any notice period, so it can never shorten one.
pub const EXECUTING_GRACE_SECS: u64 = 60 * 60;

/// The management-canister verbs this canister will never emit, published as data
/// so a client, a monitor or a test can assert on it instead of reading prose.
pub const REFUSED_OPERATIONS: [&str; 8] = [
    "install_code:reinstall",
    "install_code:install",
    "uninstall_code",
    "delete_canister",
    "update_settings",
    "stop_canister",
    "start_canister",
    "load_canister_snapshot",
];

// ---------------------------------------------------------------------------
// types
// ---------------------------------------------------------------------------

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct GuardianInit {
    /// The key holder. May propose, cancel and execute. May do nothing else.
    pub operator: Principal,
    /// The canisters this guardian is (or is about to become) the controller of.
    /// Informational: listing a canister here grants nothing. The management
    /// canister decides, from the target's own controller list, whether a call
    /// from this canister is allowed.
    pub guarded: Vec<Principal>,
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// `install_code --mode upgrade` on a guarded canister, from chunks already
    /// in that canister's own chunk store.
    UpgradeGuarded {
        target: Principal,
        wasm_module_hash: Vec<u8>,
        chunk_hashes: Vec<Vec<u8>>,
        arg: Vec<u8>,
    },
    /// `install_code --mode upgrade` on THIS canister.
    UpgradeSelf {
        wasm_module_hash: Vec<u8>,
        chunk_hashes: Vec<Vec<u8>>,
        arg: Vec<u8>,
    },
    /// Hand the operator key to a different principal.
    TransferOperator { new_operator: Principal },
}

impl Action {
    fn delay_secs(&self) -> u64 {
        match self {
            Action::UpgradeGuarded { .. } => GUARDED_DELAY_SECS,
            Action::UpgradeSelf { .. } => SELF_DELAY_SECS,
            Action::TransferOperator { .. } => OPERATOR_DELAY_SECS,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Action::UpgradeGuarded { .. } => "UpgradeGuarded",
            Action::UpgradeSelf { .. } => "UpgradeSelf",
            Action::TransferOperator { .. } => "TransferOperator",
        }
    }
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum ProposalStatus {
    /// Waiting for `ready_at_secs`, or waiting to be executed after it.
    Pending,
    /// An `execute` is in flight. Set BEFORE the await, cleared after, so a second
    /// `execute` on the same id cannot run the same management call twice.
    Executing,
    Executed { at_secs: u64 },
    Cancelled { at_secs: u64 },
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct Proposal {
    pub id: u64,
    pub kind: String,
    pub action: Action,
    pub proposed_by: Principal,
    pub proposed_at_secs: u64,
    pub ready_at_secs: u64,
    pub status: ProposalStatus,
    /// Why the last `execute` attempt failed, if one did. A failed attempt returns
    /// the proposal to `Pending` with the same `ready_at_secs`, so an operator can
    /// retry a transient failure without buying another notice period, and a
    /// reader can see that it happened.
    pub last_error: Option<String>,
    pub attempts: u32,
    /// When the current `Executing` claim was taken. `None` in every other status.
    /// See `EXECUTING_GRACE_SECS`.
    pub executing_since_secs: Option<u64>,
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
pub struct GuardianConfig {
    pub operator: Principal,
    pub guarded: Vec<Principal>,
    pub guarded_delay_secs: u64,
    pub self_delay_secs: u64,
    pub operator_delay_secs: u64,
    pub max_pending: u64,
    /// The verbs this canister will never emit. See `REFUSED_OPERATIONS`.
    pub refused_operations: Vec<String>,
}

#[derive(Clone, Debug, Default, CandidType, Deserialize, Serialize)]
struct State {
    operator: Option<Principal>,
    guarded: BTreeSet<Principal>,
    proposals: BTreeMap<u64, Proposal>,
    /// Finished proposals, newest last, truncated to `MAX_HISTORY`.
    history: Vec<Proposal>,
    next_id: u64,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    ic_cdk::api::time() / 1_000_000_000
}

/// The ONLY authorisation predicate in this canister.
///
/// Note what it is NOT: it is not `is_controller`. The guardian's controller is
/// the guardian, and a canister cannot send itself an ingress message, so an
/// `is_controller` gate here would be a gate nobody could ever pass. The operator
/// is explicit state, and changing it is a timelocked proposal like everything
/// else.
fn require_operator() -> Result<Principal, String> {
    let caller = ic_cdk::api::msg_caller();
    if caller == Principal::anonymous() {
        return Err("Anonymous callers cannot perform this action".to_string());
    }
    let operator = STATE.with(|s| s.borrow().operator);
    match operator {
        Some(op) if op == caller => Ok(caller),
        Some(_) => Err(format!(
            "Only the guardian operator may call this. Caller {caller} is not the operator."
        )),
        None => Err("Guardian has no operator: it was never initialised.".to_string()),
    }
}

fn validate_chunks(chunk_hashes: &[Vec<u8>], wasm_module_hash: &[u8]) -> Result<(), String> {
    if chunk_hashes.is_empty() {
        return Err("chunk_hashes is empty: nothing to install".to_string());
    }
    if chunk_hashes.len() > MAX_CHUNKS {
        return Err(format!(
            "chunk_hashes has {} entries, the limit is {MAX_CHUNKS}",
            chunk_hashes.len()
        ));
    }
    for (i, h) in chunk_hashes.iter().enumerate() {
        if h.len() != 32 {
            return Err(format!(
                "chunk hash {i} is {} bytes, a sha256 is 32",
                h.len()
            ));
        }
    }
    if wasm_module_hash.len() != 32 {
        return Err(format!(
            "wasm_module_hash is {} bytes, a sha256 is 32",
            wasm_module_hash.len()
        ));
    }
    Ok(())
}

fn validate(action: &Action) -> Result<(), String> {
    match action {
        Action::UpgradeGuarded {
            target,
            wasm_module_hash,
            chunk_hashes,
            arg,
        } => {
            if *target == ic_cdk::api::canister_self() {
                return Err(
                    "Use UpgradeSelf to upgrade the guardian; UpgradeGuarded is for guarded \
                     canisters and carries the shorter notice period."
                        .to_string(),
                );
            }
            if *target == Principal::management_canister() || *target == Principal::anonymous() {
                return Err("target is not a canister".to_string());
            }
            if !STATE.with(|s| s.borrow().guarded.contains(target)) {
                return Err(format!(
                    "{target} is not in this guardian's guarded set. Add it with \
                     note_guarded first, so the proposal a reader sees names a canister the \
                     guardian has declared."
                ));
            }
            if arg.len() > MAX_ARG_BYTES {
                return Err(format!("arg is {} bytes, the limit is {MAX_ARG_BYTES}", arg.len()));
            }
            validate_chunks(chunk_hashes, wasm_module_hash)
        }
        Action::UpgradeSelf {
            wasm_module_hash,
            chunk_hashes,
            arg,
        } => {
            if arg.len() > MAX_ARG_BYTES {
                return Err(format!("arg is {} bytes, the limit is {MAX_ARG_BYTES}", arg.len()));
            }
            validate_chunks(chunk_hashes, wasm_module_hash)
        }
        Action::TransferOperator { new_operator } => {
            if *new_operator == Principal::anonymous() {
                return Err("the operator cannot be the anonymous principal".to_string());
            }
            Ok(())
        }
    }
}

fn to_chunk_hashes(raw: &[Vec<u8>]) -> Vec<ChunkHash> {
    raw.iter().map(|h| ChunkHash { hash: h.clone() }).collect()
}

fn retire(p: Proposal) {
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.proposals.remove(&p.id);
        st.history.push(p);
        let len = st.history.len();
        if len > MAX_HISTORY {
            st.history.drain(0..len - MAX_HISTORY);
        }
    });
}

// ---------------------------------------------------------------------------
// lifecycle
// ---------------------------------------------------------------------------

#[ic_cdk::init]
fn init(arg: GuardianInit) {
    if arg.operator == Principal::anonymous() {
        ic_cdk::trap("guardian operator cannot be the anonymous principal");
    }
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.operator = Some(arg.operator);
        st.guarded = arg.guarded.into_iter().collect();
        st.next_id = 1;
    });
}

#[ic_cdk::pre_upgrade]
fn pre_upgrade() {
    let state = STATE.with(|s| s.borrow().clone());
    // A guardian that loses its operator and its queue across an upgrade is a
    // guardian nobody can drive, on canisters nobody else can drive either. If
    // this cannot be written, refuse the upgrade rather than complete it.
    if let Err(e) = ic_cdk::storage::stable_save((state,)) {
        ic_cdk::trap(&format!(
            "guardian pre_upgrade could not write stable state ({e}); refusing the upgrade"
        ));
    }
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    match ic_cdk::storage::stable_restore::<(State,)>() {
        Ok((mut state,)) => {
            // THIS FUNCTION RUNNING IS THE PROOF THAT THE SELF-UPGRADE LANDED.
            //
            // `self_upgrade` sends its `install_chunked_code` one-way and cannot
            // learn the result, so `execute` deliberately leaves an `UpgradeSelf`
            // in `Executing`. Nothing but a completed install can get control
            // here, so this is the only honest place to record it as executed —
            // and it means the public queue says "executed" because the code
            // really was replaced, not because a call returned `Ok`.
            //
            // Any OTHER `Executing` proposal was interrupted by this upgrade and
            // goes back to `Pending` with its original notice period, which grants
            // nothing.
            let now = ic_cdk::api::time() / 1_000_000_000;
            let ids: Vec<u64> = state
                .proposals
                .iter()
                .filter(|(_, p)| p.status == ProposalStatus::Executing)
                .map(|(id, _)| *id)
                .collect();
            for id in ids {
                let Some(mut p) = state.proposals.remove(&id) else {
                    continue;
                };
                if matches!(p.action, Action::UpgradeSelf { .. }) {
                    p.status = ProposalStatus::Executed { at_secs: now };
                    p.executing_since_secs = None;
                    state.history.push(p);
                    let len = state.history.len();
                    if len > MAX_HISTORY {
                        state.history.drain(0..len - MAX_HISTORY);
                    }
                } else {
                    p.status = ProposalStatus::Pending;
                    p.executing_since_secs = None;
                    p.last_error = Some(
                        "the execute attempt was interrupted by an upgrade of the guardian"
                            .to_string(),
                    );
                    state.proposals.insert(id, p);
                }
            }
            if state.operator.is_none() {
                ic_cdk::trap("guardian post_upgrade restored a state with no operator; refusing");
            }
            STATE.with(|s| *s.borrow_mut() = state);
        }
        Err(e) => ic_cdk::trap(&format!(
            "guardian post_upgrade could not restore stable state ({e}); refusing the upgrade"
        )),
    }
}

// ---------------------------------------------------------------------------
// the queue
// ---------------------------------------------------------------------------

#[ic_cdk::update]
fn propose(action: Action) -> Result<u64, String> {
    let caller = require_operator()?;
    validate(&action)?;

    let now = now_secs();
    let delay = action.delay_secs();

    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if st.proposals.len() >= MAX_PENDING {
            return Err(format!(
                "there are already {MAX_PENDING} open proposals; cancel one before opening another"
            ));
        }
        let id = st.next_id;
        st.next_id = st.next_id.saturating_add(1);
        st.proposals.insert(
            id,
            Proposal {
                id,
                kind: action.kind().to_string(),
                action,
                proposed_by: caller,
                proposed_at_secs: now,
                ready_at_secs: now.saturating_add(delay),
                status: ProposalStatus::Pending,
                last_error: None,
                attempts: 0,
                executing_since_secs: None,
            },
        );
        Ok(id)
    })
}

/// Whether a claim on an `Executing` proposal has gone stale. See
/// `EXECUTING_GRACE_SECS`.
fn claim_is_stale(p: &Proposal, now: u64) -> bool {
    match p.executing_since_secs {
        Some(since) => now >= since.saturating_add(EXECUTING_GRACE_SECS),
        // No timestamp means the claim predates this field; treat it as stale
        // rather than wedged.
        None => true,
    }
}

#[ic_cdk::update]
fn cancel(id: u64) -> Result<(), String> {
    require_operator()?;
    let now = now_secs();
    let p = STATE.with(|s| s.borrow().proposals.get(&id).cloned());
    let mut p = p.ok_or_else(|| format!("no open proposal {id}"))?;
    if p.status == ProposalStatus::Executing && !claim_is_stale(&p, now) {
        return Err(format!(
            "proposal {id} has an execute in flight; it becomes cancellable again \
             {EXECUTING_GRACE_SECS} seconds after the attempt started"
        ));
    }
    p.status = ProposalStatus::Cancelled { at_secs: now };
    p.executing_since_secs = None;
    retire(p);
    Ok(())
}

/// `install_chunked_code --mode upgrade` on THIS canister, sent as a ONE-WAY call.
///
/// # Why one-way, measured rather than assumed
///
/// The obvious code — `install_chunked_code(target = self).await` — does not work,
/// and it does not fail cleanly. Measured on PocketIC, on the guardian module built
/// from this tree:
///
/// ```text
///   execute() at the SELF notice period ->
///     Err("Canister called `ic0.trap` with message: 'Panicked at 'internal error:
///     entered unreachable code: CallFutureState for in-flight calls should only be
///     Executing or Trapped (callback)', ic-cdk-0.19.0/src/call.rs:984'
///     call_on_cleanup also failed: ... (cleanup)")
///   guardian module_hash after = UNCHANGED
/// ```
///
/// The upgrade replaces the module, and with it the callback table that the
/// awaiting future lives in, so when the management canister's reply arrives there
/// is nothing left to deliver it to and ic-cdk traps in its own callback. The trap
/// rolls the whole message back, so the upgrade never lands.
///
/// A one-way call registers no callback, so there is nothing for the upgrade to
/// destroy. The cost is that the guardian never learns whether the install
/// succeeded, which is why:
///
/// * the proposal is left in `Executing` and only `post_upgrade` moves it to
///   `Executed` — so the queue records the upgrade because the upgrade HAPPENED,
///   not because a call returned `Ok`;
/// * a proposal that stays `Executing` past `EXECUTING_GRACE_SECS` becomes
///   retryable and cancellable again, because a one-way call that failed leaves no
///   other trace and must not be able to wedge the queue.
///
/// A reader who wants independent confirmation does not have to trust the queue:
/// the guardian's module hash is public, at
/// `/api/v2/canister/<guardian>/read_state` path `canister/<guardian>/module_hash`.
fn self_upgrade(
    wasm_module_hash: &[u8],
    chunk_hashes: &[Vec<u8>],
    arg: &[u8],
) -> Result<(), String> {
    let me = ic_cdk::api::canister_self();
    let args = SelfInstallArgs {
        mode: CanisterInstallMode::Upgrade(None),
        target_canister: me,
        store_canister: Some(me),
        chunk_hashes_list: to_chunk_hashes(chunk_hashes),
        wasm_module_hash: wasm_module_hash.to_vec(),
        arg: arg.to_vec(),
        sender_canister_version: Some(ic_cdk::api::canister_version()),
    };
    ic_cdk::call::Call::unbounded_wait(Principal::management_canister(), "install_chunked_code")
        .with_arg(&args)
        .oneway()
        .map_err(|e| format!("one-way install_chunked_code(upgrade) on self failed to send: {e:?}"))
}

/// The management canister's `install_chunked_code` argument, spelled out here
/// because the one-way path cannot go through `ic_cdk::management_canister`, which
/// only offers awaiting calls.
/// `blob` and `vec nat8` are the same Candid type, so the plain `Vec<u8>` fields
/// are wire-identical to the management canister's declaration.
#[derive(CandidType, Serialize, Clone, Debug)]
struct SelfInstallArgs {
    mode: CanisterInstallMode,
    target_canister: Principal,
    store_canister: Option<Principal>,
    chunk_hashes_list: Vec<ChunkHash>,
    wasm_module_hash: Vec<u8>,
    arg: Vec<u8>,
    sender_canister_version: Option<u64>,
}

/// Execute a proposal whose notice period has elapsed.
///
/// Everything this canister can make the management canister do happens here, and
/// nowhere else, and it is three verbs long.
#[ic_cdk::update]
async fn execute(id: u64) -> Result<(), String> {
    require_operator()?;
    let now = now_secs();

    // --- claim the proposal BEFORE any await --------------------------------
    let action = STATE.with(|s| {
        let mut st = s.borrow_mut();
        let p = st
            .proposals
            .get_mut(&id)
            .ok_or_else(|| format!("no open proposal {id}"))?;
        match p.status {
            ProposalStatus::Pending => {}
            ProposalStatus::Executing if claim_is_stale(p, now) => {}
            ProposalStatus::Executing => {
                return Err(format!("proposal {id} already has an execute in flight"))
            }
            _ => return Err(format!("proposal {id} is not pending")),
        }
        if now < p.ready_at_secs {
            return Err(format!(
                "proposal {id} is timelocked for another {} seconds (ready at {}, now {})",
                p.ready_at_secs - now,
                p.ready_at_secs,
                now
            ));
        }
        p.status = ProposalStatus::Executing;
        p.executing_since_secs = Some(now);
        p.attempts = p.attempts.saturating_add(1);
        Ok(p.action.clone())
    })?;

    let outcome: Result<(), String> = match &action {
        Action::UpgradeGuarded {
            target,
            wasm_module_hash,
            chunk_hashes,
            arg,
        } => management_canister::install_chunked_code(&InstallChunkedCodeArgs {
            // The only mode this canister ever names. There is no code path here
            // that can produce Reinstall or Install, so no argument, no role and
            // no emergency can select one.
            mode: CanisterInstallMode::Upgrade(None),
            target_canister: *target,
            store_canister: Some(*target),
            chunk_hashes_list: to_chunk_hashes(chunk_hashes),
            wasm_module_hash: wasm_module_hash.clone(),
            arg: arg.clone(),
        })
        .await
        .map_err(|e| format!("install_chunked_code(upgrade) on {target} failed: {e:?}")),

        Action::UpgradeSelf {
            wasm_module_hash,
            chunk_hashes,
            arg,
        } => self_upgrade(wasm_module_hash, chunk_hashes, arg),

        Action::TransferOperator { new_operator } => {
            STATE.with(|s| s.borrow_mut().operator = Some(*new_operator));
            Ok(())
        }
    };

    // --- settle ------------------------------------------------------------
    let done_at = now_secs();
    let p = STATE.with(|s| s.borrow().proposals.get(&id).cloned());
    let Some(mut p) = p else { return outcome };
    match &outcome {
        // A SELF-UPGRADE that was successfully SENT is not a self-upgrade that
        // HAPPENED: it went out one-way (see `self_upgrade`), so `Ok` here means
        // only that the message left. It stays `Executing`, and the ONLY thing
        // that can mark it `Executed` is `post_upgrade` — which runs if and only
        // if the new module is actually installed. `EXECUTING_GRACE_SECS` is what
        // un-wedges it if the install was refused.
        Ok(()) if matches!(action, Action::UpgradeSelf { .. }) => {
            p.last_error = None;
            STATE.with(|s| {
                s.borrow_mut().proposals.insert(id, p);
            });
        }
        Ok(()) => {
            p.status = ProposalStatus::Executed { at_secs: done_at };
            p.last_error = None;
            p.executing_since_secs = None;
            retire(p);
        }
        Err(e) => {
            // Back to Pending with the SAME ready_at: a transient management-call
            // failure must not cost another notice period, and must not grant one
            // either.
            p.status = ProposalStatus::Pending;
            p.last_error = Some(e.clone());
            p.executing_since_secs = None;
            STATE.with(|s| {
                s.borrow_mut().proposals.insert(id, p);
            });
        }
    }
    outcome
}

// ---------------------------------------------------------------------------
// chunk plumbing
// ---------------------------------------------------------------------------
//
// A ClearDeck table wasm is ~2.8 MB, which is over BOTH the 2 MB ingress limit and
// the 2 MB inter-canister payload limit, so `install_code` with the module inline
// is not merely inconvenient here, it is impossible. Chunks go into the TARGET's
// own chunk store; the guardian never holds a module.
//
// Neither of these two touches the target's execution state. `upload_chunk` writes
// to a store that only `install_chunked_code` reads, and `clear_chunk_store`
// empties it. A proposal still has to clear its notice period before any of it can
// become code.

#[ic_cdk::update]
async fn upload_chunk(target: Principal, chunk: Vec<u8>) -> Result<Vec<u8>, String> {
    require_operator()?;
    if chunk.is_empty() {
        return Err("refusing to upload an empty chunk".to_string());
    }
    if chunk.len() > MAX_CHUNK_BYTES {
        return Err(format!(
            "chunk is {} bytes, the management canister's limit is {MAX_CHUNK_BYTES}",
            chunk.len()
        ));
    }
    let allowed = STATE.with(|s| {
        let st = s.borrow();
        st.guarded.contains(&target)
    }) || target == ic_cdk::api::canister_self();
    if !allowed {
        return Err(format!("{target} is not in this guardian's guarded set"));
    }
    let res = management_canister::upload_chunk(&UploadChunkArgs {
        canister_id: target,
        chunk,
    })
    .await
    .map_err(|e| format!("upload_chunk to {target} failed: {e:?}"))?;
    Ok(res.hash)
}

#[ic_cdk::update]
async fn clear_chunk_store(target: Principal) -> Result<(), String> {
    require_operator()?;
    management_canister::clear_chunk_store(&ClearChunkStoreArgs {
        canister_id: target,
    })
    .await
    .map_err(|e| format!("clear_chunk_store on {target} failed: {e:?}"))
}

#[ic_cdk::update]
async fn stored_chunks(target: Principal) -> Result<Vec<Vec<u8>>, String> {
    require_operator()?;
    let res = management_canister::stored_chunks(&StoredChunksArgs {
        canister_id: target,
    })
    .await
    .map_err(|e| format!("stored_chunks on {target} failed: {e:?}"))?;
    Ok(res.into_iter().map(|c| c.hash).collect())
}

/// Declare a canister as guarded. Grants NOTHING: controllership is decided by the
/// target's own controller list, which this canister cannot write. Its only effect
/// is that a proposal naming this canister can be opened, and that
/// `get_config().guarded` names it for a reader.
#[ic_cdk::update]
fn note_guarded(target: Principal) -> Result<(), String> {
    require_operator()?;
    STATE.with(|s| s.borrow_mut().guarded.insert(target));
    Ok(())
}

// ---------------------------------------------------------------------------
// the public record — every one of these is open to anybody, unauthenticated
// ---------------------------------------------------------------------------
//
// A timelock nobody can read is not a timelock. These are queries with no caller
// check on purpose.

#[ic_cdk::query]
fn get_config() -> GuardianConfig {
    STATE.with(|s| {
        let st = s.borrow();
        GuardianConfig {
            operator: st.operator.unwrap_or_else(Principal::anonymous),
            guarded: st.guarded.iter().copied().collect(),
            guarded_delay_secs: GUARDED_DELAY_SECS,
            self_delay_secs: SELF_DELAY_SECS,
            operator_delay_secs: OPERATOR_DELAY_SECS,
            max_pending: MAX_PENDING as u64,
            refused_operations: REFUSED_OPERATIONS.iter().map(|s| s.to_string()).collect(),
        }
    })
}

#[ic_cdk::query]
fn list_proposals() -> Vec<Proposal> {
    STATE.with(|s| s.borrow().proposals.values().cloned().collect())
}

#[ic_cdk::query]
fn get_proposal(id: u64) -> Option<Proposal> {
    STATE.with(|s| {
        let st = s.borrow();
        st.proposals
            .get(&id)
            .cloned()
            .or_else(|| st.history.iter().rev().find(|p| p.id == id).cloned())
    })
}

#[ic_cdk::query]
fn history() -> Vec<Proposal> {
    STATE.with(|s| s.borrow().history.clone())
}

/// Seconds remaining before `id` may execute. `Some(0)` means it is ready now;
/// `None` means there is no such open proposal.
#[ic_cdk::query]
fn seconds_until_ready(id: u64) -> Option<u64> {
    let now = now_secs();
    STATE.with(|s| {
        s.borrow()
            .proposals
            .get(&id)
            .map(|p| p.ready_at_secs.saturating_sub(now))
    })
}

/// The verbs this canister will never emit, as data.
#[ic_cdk::query]
fn refused_operations() -> Vec<String> {
    REFUSED_OPERATIONS.iter().map(|s| s.to_string()).collect()
}

ic_cdk::export_candid!();
