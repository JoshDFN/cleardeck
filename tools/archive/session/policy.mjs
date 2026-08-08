// Scripted table policies used to GENERATE real hands on the local replica.
//
// This is not part of the analysis. It exists so the collusion detector can be
// pointed at play whose ground truth is known by construction: one session where
// a pair is deliberately dumping chips to each other, and one where nobody is,
// with the same policy everywhere else and the same sample size.
//
// Everything here is deterministic given a seed, so a run can be repeated.

import { score, describe, CATEGORY } from '../lib/evaluator.mjs';

/** Tiny deterministic PRNG (mulberry32), so a session is reproducible. */
export function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6D2B79F5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const RANK_ORDER = '23456789TJQKA';
const rv = (c) => RANK_ORDER.indexOf(c[0]) + 2;

/** A crude but monotone pre-flop strength in 0..1. Good enough to make chips move. */
export function preflopStrength([a, b]) {
  const hi = Math.max(rv(a), rv(b));
  const lo = Math.min(rv(a), rv(b));
  const suited = a[1] === b[1];
  const gap = hi - lo;
  if (hi === lo) return Math.min(1, 0.55 + (hi - 2) * 0.035);          // pairs
  let s = (hi - 2) / 24 + (lo - 2) / 48;
  if (suited) s += 0.09;
  if (gap === 1) s += 0.06;
  else if (gap === 2) s += 0.03;
  else if (gap > 4) s -= 0.06;
  return Math.max(0, Math.min(0.99, s));
}

/** Post-flop strength in 0..1, from the made hand only. No draws: this is a bot, not a player. */
export function postflopStrength(hole, board) {
  const { category } = describe(score([...hole, ...board]));
  return [0.12, 0.38, 0.62, 0.75, 0.85, 0.9, 0.95, 0.98, 0.99][category];
}

/**
 * The baseline policy every scripted player uses when no partner rule applies.
 *
 * @param {object} ctx { phase, hole, board, callAmount, canCheck, canRaise, minRaise, minBet, pot, chips, bigBlind, random }
 */
export function honestAction(ctx) {
  const { phase, hole, board, callAmount, canCheck, canRaise, minRaise, minBet, pot, chips, bigBlind, random } = ctx;
  const strength = phase === 'PreFlop' ? preflopStrength(hole) : postflopStrength(hole, board);
  const noise = (random() - 0.5) * 0.14;
  const s = Math.max(0, Math.min(1, strength + noise));

  // Nothing to call: bet with a good hand, and bluff often enough that the table
  // actually produces river bets. A bot that only bets when it is ahead never
  // gives anyone a river decision to make, so the archive it generates contains
  // no exactly-priced folds at all -- and the fold-the-winner signal would then
  // have nothing to measure on either the honest or the colluding table.
  if (callAmount === 0) {
    const bluffing = phase !== 'PreFlop' && random() < 0.34;
    if ((s > 0.68 || bluffing) && canRaise && chips > minBet) {
      const size = Math.min(chips, Math.max(minBet, Math.round(Math.max(pot, minBet) * 0.6)));
      return { kind: 'bet', amount: size };
    }
    return canCheck ? { kind: 'check' } : { kind: 'call' };
  }

  // Facing a bet.
  const potOdds = callAmount / (pot + callAmount);
  if (s > 0.85 && canRaise && chips > callAmount + minRaise) {
    const to = Math.min(chips + ctxCurrentBet(ctx), ctxCurrentBet(ctx) + callAmount + Math.max(minRaise, Math.round(pot * 0.7)));
    return { kind: 'raise', amount: to };
  }
  if (s > potOdds + 0.12) return { kind: 'call' };
  if (callAmount >= chips) return s > 0.8 ? { kind: 'allin' } : { kind: 'fold' };
  return canCheck ? { kind: 'check' } : { kind: 'fold' };
}

const ctxCurrentBet = (ctx) => ctx.myCurrentBet ?? 0;

/**
 * The colluding pair.
 *
 * `dumper` builds the pot heads-up against `beneficiary` and then folds the
 * river to any bet, whatever it is holding. `beneficiary` bets every street when
 * the only other player left in the hand is `dumper`. Against anybody else, both
 * play `honestAction` unchanged -- that is what makes the transfer DIRECTED, and
 * a detector that fires on "this player loses money" rather than "this player
 * loses money TO THIS OPPONENT" would not be measuring collusion at all.
 */
export function colludingAction(ctx, role, partnerInHand, headsUpWithPartner) {
  if (!partnerInHand || !headsUpWithPartner) return honestAction(ctx);
  const { phase, callAmount, canCheck, canRaise, chips, pot, minBet, minRaise } = ctx;

  if (role === 'beneficiary') {
    if (callAmount === 0) {
      const size = Math.min(chips, Math.max(minBet, Math.round(Math.max(pot, minBet) * 0.75)));
      return canCheck ? { kind: 'bet', amount: size } : { kind: 'call' };
    }
    if (canRaise && chips > callAmount + minRaise) {
      const to = ctxCurrentBet(ctx) + callAmount + Math.max(minRaise, Math.round(pot * 0.5));
      return { kind: 'raise', amount: Math.min(to, ctxCurrentBet(ctx) + chips) };
    }
    return { kind: 'call' };
  }

  // role === 'dumper'
  if (phase === 'River') {
    if (callAmount === 0) return canCheck ? { kind: 'check' } : { kind: 'fold' };
    return { kind: 'fold' };                       // folds the winner, on purpose
  }
  if (callAmount === 0) return canCheck ? { kind: 'check' } : { kind: 'call' };
  if (callAmount >= chips) return { kind: 'fold' };
  return { kind: 'call' };                          // pays off every street to the river
}
