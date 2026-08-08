// Synthetic populations whose ground truth is set by construction.
//
// These exist for ONE reason: to measure the detector's error rates. A collusion
// signal that has never been pointed at play known to be honest has no false
// positive rate, and a signal with no known false positive rate is not evidence
// of anything. The real local sessions show the detector working on real
// canister hands; these show what it does across many independent worlds, which
// no amount of real play in one afternoon can.
//
// The output has exactly the shape lib/collusion.mjs consumes, so the detector
// under test is the shipped one, not a copy.

import { rng } from '../lib/statistics.mjs';

/** Zero-centred, heavy-tailed per-hand result in big blinds: mostly nothing, sometimes a stack. */
function drawResult(rand) {
  const u = rand();
  if (u < 0.55) return 0;                                  // folded, or won/lost the blinds
  if (u < 0.93) return (rand() - 0.5) * 24;                // a normal contested pot
  return (rand() - 0.5) * 260;                             // a stack goes in
}

/**
 * @param {object} o
 * @param {number} o.players distinct players in the pool
 * @param {number} o.tables distinct tables
 * @param {number} o.hands hands to generate
 * @param {number} o.seed
 * @param {number} [o.tableSize] players dealt into each hand
 * @param {boolean} [o.gluedPair] P00 and P01 never sit without each other
 * @param {number} [o.dumpBBPerHand] mean big blinds P00 hands to P01 per shared hand
 * @returns {Array<object>} a ledger in the shape lib/collusion.mjs consumes
 */
export function syntheticPopulation({
  players, tables, hands, seed, tableSize = 4, gluedPair = false, dumpBBPerHand = 0,
}) {
  const rand = rng(seed);
  const names = Array.from({ length: players }, (_, i) => `P${String(i).padStart(2, '0')}`);
  const size = Math.min(tableSize, players);
  const ledger = [];

  for (let h = 0; h < hands; h += 1) {
    const tableId = `T${Math.floor(rand() * tables)}`;

    // Seat selection.
    const pool = names.slice();
    for (let i = 0; i < pool.length; i += 1) {
      const j = i + Math.floor(rand() * (pool.length - i));
      const t = pool[i]; pool[i] = pool[j]; pool[j] = t;
    }
    let chosen = pool.slice(0, size);
    if (gluedPair) {
      const hasA = chosen.includes('P00');
      const hasB = chosen.includes('P01');
      if (hasA !== hasB) {
        // Whoever is missing replaces someone else, so the pair is never split.
        const missing = hasA ? 'P01' : 'P00';
        const victim = chosen.find((c) => c !== 'P00' && c !== 'P01');
        chosen = chosen.map((c) => (c === victim ? missing : c));
      }
    }

    // Results, forced to sum to zero so chips are conserved.
    const nets = chosen.map(() => drawResult(rand));
    if (dumpBBPerHand > 0 && chosen.includes('P00') && chosen.includes('P01')) {
      const d = dumpBBPerHand * 2 * rand();
      nets[chosen.indexOf('P00')] -= d;
      nets[chosen.indexOf('P01')] += d;
    }
    const drift = nets.reduce((a, b) => a + b, 0) / nets.length;
    const balanced = nets.map((n) => n - drift);

    const playersOut = chosen.map((principal, i) => ({
      seat: i, principal, contributed: Math.max(0, -balanced[i]), won: Math.max(0, balanced[i]), net: balanced[i],
    }));

    // The same attribution rule lib/collusion.mjs documents.
    const winners = playersOut.filter((p) => p.net > 0);
    const losers = playersOut.filter((p) => p.net < 0);
    const totalGain = winners.reduce((a, p) => a + p.net, 0);
    const transfers = [];
    if (totalGain > 0) {
      for (const l of losers) {
        for (const w of winners) {
          transfers.push({ from: l.principal, to: w.principal, amount: (-l.net) * (w.net / totalGain) });
        }
      }
    }

    ledger.push({
      handId: h + 1, tableId, bigBlind: 1,
      players: playersOut, principals: chosen, transfers, closedFolds: [],
    });
  }
  return ledger;
}
