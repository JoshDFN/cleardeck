//! The harness world: a PocketIC instance holding the REAL ICP ledger at
//! `ryjl3-tyaaa-aaaaa-aaaba-cai` and the REAL table canister wasm, plus typed
//! wrappers for every money-moving call and the observation code the invariants
//! read.
//!
//! Nothing here is a mock. The ledger enforces real allowances, real 10_000 e8s
//! fees and real balances, and it is installed at exactly the canister id the
//! table canister hardcodes, so the production deposit and withdraw paths run
//! unmodified.

use candid::{decode_one, encode_args, encode_one, CandidType, Decode, Encode, Nat, Principal};
use pocket_ic::{PocketIc, PocketIcBuilder, RejectResponse, Time};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::time::Duration;

use crate::ledger::{self, Account, ApproveArgs, LedgerResult, TransferArg};
use crate::table_api::*;
use crate::wasms;

/// 2026-01-01T00:00:00Z. A realistic wall clock matters: several canister paths
/// do `now - last_time` and `now + timeout`, and starting near zero would make
/// the rate limiter behave in a way it never does in production.
pub const GENESIS_NANOS: u64 = 1_767_225_600_000_000_000;

/// Starting ICP in every actor's ledger wallet: 10_000 ICP.
pub const ACTOR_START_E8S: u64 = 1_000_000_000_000;

/// What a call did. `Trap` is kept distinct from `Err` on purpose: a trap rolls
/// the canister's state back, an `Err` return does not, and "hostile input is
/// rejected cleanly" (M4) is a claim about the second, not the first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpError {
    /// The IC rejected the message: trap, out of cycles, no such method.
    Trap(String),
    /// The method returned `Err(text)`.
    Err(String),
}

pub type Outcome<T> = Result<T, OpError>;

fn describe(reject: RejectResponse) -> OpError {
    OpError::Trap(format!("{reject:?}"))
}

#[derive(Clone, Debug)]
pub struct Actor {
    pub name: String,
    pub principal: Principal,
}

/// Reads the module hash the replica reports for `canister` and hard-fails unless
/// it is exactly `expected_sha256`.
///
/// The IC module hash IS the sha256 of the wasm, so this closes the loop between
/// "the bytes the harness built" and "the code the replica executes". Without it,
/// `install_canister` succeeding is only evidence that *some* module installed.
fn assert_installed_module_is(
    pic: &PocketIc,
    canister: Principal,
    controller: Principal,
    expected_sha256: &str,
    stage: &str,
) {
    let status = pic
        .canister_status(canister, Some(controller))
        .unwrap_or_else(|e| panic!("canister_status after {stage} was rejected: {e:?}"));
    let installed = status
        .module_hash
        .map(hex::encode)
        .unwrap_or_else(|| "<empty canister>".to_string());
    assert_eq!(
        installed, expected_sha256,
        "after {stage} the replica is running module {installed} but the harness built \
         {expected_sha256}. Every invariant result would be a statement about a binary that is not \
         in this tree. Refusing to continue (docs/DEFECTS.md H-01)."
    );
}

/// Everything the invariants read, captured at one instant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// `icrc1_balance_of(table, None)` -- the canister's spendable ledger money.
    pub ledger_main: u64,
    /// `icrc1_balance_of(table, deposit_subaccount(p))`, summed over every
    /// principal in [`Snapshot::ledger_deposit_by_principal`].
    pub ledger_deposit_subaccounts: u64,
    /// **The LEDGER, per principal.** What is actually at each deposit address.
    ///
    /// The scanned set is the harness's actors, the controller, AND every
    /// principal the canister itself names in `admin_get_deposit_custody()` --
    /// the last one so the canister cannot move a claim onto an account the
    /// harness would never have thought to look at.
    pub ledger_deposit_by_principal: BTreeMap<Principal, u64>,
    /// **The CANISTER, per principal.** What it says is at each deposit address.
    pub canister_deposit_by_principal: BTreeMap<Principal, u64>,
    /// Every actor's own ledger wallet balance.
    pub actor_wallets: BTreeMap<Principal, u64>,
    /// `admin_get_all_balances()`: the canister's escrow ledger.
    pub escrow: BTreeMap<Principal, u64>,
    pub escrow_total: u64,
    /// `admin_get_table_chips()`.
    pub chips_total: u64,
    /// `admin_get_deposit_custody()`: what the CANISTER says it is holding in its
    /// own deposit subaccounts, summed. **This is the canister's claim; the field
    /// above it is the ledger's fact, and the whole of FINDING 21 / FINDING 28 is
    /// the gap between the two.**
    pub canister_unswept_deposits: u64,
    /// Enumerable deposit accounts the canister has never read. An account here
    /// has an UNKNOWN balance, which every guard has to treat as money.
    pub canister_unaudited_deposit_accounts: usize,
    /// **What the canister itself says about whether it can pay everyone**, read
    /// from `get_solvency()` as an ANONYMOUS caller, because a player must be able
    /// to ask without permission.
    ///
    /// `None` means the build has no such surface at all -- which is the state
    /// docs/SECURITY-FINDINGS.md FINDING 35 describes and is itself reported as a
    /// violation, not skipped. Every other field of this snapshot is a fact about
    /// the world; this one is the canister's CLAIM about it, and the gap between
    /// the two is what `invariants::solvency` measures.
    pub canister_solvency: Option<SolvencyReport>,
    pub table: TableState,
}

impl Snapshot {
    /// Everything the canister currently owes somebody, as the canister itself
    /// accounts for it. `side_pots` is deliberately NOT added: it is a breakdown
    /// of `pot`, and adding both would double count (see the comment in
    /// `end_hand_single_winner`).
    ///
    /// **`canister_unswept_deposits` IS added**, and that is the wave-8 change.
    /// The canister owns two kinds of ledger account and this figure used to name
    /// only the first, so a canister holding 5 ICP for a player at an address it
    /// had published to them reported that it owed nobody anything
    /// (docs/SECURITY-FINDINGS.md FINDING 21, FINDING 28).
    pub fn internal_total(&self) -> u64 {
        self.escrow_total
            .saturating_add(self.chips_total)
            .saturating_add(self.table.pot)
            .saturating_add(self.canister_unswept_deposits)
    }

    /// **THE FACT.** Every e8 the LEDGER says this canister is holding, across
    /// every account it owns: the main account plus every deposit subaccount.
    ///
    /// Every instrument in this harness used to be anchored to `ledger_main`
    /// alone. Anchor a new one here.
    pub fn ledger_holdings(&self) -> u64 {
        self.ledger_main
            .saturating_add(self.ledger_deposit_subaccounts)
    }
}

pub struct World {
    pub pic: PocketIc,
    pub ledger: Principal,
    pub table: Principal,
    pub controller: Principal,
    pub minter: Principal,
    pub actors: Vec<Actor>,
    pub config: TableConfig,
    table_wasm: Vec<u8>,
    /// Money the harness pushed straight into the canister's MAIN ledger account
    /// with a raw `icrc1_transfer` (the `notify_deposit` flow) and that the
    /// canister has not credited to anybody yet. This is the ONLY legitimate
    /// reason for the canister's ledger balance to exceed what it owes, so M1
    /// subtracts exactly this and nothing else.
    pub uncredited_raw_deposits: u64,
    /// Money the harness pushed into a per-player DEPOSIT SUBACCOUNT and has not
    /// yet asked the canister to look at.
    ///
    /// The exact analogue of `uncredited_raw_deposits`, for ledger account (2) of
    /// "THE ACCOUNT CENSUS", and bounded the same way: an entry is created by
    /// `transfer_to_deposit_subaccount` and **consumed the moment the canister is
    /// asked about that account** (`refresh_deposit_custody` /
    /// `claim_external_deposit`). After that the canister has no excuse, and the
    /// orphan check requires its books to match the ledger exactly.
    ///
    /// This is deliberately per-actor rather than a single total: a blanket
    /// allowance would let the canister be wrong about actor A and right about
    /// actor B and still net to zero, which is the CORRECT TOTALS / WRONG
    /// RECIPIENTS signature this project has produced four times.
    pub unobserved_subaccount_deposits: BTreeMap<Principal, u64>,
    pub upgrades: u32,
    /// Lowest canister-log index the harness has not read yet.
    next_log_idx: u64,
}

impl World {
    pub fn new(config: TableConfig, actor_names: &[&str]) -> Self {
        let mut builder = PocketIcBuilder::new()
            .with_nns_subnet() // ryjl3-... lives in the NNS canister range
            .with_application_subnet();
        if let Some(bin) = wasms::pocket_ic_binary() {
            builder = builder.with_server_binary(bin);
        }
        let pic = builder.build();
        pic.set_time(Time::from_nanos_since_unix_epoch(GENESIS_NANOS));

        let controller = Principal::self_authenticating(b"cleardeck-money-safety-controller");
        let minter = Principal::self_authenticating(b"cleardeck-money-safety-minter");
        let actors: Vec<Actor> = actor_names
            .iter()
            .map(|n| Actor {
                name: (*n).to_string(),
                principal: Principal::self_authenticating(
                    format!("cleardeck-money-safety-actor-{n}").as_bytes(),
                ),
            })
            .collect();

        // --- the real ICP ledger, at the id the table canister hardcodes -----
        let ledger_id = ledger::ledger_principal();
        pic.create_canister_with_id(Some(controller), None, ledger_id)
            .expect("could not create the ledger canister at ryjl3-tyaaa-aaaaa-aaaba-cai");
        pic.add_cycles(ledger_id, 100_000_000_000_000);
        let initial: Vec<(Principal, u64)> = actors
            .iter()
            .map(|a| (a.principal, ACTOR_START_E8S))
            .collect();
        pic.install_canister(
            ledger_id,
            wasms::icp_ledger_wasm(),
            ledger::init_payload(minter, &initial, controller),
            Some(controller),
        );

        // --- the table canister under test ----------------------------------
        let module = wasms::table_canister_module();
        let table_wasm = module.bytes.clone();
        let table = pic.create_canister_with_settings(Some(controller), None);
        pic.add_cycles(table, 100_000_000_000_000);
        pic.install_canister(
            table,
            table_wasm.clone(),
            encode_one(&config).expect("table init arg encode"),
            Some(controller),
        );
        // The module the replica is actually running must be the module this
        // harness just built. Anything else and the result cannot be attributed
        // to the source tree (docs/DEFECTS.md H-01).
        assert_installed_module_is(&pic, table, controller, &module.sha256, "install");

        Self {
            pic,
            ledger: ledger_id,
            table,
            controller,
            minter,
            actors,
            config,
            table_wasm,
            uncredited_raw_deposits: 0,
            unobserved_subaccount_deposits: BTreeMap::new(),
            upgrades: 0,
            next_log_idx: 0,
        }
    }

    /// A default world: 6-max ICP table, four actors.
    pub fn default_world() -> Self {
        Self::new(
            TableConfig::six_max_icp(),
            &["alice", "bob", "carol", "attacker"],
        )
    }

    pub fn actor(&self, name: &str) -> Principal {
        self.actors
            .iter()
            .find(|a| a.name == name)
            .unwrap_or_else(|| panic!("no actor named {name}"))
            .principal
    }

    pub fn now_nanos(&self) -> u64 {
        self.pic.get_time().as_nanos_since_unix_epoch()
    }

    pub fn advance(&self, d: Duration) {
        self.pic.advance_time(d);
        self.pic.tick();
    }

    /// Move the clock forward WITHOUT executing a round.
    ///
    /// `advance` ticks, and a tick is when a canister timer gets to run. That is
    /// usually what a test wants -- it is what a real subnet does -- but it makes
    /// one class of state unobservable: the moment AFTER a deadline passes and
    /// BEFORE anything has reacted to it. On a real subnet that window is however
    /// long the canister goes unexecuted; here it is zero unless the test asks for
    /// it.
    ///
    /// # SEND AN UPDATE THROUGH IT. That is the whole point.
    ///
    /// docs/DEFECTS.md E-59. This method existed for a whole wave and was used in
    /// exactly three places, **all of them queries**. A query executes no round, so
    /// it can only ever ask the canister what it believes; it can never make the
    /// canister ACT on that belief before the on-chain clock has had its turn.
    /// Meanwhile every other test reached time through `advance`, which ticks, so
    /// the clock always won the race. The result was a harness in which the state
    /// where a caller and the canister's own timer disagree about one hand was
    /// **unreachable**, and 138 comparisons later it turned out to be the fourth
    /// cross-agent defect in this project.
    ///
    /// So: `advance_time_only(stall)` and then an **update** is the race, and
    /// `tests/stall_agreement.rs` (M13) is the gate built on it. Until the harness
    /// could express the race, no gate could catch it.
    pub fn advance_time_only(&self, d: Duration) {
        self.pic.advance_time(d);
    }

    // -----------------------------------------------------------------------
    // raw plumbing
    // -----------------------------------------------------------------------

    fn update_raw(&self, sender: Principal, method: &str, arg: Vec<u8>) -> Outcome<Vec<u8>> {
        self.pic
            .update_call(self.table, sender, method, arg)
            .map_err(describe)
    }

    fn query_raw(&self, sender: Principal, method: &str, arg: Vec<u8>) -> Outcome<Vec<u8>> {
        self.pic
            .query_call(self.table, sender, method, arg)
            .map_err(describe)
    }

    /// An arbitrary query, sent AS `sender`, returning the raw reply.
    ///
    /// Exists for [`crate::invariants::custody`], which has to ask the canister
    /// what a specific PLAYER can see and must survive a method that does not
    /// exist on the build under test -- a probe that only compiles against the
    /// fixed canister cannot convict the defect. Every other reader in this
    /// harness is the controller, which is exactly how FINDING 18 stayed
    /// invisible.
    pub fn query_as(&self, sender: Principal, method: &str, arg: Vec<u8>) -> Outcome<Vec<u8>> {
        self.query_raw(sender, method, arg)
    }

    /// A `Result<T, String>`-returning update.
    fn update_result<T>(&self, sender: Principal, method: &str, arg: Vec<u8>) -> Outcome<T>
    where
        T: CandidType + DeserializeOwned,
    {
        let bytes = self.update_raw(sender, method, arg)?;
        match decode_one::<Result<T, String>>(&bytes) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(msg)) => Err(OpError::Err(msg)),
            Err(e) => Err(OpError::Trap(format!("reply decode failed for {method}: {e}"))),
        }
    }

    fn query_result<T>(&self, sender: Principal, method: &str, arg: Vec<u8>) -> Outcome<T>
    where
        T: CandidType + DeserializeOwned,
    {
        let bytes = self.query_raw(sender, method, arg)?;
        match decode_one::<Result<T, String>>(&bytes) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(msg)) => Err(OpError::Err(msg)),
            Err(e) => Err(OpError::Trap(format!("reply decode failed for {method}: {e}"))),
        }
    }

    // -----------------------------------------------------------------------
    // ledger side
    // -----------------------------------------------------------------------

    pub fn ledger_balance(&self, owner: Principal, subaccount: Option<[u8; 32]>) -> u64 {
        let account = Account {
            owner,
            subaccount: subaccount.map(|s| s.to_vec()),
        };
        let bytes = self
            .pic
            .query_call(
                self.ledger,
                Principal::anonymous(),
                "icrc1_balance_of",
                ledger::encode_balance_of(&account),
            )
            .expect("icrc1_balance_of must not be rejected");
        ledger::decode_balance(&bytes)
    }

    /// `icrc2_approve`: let the table canister pull `amount` from `who`.
    pub fn approve(&self, who: Principal, amount: u64) -> Result<u64, String> {
        let args = ApproveArgs {
            from_subaccount: None,
            spender: Account {
                owner: self.table,
                subaccount: None,
            },
            amount: Nat::from(amount),
            expected_allowance: None,
            expires_at: None,
            fee: Some(Nat::from(ledger::TRANSFER_FEE)),
            memo: None,
            created_at_time: None,
        };
        let bytes = self
            .pic
            .update_call(
                self.ledger,
                who,
                "icrc2_approve",
                Encode!(&args).expect("approve arg encode"),
            )
            .map_err(|r| format!("{r:?}"))?;
        Decode!(&bytes, LedgerResult)
            .map_err(|e| format!("approve reply decode: {e}"))?
            .block()
    }

    /// A raw `icrc1_transfer` from an actor's wallet straight into the table
    /// canister's MAIN account -- the on-ledger half of the `notify_deposit`
    /// flow. Returns the block index. The harness records the amount as
    /// uncredited until a `notify_deposit` succeeds for it.
    pub fn raw_transfer_to_canister(&mut self, who: Principal, amount: u64) -> Result<u64, String> {
        let args = TransferArg {
            from_subaccount: None,
            to: Account {
                owner: self.table,
                subaccount: None,
            },
            amount: Nat::from(amount),
            fee: Some(Nat::from(ledger::TRANSFER_FEE)),
            memo: None,
            created_at_time: None,
        };
        let bytes = self
            .pic
            .update_call(
                self.ledger,
                who,
                "icrc1_transfer",
                Encode!(&args).expect("transfer arg encode"),
            )
            .map_err(|r| format!("{r:?}"))?;
        let block = Decode!(&bytes, LedgerResult)
            .map_err(|e| format!("transfer reply decode: {e}"))?
            .block()?;
        self.uncredited_raw_deposits = self.uncredited_raw_deposits.saturating_add(amount);
        Ok(block)
    }

    /// A raw `icrc1_transfer` into an actor's own per-player deposit subaccount
    /// on the table canister -- the on-ledger half of `claim_external_deposit`.
    ///
    /// **The canister is not told.** That is the whole point of this door: it is
    /// exactly what an external wallet does, and the ledger moving without a
    /// message is the reason ledger account (2) of "THE ACCOUNT CENSUS" needs an
    /// instrument of its own. The amount is recorded in
    /// `unobserved_subaccount_deposits` until the canister is asked about it.
    pub fn transfer_to_deposit_subaccount(
        &mut self,
        who: Principal,
        amount: u64,
    ) -> Result<u64, String> {
        let args = TransferArg {
            from_subaccount: None,
            to: Account {
                owner: self.table,
                subaccount: Some(ledger::deposit_subaccount(&who).to_vec()),
            },
            amount: Nat::from(amount),
            fee: Some(Nat::from(ledger::TRANSFER_FEE)),
            memo: None,
            created_at_time: None,
        };
        let bytes = self
            .pic
            .update_call(
                self.ledger,
                who,
                "icrc1_transfer",
                Encode!(&args).expect("transfer arg encode"),
            )
            .map_err(|r| format!("{r:?}"))?;
        let block = Decode!(&bytes, LedgerResult)
            .map_err(|e| format!("transfer reply decode: {e}"))?
            .block()?;
        let entry = self
            .unobserved_subaccount_deposits
            .entry(who)
            .or_insert(0);
        *entry = entry.saturating_add(amount);
        Ok(block)
    }

    // -----------------------------------------------------------------------
    // table canister: money
    // -----------------------------------------------------------------------

    /// `deposit` (ICRC-2 pull). Requires a prior `approve`.
    pub fn deposit(&self, who: Principal, amount: u64) -> Outcome<u64> {
        self.update_result(who, "deposit", encode_one(amount).unwrap())
    }

    /// Approve then deposit, the way the frontend does it. The allowance covers
    /// the amount plus the ledger fee the transfer_from will charge.
    pub fn fund_escrow(&self, who: Principal, amount: u64) -> Outcome<u64> {
        self.approve(who, amount.saturating_add(ledger::TRANSFER_FEE))
            .map_err(OpError::Err)?;
        self.deposit(who, amount)
    }

    /// `claim_external_deposit`. The canister asks the ledger about the caller's
    /// deposit subaccount before it decides anything, so **whatever the outcome,
    /// the canister has now looked** and the harness's allowance for that actor is
    /// spent -- Err included. A refusal that leaves the canister still unaware of
    /// money it is holding is FINDING 28, and the allowance is what would hide it.
    pub fn claim_external_deposit(&mut self, who: Principal) -> Outcome<u64> {
        let out = self.update_result(who, "claim_external_deposit", Encode!().unwrap());
        if !matches!(out, Err(OpError::Trap(_))) {
            self.unobserved_subaccount_deposits.remove(&who);
        }
        out
    }

    /// `refresh_deposit_custody`: ask the ledger what is at the caller's deposit
    /// address and write it into the canister's books. Moves no money.
    pub fn refresh_deposit_custody(&mut self, who: Principal) -> Outcome<DepositAddressCustody> {
        let out = self.update_result::<DepositAddressCustody>(
            who,
            "refresh_deposit_custody",
            Encode!().unwrap(),
        );
        if !matches!(out, Err(OpError::Trap(_))) {
            self.unobserved_subaccount_deposits.remove(&who);
        }
        out
    }

    /// `get_deposit_custody` (query, caller-scoped).
    pub fn deposit_custody(&self, who: Principal) -> DepositAddressCustody {
        let bytes = self
            .query_raw(who, "get_deposit_custody", Encode!().unwrap())
            .expect("get_deposit_custody must not be rejected");
        decode_one::<DepositAddressCustody>(&bytes).unwrap_or_else(|e| {
            panic!(
                "get_deposit_custody reply did not decode: {e}. On a build where the method does \
                 not exist at all, the canister has no surface that can report money at the \
                 deposit address it published -- docs/SECURITY-FINDINGS.md FINDING 28."
            )
        })
    }

    /// `admin_audit_deposit_custody`: the controller reads every enumerable
    /// deposit subaccount. Returns `(read this call, total observed, still
    /// unaudited)`.
    pub fn admin_audit_deposit_custody(&mut self, also: &[Principal]) -> Outcome<(u64, u64, u64)> {
        let also = also.to_vec();
        let out = self.update_result::<(u64, u64, u64)>(
            self.controller,
            "admin_audit_deposit_custody",
            Encode!(&also).unwrap(),
        );
        if !matches!(out, Err(OpError::Trap(_))) {
            self.unobserved_subaccount_deposits.clear();
        }
        out
    }

    /// `admin_get_deposit_custody` (query). `(total observed, per-principal
    /// (amount, observed_at_ns), accounts never read)`.
    pub fn admin_deposit_custody(&self) -> (u64, Vec<(Principal, u64, u64)>, Vec<Principal>) {
        self.query_result::<(u64, Vec<(Principal, u64, u64)>, Vec<Principal>)>(
            self.controller,
            "admin_get_deposit_custody",
            Encode!().unwrap(),
        )
        .unwrap_or_else(|e| {
            panic!(
                "admin_get_deposit_custody must answer the controller, got {e:?}. Without it \
                 there is no controller surface that names the canister's deposit-subaccount \
                 custody at all -- docs/SECURITY-FINDINGS.md FINDING 21."
            )
        })
    }

    /// `get_custody_status` for one caller: the complete per-player answer.
    pub fn custody_status(&self, who: Principal) -> CustodyStatus {
        let bytes = self
            .query_raw(who, "get_custody_status", Encode!().unwrap())
            .expect("get_custody_status must not be rejected");
        decode_one::<CustodyStatus>(&bytes).unwrap_or_else(|e| {
            panic!(
                "get_custody_status reply did not decode into the harness's CustodyStatus: {e}. \
                 The likely cause is a build whose CustodyStatus has no `unswept_deposit` field, \
                 i.e. one where the surface that answers \"what is this canister holding for me\" \
                 cannot express money at the deposit address the canister published -- \
                 docs/SECURITY-FINDINGS.md FINDING 28."
            )
        })
    }

    /// Spend an actor's deposit-subaccount allowance by hand.
    ///
    /// For the tests that bypass the wrappers above and drive
    /// `claim_external_deposit` through `pic.submit_call` directly (the
    /// concurrency reproducers). The canister has looked, so the excuse is gone,
    /// and the allowance must be spent explicitly rather than left standing --
    /// an allowance nobody spends is an invariant switched off.
    ///
    /// Deliberately NOT automatic. Deriving "has the canister looked?" from what
    /// the canister reports would make the allowance cancel out the very
    /// discrepancy it exists to expose.
    pub fn note_deposit_address_observed(&mut self, who: Principal) {
        self.unobserved_subaccount_deposits.remove(&who);
    }

    /// The sum of every allowance the harness is still granting for money it put
    /// into deposit subaccounts and has not asked the canister to look at.
    pub fn unobserved_subaccount_total(&self) -> u64 {
        self.unobserved_subaccount_deposits
            .values()
            .fold(0u64, |a, v| a.saturating_add(*v))
    }

    pub fn notify_deposit(&self, who: Principal, block: u64) -> Outcome<u64> {
        self.update_result(who, "notify_deposit", encode_one(block).unwrap())
    }

    pub fn withdraw(&self, who: Principal, amount: u64) -> Outcome<u64> {
        self.update_result(who, "withdraw", encode_one(amount).unwrap())
    }

    pub fn get_balance(&self, who: Principal) -> u64 {
        let bytes = self
            .query_raw(who, "get_balance", Encode!().unwrap())
            .expect("get_balance must not be rejected");
        decode_one::<u64>(&bytes).expect("get_balance reply decode")
    }

    // -----------------------------------------------------------------------
    // table canister: seats and play
    // -----------------------------------------------------------------------

    pub fn join_table(&self, who: Principal, seat: u8) -> Outcome<()> {
        self.update_result(who, "join_table", encode_one(seat).unwrap())
    }

    pub fn buy_in(&self, who: Principal, seat: u8, amount: u64) -> Outcome<()> {
        self.update_result(who, "buy_in", encode_args((seat, amount)).unwrap())
    }

    pub fn reload(&self, who: Principal, amount: u64) -> Outcome<u64> {
        self.update_result(who, "reload", encode_one(amount).unwrap())
    }

    pub fn cash_out(&self, who: Principal) -> Outcome<u64> {
        self.update_result(who, "cash_out", Encode!().unwrap())
    }

    pub fn leave_table(&self, who: Principal) -> Outcome<u64> {
        self.update_result(who, "leave_table", Encode!().unwrap())
    }

    pub fn player_action(&self, who: Principal, action: PlayerAction) -> Outcome<()> {
        self.update_result(who, "player_action", encode_one(action).unwrap())
    }

    pub fn start_new_hand(&self, who: Principal) -> Outcome<ShuffleProof> {
        self.update_result(who, "start_new_hand", Encode!().unwrap())
    }

    pub fn check_timeouts(&self, who: Principal) -> Outcome<TimeoutCheckResult> {
        let bytes = self.update_raw(who, "check_timeouts", Encode!().unwrap())?;
        decode_one::<TimeoutCheckResult>(&bytes)
            .map_err(|e| OpError::Trap(format!("check_timeouts reply decode: {e}")))
    }

    pub fn heartbeat(&self, who: Principal) -> Outcome<()> {
        self.update_result(who, "heartbeat", Encode!().unwrap())
    }

    pub fn sit_out(&self, who: Principal) -> Outcome<()> {
        self.update_result(who, "sit_out", Encode!().unwrap())
    }

    pub fn sit_in(&self, who: Principal) -> Outcome<()> {
        self.update_result(who, "sit_in", Encode!().unwrap())
    }

    pub fn sit_out_next_hand(&self, who: Principal) -> Outcome<()> {
        self.update_result(who, "sit_out_next_hand", Encode!().unwrap())
    }

    pub fn use_time_bank(&self, who: Principal) -> Outcome<u64> {
        self.update_result(who, "use_time_bank", Encode!().unwrap())
    }

    pub fn show_cards(&self, who: Principal) -> Outcome<(Card, Card)> {
        self.update_result(who, "show_cards", Encode!().unwrap())
    }

    /// The escape hatch. Any principal may call it; it refuses unless the hand is
    /// provably immovable. docs/SECURITY-FINDINGS.md FINDING 15.
    pub fn abandon_stuck_hand(&self, who: Principal) -> Outcome<u64> {
        self.update_result(who, "abandon_stuck_hand", Encode!().unwrap())
    }

    /// A QUERY, deliberately: it has to answer when every update fails.
    pub fn stuck_hand_status(&self) -> StuckHandStatus {
        let bytes = self
            .query_raw(
                Principal::anonymous(),
                "get_stuck_hand_status",
                Encode!().unwrap(),
            )
            .expect("get_stuck_hand_status must not be rejected");
        decode_one::<StuckHandStatus>(&bytes).expect("get_stuck_hand_status reply decode")
    }

    // -----------------------------------------------------------------------
    // observation
    // -----------------------------------------------------------------------

    pub fn table_state(&self) -> TableState {
        self.query_result::<TableState>(self.controller, "get_table_state", Encode!().unwrap())
            .expect("get_table_state must succeed for the controller")
    }

    pub fn pot(&self) -> u64 {
        let bytes = self
            .query_raw(Principal::anonymous(), "get_pot", Encode!().unwrap())
            .expect("get_pot must not be rejected");
        decode_one::<u64>(&bytes).expect("get_pot reply decode")
    }

    pub fn escrow_all(&self) -> (u64, BTreeMap<Principal, u64>) {
        let (total, list) = self
            .query_result::<(u64, Vec<(Principal, u64)>)>(
                self.controller,
                "admin_get_all_balances",
                Encode!().unwrap(),
            )
            .expect("admin_get_all_balances must succeed for the controller");
        (total, list.into_iter().collect())
    }

    pub fn chips_total(&self) -> u64 {
        self.query_result::<u64>(self.controller, "admin_get_table_chips", Encode!().unwrap())
            .expect("admin_get_table_chips must succeed for the controller")
    }

    pub fn hand_history(&self, hand_number: u64) -> Option<HandHistoryAmounts> {
        let bytes = self
            .query_raw(
                Principal::anonymous(),
                "get_hand_history",
                encode_one(hand_number).unwrap(),
            )
            .expect("get_hand_history must not be rejected");
        decode_one::<Option<HandHistoryAmounts>>(&bytes).expect("get_hand_history reply decode")
    }

    pub fn snapshot(&self) -> Snapshot {
        let (escrow_total, escrow) = self.escrow_all();
        // What the CANISTER says it holds in its own deposit subaccounts, read
        // from the canister and never computed by the harness. The whole value of
        // this field is that it can DISAGREE with the ledger scan below;
        // computing both from the ledger would make the comparison vacuous.
        let (canister_unswept_deposits, canister_list, unaudited) = self.admin_deposit_custody();
        let canister_deposit_by_principal: BTreeMap<Principal, u64> = canister_list
            .iter()
            .map(|(p, amount, _)| (*p, *amount))
            .collect();

        // THE SET OF ADDRESSES THE HARNESS LOOKS AT. Actors and the controller,
        // because those are the principals that exist here -- and every principal
        // the CANISTER names, because a claim parked on an account the harness
        // would never scan is unfalsifiable, and "the instrument only looks where
        // the money is supposed to be" is this project's standing defect.
        let mut scan: Vec<Principal> = self.actors.iter().map(|a| a.principal).collect();
        for p in std::iter::once(self.controller).chain(canister_deposit_by_principal.keys().copied())
        {
            if !scan.contains(&p) {
                scan.push(p);
            }
        }
        let ledger_deposit_by_principal: BTreeMap<Principal, u64> = scan
            .into_iter()
            .map(|p| {
                (
                    p,
                    self.ledger_balance(self.table, Some(ledger::deposit_subaccount(&p))),
                )
            })
            .collect();
        let ledger_deposit_subaccounts = ledger_deposit_by_principal
            .values()
            .fold(0u64, |acc, v| acc.saturating_add(*v));

        let actor_wallets = self
            .actors
            .iter()
            .map(|a| (a.principal, self.ledger_balance(a.principal, None)))
            .collect();
        Snapshot {
            ledger_main: self.ledger_balance(self.table, None),
            ledger_deposit_subaccounts,
            ledger_deposit_by_principal,
            canister_deposit_by_principal,
            actor_wallets,
            escrow,
            escrow_total,
            chips_total: self.chips_total(),
            canister_unswept_deposits,
            canister_unaudited_deposit_accounts: unaudited.len(),
            canister_solvency: self.solvency(),
            table: self.table_state(),
        }
    }

    /// `get_solvency()` as an ANONYMOUS caller.
    ///
    /// Anonymous on purpose: the claim being tested is that a player can find out
    /// whether the table can pay them WITHOUT asking anybody's permission, so the
    /// harness asks with the least authority there is. `None` when the method does
    /// not exist, is rejected, or does not decode -- all three of which are "this
    /// build cannot say whether it is solvent", which is the defect, not a reason
    /// to skip the check.
    pub fn solvency(&self) -> Option<SolvencyReport> {
        let bytes = self
            .query_raw(Principal::anonymous(), "get_solvency", Encode!().unwrap())
            .ok()?;
        decode_one::<SolvencyReport>(&bytes).ok()
    }

    /// `refresh_solvency()`: ask the ledger what the main account holds, write it
    /// down, and return the whole report.
    ///
    /// Takes `who` so a test can prove any principal may call it. It moves no
    /// money, so the harness's exemption bookkeeping is untouched.
    pub fn refresh_solvency(&self, who: Principal) -> Outcome<SolvencyReport> {
        self.update_result(who, "refresh_solvency", Encode!().unwrap())
    }

    /// `refresh_main_account_custody()`: the reading on its own.
    pub fn refresh_main_account_custody(
        &self,
        who: Principal,
    ) -> Outcome<MainAccountObservation> {
        self.update_result(who, "refresh_main_account_custody", Encode!().unwrap())
    }

    // -----------------------------------------------------------------------
    // upgrades
    // -----------------------------------------------------------------------

    /// A real `install_code --mode upgrade` with the same wasm: exactly what
    /// `scripts/deploy-mainnet.sh` does. Runs `pre_upgrade` then `post_upgrade`.
    /// Install `wasm` on the table canister with `--mode install`, replacing what is
    /// there, and stop tracking the module hash as "the module under test".
    ///
    /// Only for cross-version upgrade tests: install an OLD release, create state
    /// with it, then call [`World::upgrade`] to move to the module under test. See
    /// docs/DEFECTS.md H-16.
    pub fn reinstall_module(&mut self, wasm: Vec<u8>) -> Result<(), String> {
        let arg = encode_one(&self.config).expect("install arg encode");
        self.pic
            .reinstall_canister(self.table, wasm.clone(), arg, Some(self.controller))
            .map_err(|r| format!("{r:?}"))?;
        self.table_wasm = wasm;
        Ok(())
    }

    /// Upgrade to the module under test, whatever is currently installed.
    ///
    /// [`World::upgrade`] asserts the post-upgrade module hash equals the module the
    /// harness built, which is right for new-to-new but is exactly what makes it
    /// unable to test an upgrade FROM an older module. This one upgrades to the
    /// module under test and returns the raw outcome, so a caller can assert on a
    /// REJECTED upgrade as well as on a successful one.
    pub fn upgrade_to_module_under_test(&mut self) -> Result<(), String> {
        let arg = encode_one(&self.config).expect("upgrade arg encode");
        let wasm = wasms::table_canister_wasm();
        let out = self
            .pic
            .upgrade_canister(self.table, wasm.clone(), arg, Some(self.controller))
            .map_err(|r| format!("{r:?}"));
        if out.is_ok() {
            self.table_wasm = wasm;
            self.upgrades += 1;
            assert_installed_module_is(
                &self.pic,
                self.table,
                self.controller,
                wasms::table_canister_sha256(),
                "cross-version upgrade",
            );
        }
        out
    }

    pub fn upgrade(&mut self) -> Result<(), String> {
        let arg = encode_one(&self.config).expect("upgrade arg encode");
        let wasm = self.table_wasm.clone();
        let out = self
            .pic
            .upgrade_canister(self.table, wasm, arg, Some(self.controller))
            .map_err(|r| format!("{r:?}"));
        if out.is_ok() {
            self.upgrades += 1;
            assert_installed_module_is(
                &self.pic,
                self.table,
                self.controller,
                wasms::table_canister_sha256(),
                "upgrade",
            );
        }
        out
    }

    /// Mark a raw deposit as credited. Called after a successful
    /// `notify_deposit`, which is the only method that turns a raw transfer into
    /// escrow.
    pub fn note_raw_deposit_credited(&mut self, amount: u64) {
        self.uncredited_raw_deposits = self.uncredited_raw_deposits.saturating_sub(amount);
    }

    /// Canister log lines the harness has not seen yet.
    ///
    /// The engine detects some of its own accounting inconsistencies and then
    /// prints them and pays out anyway -- `calculate_side_pots` logs
    /// `BUG: Side pots (...) exceed total pot (...)` and carries on, and
    /// `pre_upgrade` logs `CRITICAL: Failed to save state`. A fund canister
    /// telling you it is inconsistent is evidence, so the harness reads it.
    pub fn new_canister_logs(&mut self) -> Vec<String> {
        let records = match self.pic.fetch_canister_logs(self.table, self.controller) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        let mut out = Vec::new();
        for record in records {
            if record.idx < self.next_log_idx {
                continue;
            }
            self.next_log_idx = record.idx + 1;
            out.push(String::from_utf8_lossy(&record.content).to_string());
        }
        out
    }
}
