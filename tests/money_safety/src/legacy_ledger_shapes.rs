//! The two `query_blocks` reply shapes that FINDING 06 turns on.
//!
//! `notify_deposit` decodes the ICP ledger's `query_blocks` reply with types it
//! declares inline. One field type in that declaration does not match the real
//! ledger, and the mismatch is silent. Keeping both shapes here, side by side,
//! lets a reader diff them and lets the regression test decode the SAME reply
//! bytes twice to isolate the defect to exactly one field.
//!
//! `as_declared_by_the_canister` is copied verbatim out of
//! `src/table_canister/src/lib.rs`. Do not "tidy" it: its whole value is being
//! byte-for-byte what the canister asks for.
//!
//! `as_the_real_ledger_returns_it` is the same set of shapes with the one field
//! type corrected against `rs/ledger_suite/icp/ledger.did`:
//! `type AccountIdentifier = blob;`.

use candid::CandidType;
use serde::Deserialize;

// The canister's OWN declared shapes, copied verbatim out of
// `notify_deposit` in src/table_canister/src/lib.rs.
#[allow(dead_code)] // every field is load-bearing evidence, not all are read
pub mod as_declared_by_the_canister {
    use super::*;

    #[derive(CandidType, Deserialize, Debug)]
    pub struct GetBlocksArgs {
        pub start: u64,
        pub length: u64,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Tokens {
        pub e8s: u64,
    }

    /// THE DEFECT. The real ledger's `AccountIdentifier` is `blob`; the canister
    /// declares it as a record with a `hash` field.
    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct AccountIdentifier {
        pub hash: Vec<u8>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct TimeStamp {
        pub timestamp_nanos: u64,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Transfer {
        pub from: AccountIdentifier,
        pub to: AccountIdentifier,
        pub amount: Tokens,
        pub fee: Tokens,
        pub spender: Option<Vec<u8>>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Mint {
        pub to: AccountIdentifier,
        pub amount: Tokens,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Burn {
        pub from: AccountIdentifier,
        pub spender: Option<AccountIdentifier>,
        pub amount: Tokens,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Approve {
        pub from: AccountIdentifier,
        pub spender: AccountIdentifier,
        pub allowance_e8s: i128,
        pub allowance: Tokens,
        pub fee: Tokens,
        pub expires_at: Option<TimeStamp>,
        pub expected_allowance: Option<Tokens>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub enum Operation {
        Transfer(Transfer),
        Mint(Mint),
        Burn(Burn),
        Approve(Approve),
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Transaction {
        pub memo: u64,
        pub icrc1_memo: Option<Vec<u8>>,
        pub operation: Option<Operation>,
        pub created_at_time: TimeStamp,
    }

    #[derive(CandidType, Deserialize, Debug)]
    pub struct Block {
        pub parent_hash: Option<Vec<u8>>,
        pub transaction: Transaction,
        pub timestamp: TimeStamp,
    }

    #[derive(CandidType, Deserialize, Debug)]
    pub struct QueryBlocksResponse {
        pub chain_length: u64,
        pub certificate: Option<Vec<u8>>,
        pub blocks: Vec<Block>,
        pub first_block_index: u64,
        pub archived_blocks: candid::Reserved,
    }
}

// The same shapes with the ONE field type corrected: `AccountIdentifier = blob`.
#[allow(dead_code)] // every field is load-bearing evidence, not all are read
pub mod as_the_real_ledger_returns_it {
    use super::*;

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Tokens {
        pub e8s: u64,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct TimeStamp {
        pub timestamp_nanos: u64,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Transfer {
        pub from: Vec<u8>,
        pub to: Vec<u8>,
        pub amount: Tokens,
        pub fee: Tokens,
        pub spender: Option<Vec<u8>>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Mint {
        pub to: Vec<u8>,
        pub amount: Tokens,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Burn {
        pub from: Vec<u8>,
        pub spender: Option<Vec<u8>>,
        pub amount: Tokens,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Approve {
        pub from: Vec<u8>,
        pub spender: Vec<u8>,
        pub allowance_e8s: candid::Int,
        pub allowance: Tokens,
        pub fee: Tokens,
        pub expires_at: Option<TimeStamp>,
        pub expected_allowance: Option<Tokens>,
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub enum Operation {
        Transfer(Transfer),
        Mint(Mint),
        Burn(Burn),
        Approve(Approve),
    }

    #[derive(CandidType, Deserialize, Debug, Clone)]
    pub struct Transaction {
        pub memo: u64,
        pub icrc1_memo: Option<Vec<u8>>,
        pub operation: Option<Operation>,
        pub created_at_time: TimeStamp,
    }

    #[derive(CandidType, Deserialize, Debug)]
    pub struct Block {
        pub parent_hash: Option<Vec<u8>>,
        pub transaction: Transaction,
        pub timestamp: TimeStamp,
    }

    #[derive(CandidType, Deserialize, Debug)]
    pub struct QueryBlocksResponse {
        pub chain_length: u64,
        pub certificate: Option<Vec<u8>>,
        pub blocks: Vec<Block>,
        pub first_block_index: u64,
        pub archived_blocks: candid::Reserved,
    }
}

