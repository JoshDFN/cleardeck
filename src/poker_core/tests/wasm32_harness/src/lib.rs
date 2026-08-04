//! Replays the ClearDeck golden vectors INSIDE a `wasm32-unknown-unknown` module.
//!
//! `usize` is 32 bits here and 64 bits on the host. That one difference is what
//! `docs/FINDING-02-shuffle-not-verifiable.md` is about: the shuffle the canister
//! performed was not the shuffle any native test or reimplementation computed, and
//! a native-only golden test could never see it. So the vectors are replayed on
//! BOTH targets, and this module is the one that speaks for the canister.
//!
//! # ABI
//!
//! Deliberately primitive, so the driver needs nothing but `WebAssembly` and a
//! `DataView`. All functions write their output into a single report buffer that
//! the caller reads out of linear memory as UTF-8:
//!
//! | export | effect | returns |
//! |---|---|---|
//! | `run_golden()` | replay every committed vector | number of DISAGREEING vectors |
//! | `regenerate_shuffle_vectors()` | recompute the `S` family from its seeds | number of lines emitted |
//! | `deck_order()` | `create_deck()` encoded | symbol count (52) |
//! | `shuffle_seed(len)` | shuffle with the `len` bytes at `input_ptr()` | symbol count (52) |
//! | `report_ptr()` / `report_len()` | where the last output landed | pointer / byte length |
//! | `input_ptr()` / `input_cap()` | where to write a seed | pointer / capacity |
//!
//! The report is only valid until the next call.

#[path = "../../common/mod.rs"]
mod common;

use common::golden;

/// Output buffer. Owned by a `static mut` rather than returned by value because
/// the C ABI cannot return a `String` and this harness has exactly one caller.
static mut REPORT: String = String::new();

/// Input buffer for a raw seed. 4 KiB is far more than the 32-byte IC seed.
const INPUT_CAP: usize = 4096;
static mut INPUT: [u8; INPUT_CAP] = [0u8; INPUT_CAP];

fn set_report(s: String) -> u32 {
    let len = s.len();
    unsafe {
        REPORT = s;
    }
    len as u32
}

#[no_mangle]
pub extern "C" fn report_ptr() -> u32 {
    // Explicit `&*p` rather than an implicit autoref through a raw pointer, which
    // rustc rejects as dangerous.
    let p: *const String = &raw const REPORT;
    unsafe { str::as_ptr(&*p) as u32 }
}

#[no_mangle]
pub extern "C" fn report_len() -> u32 {
    let p: *const String = &raw const REPORT;
    unsafe { str::len(&*p) as u32 }
}

#[no_mangle]
pub extern "C" fn input_ptr() -> u32 {
    (&raw const INPUT) as *const u8 as u32
}

#[no_mangle]
pub extern "C" fn input_cap() -> u32 {
    INPUT_CAP as u32
}

/// Replays every committed vector. The report is the rendered outcome: a `counts:`
/// line followed by one block per disagreement.
#[no_mangle]
pub extern "C" fn run_golden() -> u32 {
    let outcome = golden::run_all(golden::GOLDEN);
    let failures = outcome.failures.len() as u32;
    set_report(outcome.render());
    failures
}

/// Recomputes the `S` family from the seeds already committed in the fixture and
/// emits replacement lines. This is how `golden_vectors.txt` gets its shuffle
/// vectors: computed by the target that actually runs the shuffle.
#[no_mangle]
pub extern "C" fn regenerate_shuffle_vectors() -> u32 {
    let lines = golden::regenerate_shuffle_lines(golden::GOLDEN);
    let count = lines.lines().count() as u32;
    set_report(lines);
    count
}

/// `create_deck()` in symbol encoding, so the driver can pin the pre-shuffle order
/// on this target too.
#[no_mangle]
pub extern "C" fn deck_order() -> u32 {
    set_report(golden::deck_order_symbols())
}

/// Shuffles a fresh deck with the first `len` bytes of the input buffer and
/// reports the 52 card symbols. Used to reproduce a REAL canister deal from its
/// revealed seed.
#[no_mangle]
pub extern "C" fn shuffle_seed(len: u32) -> u32 {
    let len = (len as usize).min(INPUT_CAP);
    let base = (&raw const INPUT) as *const u8;
    let seed: Vec<u8> = unsafe { core::slice::from_raw_parts(base, len) }.to_vec();
    let mut deck = poker_core::create_deck();
    poker_core::shuffle_deck(&mut deck, &seed);
    set_report(golden::encode_cards(&deck))
}

/// Proves the pointer width inside this module, so a driver can never be fooled
/// into thinking it tested wasm32 when it actually ran a host build.
#[no_mangle]
pub extern "C" fn pointer_width_bits() -> u32 {
    (core::mem::size_of::<usize>() * 8) as u32
}
