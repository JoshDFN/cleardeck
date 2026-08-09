//! THE FRONTEND'S CANDID BINDINGS, GENERATED RATHER THAN MAINTAINED.
//!
//! docs/DEFECTS.md D-11. `src/declarations/<n>/<n>.did.js` is what
//! `src/cleardeck_frontend/src/lib/canisters.js` builds every actor from, and it
//! is a THIRD copy of each interface: the Rust code, the committed `.did`, and
//! this. It drifted from both. Measured on 2026-08-09, before this tool existed:
//!
//! ```text
//!   table_1  canister .did 75 methods, .did.js 61
//!            MISSING 16, including claim_external_deposit, get_solvency,
//!            get_all_ledger_intents, get_deposit_replay_state, refresh_solvency
//!            EXTRA 2 (admin_restore_balance, deposit_from_external) -- methods
//!            the canister does not have, so a client that calls them fails
//!   history  18 vs 12, MISSING 6
//!   lobby    26 vs 24, MISSING 2
//! ```
//!
//! Nobody noticed because the frontend guards its calls (`if
//! (!tableActor?.get_custody_status)`), so a missing binding reads as a missing
//! FEATURE rather than as a broken build. The custody and solvency instruments
//! built in waves 7-10 were simply unreachable from the UI.
//!
//! This is `didc bind -t js|ts` with the same library `didc` itself uses, so the
//! output format is the one dfx and icp-cli produce and the files stay diffable.
//!
//! Usage:
//!   gen-declarations <input.did> <out.did.js> <out.did.d.ts>
//!
//! It reads one file and writes two. No network, no build, no canister call.

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: gen-declarations <input.did> <out.did.js> <out.did.d.ts>");
        std::process::exit(2);
    }
    let input = PathBuf::from(&args[1]);
    let (env, actor) = candid_parser::pretty_check_file(&input).unwrap_or_else(|e| {
        eprintln!("could not parse {}: {e}", input.display());
        std::process::exit(1);
    });
    // Trailing newline: the committed files have one and a generator that omits
    // it would show every file as changed forever, which is how a diff gate gets
    // switched off.
    let js = format!("{}\n", candid_parser::bindings::javascript::compile(&env, &actor).trim_end());
    let ts = format!("{}\n", candid_parser::bindings::typescript::compile(&env, &actor).trim_end());
    std::fs::write(&args[2], js).expect("write .did.js");
    std::fs::write(&args[3], ts).expect("write .did.d.ts");
    println!("  generated {} and {}", args[2], args[3]);
}
