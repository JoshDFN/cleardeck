//! The harness world: one PocketIC instance holding the REAL mainnet ICP ledger
//! at `ryjl3-tyaaa-aaaaa-aaaba-cai` and the REAL table canister wasm built from
//! this tree, plus the typed calls the settlement harness drives it with.
//!
//! Nothing here is a mock. Escrow is funded through the production ICRC-2
//! approve + `deposit` path against a ledger that charges the real 10_000 e8s fee,
//! so every chip the oracle audits traces back to real money.
//!
//! Three things this adds over a plain driver, all in service of the oracle:
//!
//! * [`World::values`] -- what the canister owes each principal, counted as
//!   `escrow + seated chips`. Chip deltas measured this way survive a player
//!   vacating their seat mid-hand (their stack moves to escrow rather than
//!   vanishing), which is precisely the E-05 scenario the oracle has to measure.
//! * [`World::take_snapshot`] / [`World::load_snapshot`] -- exact restore of the
//!   pre-deal table, so a hand that did not deal the cards the harness wanted can
//!   be rewound and re-dealt with the stacks untouched. This is what makes
//!   "reach an exact three-way tie" a deliberate act instead of a hope.
//! * a module-hash assertion after install: the replica must report the sha256 of
//!   the bytes [`crate::wasms`] just built. See trap 1 in `src/wasms.rs`.

use candid::{decode_one, encode_args, encode_one, CandidType, Decode, Encode, Nat, Principal};
use pocket_ic::{PocketIc, PocketIcBuilder, RejectResponse, Time};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::time::Duration;

use crate::ledger::{self, Account, ApproveArgs, LedgerResult, TransferArg};
use crate::table_api::*;
use crate::wasms;

/// 2026-01-01T00:00:00Z. A realistic wall clock matters: the canister does
/// `now - last_time` and `now + timeout` in several places.
pub const GENESIS_NANOS: u64 = 1_767_225_600_000_000_000;

/// Starting ICP in every actor's ledger wallet: 10_000 ICP.
pub const ACTOR_START_E8S: u64 = 1_000_000_000_000;

/// `AUTO_DEAL_DELAY_NS` in the canister, as a Duration. A hand cannot start until
/// this has passed since the last one finished.
pub const AUTO_DEAL_DELAY: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpError {
    /// The IC rejected the message: trap, out of cycles, no such method.
    Trap(String),
    /// The method returned `Err(text)`.
    Err(String),
}

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpError::Trap(s) => write!(f, "trap: {s}"),
            OpError::Err(s) => write!(f, "Err: {s}"),
        }
    }
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

pub struct World {
    pub pic: PocketIc,
    pub ledger: Principal,
    pub table: Principal,
    pub controller: Principal,
    pub minter: Principal,
    pub actors: Vec<Actor>,
    pub config: TableConfig,
    pub table_wasm_sha256: String,
    next_log_idx: u64,
}

impl World {
    pub fn new(config: TableConfig, actor_names: &[&str]) -> Self {
        let module = wasms::table_canister_module();

        let mut builder = PocketIcBuilder::new()
            .with_nns_subnet() // ryjl3-... lives in the NNS canister range
            .with_application_subnet();
        if let Some(bin) = wasms::pocket_ic_binary() {
            builder = builder.with_server_binary(bin);
        }
        let pic = builder.build();
        pic.set_time(Time::from_nanos_since_unix_epoch(GENESIS_NANOS));

        let controller = Principal::self_authenticating(b"cleardeck-settlement-controller");
        let minter = Principal::self_authenticating(b"cleardeck-settlement-minter");
        let actors: Vec<Actor> = actor_names
            .iter()
            .map(|n| Actor {
                name: (*n).to_string(),
                principal: Principal::self_authenticating(
                    format!("cleardeck-settlement-actor-{n}").as_bytes(),
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
        let table = pic.create_canister_with_settings(Some(controller), None);
        pic.add_cycles(table, 100_000_000_000_000);
        pic.install_canister(
            table,
            module.bytes.clone(),
            encode_one(&config).expect("table init arg encode"),
            Some(controller),
        );

        // The module the replica is running MUST be the module we just built.
        let status = pic
            .canister_status(table, Some(controller))
            .expect("canister_status must succeed for the controller");
        let installed = status
            .module_hash
            .map(hex::encode)
            .unwrap_or_else(|| "<empty canister>".to_string());
        assert_eq!(
            installed, module.sha256,
            "the replica reports module hash {installed} for the table canister but the \
             harness built {}. Every result below would be attributed to the wrong code.",
            module.sha256
        );

        Self {
            pic,
            ledger: ledger_id,
            table,
            controller,
            minter,
            actors,
            config,
            table_wasm_sha256: module.sha256.clone(),
            next_log_idx: 0,
        }
    }

    /// Five actors on a micro-stakes six-max table: enough seats for a four-way
    /// all-in ladder plus a folder with dead money above the short stack.
    pub fn micro() -> Self {
        Self::new(
            TableConfig::micro_six_max(),
            &["alice", "bob", "carol", "dave", "erin"],
        )
    }

    pub fn actor(&self, name: &str) -> Principal {
        self.actors
            .iter()
            .find(|a| a.name == name)
            .unwrap_or_else(|| panic!("no actor named {name}"))
            .principal
    }

    pub fn actor_name(&self, who: Principal) -> String {
        self.actors
            .iter()
            .find(|a| a.principal == who)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| who.to_text())
    }

    pub fn advance(&self, d: Duration) {
        self.pic.advance_time(d);
        self.pic.tick();
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

    #[allow(dead_code)]
    pub fn transfer_to_deposit_subaccount(
        &self,
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
        Decode!(&bytes, LedgerResult)
            .map_err(|e| format!("transfer reply decode: {e}"))?
            .block()
    }

    // -----------------------------------------------------------------------
    // table canister: money
    // -----------------------------------------------------------------------

    pub fn deposit(&self, who: Principal, amount: u64) -> Outcome<u64> {
        self.update_result(who, "deposit", encode_one(amount).unwrap())
    }

    /// Approve then deposit, exactly as the frontend does. The allowance covers
    /// the amount plus the fee `transfer_from` will charge.
    pub fn fund_escrow(&self, who: Principal, amount: u64) -> Outcome<u64> {
        self.approve(who, amount.saturating_add(ledger::TRANSFER_FEE))
            .map_err(OpError::Err)?;
        self.deposit(who, amount)
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

    /// Take `seat` with EXACTLY `amount` chips out of escrow. This is how the
    /// harness sets deliberate all-in depths.
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

    /// What the canister owes each principal right now: escrow plus the chips in
    /// front of them if they are seated.
    ///
    /// Measuring a hand's effect on this, rather than on `Player::chips` alone, is
    /// what makes a mid-hand `leave_table` measurable: that call moves the stack
    /// from the seat into escrow, which leaves `chips` misleading and this total
    /// correct.
    pub fn values(&self) -> BTreeMap<Principal, u64> {
        let (_, escrow) = self.escrow_all();
        let mut out: BTreeMap<Principal, u64> = BTreeMap::new();
        for a in &self.actors {
            out.insert(a.principal, escrow.get(&a.principal).copied().unwrap_or(0));
        }
        for (who, amount) in escrow {
            out.entry(who).or_insert(amount);
        }
        for p in self.table_state().seated() {
            *out.entry(p.principal).or_insert(0) = out
                .get(&p.principal)
                .copied()
                .unwrap_or(0)
                .saturating_add(p.chips);
        }
        out
    }

    /// `escrow + chips + pot`: everything the canister accounts for internally.
    /// Side pots are a breakdown of `pot`, so adding them would double count.
    pub fn internal_total(&self) -> u64 {
        let (escrow_total, _) = self.escrow_all();
        escrow_total
            .saturating_add(self.chips_total())
            .saturating_add(self.table_state().pot)
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

    /// Canister log lines not yet read. The engine prints its own accounting
    /// complaints (`BUG: Side pots (...) exceed total pot (...)`) and then settles
    /// anyway, so the log is evidence.
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

    // -----------------------------------------------------------------------
    // snapshots: rewind and re-deal
    // -----------------------------------------------------------------------

    /// Snapshot the table canister so a deal can be rewound.
    ///
    /// PocketIC requires a stopped canister for a consistent snapshot, so this
    /// stops and restarts it. Callers should only do this between hands.
    pub fn take_snapshot(&self, replace: Option<Vec<u8>>) -> Vec<u8> {
        self.pic
            .stop_canister(self.table, Some(self.controller))
            .expect("stop_canister before snapshot");
        let snap = self
            .pic
            .take_canister_snapshot(self.table, Some(self.controller), replace)
            .expect("take_canister_snapshot");
        self.pic
            .start_canister(self.table, Some(self.controller))
            .expect("start_canister after snapshot");
        snap.id
    }

    /// Restore a snapshot taken by [`World::take_snapshot`].
    ///
    /// The subnet's randomness is NOT part of canister state, so the next
    /// `start_new_hand` after a restore shuffles a different deck. That is what
    /// makes rewind-and-re-deal a search rather than a loop.
    pub fn load_snapshot(&self, snapshot_id: &[u8]) {
        self.pic
            .stop_canister(self.table, Some(self.controller))
            .expect("stop_canister before load");
        self.pic
            .load_canister_snapshot(self.table, Some(self.controller), snapshot_id.to_vec())
            .expect("load_canister_snapshot");
        self.pic
            .start_canister(self.table, Some(self.controller))
            .expect("start_canister after load");
    }
}
