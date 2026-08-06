//! THE UI MAY NOT STATE A LIMIT THE CANISTER DOES NOT ENFORCE.
//!
//! docs/DEFECTS.md T-26. `WithdrawModal.svelte` enforced a BTC withdrawal floor of
//! **11 sats** while the hint, the input's `min` attribute and the error string all
//! said **1,000** -- 90.9x the real one. The consequences were not cosmetic:
//!
//! * `Minimum withdrawal is 1,000 sats` was UNREACHABLE for every amount from 12 to
//!   999 sats, because the guard that printed it compared against 11.
//! * a player holding fewer than 1,000 sats was told, by the only surface they can
//!   read, that their remaining balance could not be withdrawn. It could.
//!
//! Nothing in the repository could see it. `make hygiene` greps the source for
//! protected phrases; the money-figure census in the screenshot corpus walks TEXT
//! NODES and so cannot read an input's `min` attribute at all; and no Rust test had
//! ever opened a `.svelte` file. The gap was not "somebody forgot to run the gate",
//! it was "there was no gate".
//!
//! This is that gate, and it is deliberately in the money-safety harness rather
//! than in the frontend: the numbers it protects are the canister's, the file it
//! reads them from is `src/table_canister/src/lib.rs`, and this crate is the one
//! that already treats that file as a source of truth
//! (`register_entries_are_all_still_needed` reads `docs/DEFECTS.md` the same way).
//!
//! It makes two separate claims, because either alone is escapable:
//!
//! 1. **The mirrors match.** Each modal declares its limits once, inside a
//!    `MIRRORED-LIMITS-BEGIN` / `END` fence, and every one of those numbers must
//!    equal the constant `lib.rs` enforces. This catches a canister constant being
//!    changed without the UI.
//! 2. **No surface states a limit as a literal.** Outside the fence, any line that
//!    talks about a minimum, a maximum, a fee or a cooldown must carry its figures
//!    in `{...}` interpolation, never as digits. This is the claim that catches
//!    T-26 itself: the mirror was already correct in that file, and the copy next
//!    to it was wrong. Verified against `git show HEAD` before the fix: it flags
//!    `Minimum: 1,000 sats (Fee: 10 sats)` and
//!    `Minimum withdrawal: 0.001 ICP` in the withdraw modal and five more in the
//!    deposit modal.
//!
//! Claim 2 is a source-level check and the wave-4 lesson says source-level checks
//! do not discharge a claim about the rendered page. It is not trying to: it is not
//! a claim that a player SEES the right number, it is a claim that the number a
//! player sees CANNOT be a different number from the one the canister applies,
//! because there is only one number in the file. Visibility is a separate gate.
//!
//! This binary touches no replica and builds no wasm: it is two file reads.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    money_safety::wasms::repo_root()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn canister_source() -> String {
    read(&repo_root().join("src/table_canister/src/lib.rs"))
}

fn modal_source(name: &str) -> String {
    read(&repo_root()
        .join("src/cleardeck_frontend/src/lib/components")
        .join(name))
}

// ---------------------------------------------------------------------------
// tiny parsers -- this crate has no regex dependency and does not need one
// ---------------------------------------------------------------------------

/// Digits with `_` separators, e.g. `10_000` -> 10000. Stops at the first
/// character that is neither a digit nor `_`, so a trailing `n` (a JS BigInt
/// literal) or `;` is fine.
fn parse_number(text: &str) -> Option<u128> {
    let mut digits = String::new();
    for c in text.trim_start().chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if c == '_' {
            continue;
        } else {
            break;
        }
    }
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// `const NAME: u64 = 10_000;` in the canister source.
fn rust_const(source: &str, name: &str) -> u128 {
    let needle = format!("const {name}:");
    for line in source.lines() {
        let line = line.trim_start();
        if !line.starts_with(&needle) {
            continue;
        }
        let rhs = line.split('=').nth(1).unwrap_or_else(|| {
            panic!("`{name}` in src/table_canister/src/lib.rs has no `=`: {line}")
        });
        return parse_number(rhs).unwrap_or_else(|| {
            panic!("cannot read a number out of `{name}`: {line}")
        });
    }
    panic!(
        "src/table_canister/src/lib.rs no longer declares `const {name}`. Either it was renamed \
         -- in which case this gate and the two modals must be updated together -- or the limit \
         was deleted and the UI is now stating a limit nothing enforces."
    );
}

/// The `{ btc }` / `{ icp }` pair out of a `if currency == Currency::BTC {A} else {B}`
/// line. `deposit()`'s floor is a local rather than a named constant, so it has to
/// be read where it lives.
fn rust_currency_pair(source: &str, marker: &str) -> (u128, u128) {
    let line = source
        .lines()
        .find(|l| l.contains(marker))
        .unwrap_or_else(|| {
            panic!(
                "src/table_canister/src/lib.rs no longer contains {marker:?}. The deposit floor \
                 moved; this gate and DepositModal.svelte must follow it."
            )
        });
    let mut groups = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let close = match after.find('}') {
            Some(c) => c,
            None => break,
        };
        if let Some(n) = parse_number(&after[..close]) {
            groups.push(n);
        }
        rest = &after[close + 1..];
    }
    assert_eq!(
        groups.len(),
        2,
        "expected exactly two numeric branches on {line:?}, found {groups:?}"
    );
    (groups[0], groups[1])
}

/// The fenced mirror region of a modal, as `(begin_line, end_line, text)`.
///
/// The fence is bounded on purpose: without a ceiling, moving the END marker to the
/// bottom of the file would exempt the entire template and this gate would pass on
/// anything.
const MAX_FENCE_LINES: usize = 160;

fn mirror_fence(source: &str, name: &str) -> String {
    let begins: Vec<usize> = source
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("MIRRORED-LIMITS-BEGIN"))
        .map(|(i, _)| i)
        .collect();
    let ends: Vec<usize> = source
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("MIRRORED-LIMITS-END"))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        begins.len(),
        1,
        "{name} must carry exactly ONE `MIRRORED-LIMITS-BEGIN` marker, found {}. The fence is \
         what tells this gate which numbers are the mirrors and which are copy.",
        begins.len()
    );
    assert_eq!(
        ends.len(),
        1,
        "{name} must carry exactly ONE `MIRRORED-LIMITS-END` marker, found {}",
        ends.len()
    );
    let (begin, end) = (begins[0], ends[0]);
    assert!(
        end > begin,
        "{name}: the MIRRORED-LIMITS fence closes before it opens"
    );
    assert!(
        end - begin <= MAX_FENCE_LINES,
        "{name}: the MIRRORED-LIMITS fence spans {} lines, more than the {MAX_FENCE_LINES} \
         allowed. A fence that large can hide the template inside the exempt region, which \
         would make the literal check below vacuous.",
        end - begin
    );
    source
        .lines()
        .skip(begin)
        .take(end - begin + 1)
        .collect::<Vec<_>>()
        .join("\n")
}

/// `const NAME = isBTC ? 11n : 100_000n;` -> `(11, 100000)`.
fn js_currency_const(fence: &str, name: &str) -> (u128, u128) {
    let needle = format!("const {name} = isBTC ?");
    let line = fence
        .lines()
        .find(|l| l.trim_start().starts_with(&needle))
        .unwrap_or_else(|| {
            panic!(
                "the mirror fence no longer declares `{needle} ...`. Every limit the modal states \
                 must come from a constant in the fence, so this gate can compare it against the \
                 canister."
            )
        });
    let rhs = line.split('?').nth(1).expect("a ternary has a `?`");
    let mut halves = rhs.split(':');
    let btc = parse_number(halves.next().expect("btc branch"))
        .unwrap_or_else(|| panic!("no number in the BTC branch of {line:?}"));
    let icp = parse_number(halves.next().expect("icp branch"))
        .unwrap_or_else(|| panic!("no number in the ICP branch of {line:?}"));
    (btc, icp)
}

/// `const NAME = 60;` -> 60.
fn js_scalar_const(fence: &str, name: &str) -> u128 {
    let needle = format!("const {name} = ");
    let line = fence
        .lines()
        .find(|l| l.trim_start().starts_with(&needle))
        .unwrap_or_else(|| panic!("the mirror fence no longer declares `{needle}...`"));
    let rhs = line.split('=').nth(1).expect("an assignment has an `=`");
    parse_number(rhs).unwrap_or_else(|| panic!("no number in {line:?}"))
}

// ---------------------------------------------------------------------------
// 1. the mirrors match the canister
// ---------------------------------------------------------------------------

#[test]
fn the_withdraw_modal_mirrors_the_canisters_withdrawal_limits() {
    let engine = canister_source();
    let fence = mirror_fence(&modal_source("WithdrawModal.svelte"), "WithdrawModal.svelte");

    let cases: [(&str, &str, &str); 3] = [
        ("MIN_WITHDRAWAL", "BTC_MIN_WITHDRAWAL_AMOUNT", "ICP_MIN_WITHDRAWAL_AMOUNT"),
        ("MAX_WITHDRAWAL", "BTC_MAX_WITHDRAWAL_PER_TX", "ICP_MAX_WITHDRAWAL_PER_TX"),
        ("TRANSFER_FEE", "CKBTC_TRANSFER_FEE", "ICP_TRANSFER_FEE"),
    ];
    for (js, btc_const, icp_const) in cases {
        let (ui_btc, ui_icp) = js_currency_const(&fence, js);
        let want_btc = rust_const(&engine, btc_const);
        let want_icp = rust_const(&engine, icp_const);
        assert_eq!(
            ui_btc, want_btc,
            "WithdrawModal.svelte states a BTC {js} of {ui_btc} while the canister enforces \
             {btc_const} = {want_btc}. THIS IS docs/DEFECTS.md T-26: the modal used to say 1,000 \
             sats and the canister used to accept 11, which made the modal's own error string \
             unreachable for every amount from 12 to 999 sats. Whichever number is right, both \
             files have to say it."
        );
        assert_eq!(
            ui_icp, want_icp,
            "WithdrawModal.svelte states an ICP {js} of {ui_icp} while the canister enforces \
             {icp_const} = {want_icp}"
        );
    }

    // The cooldown is stated in seconds by the UI and in nanoseconds by the
    // canister, so the comparison has to cross the unit -- which is exactly the kind
    // of place a number goes wrong unnoticed.
    let ui_secs = js_scalar_const(&fence, "WITHDRAWAL_COOLDOWN_SECS");
    let canister_ns = rust_const(&engine, "WITHDRAWAL_COOLDOWN_NS");
    assert_eq!(
        ui_secs * 1_000_000_000,
        canister_ns,
        "WithdrawModal.svelte tells the player one withdrawal every {ui_secs} seconds while \
         WITHDRAWAL_COOLDOWN_NS is {canister_ns} ns ({} s)",
        canister_ns / 1_000_000_000
    );
}

#[test]
fn the_deposit_modal_mirrors_the_canisters_deposit_floor() {
    let engine = canister_source();
    let fence = mirror_fence(&modal_source("DepositModal.svelte"), "DepositModal.svelte");

    let (ui_btc, ui_icp) = js_currency_const(&fence, "MIN_DEPOSIT");
    // `deposit()` writes its floor inline rather than as a named constant.
    let (want_btc, want_icp) =
        rust_currency_pair(&engine, "let min_deposit = if currency == Currency::BTC");
    assert_eq!(
        ui_btc, want_btc,
        "DepositModal.svelte states a BTC minimum deposit of {ui_btc} sats while `deposit()` \
         refuses anything below {want_btc}"
    );
    assert_eq!(
        ui_icp, want_icp,
        "DepositModal.svelte states an ICP minimum deposit of {ui_icp} e8s while `deposit()` \
         refuses anything below {want_icp}"
    );

    let (fee_btc, fee_icp) = js_currency_const(&fence, "TRANSFER_FEE");
    assert_eq!(fee_btc, rust_const(&engine, "CKBTC_TRANSFER_FEE"));
    assert_eq!(fee_icp, rust_const(&engine, "ICP_TRANSFER_FEE"));
}

// ---------------------------------------------------------------------------
// 2. no surface states a limit as a literal
// ---------------------------------------------------------------------------

/// Words that mean "what follows is a limit a player will hold us to".
const LIMIT_WORDS: [&str; 7] = ["minimum", "maximum", "cooldown", "min", "max", "fee", "fees"];

/// True when `word` appears in `haystack` as a whole word. `_` counts as a word
/// character, which is what keeps `TRANSFER_FEE`, `transferFee`, `expected_fee` and
/// `BadFee` out: an identifier is not a sentence, and this check is about sentences.
fn contains_word(haystack: &str, word: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = haystack[from..].find(word) {
        let at = from + rel;
        let before_ok = at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
        let after = at + word.len();
        let after_ok =
            after >= bytes.len() || !(bytes[after].is_ascii_alphanumeric() || bytes[after] == b'_');
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

/// Remove every `{...}` / `${...}` group, so what remains is only the text a
/// template renders verbatim. Nesting is handled by depth counting.
fn strip_interpolation(line: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in line.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn offending_literal_lines(source: &str) -> Vec<(usize, String)> {
    // `<style>` cannot state a limit to anybody, and its selectors legitimately
    // carry both the word "minimum" (`.minimum-notice`) and pixel values.
    let body = match source.find("<style>") {
        Some(i) => &source[..i],
        None => source,
    };

    let mut out = Vec::new();
    let mut inside_fence = false;
    for (i, raw) in body.lines().enumerate() {
        if raw.contains("MIRRORED-LIMITS-BEGIN") {
            inside_fence = true;
            continue;
        }
        if raw.contains("MIRRORED-LIMITS-END") {
            inside_fence = false;
            continue;
        }
        if inside_fence {
            continue;
        }
        // `Math.min` / `Math.max` are arithmetic, not a statement to a player. This
        // is the ONLY carve-out, and it is spelled out rather than approximated.
        let lowered = raw.to_lowercase().replace("math.min", "mathfn").replace("math.max", "mathfn");
        if !LIMIT_WORDS.iter().any(|w| contains_word(&lowered, w)) {
            continue;
        }
        if strip_interpolation(raw).chars().any(|c| c.is_ascii_digit()) {
            out.push((i + 1, raw.trim().to_string()));
        }
    }
    out
}

#[test]
fn no_money_modal_states_a_limit_as_a_literal() {
    for name in ["WithdrawModal.svelte", "DepositModal.svelte"] {
        let offenders = offending_literal_lines(&modal_source(name));
        assert!(
            offenders.is_empty(),
            "{name} states {} limit(s) as literal digits instead of interpolating the mirrored \
             constant. That is docs/DEFECTS.md T-26 exactly: a number written twice is a number \
             that will disagree with itself, and the copy is the half that no test reads.\n{}",
            offenders.len(),
            offenders
                .iter()
                .map(|(n, l)| format!("  {name}:{n}  {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// The gate has to be able to fail, and the cheapest proof is to run it on the text
/// it was written for. These are the four lines that were live at `fe72d46`.
#[test]
fn the_literal_check_convicts_the_defect_it_was_written_for() {
    let t26 = "\
<script>
  const minWithdrawal = isBTC ? 11n : 100000n; // 11 sats (fee is 10) vs 0.001 ICP
</script>
      <input min={isBTC ? (inputUnit === 'sats' ? \"1000\" : \"0.00001\") : \"0.001\"} />
      <p class=\"hint\">
          Minimum: 1,000 sats (Fee: 10 sats)
          Minimum withdrawal: 0.001 ICP. A small network fee applies.
      </p>
";
    let offenders = offending_literal_lines(t26);
    let lines: Vec<&str> = offenders.iter().map(|(_, l)| l.as_str()).collect();
    assert!(
        lines.iter().any(|l| l.contains("Minimum: 1,000 sats")),
        "the check must convict the T-26 hint; it found {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("Minimum withdrawal: 0.001 ICP")),
        "the check must convict the T-26 ICP hint; it found {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("const minWithdrawal")),
        "an unfenced mirror is itself a literal: nothing compares it to the canister"
    );

    // ... and it must not convict the fixed text.
    let fixed = "\
      <input min={inputMinAttr} max={inputMaxAttr} />
      <p class=\"hint\">
        Minimum {minDisplay}, maximum {maxDisplay} per transaction. The network fee
        is {feeDisplay} and is taken out of what you withdraw.
      </p>
";
    assert!(
        offending_literal_lines(fixed).is_empty(),
        "interpolated copy must pass: {:?}",
        offending_literal_lines(fixed)
    );
}

/// The mirror comparison has to be able to fail too, and its two halves are read by
/// two different parsers. This drives both on the exact text that was live at
/// `fe72d46` and shows they return numbers that DISAGREE -- which is what makes
/// `assert_eq!` in test 1 a real assertion rather than a tautology.
#[test]
fn the_mirror_comparison_convicts_the_defect_it_was_written_for() {
    // What the modal said, if its mirror had matched its own copy.
    let fenced_wrong = "  // >>> MIRRORED-LIMITS-BEGIN\n  \
                        const MIN_WITHDRAWAL = isBTC ? 1000n : 100_000n;\n  \
                        // <<< MIRRORED-LIMITS-END";
    let (btc, icp) = js_currency_const(fenced_wrong, "MIN_WITHDRAWAL");
    assert_eq!((btc, icp), (1000, 100_000));

    // What the canister enforces.
    let engine = "const BTC_MIN_WITHDRAWAL_AMOUNT: u64 = 11; // Just above 10 sat fee\n\
                  const ICP_MIN_WITHDRAWAL_AMOUNT: u64 = 100_000; // 0.001 ICP minimum\n";
    assert_eq!(rust_const(engine, "BTC_MIN_WITHDRAWAL_AMOUNT"), 11);
    assert_eq!(rust_const(engine, "ICP_MIN_WITHDRAWAL_AMOUNT"), 100_000);

    assert_ne!(
        btc,
        rust_const(engine, "BTC_MIN_WITHDRAWAL_AMOUNT"),
        "1,000 sats stated against 11 sats enforced is a 90.9x disagreement and the comparison \
         in test 1 must see it"
    );
    assert_eq!(
        icp,
        rust_const(engine, "ICP_MIN_WITHDRAWAL_AMOUNT"),
        "the ICP side was already consistent, and a gate that cannot tell the consistent case \
         from the broken one is not measuring anything"
    );

    // The nanosecond crossing, on the real text of both sides.
    assert_eq!(
        js_scalar_const(
            "  // >>> MIRRORED-LIMITS-BEGIN\n  const WITHDRAWAL_COOLDOWN_SECS = 60;\n  // <<< MIRRORED-LIMITS-END",
            "WITHDRAWAL_COOLDOWN_SECS"
        ) * 1_000_000_000,
        rust_const(
            "const WITHDRAWAL_COOLDOWN_NS: u64 = 60_000_000_000; // 60 second cooldown\n",
            "WITHDRAWAL_COOLDOWN_NS"
        )
    );

    // And the deposit floor, which is an inline local rather than a named constant.
    assert_eq!(
        rust_currency_pair(
            "    let min_deposit = if currency == Currency::BTC { 1_000 } else { 20_000 }; // x\n",
            "let min_deposit = if currency == Currency::BTC"
        ),
        (1_000, 20_000)
    );
}

/// A mirror outside the fence is invisible to test 1, so the fence must actually
/// contain the constants test 1 reads. Cheap, but it is the hinge both tests hang
/// on: move `MIN_WITHDRAWAL` out of the fence and test 1 starts panicking with
/// "no longer declares", which is a failure, not a silent pass. This asserts the
/// good state directly so the reason is legible.
#[test]
fn every_mirrored_constant_lives_inside_the_fence() {
    let withdraw = modal_source("WithdrawModal.svelte");
    let fence = mirror_fence(&withdraw, "WithdrawModal.svelte");
    for name in ["MIN_WITHDRAWAL", "MAX_WITHDRAWAL", "TRANSFER_FEE"] {
        assert_eq!(
            withdraw.matches(&format!("const {name} =")).count(),
            1,
            "{name} must be declared exactly once in WithdrawModal.svelte"
        );
        assert!(
            fence.contains(&format!("const {name} =")),
            "{name} is declared outside the MIRRORED-LIMITS fence, where this gate cannot \
             compare it against the canister"
        );
    }

    let deposit = modal_source("DepositModal.svelte");
    let dfence = mirror_fence(&deposit, "DepositModal.svelte");
    for name in ["MIN_DEPOSIT", "TRANSFER_FEE"] {
        assert_eq!(
            deposit.matches(&format!("const {name} =")).count(),
            1,
            "{name} must be declared exactly once in DepositModal.svelte"
        );
        assert!(
            dfence.contains(&format!("const {name} =")),
            "{name} is declared outside the MIRRORED-LIMITS fence"
        );
    }
}

// ===========================================================================
// THE OUTSTANDING-STAKE SURFACE (docs/SECURITY-FINDINGS.md FINDING 18)
// ===========================================================================
//
// Same shape of gate as the mirrored limits above, and it exists for the same
// reason: the canister can be right while the screen a player reads is silent.
// FINDING 18 was exactly that -- money in a pot, `get_balance() -> 0`, and no
// surface between the two. These read the two money screens as TEXT, so they cost
// no replica and no browser, and they fail if the surface is edited away.
//
// They deliberately assert on the CALL and the FIELD rather than on any wording:
// copy is the frontend's to change, and a test that pins prose is a test that gets
// deleted the first time somebody rewrites a sentence.

fn table_component_source(name: &str) -> String {
    read(&repo_root()
        .join("src/cleardeck_frontend/src/lib/components")
        .join(name))
}

#[test]
fn the_withdraw_screen_asks_the_canister_what_else_it_is_holding() {
    let src = modal_source("WithdrawModal.svelte");
    for needle in [
        // The query that answers "what else of mine is in there".
        "get_custody_status",
        // The figure it exists to show.
        "committed_in_pot",
        // The recovery the canister's own advice names, reachable from this screen.
        "abandon_stuck_hand",
    ] {
        assert!(
            src.contains(needle),
            "WithdrawModal.svelte no longer mentions `{needle}`. This is the last screen a \
             leaving player looks at, and docs/SECURITY-FINDINGS.md FINDING 18 is what happens \
             when it shows only get_balance(): an auditor read 0, withdrew \"everything\", and \
             left 2.98 ICP in a pot."
        );
    }
}

#[test]
fn the_table_screen_shows_a_stake_that_is_still_in_the_pot() {
    let src = table_component_source("PokerTable.svelte");
    assert!(
        src.contains("my_committed_in_pot"),
        "PokerTable.svelte no longer reads `my_committed_in_pot` from the table view. That field \
         exists so the call every client already polls carries the player's own stake; dropping \
         it puts the player back where FINDING 18 found them."
    );
    assert!(
        src.contains("hand_is_unmovable"),
        "PokerTable.svelte no longer reads `hand_is_unmovable`, so it cannot tell a player the \
         difference between money that is contested and money nobody can win."
    );
}

/// The one line that locked the auditor out of the screen that would have told
/// them: `disabled={tableBalance <= 0}` on the Withdraw button, on a player whose
/// escrow read zero BECAUSE their 2.98 ICP was in a pot.
#[test]
fn the_withdraw_button_opens_when_the_only_money_left_is_in_a_pot() {
    let src = table_component_source("PokerTable.svelte");
    assert!(
        !src.contains("onclick={onShowWithdraw} disabled={tableBalance <= 0}"),
        "the Withdraw button is disabled on `tableBalance <= 0` alone again. A player whose \
         balance is zero BECAUSE their stake is in a pot is exactly the player who needs that \
         dialog: it is where the stake and its recovery button are. FINDING 18."
    );
    assert!(
        src.contains("tableBalance <= 0 && myCommittedInPot <= 0"),
        "PokerTable.svelte must open the withdraw dialog whenever a stake is outstanding, even \
         with a zero balance. FINDING 18."
    );
}

/// Neither surface may be an overlay. HARD RULE 2: nothing in this app may cover
/// the unaudited-alpha disclaimer, the 18+ notice, the jurisdiction warning or the
/// no-rake property, and E-52 is the wave-6 measurement of the app's own toast
/// doing precisely that at 9 of 9 sample points.
#[test]
fn the_outstanding_stake_surface_cannot_cover_the_protected_notices() {
    for (file, blocks) in [
        (
            "WithdrawModal.svelte",
            vec![".committed-stake", ".recover-btn", ".committed-unknown"],
        ),
        ("PokerTable.svelte", vec![".wallet-committed"]),
    ] {
        let src = table_component_source(file);
        for selector in blocks {
            let Some(start) = src.find(&format!("{selector} {{")) else {
                panic!("{file}: the custody surface rule `{selector}` is gone");
            };
            let end = src[start..]
                .find('}')
                .map(|e| start + e)
                .unwrap_or(src.len());
            let rule = &src[start..end];
            for forbidden in ["position: fixed", "position: absolute", "z-index"] {
                assert!(
                    !rule.contains(forbidden),
                    "{file}: `{selector}` uses `{forbidden}`. The outstanding-stake surface is \
                     rendered in the document flow precisely so it can never sit on top of the \
                     four player-protection notices (HARD RULE 2, docs/DEFECTS.md E-52).\n{rule}"
                );
            }
        }
    }
}
