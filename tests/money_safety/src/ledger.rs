//! Candid surface of the REAL ICP ledger, plus the account-identifier maths.
//!
//! Only the parts the harness needs. The shapes are taken from
//! `rs/ledger_suite/icp/ledger.did` at the pinned commit, NOT from any
//! hand-maintained copy in this repo.

use candid::{CandidType, Decode, Encode, Nat, Principal};
use serde::Deserialize;
use sha2::{Digest, Sha224};

/// The canister id the table canister hardcodes as `ICP_LEDGER_CANISTER`.
/// The harness installs the real ledger at exactly this id so the constant
/// resolves with zero code changes.
pub const ICP_LEDGER_ID: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";

/// Must equal `table_canister::ICP_TRANSFER_FEE`.
pub const TRANSFER_FEE: u64 = 10_000;

pub fn ledger_principal() -> Principal {
    Principal::from_text(ICP_LEDGER_ID).expect("ICP ledger id constant is malformed")
}

// ---------------------------------------------------------------------------
// init payload
// ---------------------------------------------------------------------------

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Tokens {
    pub e8s: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct LedgerDuration {
    pub secs: u64,
    pub nanos: u32,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ArchiveOptions {
    pub trigger_threshold: u64,
    pub num_blocks_to_archive: u64,
    pub node_max_memory_size_bytes: Option<u64>,
    pub max_message_size_bytes: Option<u64>,
    pub controller_id: Principal,
    pub more_controller_ids: Option<Vec<Principal>>,
    pub cycles_for_archive_creation: Option<u64>,
    pub max_transactions_per_response: Option<u64>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct FeatureFlags {
    pub icrc2: bool,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct InitArgs {
    pub minting_account: String,
    pub icrc1_minting_account: Option<Account>,
    pub initial_values: Vec<(String, Tokens)>,
    pub max_message_size_bytes: Option<u64>,
    pub transaction_window: Option<LedgerDuration>,
    pub archive_options: Option<ArchiveOptions>,
    pub send_whitelist: Vec<Principal>,
    pub transfer_fee: Option<Tokens>,
    pub token_symbol: Option<String>,
    pub token_name: Option<String>,
    pub feature_flags: Option<FeatureFlags>,
    pub maximum_number_of_accounts: Option<u64>,
    pub accounts_overflow_trim_quantity: Option<u64>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub enum LedgerCanisterPayload {
    Init(InitArgs),
    Upgrade(Option<()>),
}

/// Build an init payload that funds `initial` and enables ICRC-2 (required by the
/// table canister's `deposit`), with the real 10_000 e8s fee.
pub fn init_payload(
    minter: Principal,
    initial: &[(Principal, u64)],
    archive_controller: Principal,
) -> Vec<u8> {
    let args = InitArgs {
        minting_account: account_identifier_hex(&minter, None),
        icrc1_minting_account: Some(Account {
            owner: minter,
            subaccount: None,
        }),
        initial_values: initial
            .iter()
            .map(|(p, e8s)| (account_identifier_hex(p, None), Tokens { e8s: *e8s }))
            .collect(),
        max_message_size_bytes: Some(2 * 1024 * 1024),
        transaction_window: Some(LedgerDuration {
            secs: 24 * 60 * 60,
            nanos: 0,
        }),
        // No archiving: the harness reads blocks back with `query_blocks`, and an
        // archived block would silently change what `notify_deposit` can see.
        archive_options: Some(ArchiveOptions {
            trigger_threshold: 1_000_000_000,
            num_blocks_to_archive: 1,
            node_max_memory_size_bytes: None,
            max_message_size_bytes: None,
            controller_id: archive_controller,
            more_controller_ids: None,
            cycles_for_archive_creation: Some(0),
            max_transactions_per_response: None,
        }),
        send_whitelist: vec![],
        transfer_fee: Some(Tokens { e8s: TRANSFER_FEE }),
        token_symbol: Some("ICP".to_string()),
        token_name: Some("Internet Computer".to_string()),
        feature_flags: Some(FeatureFlags { icrc2: true }),
        maximum_number_of_accounts: None,
        accounts_overflow_trim_quantity: None,
    };
    Encode!(&LedgerCanisterPayload::Init(args)).expect("ledger init payload encode")
}

// ---------------------------------------------------------------------------
// ICRC-1 / ICRC-2
// ---------------------------------------------------------------------------

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub owner: Principal,
    pub subaccount: Option<Vec<u8>>,
}

#[derive(CandidType, Clone, Debug)]
pub struct TransferArg {
    pub from_subaccount: Option<Vec<u8>>,
    pub to: Account,
    pub amount: Nat,
    pub fee: Option<Nat>,
    pub memo: Option<Vec<u8>>,
    pub created_at_time: Option<u64>,
}

#[derive(CandidType, Clone, Debug)]
pub struct ApproveArgs {
    pub from_subaccount: Option<Vec<u8>>,
    pub spender: Account,
    pub amount: Nat,
    pub expected_allowance: Option<Nat>,
    pub expires_at: Option<u64>,
    pub fee: Option<Nat>,
    pub memo: Option<Vec<u8>>,
    pub created_at_time: Option<u64>,
}

/// Errors are decoded as `candid::Reserved`-free opaque values: the harness only
/// needs Ok/Err, and mirroring every error variant would couple it to ledger
/// versions for no benefit.
#[derive(CandidType, Deserialize, Debug)]
pub enum LedgerResult {
    Ok(Nat),
    Err(candid::Reserved),
}

impl LedgerResult {
    pub fn block(self) -> Result<u64, String> {
        match self {
            LedgerResult::Ok(n) => Ok(nat_to_u64(&n)),
            LedgerResult::Err(_) => Err("ledger call returned Err".to_string()),
        }
    }
}

pub fn nat_to_u64(n: &Nat) -> u64 {
    u64::try_from(n.0.clone()).unwrap_or(u64::MAX)
}

pub fn encode_balance_of(account: &Account) -> Vec<u8> {
    Encode!(account).expect("icrc1_balance_of arg encode")
}

pub fn decode_balance(bytes: &[u8]) -> u64 {
    let n = Decode!(bytes, Nat).expect("icrc1_balance_of reply decode");
    nat_to_u64(&n)
}

// ---------------------------------------------------------------------------
// account identifiers
// ---------------------------------------------------------------------------

/// ICP account identifier: `CRC32(h) || h` where
/// `h = SHA224("\x0Aaccount-id" || principal || subaccount)`.
///
/// Deliberately reimplemented here rather than imported from the canister, so a
/// bug in the canister's copy cannot make the harness agree with it by
/// construction. `notify_deposit`'s sender check depends on this maths.
pub fn account_identifier(principal: &Principal, subaccount: Option<[u8; 32]>) -> [u8; 32] {
    let mut hasher = Sha224::new();
    hasher.update(b"\x0Aaccount-id");
    hasher.update(principal.as_slice());
    hasher.update(subaccount.unwrap_or([0u8; 32]));
    let hash = hasher.finalize();

    let crc = crc32fast::hash(&hash);
    let mut out = [0u8; 32];
    out[0..4].copy_from_slice(&crc.to_be_bytes());
    out[4..32].copy_from_slice(&hash);
    out
}

pub fn account_identifier_hex(principal: &Principal, subaccount: Option<[u8; 32]>) -> String {
    hex::encode(account_identifier(principal, subaccount))
}

/// The table canister's per-player deposit subaccount:
/// `sha256("cleardeck-deposit:" || principal)`.
pub fn deposit_subaccount(principal: &Principal) -> [u8; 32] {
    let mut hasher = sha2::Sha256::new();
    hasher.update(b"cleardeck-deposit:");
    hasher.update(principal.as_slice());
    hasher.finalize().into()
}
