/**
 * Pure derivations over ONE hand record, so `HandHistory.svelte` renders and
 * this file reasons.
 *
 * WHY THIS FILE EXISTS
 * --------------------
 * The replayer's action log used to lose its A/B against PokerNow's session log
 * badly: nought of five action lines carried an amount, and blinds were absent
 * entirely. The data was never missing. `ActionRecord` in
 * `src/table_canister/src/lib.rs` has carried
 *
 *     phase: String    // "preflop" | "flop" | "turn" | "river"
 *     amount: u64      // the actual amount, including Call and AllIn
 *
 * since before wave 1, and the canister fills both for every action it records.
 * What dropped them was the CANDID INTERFACE: `src/table_canister/table_canister.did`
 * and the generated `src/declarations/table_1/table_1.did.js` both declared
 * `ActionRecord` with only `{ action; seat; timestamp }`, so the agent decoded
 * three fields off a five-field record and the client could not have read the
 * other two however it was written. See docs/DEFECTS.md H-32.
 *
 * WHAT IS DERIVED HERE AND WHAT IS NOT
 * ------------------------------------
 * Everything below is a restatement of fields the canister actually returned.
 * The two things the hand record does NOT contain are called out rather than
 * guessed:
 *
 *   1. BLIND POSTS. `CURRENT_ACTIONS` only ever receives voluntary actions, so
 *      no record exists of who posted what. The LEVEL comes from the table's
 *      config (or, when the history canister holds the hand, from that hand's
 *      own `small_blind` / `big_blind` / `ante`), and it is labelled with which.
 *      The SEATS are attributed only from the table's live dealer button, and
 *      only while that button still points at the hand being replayed.
 *   2. HOW MANY SEATS WERE DEALT IN. Not recorded either; the shuffle verifier
 *      solves for it and says when it could not.
 */

/** `ActionRecord.phase` -> the street name a player would recognise. */
const STREET_LABEL = {
  preflop: 'Pre-flop',
  flop: 'Flop',
  turn: 'Turn',
  river: 'River',
};

/** Street order for the log, blinds first because that is when they are posted. */
export const STREET_ORDER = ['Pre-flop', 'Flop', 'Turn', 'River'];

/** Community cards on the table by the END of a street. */
export const BOARD_AFTER = { Blinds: 0, 'Pre-flop': 0, Flop: 3, Turn: 4, River: 5 };

/** The unrecorded-street bucket, for records written before `phase` existed. */
export const STREET_UNKNOWN = 'Street not recorded';

/** @param {unknown} v */
function numberOrNull(v) {
  if (typeof v === 'bigint') return Number(v);
  if (typeof v === 'number' && Number.isFinite(v)) return v;
  return null;
}

/** @param {unknown} phase @returns {string|null} */
export function streetLabelOf(phase) {
  if (typeof phase !== 'string' || phase.length === 0) return null;
  return STREET_LABEL[phase.trim().toLowerCase()] ?? null;
}

/**
 * Whether an action's recorded `amount` is the chips that action ADDED to the
 * pot, or the total the street was raised TO.
 *
 * This distinction is the whole reason the pot audit below can be trusted:
 *   Call    the canister stores `current_bet - player_current_bet`, an INCREMENT.
 *   Bet     legal only when the street has no bet yet, so the actor had nothing
 *           in for this street and the amount is also the increment.
 *   Raise   the amount is the total the street is raised TO. What the raiser
 *           ADDED is that total less whatever they already had in, which the
 *           record does not carry.
 *   AllIn   the amount is the player's final `current_bet` for the street, i.e.
 *           a TO-amount as well.
 *   Fold/Check  no money.
 *
 * @param {string} kind
 * @returns {'increment'|'street-total'|'none'}
 */
export function amountMeaningOf(kind) {
  if (kind === 'Call' || kind === 'Bet') return 'increment';
  if (kind === 'Raise' || kind === 'AllIn') return 'street-total';
  return 'none';
}

const ACTION_WORD = {
  Fold: 'folds',
  Check: 'checks',
  Call: 'calls',
  Bet: 'bets',
  Raise: 'raises to',
  AllIn: 'is all in for',
  PostBlind: 'posts a blind of',
};

/** The verb for an action, never inventing one for a variant we do not know. */
export function actionWord(kind) {
  return ACTION_WORD[kind] || String(kind || 'acted').toLowerCase();
}

/**
 * `{Bet: 500n}` / `{Fold: null}` + the record's own `phase`/`amount` fields ->
 * one flat action.
 *
 * `amount` is read from the RECORD first and from the variant payload only as a
 * fallback, because the record field is the one the canister fills for every
 * action: `Call` and `AllIn` carry no payload at all, which is precisely why
 * every call and every all-in used to render with no amount.
 *
 * @param {any} record one `ActionRecord`
 */
export function normalizeAction(record) {
  const variant = record?.action;
  const kind = variant && typeof variant === 'object' ? Object.keys(variant)[0] : null;
  const fromVariant = numberOrNull(kind ? variant[kind] : null);
  const fromRecord = numberOrNull(record?.amount);
  const amount = fromRecord !== null && fromRecord > 0
    ? fromRecord
    : (fromVariant !== null && fromVariant > 0 ? fromVariant : null);
  const rawPhase = typeof record?.phase === 'string' ? record.phase : null;
  return {
    kind: kind || 'Unknown',
    amount,
    amountFrom: amount === null ? null : (fromRecord !== null && fromRecord > 0 ? 'record.amount' : 'variant payload'),
    amountMeaning: amount === null ? 'none' : amountMeaningOf(kind),
    seat: Number(record?.seat ?? 0),
    timestamp: record?.timestamp ?? 0n,
    rawPhase,
    street: streetLabelOf(rawPhase),
  };
}

/**
 * The blind level and, where the record supports it, the seats that posted.
 *
 * @param {object} args
 * @param {{handNumber:number, blinds?:{small:number,big:number,ante:number}|null}} args.hand
 * @param {{config?:{smallBlind:number,bigBlind:number,ante:number}|null,
 *          blindSeats?:{sb:number,bb:number,dealer:number}|null,
 *          liveHandNumber?:number|null, settled?:boolean}|null} args.tableFacts
 */
export function blindsForHand({ hand, tableFacts }) {
  const own = hand?.blinds || null;
  const cfg = tableFacts?.config || null;
  const level = own
    ? { ...own }
    : (cfg ? { small: cfg.smallBlind, big: cfg.bigBlind, ante: cfg.ante } : null);
  const levelSource = own
    ? "this hand's own record"
    : (cfg ? "the table's configured level" : null);

  // The dealer button is only evidence about THIS hand while it still points at
  // it: `start_new_hand` rotates it, so the moment a later hand begins the live
  // view says nothing about who posted the blinds in this one.
  const seatsUsable = Boolean(
    tableFacts?.blindSeats
    && tableFacts.settled
    && tableFacts.liveHandNumber === hand?.handNumber,
  );
  return {
    level,
    levelSource,
    seats: seatsUsable ? { sb: tableFacts.blindSeats.sb, bb: tableFacts.blindSeats.bb } : null,
    seatSource: seatsUsable
      ? "the table's dealer button, which has not moved off this hand yet"
      : null,
  };
}

/**
 * The action log as street groups, blinds included.
 *
 * @param {object} args
 * @param {{actions:Array<object>}} args.hand
 * @param {ReturnType<typeof blindsForHand>} args.blinds
 * @returns {Array<{street:string, boardAfter:number|null, lines:Array<object>}>}
 */
export function logGroupsFor({ hand, blinds }) {
  const groups = [];
  let index = 0;
  const push = (street, line) => {
    index += 1;
    const group = groups.find((g) => g.street === street);
    const entry = { ...line, index, street };
    if (group) group.lines.push(entry);
    else groups.push({ street, boardAfter: BOARD_AFTER[street] ?? null, lines: [entry] });
    return entry;
  };

  if (blinds?.level) {
    const seatOf = (which) => (blinds.seats ? blinds.seats[which] : null);
    push('Blinds', {
      kind: 'PostBlind',
      seat: seatOf('sb'),
      word: 'posts the small blind',
      amount: blinds.level.small,
      amountMeaning: 'increment',
      timestamp: null,
      reconstructed: true,
    });
    push('Blinds', {
      kind: 'PostBlind',
      seat: seatOf('bb'),
      word: 'posts the big blind',
      amount: blinds.level.big,
      amountMeaning: 'increment',
      timestamp: null,
      reconstructed: true,
    });
    if (blinds.level.ante > 0) {
      push('Blinds', {
        kind: 'PostAnte',
        seat: null,
        word: 'ante, per player dealt in',
        amount: blinds.level.ante,
        amountMeaning: 'increment',
        timestamp: null,
        reconstructed: true,
      });
    }
  }

  for (const action of hand?.actions || []) {
    push(action.street || STREET_UNKNOWN, {
      kind: action.kind,
      seat: action.seat,
      word: actionWord(action.kind),
      amount: action.amount,
      amountMeaning: action.amountMeaning,
      amountFrom: action.amountFrom,
      timestamp: action.timestamp,
      reconstructed: false,
    });
  }

  // Streets in playing order, with any unrecorded-street bucket last.
  const rank = (street) => {
    if (street === 'Blinds') return -1;
    const at = STREET_ORDER.indexOf(street);
    return at === -1 ? STREET_ORDER.length : at;
  };
  return groups.sort((a, b) => rank(a.street) - rank(b.street));
}

/**
 * Does the log account for the whole pot?
 *
 * When every amount in the log is an INCREMENT (see `amountMeaningOf`), the
 * blinds plus those amounts must equal the pot the canister paid out, to the
 * e8. That makes the log self-checking: a missing line or a wrong amount shows
 * up as a difference. When the log contains a raise or an all-in it does not
 * hold, because those amounts are street totals, and the audit says so instead
 * of quietly summing the wrong numbers.
 *
 * An ante also disqualifies it: the record does not say how many players were
 * dealt in, so how much ante reached the pot is not derivable.
 *
 * @param {object} args
 * @param {{actions:Array<object>, potTotal:number}} args.hand
 * @param {ReturnType<typeof blindsForHand>} args.blinds
 */
export function potAudit({ hand, blinds }) {
  const level = blinds?.level || null;
  const streetTotals = (hand?.actions || []).filter((a) => a.amountMeaning === 'street-total');
  if (!level) {
    return { checkable: false, reason: 'the blind level for this hand is not known here' };
  }
  // A HAND STILL IN PLAY HAS NO POT TO CHECK AGAINST. The table writes a hand's
  // record when the hand STARTS, so the replayer can be opened on a record with
  // actions and no winners. Summing against a zero pot there would print
  // "the amounts come to 0.03 ICP but the table paid out 0.00 ICP. One of the two
  // is wrong" about a hand that is simply not finished: a false accusation, and
  // the exact class of over-claim this component is being cleaned of.
  if (!(hand?.winners || []).length) {
    return {
      checkable: false,
      reason: 'this hand has not been settled yet, so there is no pot for the amounts to '
        + 'be checked against',
    };
  }
  if (level.ante > 0) {
    return {
      checkable: false,
      reason: 'this table charges an ante, and the record does not say how many '
        + 'players were dealt in, so the ante that reached the pot is not derivable',
    };
  }
  if (streetTotals.length > 0) {
    const kinds = [...new Set(streetTotals.map((a) => a.kind.toLowerCase()))].join(' and ');
    return {
      checkable: false,
      reason: `the log contains ${streetTotals.length} ${kinds} action(s), whose recorded `
        + 'amount is the total the street was raised TO rather than the chips that action '
        + 'added, so these amounts cannot be summed',
    };
  }
  const actions = (hand?.actions || []).reduce((sum, a) => sum + (a.amount || 0), 0);
  const sum = level.small + level.big + actions;
  const pot = Number(hand?.potTotal || 0);
  return {
    checkable: true,
    sum,
    pot,
    match: sum === pot,
    difference: sum - pot,
  };
}

/** How far a hand got, from the board the record carries. */
export function reachedStreet(communityCount) {
  const n = Number(communityCount) || 0;
  if (n >= 5) return 'River';
  if (n === 4) return 'Turn';
  if (n >= 3) return 'Flop';
  return 'Pre-flop';
}
