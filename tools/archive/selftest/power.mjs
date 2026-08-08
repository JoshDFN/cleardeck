#!/usr/bin/env node
// HOW MANY HANDS A COLLUSION SIGNAL NEEDS BEFORE IT MEANS ANYTHING.
//
// "Not enough data" is only an honest answer if it comes with a number, so this
// script produces the numbers docs/ARCHIVE.md quotes. It does two things:
//
//   1. Measures the per-hand variance of directed chip flow -- from a real
//      bundle if one is given, otherwise from a synthetic population -- and
//      turns it into "hands needed" for a range of leak sizes, analytically.
//   2. Checks that analytic answer against a simulation at a couple of points,
//      because a power formula nobody has tested against its own detector is
//      just another claim.
//
//   node tools/archive/selftest/power.mjs [bundle.json]

import fs from 'node:fs';
import { parseBundle } from '../lib/bundle.mjs';
import { analyse } from '../lib/analyse.mjs';
import { buildLedger, GATES } from '../lib/collusion.mjs';
import { analyticPower, bootstrapMeanPValue, mean, samplesNeeded, rng } from '../lib/statistics.mjs';
import { syntheticPopulation } from './synthetic.mjs';

const bundlePath = process.argv[2] ?? null;

function flowsFrom(ledger) {
  const out = [];
  for (const h of ledger) {
    const uniq = [...new Set(h.principals)];
    const flow = new Map();
    for (const t of h.transfers) {
      const k = `${t.from}|${t.to}`;
      flow.set(k, (flow.get(k) ?? 0) + t.amount);
    }
    for (const a of uniq) {
      for (const b of uniq) {
        if (a === b) continue;
        out.push((((flow.get(`${a}|${b}`) ?? 0) - (flow.get(`${b}|${a}`) ?? 0)) / h.bigBlind));
      }
    }
  }
  return out;
}

let flows;
let source;
if (bundlePath) {
  const bundle = parseBundle(JSON.parse(fs.readFileSync(bundlePath, 'utf8')));
  const { usable } = analyse(bundle);
  flows = flowsFrom(buildLedger(usable));
  source = `${bundlePath} (${usable.length} usable hands)`;
} else {
  flows = flowsFrom(syntheticPopulation({ players: 8, tables: 2, hands: 8000, seed: 1 }));
  source = 'a synthetic population (no bundle given)';
}

const mu = mean(flows);
const sd = Math.sqrt(flows.reduce((a, b) => a + (b - mu) ** 2, 0) / (flows.length - 1));

console.log('DIRECTED CHIP FLOW: HOW BIG A SAMPLE THE TEST NEEDS');
console.log('===================================================\n');
console.log(`per-hand directed flow measured from ${source}`);
console.log(`  observations         ${flows.length}`);
console.log(`  mean                 ${mu.toFixed(4)} bb/hand  (must be ~0: chips are conserved)`);
console.log(`  standard deviation   ${sd.toFixed(2)} bb/hand`);
console.log('\nOne hand of poker is almost all noise. That standard deviation is why a');
console.log('co-occurrence or chip-flow number over 20 hands is worth exactly nothing, and');
console.log('why this tool refuses to print one.\n');

const PAIRS = 30;                       // ordered pairs in a six-player pool
const CORRECTED = 0.05 / PAIRS;         // the threshold the detector actually applies

console.log('hands two players must share before the test has 80% power.');
console.log('  "at 5%" is the textbook number. "corrected" is the threshold this tool ACTUALLY');
console.log(`  operates at once ${PAIRS} pairs have been examined, and it is the honest column.`);
console.log('  leak (bb/100)   at 5%      corrected');
console.log('  -------------   --------   ---------');
for (const bbPer100 of [1, 2, 5, 10, 25, 50, 100]) {
  const naive = samplesNeeded(flows, bbPer100 / 100);
  const corrected = samplesNeeded(flows, bbPer100 / 100, { alpha: CORRECTED });
  console.log(`  ${String(bbPer100).padStart(13)}   ${String(naive).padStart(8)}   ${String(corrected).padStart(9)}`);
}
console.log(`\nThe tool's own floor is ${GATES.MIN_PAIR_HANDS} shared hands, which is the point below which it`);
console.log('refuses to report at all. Read the table: that floor is enough to see a gross chip');
console.log('dump and nowhere near enough to see a subtle one. The tool says which, every time.');

// --- does the formula agree with the detector? -----------------------------

function detectionRate({ nPair, effectBBPerHand, trials = 30, iters = 1500, pairs = 30 }) {
  const rand = rng(4711);
  let detected = 0;
  const alpha = GATES.ALPHA / pairs;               // Bonferroni, conservative vs BH
  for (let t = 0; t < trials; t += 1) {
    const sample = [];
    const baseline = [];
    for (let i = 0; i < nPair; i += 1) {
      sample.push(flows[Math.floor(rand() * flows.length)] + effectBBPerHand);
    }
    for (let i = 0; i < Math.max(nPair, 400); i += 1) {
      baseline.push(flows[Math.floor(rand() * flows.length)]);
    }
    const p = bootstrapMeanPValue(sample, baseline, { iters, seed: 1000 + t });
    if (p < alpha) detected += 1;
  }
  return detected / trials;
}

console.log('\nsimulated check of that table, against the shipped test');
console.log('  (30 trials each, Bonferroni threshold for 30 ordered pairs)');
console.log('  hands   leak bb/100   the table predicts   the shipped test achieved');
console.log('  -----   -----------   ------------------   -------------------------');
const cells = [[150, 0], [150, 100], [800, 100], [800, 0], [3200, 25], [3200, 0]];
for (const [nPair, bbPer100] of cells) {
  const rate = detectionRate({ nPair, effectBBPerHand: bbPer100 / 100 });
  const predicted = analyticPower(flows, bbPer100 / 100, nPair, { alpha: CORRECTED });
  console.log(`  ${String(nPair).padStart(5)}   ${String(bbPer100).padStart(11)}   `
    + `${`${(predicted * 100).toFixed(0)}%`.padStart(18)}   ${`${(rate * 100).toFixed(0)}%`.padStart(25)}`);
}
console.log('\nThe rows with a leak of 0 are the false-positive rate. They should sit near zero,');
console.log('and they are the only reason the other rows are worth reading.');
console.log('\nCompare the last two columns rather than trusting either. The table is a normal');
console.log('approximation; the shipped test is a bootstrap on a heavy-tailed quantity, and the');
console.log('heavier the tail of the data you point this at, the further the test falls SHORT of');
console.log('the table. Where they diverge, believe the simulation and treat "hands needed" as a');
console.log('floor.');
