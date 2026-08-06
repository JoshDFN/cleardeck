//! **M13 -- ONE BELIEF.** After a stall, does this canister agree with itself
//! about whether one hand is dead?
//!
//! docs/DEFECTS.md E-59 / docs/SECURITY-FINDINGS.md FINDING 25. Six surfaces ask
//! "is this hand stuck?" -- the on-chain clock, `abandon_stuck_hand`, `cash_out`,
//! `leave_table`, `get_custody_status` and `TableView.hand_is_unmovable`. If two
//! of them answer differently on one state, the canister pays out two different
//! sets of recipients for the same hand and **every conservation invariant stays
//! silent**, because both answers conserve exactly. That is the signature all four
//! cross-agent defects in this project have had.
//!
//! # Why no gate in this repository could reach this before
//!
//! `World::advance` is `advance_time(d); tick()`. A tick is a round, and a round
//! is when the on-chain clock gets to run, so the clock always won the race and
//! the window in which a caller can disagree with it never opened.
//! [`World::advance_time_only`] opens it; it existed, it was used in exactly three
//! places, and **all three used it for QUERIES only**. No test in this tree had
//! ever sent an UPDATE through the window. Until the harness could express the
//! race, no gate could catch it.
//!
//! # The shape of the gate
//!
//! One state, driven down two paths:
//!
//! * **ARM A** -- nobody calls anything. Only the subnet produces blocks.
//! * **ARM B** -- one door call, sent at the first instant it can be sent, then
//!   the same blocks.
//!
//! The two arms must end with every principal holding the same number of e8s. Not
//! the same TOTAL -- the totals were never wrong, which is the whole difficulty of
//! this class -- **the same amount EACH**.
//!
//! The second property here is narrower and absolute: **a call that replies `Err`
//! must not change state.** `Err` from an ic-cdk update is an ordinary reply, not
//! a rollback, and the settle at the top of `cash_out` used to run before the seat
//! lookup -- so a principal who had never sat at the table could void a hand and
//! be told `Err("Not at table")`.
//!
//! ```text
//! cd tests/money_safety
//! cargo test --test stall_agreement -- --nocapture --test-threads=1
//! ```

use candid::Principal;
use money_safety::table_api::*;
use money_safety::world::*;
use std::collections::BTreeMap;
use std::time::Duration;

const ICP: u64 = 100_000_000;

/// The stall every scenario in this file opens with: an hour in which the
/// canister does not execute at all. A subnet halt, or a canister frozen for want
/// of cycles and then topped up, or a controller stopping it to upgrade. Well past
/// `STUCK_HAND_GRACE_NS` (300 s), which is the point: the wall clock says the
/// grace has elapsed and no attempt to move the hand has been made or missed.
const STALL: Duration = Duration::from_secs(3600);

/// A stall SHORTER than the grace. Nothing may open on either path.
const SHORT_STALL: Duration = Duration::from_secs(60);

/// Everybody the harness knows about. Only the first `seats` of them ever sit
/// down.
const ACTORS: [&str; 5] = ["alice", "bob", "carol", "dave", "stranger"];

/// The index of the principal who NEVER sits down, in any scenario. The one that
/// convicted the `Err`-that-commits: `cash_out` settled the hand and replied
/// `Err("Not at table")`.
const STRANGER: usize = ACTORS.len() - 1;

// ---------------------------------------------------------------------------
// doors
// ---------------------------------------------------------------------------

/// Every door that reads the "is this hand stuck?" predicate and can move money.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Door {
    Abandon,
    CashOut,
    LeaveTable,
    /// The control arm: send nothing at all. Must always agree with ARM A, and if
    /// it ever does not, the harness itself is not deterministic and every other
    /// row in the table is meaningless.
    Nothing,
}

impl Door {
    fn name(self) -> &'static str {
        match self {
            Door::Abandon => "abandon_stuck_hand",
            Door::CashOut => "cash_out",
            Door::LeaveTable => "leave_table",
            Door::Nothing => "(nothing)",
        }
    }

    /// Is this door an ORDINARY GAME ACTION when a seated player uses it?
    ///
    /// `leave_table` mid-hand IS a fold -- it is documented as one and it has
    /// always been one -- so of course the table ends up somewhere else than if
    /// nobody had left. Requiring `leave_table` by a seated player to produce the
    /// same recipients as "nobody did anything" would be a gate that cries wolf on
    /// the rules of poker, and a gate everybody turns off catches nothing.
    ///
    /// Every other door here is not: `abandon_stuck_hand` is documented as a
    /// recovery method that grants no advantage, and `cash_out` refuses outright
    /// while the caller is in a movable hand. Those must match ARM A exactly.
    ///
    /// The property that binds ALL of them, including this one, is the separate
    /// and stronger one: **no door may settle a hand as unmovable that the clock
    /// goes on to play out.** That is checked on every row.
    fn is_an_ordinary_action_for_a_seated_player(self) -> bool {
        matches!(self, Door::LeaveTable)
    }

    fn knock(self, world: &World, who: Principal) -> String {
        fn render<T: std::fmt::Debug>(r: Outcome<T>) -> String {
            match r {
                Ok(v) => format!("Ok({v:?})"),
                Err(OpError::Err(m)) => {
                    format!("Err({:?})", m.chars().take(38).collect::<String>())
                }
                Err(other) => format!("{other:?}"),
            }
        }
        match self {
            Door::Abandon => render(world.abandon_stuck_hand(who)),
            Door::CashOut => render(world.cash_out(who)),
            Door::LeaveTable => render(world.leave_table(who)),
            Door::Nothing => "-".to_string(),
        }
    }

    /// Did the reply refuse? An `Err` reply is the case the state must be
    /// untouched by.
    fn refused(reply: &str) -> bool {
        reply.starts_with("Err(")
    }
}

// ---------------------------------------------------------------------------
// measuring one state
// ---------------------------------------------------------------------------

/// Everything the canister holds FOR EACH PRINCIPAL. The number that has to match
/// across the two arms.
///
/// Escrow plus chips plus, WHILE A HAND IS LIVE, that principal's stake in the
/// middle -- their seat's `total_bet_this_hand` and any stake recorded for a seat
/// of theirs that has already been vacated. Once the hand is over the stake has
/// been paid into a stack or an escrow balance, so counting it again double
/// counts (that mistake showed a phantom 2,000,000 e8 surplus on the first run of
/// this file, which is worth leaving in the comment: a per-recipient gate is only
/// as good as its accounting).
fn per_principal(world: &World) -> BTreeMap<Principal, u64> {
    let snap = world.snapshot();
    let mut out: BTreeMap<Principal, u64> = BTreeMap::new();
    for a in world.actors.iter() {
        out.insert(a.principal, 0);
    }
    for (p, e) in snap.escrow.iter() {
        *out.entry(*p).or_insert(0) += *e;
    }
    for p in snap.table.seated() {
        *out.entry(p.principal).or_insert(0) += p.chips;
    }
    if snap.table.phase.hand_in_progress() {
        for p in snap.table.seated() {
            *out.entry(p.principal).or_insert(0) += p.total_bet_this_hand;
        }
        for d in snap.table.departed() {
            *out.entry(d.principal).or_insert(0) += d.contributed;
        }
    }
    out
}

/// Enough of the table to tell whether an `Err` reply moved anything: the phase,
/// the pot, whose clock it is, and who is sitting where with what.
fn table_fingerprint(world: &World) -> String {
    let t = world.table_state();
    format!(
        "phase={:?} pot={} action_on={} timer={:?} seats=[{}] departed={}",
        t.phase,
        t.pot,
        t.action_on,
        t.action_timer.as_ref().map(|a| a.expires_at),
        t.players
            .iter()
            .map(|p| match p {
                Some(p) => format!(
                    "{}:{}/{}/{}",
                    p.seat, p.chips, p.total_bet_this_hand, p.has_folded
                ),
                None => "-".to_string(),
            })
            .collect::<Vec<_>>()
            .join(","),
        t.departed().len()
    )
}

/// Blocks, and nothing else: the subnet doing what it does whether or not anybody
/// is watching. No ingress message of any kind.
fn only_blocks(world: &World, rounds: u32) {
    for _ in 0..rounds {
        world.pic.advance_time(Duration::from_secs(10));
        for _ in 0..4 {
            world.pic.tick();
        }
    }
}

// ---------------------------------------------------------------------------
// scenarios
// ---------------------------------------------------------------------------

/// A named starting position. Deterministic: two `World`s built from the same
/// `Scenario` are the same table down to the cards, which is what makes the two
/// arms a comparison of ONE state rather than of two similar ones. The gate
/// asserts that equality before it compares anything else.
#[derive(Clone, Copy, Debug)]
struct Scenario {
    name: &'static str,
    seats: usize,
    ante: bool,
    /// Actions taken before the stall, as `(actor index, action)`.
    opening: &'static [(usize, PlayerAction)],
    /// Simulated seconds to advance, WITH rounds, between the deal and the stall.
    warm_secs: u64,
    stall: Duration,
}

impl Scenario {
    const fn base(name: &'static str, seats: usize) -> Self {
        Scenario {
            name,
            seats,
            ante: false,
            opening: &[],
            warm_secs: 0,
            stall: STALL,
        }
    }
}

fn build(scenario: Scenario) -> World {
    let names: Vec<&str> = ACTORS.to_vec();
    let config = match (scenario.seats, scenario.ante) {
        (2, _) => TableConfig::heads_up_icp(),
        (_, true) => TableConfig::six_max_with_ante(),
        _ => TableConfig::six_max_icp(),
    };
    let world = World::new(config, &names);
    // Everybody is funded; only `seats` of them sit down, so `attacker` is a
    // principal the canister has never seen at the table.
    for name in names.iter() {
        let who = world.actor(name);
        world.fund_escrow(who, 20 * ICP).expect("deposit");
    }
    for (i, name) in names.iter().take(scenario.seats).enumerate() {
        world.join_table(world.actor(name), i as u8).expect("seat");
    }
    world.advance(Duration::from_secs(4));
    world
        .start_new_hand(world.actor(names[0]))
        .expect("deal");
    for (idx, action) in scenario.opening.iter() {
        // Best effort: an out-of-turn action is a legitimate way to reach a
        // position, and its refusal is not the subject of this file.
        let _ = world.player_action(world.actor(ACTORS[*idx]), action.clone());
    }
    if scenario.warm_secs > 0 {
        world.advance(Duration::from_secs(scenario.warm_secs));
    }
    world
}

/// One row of the comparison table.
struct Row {
    scenario: &'static str,
    door: Door,
    actor: &'static str,
    seated: bool,
    reply: String,
    arm_a: Vec<u64>,
    arm_b: Vec<u64>,
    phase_a: GamePhase,
    phase_b: GamePhase,
    /// Did each arm void the hand and hand every stake back?
    voided_a: bool,
    voided_b: bool,
    /// Non-empty when an `Err` reply changed the table anyway.
    err_committed: Option<String>,
}

impl Row {
    /// The universal property: a door may not void a hand the clock plays out.
    fn one_belief(&self) -> bool {
        self.voided_b == self.voided_a
    }

    /// The stronger property, for the doors that are not ordinary game actions.
    fn recipients_match(&self) -> bool {
        if self.seated && self.door.is_an_ordinary_action_for_a_seated_player() {
            return true;
        }
        self.arm_a == self.arm_b && self.phase_a == self.phase_b
    }

    fn ok(&self) -> bool {
        self.one_belief() && self.recipients_match() && self.err_committed.is_none()
    }
}

/// The canister's own testimony that it voided a hand and refunded every stake.
/// Both `UnmovableReason` variants an exit door or the recovery method can
/// produce say `ABANDONED as unmovable`.
fn voided(logs: &[String]) -> bool {
    logs.iter().any(|l| l.contains("ABANDONED as unmovable"))
}

/// Fork one scenario: two worlds, asserted identical at the fork point, then
/// driven down the two arms.
fn compare(scenario: Scenario, door: Door, actor_idx: usize) -> Row {
    let mut world_a = build(scenario);
    let mut world_b = build(scenario);
    let _ = world_a.new_canister_logs(); // drop setup chatter
    let _ = world_b.new_canister_logs();

    // THE WINDOW. Time moves; no round runs; nothing has had a chance to react.
    world_a.advance_time_only(scenario.stall);
    world_b.advance_time_only(scenario.stall);

    let fork_a = per_principal(&world_a);
    let fork_b = per_principal(&world_b);
    let print_a = table_fingerprint(&world_a);
    let print_b = table_fingerprint(&world_b);
    assert_eq!(
        (&fork_a, &print_a),
        (&fork_b, &print_b),
        "[{}] the two arms did not start from the same state, so nothing below means anything",
        scenario.name
    );
    assert!(
        world_a.table_state().phase.hand_in_progress(),
        "[{}] the scenario must fork on a LIVE hand; got {:?}",
        scenario.name,
        world_a.table_state().phase
    );

    // ---- ARM A: the clock alone, zero ingress ----------------------------
    only_blocks(&world_a, 12);
    let arm_a = per_principal(&world_a);
    let phase_a = world_a.table_state().phase;
    let voided_a = voided(&world_a.new_canister_logs());

    // ---- ARM B: one door call at the first instant it can be sent --------
    let who = world_b.actor(ACTORS[actor_idx]);
    let reply = door.knock(&world_b, who);
    // A refusal must have moved NOTHING. Read before the blocks run, so what is
    // measured is the reply's own effect and not the clock's.
    let err_committed = if Door::refused(&reply) {
        let after = table_fingerprint(&world_b);
        let money = per_principal(&world_b);
        if after != print_b || money != fork_b {
            Some(format!("{print_b}\n         ->  {after}"))
        } else {
            None
        }
    } else {
        None
    };
    only_blocks(&world_b, 12);
    let arm_b = per_principal(&world_b);
    let phase_b = world_b.table_state().phase;
    let voided_b = voided(&world_b.new_canister_logs());

    let seated = actor_idx < scenario.seats;
    Row {
        scenario: scenario.name,
        door,
        actor: ACTORS[actor_idx],
        seated,
        reply,
        arm_a: arm_a.values().copied().collect(),
        arm_b: arm_b.values().copied().collect(),
        phase_a,
        phase_b,
        voided_a,
        voided_b,
        err_committed,
    }
}

/// Every scenario the gate forks on.
fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario::base("HU preflop, nobody acted", 2),
        Scenario {
            opening: &[(0, PlayerAction::Call)],
            ..Scenario::base("HU preflop, sb limped", 2)
        },
        Scenario {
            opening: &[(0, PlayerAction::Raise(6_000_000))],
            ..Scenario::base("HU preflop, sb raised", 2)
        },
        Scenario {
            opening: &[(0, PlayerAction::Call), (1, PlayerAction::Check)],
            ..Scenario::base("HU flop, checked to", 2)
        },
        Scenario {
            opening: &[(0, PlayerAction::AllIn)],
            ..Scenario::base("HU preflop, sb all-in", 2)
        },
        Scenario {
            warm_secs: 20,
            ..Scenario::base("HU preflop, clock part-run", 2)
        },
        Scenario {
            stall: SHORT_STALL,
            ..Scenario::base("HU preflop, SHORT stall", 2)
        },
        Scenario::base("3-handed preflop", 3),
        Scenario {
            opening: &[(2, PlayerAction::Call)],
            ..Scenario::base("3-handed, utg called", 3)
        },
        Scenario::base("4-handed preflop", 4),
        Scenario {
            ante: true,
            ..Scenario::base("3-handed with ante", 3)
        },
        Scenario {
            ante: true,
            opening: &[(2, PlayerAction::Raise(8_000_000))],
            ..Scenario::base("3-handed ante, utg raised", 3)
        },
    ]
}

// ===========================================================================
// M13 -- the agreement gate
// ===========================================================================

/// **THE GATE.** Every door, every seat, every scenario: the clock alone and the
/// door call must end with the same principals holding the same e8s.
#[test]
fn m13_every_door_agrees_with_the_clock() {
    let mut rows: Vec<Row> = Vec::new();
    for scenario in scenarios() {
        // The control arm first: if sending nothing disagrees with sending
        // nothing, the harness is not deterministic and no other row can be read.
        rows.push(compare(scenario, Door::Nothing, 0));
        for door in [Door::Abandon, Door::CashOut, Door::LeaveTable] {
            for seat in 0..scenario.seats {
                rows.push(compare(scenario, door, seat));
            }
            // And the principal who has never been at the table, in every
            // scenario -- the caller that convicted the `Err` that commits.
            rows.push(compare(scenario, door, STRANGER));
        }
    }

    eprintln!("\n=== M13 ONE BELIEF: {} states compared ===", rows.len());
    eprintln!(
        "  {:<28} {:<19} {:<9} {:<36} {:<7} {:<3}",
        "scenario", "door", "caller", "reply", "voided", "=="
    );
    for r in rows.iter() {
        eprintln!(
            "  {:<28} {:<19} {:<9} {:<36} {:<7} {}",
            r.scenario,
            r.door.name(),
            r.actor,
            r.reply,
            format!("{}/{}", r.voided_a as u8, r.voided_b as u8),
            if r.ok() { "ok" } else { "**" }
        );
        if !r.ok() {
            eprintln!(
                "      ARM A {:?} phase {:?}\n      ARM B {:?} phase {:?}",
                r.arm_a, r.phase_a, r.arm_b, r.phase_b
            );
        }
        if let Some(ref d) = r.err_committed {
            eprintln!("      Err COMMITTED: {d}");
        }
    }

    let two_beliefs: Vec<&Row> = rows.iter().filter(|r| !r.one_belief()).collect();
    let disagreements: Vec<&Row> = rows.iter().filter(|r| !r.recipients_match()).collect();
    let commits: Vec<&Row> = rows.iter().filter(|r| r.err_committed.is_some()).collect();
    eprintln!(
        "\n  {} states compared\n  {} hands voided by a door the clock would have played out\n  \
         {} recipient disagreements\n  {} Err-replies that changed state\n",
        rows.len(),
        two_beliefs.len(),
        disagreements.len(),
        commits.len()
    );

    assert!(
        commits.is_empty(),
        "\nAN Err THAT COMMITS. {} call(s) replied with an error and changed the table anyway.\n\
         `Err` from an ic-cdk update is an ordinary reply, not a rollback: every refusal must be\n\
         decided BEFORE the first mutation.\n{}\n",
        commits.len(),
        commits
            .iter()
            .map(|r| format!(
                "  {} :: {} by {} -> {}\n      {}",
                r.scenario,
                r.door.name(),
                r.actor,
                r.reply,
                r.err_committed.as_deref().unwrap_or("")
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    assert!(
        two_beliefs.is_empty(),
        "\nTWO BELIEFS about one hand, on {} of {} states.\n\
         A door VOIDED a hand and handed every stake back on a state where the clock, left \n\
         alone, played the hand out and paid a winner -- or the reverse. This is the property \n\
         every surface has to share, and it is checked on the canister's OWN log line.\n{}\n\n\
         See docs/DEFECTS.md E-59 and docs/SECURITY-FINDINGS.md FINDING 25.\n",
        two_beliefs.len(),
        rows.len(),
        two_beliefs
            .iter()
            .map(|r| format!(
                "  {} :: {} by {} -> {}\n      ARM A voided={} {:?} phase {:?}\n      ARM B voided={} {:?} phase {:?}",
                r.scenario,
                r.door.name(),
                r.actor,
                r.reply,
                r.voided_a,
                r.arm_a,
                r.phase_a,
                r.voided_b,
                r.arm_b,
                r.phase_b
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    assert!(
        disagreements.is_empty(),
        "\nDIFFERENT RECIPIENTS from one state, on {} of {} states.\n{}\n\n\
         Both arms conserve exactly. The TOTAL was never the defect: the RECIPIENTS are.\n\
         Every surface that answers \"is this hand stuck?\" must answer it the same way, or the\n\
         canister pays two different sets of people for the same hand depending on who sends a\n\
         message first, and the seat about to be folded out is the one with the incentive.\n\
         See docs/DEFECTS.md E-59 and docs/SECURITY-FINDINGS.md FINDING 25.\n",
        disagreements.len(),
        rows.len(),
        disagreements
            .iter()
            .map(|r| format!(
                "  {} :: {} by {} -> {}\n      ARM A (clock alone) {:?} phase {:?}\n      ARM B (that call)   {:?} phase {:?}",
                r.scenario,
                r.door.name(),
                r.actor,
                r.reply,
                r.arm_a,
                r.phase_a,
                r.arm_b,
                r.phase_b
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ===========================================================================
// M13b -- the surfaces, in the same window
// ===========================================================================

/// **THE SENTENCE THAT STARTED IT.** In the stall window, with the on-chain clock
/// two rounds away from folding the seat that did not act, the canister told that
/// seat:
///
/// ```text
///   0.0100 ICP (1000000 e8s) of yours is still committed to hand 1, and that hand
///   can no longer be moved by any message, so nobody can win it. Call
///   abandon_stuck_hand() -- any principal may -- and every stake goes back to
///   whoever put it in, including yours, into your withdrawable balance.
/// ```
///
/// Every clause after "committed to hand 1" was false, and it was aimed at the
/// player with the incentive to act on it. `get_custody_status` is a QUERY, so it
/// executes no round and cannot be caught by any gate that goes through
/// `World::advance`: it answers from the belief the canister is holding right now.
///
/// This is the cheap half of M13 -- one world, no arms, queries only -- and it is
/// the half that gates the *advice*.
#[test]
fn m13b_no_surface_calls_a_playable_hand_dead() {
    use money_safety::invariants::custody::read_surfaces;
    use candid::{decode_one, Encode};

    let scenario = Scenario::base("HU preflop, nobody acted", 2);
    let world = build(scenario);
    world.advance_time_only(STALL);

    let t = world.table_state();
    assert!(t.phase.hand_in_progress() && t.pot > 0, "fixture must be a live hand with a pot");

    let status = world.stuck_hand_status();
    eprintln!(
        "\n=== M13b SURFACES IN THE WINDOW ===\n  get_stuck_hand_status: is_stuck={} hand_in_progress={} abandonable_in_ns={:?} refundable_pot={}",
        status.is_stuck, status.hand_in_progress, status.abandonable_in_ns, status.refundable_pot
    );
    assert!(
        !status.is_stuck,
        "get_stuck_hand_status says the hand is dead in a window the clock plays out"
    );

    for name in ACTORS.iter().take(scenario.seats) {
        let who = world.actor(name);
        let surfaces = read_surfaces(&world, who);
        let custody = world
            .query_as(who, "get_custody_status", Encode!().unwrap())
            .ok()
            .and_then(|b| {
                decode_one::<money_safety::invariants::custody::CustodyStatus>(&b).ok()
            })
            .expect("get_custody_status must answer");
        eprintln!(
            "  {name}: committed={} stuck={} abandonable_in_ns={:?}\n    advice: {}",
            custody.committed_in_pot,
            custody.committed_is_stuck,
            custody.abandonable_in_ns,
            custody.advice
        );
        assert!(
            custody.committed_in_pot > 0,
            "the fixture must leave every seat with a real stake, or the advice is not exercised"
        );
        assert!(
            !custody.committed_is_stuck,
            "get_custody_status told {name} the hand is dead while the clock was about to play \
             it out"
        );
        assert!(
            !custody.advice.contains("no longer be moved"),
            "THE ADVICE STRING. The canister told {name} to void a playable hand:\n    {}",
            custody.advice
        );
        assert!(
            !custody.advice.contains("nobody can win it"),
            "same sentence, other clause:\n    {}",
            custody.advice
        );
        let view = world
            .query_as(who, "get_table_view", Encode!().unwrap())
            .ok()
            .and_then(|b| {
                decode_one::<Option<money_safety::invariants::custody::CustodyView>>(&b).ok()
            })
            .flatten()
            .expect("get_table_view must carry the custody figures");
        assert!(
            !view.hand_is_unmovable,
            "TableView.hand_is_unmovable is set on a hand the clock plays out, for {name}. This \
             is the flag a client paints \"this hand is dead, recover your money\" from.\n    {}",
            surfaces.log.join("\n    ")
        );
    }

    // And the clock proves the point, with zero ingress.
    only_blocks(&world, 12);
    let after = world.table_state();
    eprintln!("  the clock resolved it with no caller: phase={:?} pot={}\n", after.phase, after.pot);
    assert!(
        !after.phase.hand_in_progress(),
        "the hand every surface was asked about must be one the clock plays out; got {:?}",
        after.phase
    );
}
