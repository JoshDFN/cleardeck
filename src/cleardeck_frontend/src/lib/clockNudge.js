// WHEN SHOULD A BROWSER TAB ASK THE CANISTER TO ADVANCE ITS CLOCK?
//
// ===========================================================================
// THE DEFECT THIS EXISTS FOR (docs/DEFECTS.md E-92)
// ===========================================================================
//
// `+page.svelte` used to answer that question with "twice a second, forever":
//
//     const POLL_INTERVAL = 500;
//     pollInterval = setInterval(loadTableState, POLL_INTERVAL);
//
//     async function loadTableState() {
//       const timeoutResult = await tableActor.check_timeouts();   // FIRST statement
//
// `check_timeouts` is `#[ic_cdk::update]` (`lib.rs`) and carries no `query` in
// `table_canister.did`. So the RENDER RATE was driving an UPDATE LOOP, one per
// open browser tab, and it was the single largest per-tab cost of running this
// game, larger than the heartbeat by more than an order of magnitude, and
// counted by no cycles figure anywhere in this repository.
//
// Measured on the local replica (`tools/cycles/tab-burn.mjs`, method in that
// file's header), against a control run with no tabs open:
//
//     see docs/DEFECTS.md E-92 for the table this policy is measured against.
//
// ===========================================================================
// WHAT SEPARATES "READ THE TABLE" FROM "ADVANCE THE CLOCK"
// ===========================================================================
//
// Reading the table is a QUERY (`get_table_view`, `get_shuffle_proof`) and it is
// what the render rate is for. It costs the canister nothing per call and it can
// run as fast as the eye needs.
//
// Advancing the clock is an UPDATE, it changes state, and the rate it needs is
// the rate at which DEADLINES ARRIVE, which has nothing to do with how often a
// browser repaints. The canister already advances its own clock on an on-chain
// timer (`schedule_next_wake` arms a precise wake at the next deadline, with a
// 30-second watchdog behind it). So a tab is a BACKSTOP, not the engine, and it
// only has to fire when:
//
//   1. AN ACTION CLOCK HAS EXPIRED and the hand still has not moved. Ordinarily
//      the on-chain timer resolves this. If it does not, the timer is late or
//      lost, and a client is the only thing that can rescue the table.
//
//   2. THE TABLE IS BETWEEN HANDS WITH TWO OR MORE PLAYERS WHO WOULD BE DEALT IN.
//      This one is load-bearing rather than a backstop: the on-chain clock
//      deliberately does NOT deal hands by itself (dealing to seats whose clients
//      are gone would post their blinds hand after hand), and `advance_table_clock`
//      does not arm a wake for `auto_deal_at` when the table could actually deal,
//      because crossing it would change nothing. So without a client asking,
//      `AutoDealReady` is never returned to anybody and no hand ever starts.
//
//   3. THE HAND CANNOT BE MOVED. `check_timeouts` is permissionless and each call
//      counts as one stall opportunity given and failed; three across a grace
//      period is what makes `abandon_stuck_hand` reachable when the timer is dead
//      (see `note_stall_opportunity` in `lib.rs`). A tab that stops asking
//      entirely would rebuild the fund lock FINDING 15 describes.
//
// Everything else is the canister's own timer's job, and paying for it twice a
// second per tab bought nothing.
//
// ===========================================================================
// THE RATE LIMIT IS THE POINT, NOT THE CONDITION
// ===========================================================================
//
// A purely conditional policy is still an unbounded loop: a table that is
// permanently between hands because `start_new_hand` keeps failing is "due"
// forever. So every decision passes through a floor, and the floor WIDENS when a
// call changes nothing:
//
//     gap starts at MIN_GAP_MS (2 s) and DOUBLES, to MAX_GAP_MS (30 s), every
//     time two consecutive nudges see the table in the same state. It snaps back
//     to MIN_GAP_MS the moment the table actually moves.
//
// The worst case a tab can reach is therefore one update per MAX_GAP_MS, which is
// 2,880 calls a day, against 43,200 to 172,800 for the loop this replaces. The
// normal case is two or three calls per hand.
//
// `MAX_GAP_MS` is 30 s because that is `CLOCK_WATCHDOG_SECS` in the canister: if
// the tab is a backstop for a lost timer, firing faster than the timer's own
// watchdog buys nothing, and firing slower would make a tab a worse rescuer than
// the thing it is backing up.
//
// ===========================================================================
// PURE, SO IT CAN BE GATED WITHOUT A REPLICA
// ===========================================================================
//
// Nothing in this file calls a canister. `decide()` is a function of a decoded
// `TableView` and a clock; `observe()` is a function of what came back. That is
// what lets `tools/shots/test-clock-nudge.mjs` simulate a full day of table
// states offline and assert an upper bound on the number of update calls a tab
// can emit, an assertion that is meaningless if the policy can only be exercised
// against a live replica.

/** Every tunable in one frozen object, so a caller cannot drift from the gate. */
export const CLOCK_NUDGE = Object.freeze({
    /**
     * How often the driver ASKS the policy.
     *
     * EQUAL TO `MIN_GAP_MS`, AND THAT IS A GATE, NOT A COINCIDENCE.
     * `tools/shots/test-poll-updates.mjs` refuses any repeating timer whose
     * period is below 2 s and which can reach an update method. A driver that
     * ticked faster than its own floor would be exactly that shape, a fast
     * update loop with a runtime excuse, and a runtime excuse is not something a
     * static reader of this tree can check. So the timer's period and the
     * policy's floor are the same number, and the shape of the code carries the
     * bound rather than only the logic inside it.
     *
     * A CONSEQUENCE, MEASURED AND KEPT: because a 2 s timer's tick usually lands
     * a millisecond or two SHORT of 2 s since the previous call, roughly every
     * other tick is refused by the floor, and the effective cadence on a
     * permanently-due table is one call per 2 to 4 seconds rather than one per 2.
     * Measured on the local replica at 30 calls in 120 s, not 60
     * (`artifacts/cycles/fixed-max-1tab.json`). That is the conservative
     * direction, fewer calls, a slightly longer pause between hands, and it is
     * left alone rather than "corrected" with a tolerance, because a tolerance is
     * a floor that can be argued with.
     */
    TICK_MS: 2_000,
    /**
     * The floor. Never more than one `check_timeouts` per 2 s per tab, whatever
     * the table looks like. Two seconds is roughly one mainnet update finality,
     * so a tighter floor could not produce a second call anyway.
     */
    MIN_GAP_MS: 2_000,
    /**
     * The ceiling the backoff walks up to: `CLOCK_WATCHDOG_SECS` in the canister.
     * A tab backing up a lost timer has no reason to be faster than the timer's
     * own watchdog.
     */
    MAX_GAP_MS: 30_000,
    /**
     * The unconditional backstop, used ONLY when at least one player is seated.
     * Its job is the case where the tab's read of the table is wrong or stale,
     * including a view that failed to decode, so "nothing looks due" is not
     * allowed to mean "never call again". An empty table gets nothing at all: a
     * table with no players has no deadline anybody is waiting on.
     */
    BACKSTOP_MS: 60_000,
});

/** Phases in which a hand is NOT in progress and the next one can be dealt. */
const BETWEEN_HANDS = new Set(['WaitingForPlayers', 'HandComplete']);

/**
 * Candid variants decode as single-key objects (`{ HandComplete: null }`).
 * @param {object|null|undefined} variant
 * @returns {string|null}
 */
export function variantName(variant) {
    if (!variant || typeof variant !== 'object') return null;
    const keys = Object.keys(variant);
    return keys.length === 1 ? keys[0] : null;
}

/** Candid `opt t` decodes as `[]` or `[value]`; a bare value is tolerated. */
function optOf(v) {
    if (Array.isArray(v)) return v.length ? v[0] : null;
    return v === undefined ? null : v;
}

/**
 * Seats that `will_be_dealt_in` would count: Active, with chips.
 *
 * Mirrors `will_be_dealt_in` in `lib.rs`, `status == Active && chips > 0`, and
 * it is a *lower bound on when to ask*, not a decision the client acts on alone.
 * If this ever disagrees with the canister the only consequence is that the tab
 * asks slightly more or less often; the canister still decides whether a hand
 * starts. That is why a client-side copy of the rule is acceptable here and would
 * not be acceptable in a payout path.
 *
 * @param {object|null} view decoded TableView
 * @returns {number}
 */
export function dealtInCount(view) {
    if (!view || !Array.isArray(view.players)) return 0;
    let n = 0;
    for (const slot of view.players) {
        const p = Array.isArray(slot) ? (slot.length ? slot[0] : null) : slot;
        if (!p) continue;
        if (variantName(p.status) !== 'Active') continue;
        let chips;
        try { chips = BigInt(p.chips ?? 0); } catch { chips = 0n; }
        if (chips > 0n) n += 1;
    }
    return n;
}

/**
 * Is there a deadline this tab could usefully help cross?
 *
 * @param {object|null} view decoded TableView, or null if none has decoded yet
 * @returns {{due: boolean, reason: string}}
 */
export function clockIsDue(view) {
    if (!view) return { due: false, reason: 'no view' };

    // 3. A hand nothing can move. Every call is one counted stall opportunity,
    //    and three across the grace period is what unlocks the refund.
    if (view.hand_is_unmovable === true) {
        return { due: true, reason: 'hand is unmovable: counting stall opportunities' };
    }

    const phase = variantName(view.phase);

    // 1. An action clock at zero on a hand that is still in progress. The
    //    on-chain timer normally resolves this before a tab notices; when it has
    //    not, the timer is late and the tab is the rescue.
    if (phase && !BETWEEN_HANDS.has(phase)) {
        const remaining = optOf(view.time_remaining_secs);
        if (remaining !== null && Number(remaining) <= 0) {
            return { due: true, reason: 'action clock expired' };
        }
        return { due: false, reason: 'hand in progress, clock running' };
    }

    // 2. Between hands with a dealable table. Nothing on chain will return
    //    AutoDealReady to anybody unless somebody asks.
    if (phase && BETWEEN_HANDS.has(phase) && dealtInCount(view) >= 2) {
        return { due: true, reason: 'between hands, two or more dealt-in seats' };
    }

    return { due: false, reason: 'nothing due' };
}

/**
 * A compact fingerprint of everything a `check_timeouts` call could change.
 *
 * Two consecutive nudges that see the SAME fingerprint mean the calls are not
 * moving the table, which is what widens the gap. Deliberately coarse: it must
 * not include anything that changes on its own (a countdown, a clock reading),
 * or the backoff would never engage on exactly the table it exists for.
 *
 * @param {object|null} view
 * @returns {string}
 */
export function tableFingerprint(view) {
    if (!view) return 'none';
    const seats = Array.isArray(view.players)
        ? view.players.map((slot) => {
            const p = Array.isArray(slot) ? (slot.length ? slot[0] : null) : slot;
            if (!p) return '-';
            return `${variantName(p.status) || '?'}:${p.chips ?? 0}`;
        }).join(',')
        : '';
    return [
        view.hand_number ?? 0,
        variantName(view.phase) || '?',
        view.action_on ?? 0,
        view.pot ?? 0,
        view.hand_is_unmovable === true ? 'stuck' : 'ok',
        seats,
    ].join('|');
}

/**
 * The per-tab decision, with its floor and its backoff.
 *
 * Usage, per tab:
 *
 *     const policy = new ClockNudgePolicy();
 *     // every CLOCK_NUDGE.TICK_MS:
 *     const d = policy.decide(latestView, Date.now());
 *     if (d.call) {
 *         const result = await actor.check_timeouts();
 *         policy.observe(latestView, result, Date.now());
 *     }
 *
 * `observe` MUST be called after every call `decide` authorised, including a
 * failed one, a policy that only learns from successes cannot back off on a
 * canister that is refusing, which is the one time backing off matters most.
 */
export class ClockNudgePolicy {
    constructor(opts = {}) {
        this.minGap = opts.minGap ?? CLOCK_NUDGE.MIN_GAP_MS;
        this.maxGap = opts.maxGap ?? CLOCK_NUDGE.MAX_GAP_MS;
        this.backstop = opts.backstop ?? CLOCK_NUDGE.BACKSTOP_MS;
        this.gap = this.minGap;
        this.lastCallAt = null;
        this.lastFingerprint = null;
        /** Counters, for the offline simulation gate and for the burn harness. */
        this.calls = 0;
        this.lastReason = 'never asked';
    }

    /**
     * @param {object|null} view the most recent decoded TableView
     * @param {number} now Date.now()
     * @returns {{call: boolean, reason: string}}
     */
    decide(view, now) {
        const since = this.lastCallAt === null ? Infinity : now - this.lastCallAt;
        const { due, reason } = clockIsDue(view);

        if (due) {
            if (since < this.gap) {
                this.lastReason = `due (${reason}) but floored: ${since}ms of ${this.gap}ms`;
                return { call: false, reason: this.lastReason };
            }
            this.lastReason = reason;
            return { call: true, reason };
        }

        // NOT due. The only thing that still fires is the slow backstop, and only
        // when somebody is sitting at this table. A view that failed to decode
        // reads as "no view" here, which is exactly when a backstop is wanted:
        // "I could not tell" must not become "so I stopped calling".
        const seated = view === null
            // UNKNOWN IS NOT EMPTY (docs/DEFECTS.md T-48). This read
            // `this.lastCallAt !== null`, which sounds like "we have seen a table
            // before, so keep a pulse" and is not: `lastCallAt` is only set when a
            // call is MADE, so a tab that has never had a decodable view has never
            // called, is never seated, never reaches the backstop, and emits ZERO
            // `check_timeouts` for as long as it stays open. Measured: 0 calls in a
            // synthetic day. The pre-split client called `check_timeouts` as the
            // FIRST statement of the 500 ms poll, before `get_table_view`, so it
            // kept firing at 2/s exactly when the view could not be read.
            //
            // `check_timeouts` is what records `note_stall_opportunity`, and three
            // of those across the grace period is what makes `abandon_stuck_hand`
            // reachable when the on-chain timer is dead. A client that goes silent
            // precisely when it cannot read the table is FINDING 15's fund lock
            // rebuilt on the client side, which is the one thing the header of this
            // module says it must not do. So an unreadable view counts as seated:
            // the cost is the 60 s backstop, 1,440 calls/day worst case.
            ? true
            : (Array.isArray(view.players)
                ? view.players.some((s) => (Array.isArray(s) ? s.length > 0 : Boolean(s)))
                : false);
        if (seated && since >= this.backstop) {
            this.lastReason = 'backstop: nothing looked due for a minute';
            return { call: true, reason: this.lastReason };
        }
        this.lastReason = reason;
        return { call: false, reason };
    }

    /**
     * Record what a call did, and widen or reset the floor.
     *
     * @param {object|null} viewAtCall the view the decision was made from
     * @param {object|null} result decoded TimeoutCheckResult, or null if it threw
     * @param {number} now Date.now()
     */
    observe(viewAtCall, result, now) {
        this.calls += 1;
        this.lastCallAt = now;
        this.lastResult = variantName(result);
        const fp = tableFingerprint(viewAtCall);

        // THE ONLY THING THAT RESETS THE FLOOR IS THE TABLE ACTUALLY MOVING.
        //
        // The tempting alternative is to reset whenever the reply is *actionable*
        //, `AutoDealReady`, `PlayerTimedOut`, on the grounds that the call did
        // work. That is wrong, and measurably so: a table jammed between hands
        // returns `AutoDealReady` to every single call while `start_new_hand`
        // keeps failing, so "actionable" would hold the gap at its floor for as
        // long as the jam lasts. Simulated over four jammed hours
        // (`tools/shots/test-clock-nudge.mjs`) that was 7,200 calls against 480.
        //
        // A deal that really happened changes the fingerprint, a new hand
        // number, a new phase, moved chips, so the honest signal is already
        // there and does not need a second, more optimistic one.
        if (fp !== this.lastFingerprint) {
            this.gap = this.minGap;
        } else {
            this.gap = Math.min(this.gap * 2, this.maxGap);
        }
        this.lastFingerprint = fp;
    }
}

/**
 * The most calls one tab can emit in `ms` milliseconds, whatever the table does.
 *
 * The hard ceiling this policy cannot exceed: one call per `MIN_GAP_MS`, and the
 * driver in `+page.svelte` ticks at the same period, so the measured rate is
 * lower again (see `TICK_MS`). `tools/shots/test-clock-nudge.mjs` asserts the
 * SIMULATED figure against a cycles budget, which is much tighter than this;
 * this is the bound that holds even if every assumption in that simulation is
 * wrong.
 *
 * @param {number} ms
 * @returns {number}
 */
export function maxCallsIn(ms) {
    return Math.ceil(ms / CLOCK_NUDGE.MIN_GAP_MS);
}
