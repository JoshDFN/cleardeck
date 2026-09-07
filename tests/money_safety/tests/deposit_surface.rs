//! THE DEPOSIT SURFACE: ONE ADDRESS, AND IT IS YOURS.
//!
//! docs/SECURITY-FINDINGS.md FINDING 06, FINDING 11, FINDING 34, FINDING 40.
//!
//! Every test here is about the same sentence: **a player is given an address, a
//! player sends money to it, and the money becomes theirs.** Three auditors
//! raised three different ways that sentence was false, and the three have one
//! shape in common -- the money moved on the ledger and no surface of this
//! canister could say whose it was.
//!
//! Run: `cd tests/money_safety && cargo test --test deposit_surface -- --nocapture`

use candid::{decode_one, Encode, Principal};
use money_safety::ledger;
use money_safety::table_api::*;
use money_safety::world::*;

const ICP: u64 = 100_000_000;
const FEE: u64 = ledger::TRANSFER_FEE;

fn subaccount_of(world: &World, who: Principal) -> Vec<u8> {
    let bytes = world
        .pic
        .query_call(world.table, who, "get_deposit_subaccount", Encode!().unwrap())
        .expect("get_deposit_subaccount");
    decode_one::<Vec<u8>>(&bytes).expect("get_deposit_subaccount reply decode")
}

/// The address a client would derive for `who` with NO help from the canister:
/// `account_identifier(table, sha256("cleardeck-deposit:" || who))`.
///
/// Reimplemented in `money_safety::ledger` from the published derivation rather
/// than imported from the canister, so the canister cannot make this agree with
/// itself by construction.
fn derived_address(world: &World, who: Principal) -> String {
    ledger::account_identifier_hex(&world.table, Some(ledger::deposit_subaccount(&who)))
}

fn main_account_address(world: &World) -> String {
    ledger::account_identifier_hex(&world.table, None)
}

// ===========================================================================
// FINDING 34 -- ONE ADDRESS, AND IT IS YOURS
// ===========================================================================

/// **THE GATE.** `get_deposit_address()` must name the CALLER's own account.
///
/// # The defect this replaced
///
/// It returned `compute_account_identifier(canister_id(), None)` -- the canister's
/// MAIN account -- under a Candid comment reading "Get the canister's account for
/// deposits". The same 64 hex characters for every player on earth. The third
/// auditor sent 1 ICP there and no surface, player or controller, attributed it
/// to anybody, because *no surface can*: the address carries no name.
///
/// # Reverting it turns this red
///
/// Restoring `compute_account_identifier(&canister_id(), None)` makes alice's and
/// bob's addresses equal, which the first assertion below refuses.
#[test]
fn the_published_deposit_address_is_different_for_every_player_and_is_not_the_main_account() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    let a = world.published_deposit_address(alice);
    let b = world.published_deposit_address(bob);
    let main = main_account_address(&world);

    println!("get_deposit_address() as alice -> {a}");
    println!("get_deposit_address() as bob   -> {b}");
    println!("the canister's MAIN account    -> {main}");

    assert_ne!(
        a, b,
        "FINDING 34: get_deposit_address() published ONE address to every player. \
         An address that is not yours cannot attribute your money to you, and the \
         auditor's 1 ICP proved it: money arrived and no surface could name an owner."
    );
    assert_ne!(
        a, main,
        "FINDING 34: get_deposit_address() published the canister's MAIN account, \
         which is where sweeps and pulls land and is nobody's in particular"
    );
    assert_ne!(b, main, "the same, for bob");

    // And it is the SAME account `get_deposit_subaccount()` names, in the other
    // addressing scheme. Two methods, two formats, ONE account -- which is the
    // whole fix. A client that trusts either one must land in the same place.
    assert_eq!(
        a,
        derived_address(&world, alice),
        "the published address must be account_identifier(table, deposit_subaccount(caller)); \
         if these differ, the hex address and the ICRC-1 address are DIFFERENT ACCOUNTS and \
         one of them is unreachable by claim_external_deposit()"
    );
    assert_eq!(
        subaccount_of(&world, alice),
        ledger::deposit_subaccount(&alice).to_vec(),
        "and get_deposit_subaccount() must still be the published derivation"
    );
}

/// **THE TWO CASES WHERE THERE IS NO ADDRESS, AND THEY SAY SO.**
///
/// `get_deposit_address()` returns `text`, so its only way to say "there is no
/// address for you" is in words. The contract is that a reply is payable **iff**
/// it is 64 lowercase hex characters, and every refusal starts with
/// `"NO ADDRESS: "`. Both refusals used to be 64 hex characters of the shared main
/// account, which is the worst possible answer to both questions.
///
/// * **anonymous** -- `claim_external_deposit()` refuses anonymous callers, so
///   money at the anonymous principal's deposit account could never be swept by
///   anybody. An address nobody can sweep is a hole with a name.
/// * **ckBTC** -- the ckBTC ledger is ICRC-1 only. It has no account-identifier
///   form and no endpoint that accepts one, so 64 hex characters is not a
///   destination in any wallet.
#[test]
fn there_is_no_hex_address_for_an_anonymous_caller_or_on_a_ckbtc_table_and_both_say_so() {
    let icp = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = icp.actor("alice");

    let anon = icp.published_deposit_address(Principal::anonymous());
    println!("get_deposit_address() as anonymous -> {anon}");
    assert!(
        anon.starts_with("NO ADDRESS: "),
        "an anonymous caller must be refused, not handed an unsweepable account: {anon}"
    );
    assert!(
        hex::decode(&anon).is_err(),
        "and the refusal must not be pasteable as an address"
    );

    let payable = icp.published_deposit_address(alice);
    assert_eq!(payable.len(), 64, "a signed-in ICP caller gets 64 hex chars");
    assert!(
        hex::decode(&payable).is_ok() && payable == payable.to_lowercase(),
        "and they are lowercase hex"
    );

    let btc = World::new(
        TableConfig {
            currency: Currency::BTC,
            ..TableConfig::six_max_icp()
        },
        &["alice"],
    );
    let on_btc = btc.published_deposit_address(btc.actor("alice"));
    println!("get_deposit_address() on a ckBTC table -> {on_btc}");
    assert!(
        on_btc.starts_with("NO ADDRESS: "),
        "a ckBTC table has no account-identifier form and must not invent one: {on_btc}"
    );
    assert!(
        on_btc.contains("get_deposit_subaccount"),
        "and it must name the addressing scheme ckBTC actually uses: {on_btc}"
    );
}

/// One account, one answer, on every surface that names it.
///
/// `get_deposit_custody()` carries the address alongside the amount so a client
/// needs one call, and it is built from the same derivation, so the two cannot
/// drift. FINDING 34 was two surfaces disagreeing about where "your deposit
/// address" is; a second copy of the derivation would rebuild it.
#[test]
fn every_surface_that_names_the_address_names_the_same_one() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    let custody = world.deposit_custody(alice);
    assert_eq!(
        custody.address,
        world.published_deposit_address(alice),
        "get_deposit_custody().address and get_deposit_address() must be one string"
    );
    assert_eq!(custody.subaccount, subaccount_of(&world, alice));
    assert_eq!(custody.canister, world.table);
    assert_eq!(
        custody.address,
        derived_address(&world, alice),
        "and both must equal the published derivation"
    );
    assert_ne!(
        custody.address,
        world.deposit_custody(bob).address,
        "and it must be per-player on this surface too"
    );
}

/// **THE LOAD-BEARING FACT, PROVED SEPARATELY.** The 64-hex account identifier of
/// `(table, deposit_subaccount(alice))` and the ICRC-1 account
/// `Account { owner: table, subaccount: deposit_subaccount(alice) }` are ONE
/// account on the real ICP ledger.
///
/// The whole fix rests on this. It is a fact about the ledger, not about
/// ClearDeck, and it is proved here on the real mainnet ledger module rather than
/// assumed from the specification -- because if it were false, publishing the hex
/// form of a subaccount would send every player's money to an account
/// `claim_external_deposit()` cannot reach, which is a worse defect than the one
/// being fixed.
#[test]
fn the_hex_address_and_the_icrc1_account_are_one_account_on_the_real_ledger() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");

    let hex = derived_address(&world, alice);
    println!("account_identifier(table, deposit_subaccount(alice)) = {hex}");

    world
        .legacy_transfer_to_address(alice, &hex, 2 * ICP, Some(alice))
        .expect("legacy transfer to the derived hex address");

    let by_icrc1 = world.ledger_balance(world.table, Some(ledger::deposit_subaccount(&alice)));
    println!("icrc1_balance_of(table, deposit_subaccount(alice)) = {by_icrc1}");
    assert_eq!(
        by_icrc1,
        2 * ICP,
        "the ICP ledger must resolve the legacy account identifier and the ICRC-1 \
         Account to the same balance; if it does not, publishing the hex form of a \
         deposit subaccount is a fund trap"
    );
    assert_eq!(
        world.ledger_balance(world.table, None),
        0,
        "and it must NOT have landed in the main account"
    );
}

/// **THE GATE, EXECUTED.** A player sends to the address they were given, with
/// the LEGACY `transfer` endpoint a hex string implies, and the money lands in
/// THEIR escrow.
///
/// # Why the legacy endpoint and not `icrc1_transfer`
///
/// `get_deposit_address()` returns 64 hex characters. That form is only spendable
/// through the ICP ledger's `transfer` method. Every existing test in this project
/// funds a deposit subaccount with `icrc1_transfer` to
/// `Account { owner, subaccount }`, which proves the ICRC-1 half and ASSUMES the
/// half a player with a hex string actually uses. This test executes the assumed
/// half: the address goes out of the canister, into the ledger, and the sweep
/// finds it.
#[test]
fn money_sent_to_the_address_a_player_is_given_arrives_in_that_players_escrow() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    let address = world.published_deposit_address(alice);
    println!("alice is given -> {address}");

    let block = world
        .legacy_transfer_to_address(alice, &address, 3 * ICP, Some(alice))
        .expect("legacy transfer to the published address");
    println!("alice sent 3 ICP with the LEGACY transfer endpoint, block {block}");

    // The ledger must agree that the hex address and the ICRC-1 account are the
    // same account. If it does not, the sweep below cannot see the money.
    let at_subaccount = world.ledger_balance(world.table, Some(ledger::deposit_subaccount(&alice)));
    println!("icrc1_balance_of(table, deposit_subaccount(alice)) = {at_subaccount}");
    assert_eq!(
        at_subaccount,
        3 * ICP,
        "the 64-hex address and (table, deposit_subaccount(alice)) must be ONE account"
    );
    assert_eq!(
        world.ledger_balance(world.table, None),
        0,
        "and NOTHING may land in the shared main account"
    );

    let credited = world
        .claim_external_deposit(alice)
        .expect("claim_external_deposit sweeps what arrived at the published address");
    println!("claim_external_deposit(alice) -> credited {credited}");

    assert_eq!(
        credited,
        3 * ICP - FEE,
        "alice's escrow must hold what she sent, less the one ledger fee the sweep costs"
    );
    assert_eq!(world.get_balance(alice), 3 * ICP - FEE);
    assert_eq!(
        world.get_balance(bob),
        0,
        "and it must be ALICE's -- correct totals with the wrong recipient is this \
         project's signature failure"
    );

    // ...and out again, which is the only definition of "it is yours" that counts.
    let out = world
        .withdraw(alice, 3 * ICP - FEE)
        .expect("alice withdraws everything she deposited");
    println!("withdraw -> block {out}");
    assert_eq!(
        world.get_balance(alice),
        0,
        "the money left the canister on alice's own call"
    );
}

/// **THE SHARED ACCOUNT, DECLARED.** Money at the canister's main account with no
/// message attached is not attributable, and the canister must say so rather than
/// imply an owner.
///
/// This is the other half of FINDING 34: `get_deposit_address()` no longer sends
/// anybody there, but ICP is sitting at that account on mainnet today, so the
/// recovery door must be real and it must be honest about its one precondition --
/// `notify_deposit` attributes by the block's `from`, so only the principal who
/// sent it can claim it, and only if they still have the block index.
#[test]
fn money_at_the_shared_main_account_is_recoverable_by_its_sender_and_by_nobody_else() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    let main = main_account_address(&world);
    let block = world
        .legacy_transfer_to_address(alice, &main, 1 * ICP, None)
        .expect("legacy transfer to the shared main account");
    println!("alice sent 1 ICP to the SHARED main account, block {block}");
    assert_eq!(world.ledger_balance(world.table, None), ICP);

    // Nobody is credited, and no surface can name an owner. That is a property of
    // the ADDRESS, not a defect in a surface: the account carries no name.
    assert_eq!(world.get_balance(alice), 0);
    assert_eq!(world.custody_status(alice).total, 0);
    assert_eq!(
        world.claim_external_deposit(alice).is_err(),
        true,
        "the sweep looks at alice's own subaccount, which is empty"
    );

    // The canister must nevertheless ACCOUNT for it. This is FINDING 35's record.
    let refreshed = world
        .refresh_main_account_custody(world.controller)
        .expect("refresh_main_account_custody");
    println!("main account observation -> {refreshed:?}");
    assert_eq!(
        refreshed.amount, ICP,
        "the canister must be able to see money at its own main account"
    );

    // Bob cannot take it: `notify_deposit` attributes by the block's `from`.
    let bob_try = world.notify_deposit(bob, block);
    println!("notify_deposit(bob, {block}) -> {bob_try:?}");
    assert!(
        bob_try.is_err(),
        "a shared address plus a public block index must NOT be a claim ticket for \
         anybody who can read the ledger"
    );

    // Alice can. This is the recovery door, executed.
    let credited = world
        .notify_deposit(alice, block)
        .expect("notify_deposit credits the sender of the block");
    println!("notify_deposit(alice, {block}) -> credited {credited}");
    assert_eq!(credited, ICP, "the whole amount, no second fee");
    assert_eq!(world.get_balance(alice), ICP);
}

/// The commonest wrong call must not be answered with a false sentence.
///
/// A player sends to the address they were given and then reaches for
/// `notify_deposit`, because that is the call whose name sounds like "tell the
/// canister about my deposit". `notify_deposit` credits only the MAIN account, so
/// it used to reply *"Transfer was not to this canister"* -- which is false, and
/// sends somebody hunting for a transfer that is sitting safely at their own
/// address. The refusal must name the door that works.
#[test]
fn notify_deposit_does_not_tell_a_player_their_own_deposit_address_is_not_this_canister() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    let address = world.published_deposit_address(alice);

    let block = world
        .legacy_transfer_to_address(alice, &address, 2 * ICP, Some(alice))
        .expect("alice funds her own deposit address");

    let refusal = format!("{:?}", world.notify_deposit(alice, block).unwrap_err());
    println!("notify_deposit on a SUBACCOUNT arrival -> {refusal}");
    assert!(
        !refusal.contains("Transfer was not to this canister"),
        "the money IS at an account of this canister; saying otherwise sends a player \
         looking for a lost transfer: {refusal}"
    );
    assert!(
        refusal.contains("claim_external_deposit"),
        "and the refusal must name the door that does work: {refusal}"
    );

    // And that door does work, from the same state.
    assert_eq!(
        world.claim_external_deposit(alice).expect("the named door"),
        2 * ICP - FEE
    );
}

// ===========================================================================
// THE UNCERTIFIED QUERY -- FINDING 40
// ===========================================================================

/// **THE REPRODUCTION.** A substituted address is a completed theft, and the
/// canister cannot detect it, because the substitution happens on the way OUT.
///
/// The auditor's sequence, verbatim: alice is shown bob's address, alice pays it,
/// bob sweeps it. Nothing here is a canister bug -- the canister behaves
/// perfectly at every step. The defect is that the answer to "what is my address"
/// travelled over an ordinary query, which a single replica can answer alone and
/// which carries no signature the client checks.
///
/// This test does NOT assert the theft is impossible, because it is not: the
/// canister cannot tell a well-formed transfer from a misdirected one, and no
/// canister-side change can. It asserts the property that makes the substitution
/// harmless -- that the client never has to ask.
#[test]
fn a_substituted_address_is_a_completed_theft_and_local_derivation_is_what_prevents_it() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice", "bob"]);
    let alice = world.actor("alice");
    let bob = world.actor("bob");

    // What ONE dishonest replica would put in the reply to alice's query.
    let substituted = world.published_deposit_address(bob);
    println!("alice asks for her address; a dishonest reply gives her -> {substituted}");

    world
        .legacy_transfer_to_address(alice, &substituted, 5 * ICP, Some(bob))
        .expect("alice pays the address she was shown");

    let bob_took = world
        .claim_external_deposit(bob)
        .expect("bob sweeps what landed at HIS address");
    println!("bob claim_external_deposit -> {bob_took}");
    assert_eq!(
        bob_took,
        5 * ICP - FEE,
        "the theft completes: alice's 5 ICP is in bob's escrow, and every invariant \
         is silent because no e8 went missing"
    );
    assert_eq!(world.get_balance(alice), 0, "alice has nothing");

    // THE FIX, STATED AS A PROPERTY. The address is a pure function of the
    // player's principal and the canister id, both of which the client already
    // holds. A client that DERIVES has no query to substitute.
    //
    // `derived_address` is computed here from the published derivation with no
    // call to the canister at all -- that is the point -- and it must equal what
    // the canister would have said.
    let honest = world.published_deposit_address(alice);
    assert_eq!(
        derived_address(&world, alice),
        honest,
        "a client deriving locally must land on the same account the canister names. \
         If these ever differ, the frontend's derivation and the canister's have \
         drifted and the client is paying an address the sweep cannot reach."
    );
    assert_ne!(
        derived_address(&world, alice),
        substituted,
        "and the derivation is what the substituted reply cannot match"
    );

    // The derivation is a pure function of (canister, principal): it does not
    // depend on any canister state, so a client can compute it before the
    // canister is even reachable, and two clients must agree.
    assert_eq!(
        derived_address(&world, bob),
        world.published_deposit_address(bob),
        "the same, for any principal"
    );
}

/// **THE GATE ON THE CLIENT'S DERIVATION.** The FRONTEND's derivation, executed,
/// must equal the CANISTER's answer for the same principals.
///
/// The fix for the uncertified query is that the client never asks. That only
/// makes a player safer if the client's own arithmetic is right: a client that
/// derives a WRONG address sends money to an account `claim_external_deposit()`
/// cannot reach, which is worse than trusting a query. So the two
/// implementations are compared here, by running the real
/// `src/cleardeck_frontend/src/lib/depositAddress.js` in node against the real
/// canister on the replica -- not by comparing two copies of the same constant.
///
/// Changing either derivation without changing the other turns this red.
#[test]
fn the_frontends_own_derivation_agrees_with_the_canister_for_every_principal() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .to_path_buf();
    let module = repo.join("src/cleardeck_frontend/src/lib/depositAddress.js");
    assert!(
        module.exists(),
        "the frontend derivation module is missing: {}",
        module.display()
    );

    let people: Vec<Principal> = ["alice", "bob", "carol"]
        .iter()
        .map(|n| world.actor(n))
        .collect();
    let args: Vec<String> = people.iter().map(|p| p.to_text()).collect();

    let script = format!(
        "import {{ deriveDepositAddress }} from {module:?};\n\
         const table = {table:?};\n\
         for (const p of process.argv.slice(1)) {{\n\
           console.log(deriveDepositAddress(table, p).address);\n\
         }}\n",
        module = module.to_string_lossy(),
        table = world.table.to_text(),
    );

    let out = std::process::Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(&script)
        .arg("--")
        .args(&args)
        .current_dir(repo.join("src/cleardeck_frontend"))
        .output()
        .expect(
            "node is required to run this gate; scripts/dev.sh doctor already requires it, \
             and the frontend cannot be built without it",
        );
    assert!(
        out.status.success(),
        "the frontend derivation module failed to run:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let derived: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(
        derived.len(),
        people.len(),
        "expected one address per principal, got {derived:?}"
    );

    for (who, from_js) in people.iter().zip(derived.iter()) {
        let from_canister = world.published_deposit_address(*who);
        println!("{}  js={from_js}  canister={from_canister}", who.to_text());
        assert_eq!(
            from_js, &from_canister,
            "depositAddress.js and the canister disagree about {}'s deposit address. \
             A client that derives locally is only safer than one that asks if its \
             arithmetic is right; a wrong derivation sends money to an account \
             claim_external_deposit() cannot reach.",
            who.to_text()
        );
    }
}

/// **THE OTHER SPELLING OF THE SAME SUBSTITUTION.** The frontend's 32-BYTE
/// derivation must equal `get_deposit_subaccount()` for every principal.
///
/// # Why this test exists and the one above it was not enough
///
/// The gate above compares the 64-HEX address, which is the form the address
/// panel renders. The OISY branch of `DepositModal.svelte` never touches that
/// form: an ICRC-1 wallet is paid with `Account { owner, subaccount }`, so the
/// money-moving path uses the 32-byte spelling, and it used to fetch that
/// spelling from `tableActor.get_deposit_subaccount()` -- the SAME uncertified
/// query FINDING 40 is about, forty lines below the code that stopped trusting
/// it. A green run of the hex gate was therefore fully consistent with a live
/// substitution on the only OISY path that spends money: the instrument was
/// built by somebody who knew which door they had repaired, and it tested that
/// door. docs/SECURITY-FINDINGS.md FINDING 40.
#[test]
fn the_frontends_32_byte_derivation_agrees_with_the_canister_for_every_principal() {
    let world = World::new(TableConfig::six_max_icp(), &["alice", "bob", "carol"]);
    let repo = repo_root();
    let module = repo.join("src/cleardeck_frontend/src/lib/depositAddress.js");

    let people: Vec<Principal> = ["alice", "bob", "carol"]
        .iter()
        .map(|n| world.actor(n))
        .collect();
    let args: Vec<String> = people.iter().map(|p| p.to_text()).collect();

    let script = format!(
        "import {{ depositSubaccount }} from {module:?};\n\
         for (const p of process.argv.slice(1)) {{\n\
           console.log(Buffer.from(depositSubaccount(p)).toString('hex'));\n\
         }}\n",
        module = module.to_string_lossy(),
    );

    let out = std::process::Command::new("node")
        .arg("--input-type=module")
        .arg("-e")
        .arg(&script)
        .arg("--")
        .args(&args)
        .current_dir(repo.join("src/cleardeck_frontend"))
        .output()
        .expect("node is required to run this gate");
    assert!(
        out.status.success(),
        "the frontend derivation module failed to run:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let derived: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(derived.len(), people.len(), "one subaccount per principal");

    for (who, from_js) in people.iter().zip(derived.iter()) {
        let from_canister: String = subaccount_of(&world, *who)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        println!("{}  js={from_js}  canister={from_canister}", who.to_text());
        assert_eq!(
            from_js, &from_canister,
            "depositAddress.js and the canister disagree about {}'s deposit SUBACCOUNT. \
             This is the form an ICRC-1 wallet is paid with, so a drift here sends the \
             money to an account claim_external_deposit() cannot reach.",
            who.to_text()
        );
    }
}

/// **THE DESTINATION OF A REAL PAYMENT MUST BE DERIVED, NOT FETCHED.**
///
/// The two gates above prove the client's arithmetic is right. Neither of them
/// can see whether the client USES it: `DepositModal.svelte` shipped a correct
/// local derivation for the address it *displays* and a fetched one for the
/// address it *pays*, and every test in this project stayed green.
///
/// So this reads the source and follows the value. The OISY door lives in
/// `lib/deposit-flow.js` (`depositViaOisy`) since the cashier phase moved it out
/// of the modal statement for statement (docs/UI-WAVE.md section 4): the modal
/// presses it through `lib/deposit-submit.js`. It finds the subaccount the OISY
/// transfer is addressed to and requires that the name it uses was assigned from
/// the local `depositSubaccount(...)`, never from an `await tableActor.` call;
/// then it follows the import chain back to the modal so the door the modal
/// presses IS this one; and it requires the modal to hold NO copy of the door
/// (no transfer destination, no `wallet.transfer`, no `icrc1_transfer`, no
/// `get_deposit_subaccount`), because an unguarded second copy in the component
/// is how a gate pointed at the right module stays green while the app pays a
/// fetched address. It goes red the moment a payment destination comes back
/// over the wire again, or the door grows a copy in the modal.
/// docs/SECURITY-FINDINGS.md FINDING 40.
#[test]
fn the_oisy_transfer_destination_is_derived_locally_and_not_fetched() {
    let door = repo_root().join("src/cleardeck_frontend/src/lib/deposit-flow.js");
    let src = std::fs::read_to_string(&door)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", door.display()));

    // The one place the OISY route addresses an ICRC-1 transfer.
    let marker = "subaccount: [";
    let mut destinations: Vec<String> = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let Some(rest) = line.split_once(marker).map(|(_, r)| r) else {
            continue;
        };
        // `subaccount: []` is the MAIN account of a wallet being read, not a
        // destination this module pays.
        let Some(name) = rest.split(']').next().map(str::trim) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        println!("line {}: transfer destination subaccount = {name}", i + 1);
        destinations.push(name.to_string());
    }
    assert!(
        !destinations.is_empty(),
        "no ICRC-1 transfer destination found in {}. If the OISY deposit path was \
         removed this gate must be removed with it, deliberately; a gate that silently \
         stops measuring anything is how FINDING 40 shipped.",
        door.display()
    );

    for name in &destinations {
        let derived_here = format!("const {name} = depositSubaccount(");
        assert!(
            src.contains(&derived_here),
            "deposit-flow.js pays an ICRC-1 transfer to `subaccount: [{name}]`, but \
             `{name}` is not assigned from the local derivation `depositSubaccount(...)`. \
             If it comes from `await tableActor.get_deposit_subaccount()` the destination \
             is an UNCERTIFIED QUERY REPLY: one replica can answer with another player's \
             subaccount and this client will pay it. The ledger totals stay right, the \
             canister holds every e8, and no invariant in this project can see it -- only \
             the recipient is wrong. See docs/SECURITY-FINDINGS.md FINDING 40."
        );
        let fetched_into = format!("{name} = await");
        assert!(
            !src.contains(&fetched_into),
            "deposit-flow.js reassigns the transfer destination `{name}` from an await: \
             the derived value is being overwritten by a reply. FINDING 40."
        );
    }

    // The belt: the fetched value must not be what is handed to the wallet.
    assert!(
        !src.contains("subaccount: [depositSubaccount]") && !src.contains("subaccount: [reportedSub]"),
        "the OISY branch is addressing its transfer with the value returned by \
         `tableActor.get_deposit_subaccount()`. See docs/SECURITY-FINDINGS.md FINDING 40."
    );
    // The canister is asked only to cross-check, and a disagreement REFUSES.
    assert!(
        src.contains("get_deposit_subaccount()") && src.contains("Refusing to send"),
        "deposit-flow.js no longer cross-checks the derived subaccount against the \
         canister's, or no longer refuses on a disagreement. The check is the only thing \
         that catches a drift between the two derivations before real money moves."
    );

    // FOLLOW THE DOOR BACK TO THE MODAL: the component presses THIS module.
    let submit = repo_root().join("src/cleardeck_frontend/src/lib/deposit-submit.js");
    let submit_src = std::fs::read_to_string(&submit)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", submit.display()));
    assert!(
        submit_src.contains("from './deposit-flow.js'") && submit_src.contains("depositViaOisy("),
        "deposit-submit.js does not press deposit-flow.js's depositViaOisy: the guarded \
         door is not the one the sheet uses (the FINDING-41 lesson)."
    );
    let modal = repo_root().join("src/cleardeck_frontend/src/lib/components/DepositModal.svelte");
    // The CODE of the modal, comments stripped: its prose names the calls it no
    // longer makes, and the scan below is for those names.
    let modal_src = strip_comments(
        &std::fs::read_to_string(&modal)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", modal.display())),
    );
    assert!(
        modal_src.contains("from '$lib/deposit-submit.js'") && modal_src.contains("submitDeposit("),
        "DepositModal.svelte does not press submitDeposit() from lib/deposit-submit.js: \
         the OISY door this gate reads is not the one the modal uses."
    );

    // NO COPY OF THE DOOR IN THE MODAL. A second, unguarded transfer in the
    // component would be invisible to a gate pointed at the module.
    for forbidden in [
        "wallet.transfer",
        "icrc1_transfer",
        "get_deposit_subaccount",
        "depositSubaccount(",
    ] {
        assert!(
            !modal_src.contains(forbidden),
            "DepositModal.svelte contains `{forbidden}`: a copy of the OISY money door \
             outside lib/deposit-flow.js, which this gate does not read. Every transfer \
             destination must be derived in the one module that pays it. FINDING 40."
        );
    }
    let modal_destinations = modal_src
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            l.split_once(marker)
                .and_then(|(_, r)| r.split(']').next())
                .is_some_and(|name| !name.trim().is_empty())
        })
        .map(|(i, l)| format!("  line {}: {}", i + 1, l.trim()))
        .collect::<Vec<_>>();
    assert!(
        modal_destinations.is_empty(),
        "DepositModal.svelte addresses an ICRC-1 transfer of its own:\n{}\nThe modal \
         must hold no transfer destination; the door is lib/deposit-flow.js.",
        modal_destinations.join("\n")
    );
}

/// **CLOSED IN WAVE 14, AND NO LONGER `#[ignore]`d.** Money at the shared main
/// account is a LIABILITY, and the public solvency instrument says so.
///
/// docs/SECURITY-FINDINGS.md FINDING 43. This was committed red-on-purpose and
/// ignored for one wave, for the reason `dev.sh known-defects` exists: a suite
/// that is red by design teaches everyone to ignore red. It is an ordinary gate
/// now and it runs in the default suite:
///
/// ```text
/// cd tests/money_safety
/// cargo test --test deposit_surface -- --nocapture \
///   the_solvency_verdict_counts_money_at_the_main_account_as_surplus
/// ```
///
/// # The two totals, and the one that replaced them
///
/// `total_liability()` -- the number the currency guard reads -- had five terms
/// and the fifth was `main_uncredited_observed()` (FINDING 35). `get_solvency()`'s
/// own `owed` had six terms and that was not one of them, while `held` DID
/// include the main-account balance. So the same e8 was counted as an asset and
/// not as a liability, and the difference was reported as surplus. The report even
/// carried both numbers -- `guard_liability` and `owed`, side by side, disagreeing.
///
/// There is one definition now and `owed` IS `guard_liability`, the same call.
/// Reverting either half turns this red: `owed` loses the main-account residual
/// and the difference goes to +1 ICP.
///
/// This is the money FINDING 34's shared address collected for eleven waves, and
/// there is ICP at that account on mainnet today.
#[test]
fn the_solvency_verdict_counts_money_at_the_main_account_as_surplus() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");

    let main = main_account_address(&world);
    world
        .legacy_transfer_to_address(alice, &main, 1 * ICP, None)
        .expect("legacy transfer to the shared main account");

    // Public, moves no money: ask the ledger what the main account holds and
    // write it down. This is the call the deposit screen makes.
    let report = world
        .refresh_solvency(alice)
        .expect("refresh_solvency is public and moves no money");

    println!("verdict              = {:?}", report.verdict);
    println!("owed                 = {}", report.owed);
    println!("guard_liability      = {}", report.guard_liability);
    println!("held                 = {:?}", report.held);
    println!("difference_e8s       = {:?}", report.difference_e8s);
    println!("unattributed_at_main = {:?}", report.unattributed_at_main);
    println!("summary              = {}", report.summary);

    assert_eq!(
        report.unattributed_at_main,
        Some(ICP),
        "the datum is present: this canister knows it holds 1 ICP for somebody it cannot name"
    );
    assert!(
        report.owed >= report.unattributed_at_main.unwrap_or(0),
        "THE DEFECT. `owed` -- the number the verdict and the summary are computed \
         from -- omits `main_uncredited_observed()`, which `guard_liability` \
         (`total_liability()`, what the currency guard actually reads) includes. Two \
         totals of the same liability, in one reply, and the public one is the smaller. \
         owed={} guard_liability={} unattributed_at_main={:?}",
        report.owed,
        report.guard_liability,
        report.unattributed_at_main
    );
    assert_eq!(
        report.owed, report.guard_liability,
        "ONE DEFINITION. `owed` and `guard_liability` are one call to total_liability(), \
         not two sums that are expected to differ in a documented way -- that expectation \
         is what let this ship. owed={} guard_liability={}",
        report.owed, report.guard_liability
    );
    assert_eq!(
        report.difference_e8s,
        Some(0),
        "a canister holding exactly what it owes has a difference of ZERO. Before wave 14 \
         this read +1 ICP and the summary called a player's own money a surplus."
    );
    assert!(
        !report.summary.contains("a surplus of"),
        "and the sentence a player reads on the deposit screen must not call it profit: {}",
        report.summary
    );
    assert!(
        report.summary.contains("100000000") && report.summary.contains("cannot yet name"),
        "it must NAME the unattributed 1 ICP in words. Publishing the number in a field \
         nobody reads, three fields above calling the same money a surplus, is exactly what \
         FINDING 43 was. summary = {}",
        report.summary
    );
}

/// The repository root, from this crate's manifest.
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root")
        .to_path_buf()
}

/// Source with its `//`, `/* */` and `<!-- -->` comments removed, so a scan for
/// a money call reads the code and not the prose that explains it. Deliberately
/// naive (no string or regex parsing); the failure direction is safe: a token
/// inside a string literal still trips the scan.
fn strip_comments(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0usize;
    while i < chars.len() {
        let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
        let four: String = chars[i..chars.len().min(i + 4)].iter().collect();
        if two == "//" {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if two == "/*" {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
        } else if four == "<!--" {
            i += 4;
            while i + 2 < chars.len() && !(chars[i] == '-' && chars[i + 1] == '-' && chars[i + 2] == '>') {
                i += 1;
            }
            i = (i + 3).min(chars.len());
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

// ===========================================================================
// FINDING 11 -- DUST
// ===========================================================================

/// FINDING 11, executed through the address a player is actually GIVEN.
///
/// `deposit_subaccount_anchor::dust_below_the_fee_is_accounted_for_and_recoverable_by_topping_up`
/// drives the same arithmetic through `icrc1_transfer`. This one drives it
/// through the 64-hex address and the legacy endpoint, because that is the door
/// the product's own text sends a player to, and a recovery that only works in
/// the addressing scheme nobody was given is not a recovery.
#[test]
fn dust_at_the_published_address_is_visible_and_recovered_by_topping_up_the_same_address() {
    let mut world = World::new(TableConfig::six_max_icp(), &["alice"]);
    let alice = world.actor("alice");
    let address = world.published_deposit_address(alice);

    world
        .legacy_transfer_to_address(alice, &address, FEE - 1, Some(alice))
        .expect("alice sends one e8 below the fee");

    let refused = world.claim_external_deposit(alice).unwrap_err();
    println!("claim_external_deposit with 9_999 e8s at the address -> {refused:?}");
    let text = format!("{refused:?}");
    assert!(
        text.contains("9999") || text.contains("9,999"),
        "the refusal must state the amount it is holding: {text}"
    );
    assert!(
        !text.to_lowercase().contains("no claimable balance"),
        "and it must NOT be the FINDING 11 sentence that told a player holding dust \
         that they had nothing: {text}"
    );

    let custody = world.deposit_custody(alice);
    println!("get_deposit_custody -> {custody:?}");
    assert_eq!(
        custody.observed_amount,
        FEE - 1,
        "every surface must carry it, at the address the player was GIVEN"
    );
    assert_eq!(world.custody_status(alice).total, FEE - 1);

    // The top-up, at the same address, through the same door.
    world
        .legacy_transfer_to_address(alice, &address, 2 * ICP, Some(alice))
        .expect("alice tops the same address up");
    let credited = world
        .claim_external_deposit(alice)
        .expect("the whole balance, dust included, is now sweepable");
    println!("after the top-up, claim_external_deposit -> {credited}");
    assert_eq!(
        credited,
        (FEE - 1) + 2 * ICP - FEE,
        "the dust came out with the top-up: immovable alone, never lost"
    );
}
