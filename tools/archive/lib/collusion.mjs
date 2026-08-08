// Collusion SIGNALS. Not verdicts, and never an accusation.
//
// WHY THIS IS THE HONEST THING TO SHIP ALONGSIDE BOTS
//
// Automation is not what hurts a poker player. Two people sharing hole cards at
// one table is, and so is one account quietly handing chips to another. A room
// that is about to allow bots owes its players the detector, and the detector has
// to be one an outsider can run, or it is just another thing to take on trust.
//
// THE RULE THIS FILE OBEYS
//
// A FALSE ACCUSATION IS WORSE THAN A MISSED ONE. So:
//   * every test states its sample size and refuses below it;
//   * every test is corrected for the number of pairs examined, because looking
//     at 40 pairs and reporting the most extreme one at p<0.05 finds "collusion"
//     in honest play roughly twice out of every three attempts;
//   * a test that CANNOT produce a significant answer given the shape of the data
//     says so, instead of reporting a reassuring "no signal" it never had the
//     power to earn;
//   * nothing in the output says a person cheated. The strongest verdict is
//     "REVIEW", and it is a request to go and look, addressed to a human.
//
// THREE SIGNALS, in increasing order of how much they mean.

import {
  bootstrapMeanCI, bootstrapMeanPValue, benjaminiHochberg, binomialRightTail,
  detectableEffect, fisherRightTail, mean, minimumAttainableP, samplesNeeded, wilson,
} from './statistics.mjs';

/** Sample gates. Below these the tool refuses to report a p-value at all. */
export const GATES = {
  /** Hands two players must share before a co-occurrence figure means anything. */
  MIN_COOCCURRENCE: 100,
  /** Distinct players a population needs before "who sits with whom" carries information. */
  MIN_DISTINCT_PLAYERS: 6,
  /** Hands a directed pair must have played before their chip flow is tested. */
  MIN_PAIR_HANDS: 150,
  /** Hands in the baseline pool (the same player against everyone else). */
  MIN_BASELINE_HANDS: 150,
  /** Closed-counterfactual folds needed before a fold-the-winner rate is tested. */
  MIN_CLOSED_FOLDS: 40,
  /** Population folds needed before the weaker population baseline may stand in. */
  MIN_POPULATION_FOLDS: 200,
  /** Corrected significance threshold. */
  ALPHA: 0.05,
};

const key = (a, b) => `${a}|${b}`;

/**
 * Turns the analysed hands into the per-hand facts the signals need.
 *
 * @param {Array<{hand:object, replay:object, recon:object, ev:object|null}>} usable
 */
export function buildLedger(usable) {
  const hands = [];
  for (const { hand, replay, ev } of usable) {
    // EVERY participant, not just the seats the deal fed. A chair can change hands
    // inside one hand, and money that went in under the previous occupant is still
    // in the pot. Building the chip-flow ledger from the dealt seats alone would
    // drop that money: the totals would still be right and the ATTRIBUTION would
    // be short by exactly one person, which is how a transfer goes unseen.
    const players = hand.players.map((p) => ({
      seat: p.seat,
      principal: p.principal,
      contributed: p.contributed ?? replay.contributed.get(p.seat) ?? 0,
      won: p.amountWon,
      net: p.amountWon - (p.contributed ?? replay.contributed.get(p.seat) ?? 0),
    }));
    const drift = players.reduce((a, p) => a + p.net, 0);
    if (drift !== 0) {
      // Cannot happen once the money checks have passed, and if it ever does the
      // hand must not silently contribute a lopsided transfer to somebody's tally.
      continue;
    }

    // CHIP TRANSFER ATTRIBUTION, stated so it can be argued with.
    //
    // In one hand the losers' chips go to the winners; which loser paid which
    // winner is not a physical fact when three or more people are in the pot, so
    // a rule is needed. This one splits each loser's loss across the winners in
    // proportion to what each winner netted. It conserves chips exactly
    // (sum over winners of transfer(L->W) == L's loss) and is symmetric in the
    // winners, which is the most that can be claimed for it.
    const winners = players.filter((p) => p.net > 0);
    const losers = players.filter((p) => p.net < 0);
    const totalGain = winners.reduce((a, p) => a + p.net, 0);
    const transfers = [];
    if (totalGain > 0) {
      for (const l of losers) {
        for (const w of winners) {
          transfers.push({ from: l.principal, to: w.principal, amount: (-l.net) * (w.net / totalGain) });
        }
      }
    }

    // Folds whose price is EXACT, and who benefited.
    const closedFolds = [];
    if (ev?.usable) {
      for (const d of ev.decisions) {
        if (!d.foldPrice) continue;
        const beneficiaries = d.foldPrice.opponents.map((o) => replay.principalOfSeat.get(o.seat));
        closedFolds.push({
          folder: replay.principalOfSeat.get(d.seat),
          beneficiaries,
          delta: d.foldPrice.delta,
          foldedTheWinner: d.foldPrice.heroScore >= d.foldPrice.bestScore,
          street: d.street,
          bigBlind: ev.bigBlind,
        });
      }
    }

    hands.push({
      handId: hand.handId,
      tableId: hand.tableId,
      bigBlind: hand.bigBlind || 1,
      players,
      // Who was IN the hand, for "do these two sit down together": somebody who
      // bought an empty chair after the deal did not play this hand.
      principals: hand.dealtIn.map((d) => d.principal),
      transfers,
      closedFolds,
    });
  }
  return hands;
}

// ---------------------------------------------------------------------------
// SIGNAL 1 -- seat co-occurrence beyond chance
// ---------------------------------------------------------------------------

/**
 * Do these two sit down together more often than their own hand counts explain?
 *
 * THE TRAP THIS FUNCTION EXISTS TO AVOID. At a two-seat table everybody who plays
 * co-occurs with their opponent in 100% of their hands, by construction, and a
 * naive test reports every pair as astronomically significant. The same happens,
 * more subtly, in any population where the table is a large fraction of the
 * player pool. So before any p-value is computed the function asks whether the
 * test COULD distinguish anything here, and says NO POWER when it could not.
 */
export function coOccurrence(ledger) {
  const total = ledger.length;
  const seen = new Map();
  const pairCount = new Map();
  const perTable = new Map();

  for (const h of ledger) {
    const uniq = [...new Set(h.principals)];
    for (const p of uniq) seen.set(p, (seen.get(p) ?? 0) + 1);
    for (let i = 0; i < uniq.length; i += 1) {
      for (let j = i + 1; j < uniq.length; j += 1) {
        const [a, b] = [uniq[i], uniq[j]].sort();
        pairCount.set(key(a, b), (pairCount.get(key(a, b)) ?? 0) + 1);
      }
    }
    perTable.set(h.tableId, (perTable.get(h.tableId) ?? 0) + 1);
  }

  const players = [...seen.keys()];
  const maxTableSize = Math.max(0, ...ledger.map((h) => new Set(h.principals).size));

  // Do people MOVE? Co-occurrence is a statement about choosing to sit together.
  // If every player only ever appears at one table, who sits with whom was decided
  // by the roster and not by anybody, and the test measures the roster.
  const tablesOf = new Map();
  for (const h of ledger) {
    for (const p of new Set(h.principals)) {
      if (!tablesOf.has(p)) tablesOf.set(p, new Set());
      tablesOf.get(p).add(h.tableId);
    }
  }
  const movers = [...tablesOf.values()].filter((s) => s.size > 1).length;

  const structural = [];
  if (movers < 3 || movers / Math.max(1, players.length) < 0.5) {
    structural.push(
      `only ${movers} of ${players.length} players in this sample ever appear at more than one ` +
      'table, so who sits with whom is fixed by the roster rather than chosen. Every pair at a ' +
      'fixed table co-occurs constantly, which this test would read as conspiracy',
    );
  }
  if (players.length < GATES.MIN_DISTINCT_PLAYERS) {
    structural.push(
      `only ${players.length} distinct players in this sample; who-sits-with-whom carries no ` +
      `information below ${GATES.MIN_DISTINCT_PLAYERS}`,
    );
  }
  if (maxTableSize >= players.length) {
    structural.push(
      `every player in this sample can be at the table at once (largest hand had ${maxTableSize} ` +
      `players, ${players.length} players exist), so co-occurrence is forced, not chosen`,
    );
  }

  const rows = [];
  for (const [k, both] of pairCount) {
    const [a, b] = k.split('|');
    const aTotal = seen.get(a);
    const bTotal = seen.get(b);
    const expected = (aTotal * bTotal) / total;
    const minP = minimumAttainableP(aTotal, bTotal, total);
    const row = {
      a, b, both, aTotal, bTotal, total, expected,
      ratio: expected > 0 ? both / expected : null,
      p: null, q: null, verdict: 'INSUFFICIENT DATA', why: [],
    };
    if (structural.length > 0) { row.verdict = 'NO POWER'; row.why = [...structural]; rows.push(row); continue; }
    if (both < GATES.MIN_COOCCURRENCE) {
      row.why.push(`${both} shared hands is below the ${GATES.MIN_COOCCURRENCE}-hand floor`);
      rows.push(row); continue;
    }
    if (minP > GATES.ALPHA / Math.max(1, pairCount.size)) {
      row.verdict = 'NO POWER';
      row.why.push(
        `even perfect co-occurrence would only reach p=${minP.toExponential(2)}, which cannot clear ` +
        'the corrected threshold for this many pairs',
      );
      rows.push(row); continue;
    }
    row.p = fisherRightTail(both, aTotal, bTotal, total);
    rows.push(row);
  }

  const tested = rows.filter((r) => r.p !== null);
  const q = benjaminiHochberg(tested.map((r) => r.p));
  tested.forEach((r, i) => {
    r.q = q[i];
    r.verdict = r.q < GATES.ALPHA ? 'REVIEW' : 'NO SIGNAL';
  });

  return { rows: rows.sort((x, y) => (y.ratio ?? 0) - (x.ratio ?? 0)), structural, players: players.length, total };
}

// ---------------------------------------------------------------------------
// SIGNAL 2 -- directed chip transfer against the player's OWN baseline
// ---------------------------------------------------------------------------

/**
 * Does A lose to B faster than A loses to everybody else?
 *
 * The comparison is against A's OWN baseline, not against the field's, because
 * "this player loses money" is not collusion and a detector that fires on it will
 * spend its life accusing bad players. What collusion looks like is a loss that
 * is DIRECTED: normal against every other opponent, anomalous against one.
 *
 * The null is built by resampling A's per-hand flows against their other
 * opponents. No distribution is assumed: per-hand poker results are mostly zero
 * with occasional whole stacks, and a t-test on that shape invents significance.
 */
export function directedTransfer(ledger, { iters = 20_000, seed = 4242 } = {}) {
  const series = new Map();       // "A|B" -> per-hand net flow A->B, in big blinds
  const coHands = new Map();      // "A|B" -> hands both were dealt in

  for (const h of ledger) {
    const uniq = [...new Set(h.principals)];
    const flow = new Map();
    for (const t of h.transfers) {
      flow.set(key(t.from, t.to), (flow.get(key(t.from, t.to)) ?? 0) + t.amount);
    }
    for (const a of uniq) {
      for (const b of uniq) {
        if (a === b) continue;
        const net = ((flow.get(key(a, b)) ?? 0) - (flow.get(key(b, a)) ?? 0)) / h.bigBlind;
        if (!series.has(key(a, b))) series.set(key(a, b), []);
        series.get(key(a, b)).push(net);
        coHands.set(key(a, b), (coHands.get(key(a, b)) ?? 0) + 1);
      }
    }
  }

  const opponentsOf = new Map();
  for (const k of series.keys()) {
    const [a, b] = k.split('|');
    if (!opponentsOf.has(a)) opponentsOf.set(a, []);
    opponentsOf.get(a).push(b);
  }

  const rows = [];
  for (const [k, sample] of series) {
    const [a, b] = k.split('|');
    const baseline = [];
    let baselineOpponents = 0;
    for (const c of opponentsOf.get(a) ?? []) {
      if (c === b) continue;
      const s = series.get(key(a, c)) ?? [];
      if (s.length === 0) continue;
      baseline.push(...s);
      baselineOpponents += 1;
    }
    const row = {
      from: a, to: b, hands: sample.length, baselineHands: baseline.length,
      baselineOpponents,
      meanBBPerHand: mean(sample), baselineBBPerHand: mean(baseline),
      excessBBPer100: null, ci: null, p: null, q: null,
      verdict: 'INSUFFICIENT DATA', why: [], caveat: null, detectable: null, needed: null,
    };
    if (baselineOpponents === 1) {
      // THE BASELINE CAN BE THE COLLUSION.
      //
      // Found on real data, by this tool, on a table it was pointed at: at a
      // three-handed table the "own baseline" IS a single opponent, so when A
      // dumps to B, B's baseline (which is B-against-A) is enormously positive
      // for B -- and B then looks like it is leaking to the innocent third
      // player. The dumper's row and a completely spurious row both reach REVIEW,
      // and nothing in the numbers says which is which.
      //
      // The verdict is not suppressed, because on a bigger pool the same
      // arithmetic is sound. It is labelled, so it cannot be read as clean.
      row.caveat = 'this player has only ONE other opponent, so the "baseline" is a single '
        + 'relationship. If that relationship is itself part of a pair, the baseline is '
        + 'contaminated and this row can be an artifact of somebody else\'s collusion. Three or '
        + 'more distinct opponents are needed before this comparison stands on its own';
    }
    if (sample.length < GATES.MIN_PAIR_HANDS) {
      row.why.push(`${sample.length} shared hands is below the ${GATES.MIN_PAIR_HANDS}-hand floor`);
      rows.push(row); continue;
    }
    if (baseline.length < GATES.MIN_BASELINE_HANDS) {
      row.why.push(
        `${a.slice(0, 5)}… has only ${baseline.length} hands against other opponents, below the ` +
        `${GATES.MIN_BASELINE_HANDS}-hand floor, so there is no baseline to compare against`,
      );
      rows.push(row); continue;
    }
    row.excessBBPer100 = (mean(sample) - mean(baseline)) * 100;
    row.ci = bootstrapMeanCI(sample, { seed });
    row.p = bootstrapMeanPValue(sample, baseline, { iters, seed });
    row.detectable = detectableEffect(baseline, sample.length);
    rows.push(row);
  }

  const tested = rows.filter((r) => r.p !== null);
  const q = benjaminiHochberg(tested.map((r) => r.p));
  tested.forEach((r, i) => {
    r.q = q[i];
    if (r.q < GATES.ALPHA) r.verdict = 'REVIEW';
    else {
      r.verdict = 'NO SIGNAL';
      r.needed = r.detectable !== null
        ? `this sample could have detected a directed loss of ${(r.detectable * 100).toFixed(1)} BB/100 or more`
        : null;
    }
  });

  return rows.sort((x, y) => (y.excessBBPer100 ?? -Infinity) - (x.excessBBPer100 ?? -Infinity));
}

// ---------------------------------------------------------------------------
// SIGNAL 3 -- folding the winner, to one specific opponent
// ---------------------------------------------------------------------------

/**
 * The signal an operator normally has and nobody else does -- and here anybody
 * has it, because the folded cards are reconstructible from the seed.
 *
 * A good player wins by betting when they are ahead. They do not win by their
 * opponent folding the best hand at an anomalous rate. So "how often did A fold a
 * hand that would have won, and who was on the other side of it" separates skill
 * from a chip dump in a way pure chip flow cannot.
 *
 * Only folds whose price is EXACT are counted (see lib/ev.mjs): the ones where
 * calling would have closed the action, so what would have happened is not a
 * model, it is the cards.
 */
export function foldsToOpponent(ledger) {
  const byPair = new Map();      // "folder|beneficiary" -> {folds, foldedWinner, chips, bb}
  const byFolder = new Map();    // folder -> same, over everyone

  const bump = (map, k, f) => {
    if (!map.has(k)) map.set(k, { folds: 0, foldedWinner: 0, chipsForgone: 0, bbForgone: 0 });
    f(map.get(k));
  };

  for (const h of ledger) {
    for (const f of h.closedFolds) {
      // A fold with more than one beneficiary is not attributable; skip it rather
      // than split a binary event.
      if (f.beneficiaries.length !== 1) continue;
      const b = f.beneficiaries[0];
      bump(byPair, key(f.folder, b), (o) => {
        o.folds += 1;
        if (f.foldedTheWinner) o.foldedWinner += 1;
        o.chipsForgone += Math.max(0, f.delta);
        o.bbForgone += Math.max(0, f.delta) / (f.bigBlind || 1);
      });
      bump(byFolder, f.folder, (o) => {
        o.folds += 1;
        if (f.foldedTheWinner) o.foldedWinner += 1;
        o.chipsForgone += Math.max(0, f.delta);
        o.bbForgone += Math.max(0, f.delta) / (f.bigBlind || 1);
      });
    }
  }

  const population = { folds: 0, foldedWinner: 0 };
  for (const v of byFolder.values()) { population.folds += v.folds; population.foldedWinner += v.foldedWinner; }

  const rows = [];
  for (const [k, v] of byPair) {
    const [folder, beneficiary] = k.split('|');
    const all = byFolder.get(folder);
    const others = {
      folds: all.folds - v.folds,
      foldedWinner: all.foldedWinner - v.foldedWinner,
    };
    const row = {
      folder, beneficiary,
      folds: v.folds, foldedWinner: v.foldedWinner,
      rate: wilson(v.foldedWinner, v.folds),
      baselineFolds: others.folds, baselineFoldedWinner: others.foldedWinner,
      baselineRate: wilson(others.foldedWinner, others.folds),
      bbForgone: v.bbForgone,
      // DESCRIPTIVE, with no p-value attached. When a player's exactly-priced
      // folds are almost all against one opponent, that concentration is striking
      // and it is also not a test: it can be an artifact of who they happened to
      // reach the river with. It is printed because withholding a plain count in
      // the name of rigour is its own kind of dishonesty.
      shareOfForgone: all.bbForgone > 0 ? v.bbForgone / all.bbForgone : null,
      shareOfFolds: all.folds > 0 ? v.folds / all.folds : null,
      allFolds: all.folds,
      baselineSource: null, caveat: null,
      p: null, q: null, verdict: 'INSUFFICIENT DATA', why: [],
    };
    if (v.folds < GATES.MIN_CLOSED_FOLDS) {
      row.why.push(`${v.folds} exactly-priced folds is below the ${GATES.MIN_CLOSED_FOLDS} floor`);
      rows.push(row); continue;
    }

    // PREFERRED: the folder's own rate against everybody else. Directed, and
    // immune to "this player is just loose".
    if (others.folds >= GATES.MIN_CLOSED_FOLDS) {
      row.baselineSource = 'own';
      row.p = binomialRightTail(v.foldedWinner, v.folds, Math.max(others.foldedWinner / others.folds, 1e-6));
      rows.push(row); continue;
    }

    // FALLBACK: the whole population's rate. This is a WEAKER claim and the row
    // says so: a player who folds the winner unusually often against everyone
    // would look the same, and with a thin own-baseline there is no way to tell
    // the two apart from this signal alone.
    const pool = { folds: population.folds - all.folds, foldedWinner: population.foldedWinner - all.foldedWinner };
    if (pool.folds >= GATES.MIN_POPULATION_FOLDS) {
      row.baselineSource = 'population';
      row.baselineFolds = pool.folds;
      row.baselineFoldedWinner = pool.foldedWinner;
      row.baselineRate = wilson(pool.foldedWinner, pool.folds);
      row.caveat = `compared against the population (${pool.folds} folds), not this player's own record: `
        + `they have only ${others.folds} exactly-priced folds against anybody else. A player who folds `
        + 'the winner too often against EVERYONE would look identical here';
      row.p = binomialRightTail(v.foldedWinner, v.folds, Math.max(pool.foldedWinner / pool.folds, 1e-6));
      rows.push(row); continue;
    }

    row.why.push(
      `only ${others.folds} exactly-priced folds against other opponents and only ${pool.folds} in the `
      + 'rest of the population, so there is no baseline to compare against',
    );
    rows.push(row);
  }

  const tested = rows.filter((r) => r.p !== null);
  const q = benjaminiHochberg(tested.map((r) => r.p));
  tested.forEach((r, i) => {
    r.q = q[i];
    r.verdict = r.q < GATES.ALPHA ? 'REVIEW' : 'NO SIGNAL';
  });
  return rows.sort((x, y) => (y.bbForgone ?? 0) - (x.bbForgone ?? 0));
}

/**
 * How many shared hands a pair would need before the chip-flow test could see a
 * transfer of `effectBBPer100`, given the variance this population actually has.
 * Printed next to every refusal, so "not enough data" comes with a number.
 */
export function handsNeededFor(ledger, effectBBPer100 = 5) {
  const pooled = [];
  for (const h of ledger) {
    const uniq = [...new Set(h.principals)];
    const flow = new Map();
    for (const t of h.transfers) flow.set(key(t.from, t.to), (flow.get(key(t.from, t.to)) ?? 0) + t.amount);
    for (const a of uniq) {
      for (const b of uniq) {
        if (a === b) continue;
        pooled.push((((flow.get(key(a, b)) ?? 0) - (flow.get(key(b, a)) ?? 0)) / h.bigBlind));
      }
    }
  }
  return samplesNeeded(pooled, effectBBPer100 / 100);
}
