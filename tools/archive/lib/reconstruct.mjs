// Reconstruct one archived hand from its seed, and CHECK the reconstruction
// against every card the record published.
//
// The output is deliberately a pile of intermediate values rather than a
// verdict: a reconstruction that only says "OK" is an assertion, and an
// assertion is the thing this tool exists to replace. Every card carries the
// deck index it came from, so a reader can follow SHUFFLE-SPEC section 4 with a
// finger and land on the same card.
//
// The rule the whole tool hangs off: IF THE RECONSTRUCTION DISAGREES WITH THE
// RECORD ANYWHERE, THE HAND IS UNUSABLE. Nothing downstream -- statistics, EV,
// collusion -- may consume a hand whose cards did not check out. `status` is the
// gate, and every consumer must look at it.

import { checkCommitment, seedBytes, shuffle } from './spec-shuffle.mjs';
import { boardDepth, deal } from './spec-deal.mjs';
import { deckSymbols } from './cards.mjs';

/** @typedef {'VERIFIED'|'UNVERIFIABLE'|'MISMATCH'} ReconStatus */

/**
 * @param {object} hand a validated record from lib/bundle.mjs
 * @param {{trace?: boolean, nonVacuity?: boolean}} [opts]
 */
export function reconstructHand(hand, { trace = false, nonVacuity = true } = {}) {
  const notes = [];
  const problems = [];

  // ---- Step 0: does the revealed seed hash to the commitment? ---------------
  const commitment = checkCommitment(hand.shuffleProof.seedHash, hand.shuffleProof.revealedSeed);
  if (!commitment.ok) {
    return {
      handId: hand.handId,
      handNumber: hand.handNumber,
      tableId: hand.tableId,
      // A broken commitment is not the same failure as a missing seed. Only one
      // of the two is an accusation, so they must not share a status.
      status: hand.shuffleProof.revealedSeed.trim() === '' ? 'UNVERIFIABLE' : 'MISMATCH',
      commitment,
      problems: commitment.problems,
      notes,
      checks: [],
    };
  }

  // ---- Step 1: the deck ----------------------------------------------------
  const seed = seedBytes(hand.shuffleProof.revealedSeed);
  const { deck, rejections, steps } = shuffle(seed, { trace });

  // ---- Step 2: P, READ from the record, never counted ----------------------
  if (hand.dealtIn === null) {
    notes.push(
      'dealt_in is null: this record predates the field that states P. SHUFFLE-SPEC section 4 ' +
      'says not to guess -- the hole cards would match at any P while the board silently came out ' +
      'wrong. Board not reconstructed.',
    );
    return {
      handId: hand.handId,
      handNumber: hand.handNumber,
      tableId: hand.tableId,
      status: 'UNVERIFIABLE',
      commitment,
      deck,
      rejections,
      deckSymbols: deckSymbols(deck),
      problems: ['the record does not state dealt_in, so P is unknown'],
      notes,
      checks: [],
    };
  }
  const P = hand.dealtIn.length;
  if (P < 2) {
    problems.push(`dealt_in states P = ${P}, which cannot be a hand`);
  }

  const layout = P >= 2 ? deal(deck, P) : null;

  // ---- Step 3: every card the record published, checked --------------------
  const checks = [];
  const seatOf = new Map(); // "seat|principal" -> deal index k
  if (layout) {
    hand.dealtIn.forEach((d, k) => seatOf.set(`${d.seat}|${d.principal}`, k));
  }

  const depth = boardDepth(hand);
  if (layout) {
    if (depth >= 3) {
      hand.flop.forEach((card, k) => checks.push({
        what: `flop[${k}]`,
        deckIndex: layout.indices.flop[k],
        derived: layout.flop[k],
        recorded: card,
        agrees: layout.flop[k] === card,
      }));
    }
    if (depth >= 4) {
      checks.push({
        what: 'turn', deckIndex: layout.indices.turn,
        derived: layout.turn, recorded: hand.turn, agrees: layout.turn === hand.turn,
      });
    }
    if (depth >= 5) {
      checks.push({
        what: 'river', deckIndex: layout.indices.river,
        derived: layout.river, recorded: hand.river, agrees: layout.river === hand.river,
      });
    }
  }

  // Hole cards. Only showdown players publish them; the rest are DERIVED, which
  // is the whole asset -- see `derived` below.
  const derived = [];
  if (layout) {
    for (const p of hand.players) {
      const k = seatOf.get(`${p.seat}|${p.principal}`);
      if (k === undefined) {
        if (p.dealtIn === true) {
          problems.push(
            `player seat ${p.seat} ${short(p.principal)} is marked dealt_in but is not in the ` +
            'hand\'s dealt_in list, so the record contradicts itself',
          );
        }
        continue;
      }
      const [a, b] = layout.holeCards[k];
      if (p.holeCards) {
        checks.push({
          what: `hole seat ${p.seat} ${short(p.principal)} card 1`,
          deckIndex: layout.indices.hole[k][0],
          derived: a, recorded: p.holeCards[0], agrees: a === p.holeCards[0],
        });
        checks.push({
          what: `hole seat ${p.seat} ${short(p.principal)} card 2`,
          deckIndex: layout.indices.hole[k][1],
          derived: b, recorded: p.holeCards[1], agrees: b === p.holeCards[1],
        });
      } else {
        derived.push({
          seat: p.seat, principal: p.principal, dealIndex: k,
          cards: [a, b], deckIndices: layout.indices.hole[k],
          // Why the record does not have them, so nobody reads a derived card as
          // a published one.
          reason: hand.wentToShowdown ? 'folded or left before the showdown' : 'no showdown',
        });
      }
    }
  }

  // Every seat the deal fed, whether or not it appears in `players`.
  const holeCardsByDealIndex = layout
    ? hand.dealtIn.map((d, k) => ({
      dealIndex: k, seat: d.seat, principal: d.principal,
      cards: layout.holeCards[k], deckIndices: layout.indices.hole[k],
    }))
    : [];

  // IS THE DEAL ORDER PINNED ON THIS HAND?
  //
  // The reconstruction's central product -- the hole cards of somebody who
  // folded -- is the one thing in the output with no cross-check: there is no
  // published card to compare it against. It rests entirely on the mapping
  // `dealt_in[k] -> deck[2k], deck[2k+1]`. If that mapping were permuted, a hand
  // where only ONE seat showed would still verify perfectly and every folded hand
  // would come out wrong, silently.
  //
  // Two or more published hole-card pairs pin the order, because they land on
  // different deck positions and a permutation moves at least one of them. So the
  // hand says which it is instead of implying a check it did not do. (The live
  // check is `tools/archive/session/verify-live.mjs`, which watches the deal.)
  const publishedPairs = checks.filter((c) => c.what.startsWith('hole') && c.what.endsWith('card 1')).length;
  const dealOrderPinned = publishedPairs >= 2;

  const disagreements = checks.filter((c) => !c.agrees);
  for (const d of disagreements) {
    problems.push(`${d.what}: reconstructed ${d.derived}, record says ${d.recorded}`);
  }

  // ---- Step 4: the check must not be vacuous -------------------------------
  // A verifier that would have said VERIFIED for the wrong P has checked nothing.
  // So state, for this hand, what the neighbouring P would have produced.
  let nonVacuityReport = null;
  if (nonVacuity && layout && depth >= 3) {
    nonVacuityReport = [];
    for (const alt of [P - 1, P + 1]) {
      if (alt < 2 || 2 * alt + 8 > deck.length) continue;
      const altLayout = deal(deck, alt);
      const altBoard = [...altLayout.flop, altLayout.turn, altLayout.river].slice(0, depth);
      const realBoard = [...layout.flop, layout.turn, layout.river].slice(0, depth);
      nonVacuityReport.push({
        P: alt,
        board: altBoard,
        differs: altBoard.some((c, i) => c !== realBoard[i]),
      });
    }
    if (nonVacuityReport.length > 0 && nonVacuityReport.every((r) => !r.differs)) {
      notes.push(
        'WARNING: the neighbouring values of P give the SAME board on this hand, so the board ' +
        'check cannot distinguish the right P from the wrong one here. The hole-card checks still ' +
        'bind; the board check does not.',
      );
    }
  }

  const status = problems.length > 0
    ? (disagreements.length > 0 ? 'MISMATCH' : 'UNVERIFIABLE')
    : 'VERIFIED';

  return {
    handId: hand.handId,
    handNumber: hand.handNumber,
    tableId: hand.tableId,
    status,
    commitment,
    P,
    deck,
    deckSymbols: deckSymbols(deck),
    rejections,
    steps: trace ? steps : null,
    layout,
    boardDepth: depth,
    checks,
    derivedHoleCards: derived,
    holeCardsByDealIndex,
    publishedPairs,
    dealOrderPinned,
    nonVacuity: nonVacuityReport,
    problems,
    notes,
  };
}

export const short = (p) => (typeof p === 'string' && p.length > 12 ? `${p.slice(0, 5)}…${p.slice(-3)}` : String(p));

/** Reconstructs a whole bundle. Never throws on a bad hand: it reports one. */
export function reconstructAll(bundle, opts = {}) {
  return bundle.hands.map((h) => {
    try {
      return reconstructHand(h, opts);
    } catch (e) {
      return {
        handId: h.handId, handNumber: h.handNumber, tableId: h.tableId,
        status: 'UNVERIFIABLE', problems: [`reconstruction threw: ${e.message}`],
        notes: [], checks: [],
      };
    }
  });
}
