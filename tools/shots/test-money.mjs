// Self-check for the money parser: node tools/shots/test-money.mjs
// Every case here is a shape the app really renders (see lib/money.mjs).
import { checkFigure, checkPlainNumber } from './lib/money.mjs';

const cases = [
  // [label, chain e8s, dom text, expect agrees, expect discriminates2x]
  ['pot exact 0.30 ICP', 30_000_000, '0.30', true, true],
  ['pot 2x displayed', 30_000_000, '0.60', false, true],
  ['pot half displayed', 30_000_000, '0.15', false, true],
  ['stack 11.85', 1_185_000_000, '11.85', true, true],
  ['stack wrong seat', 1_185_000_000, '18.00', false, true],
  ['blind 0.05', 5_000_000, '0.05', true, true],
  ['tiny 0.0010', 100_000, '0.0010', true, true],
  ['K form 1.5K ICP', 150_000_000_000, '1.5K', true, true],
  ['balance with unit', 1_200_000_000, '12.00 ICP', true, true],
  ['zero shown as --', 0, '--', true, false],
  ['nonzero but absent', 10_000_000, null, false, false],
  ['coarse 0.01 vs 0.02 -> caught', 1_000_000, '0.02', false, true],
  ['0.01 blind is NOT flagged (doubling detectable)', 1_000_000, '0.01', true, true],
];
let fails = 0;
for (const [label, chain, text, wantAgree, wantDisc] of cases) {
  const r = checkFigure(label, chain, text, { currency: 'ICP', allowAbsentWhenZero: true });
  const ok = r.agrees === wantAgree && r.discriminates2x === wantDisc;
  if (!ok) fails++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}: agrees=${r.agrees} disc2x=${r.discriminates2x} :: ${r.detail}`);
}

// a deliberately blind case: a figure so small the 2dp display cannot tell x from 2x
const blind = checkFigure('blind check', 100_000, '0.00', { currency: 'ICP' });
console.log(`blind-case: agrees=${blind.agrees} disc2x=${blind.discriminates2x} ok=${blind.ok} :: ${blind.detail}`);

// sats
const sats = checkFigure('btc sats', 1500, '1.5K', { currency: 'BTC' });
console.log(`sats: ${sats.detail}`);

// usd
const usd = checkPlainNumber('usd', 1.2 * 12.34, '(~$14.81)');
console.log(`usd: ${usd.detail} ok=${usd.ok}`);
const usdBad = checkPlainNumber('usd bad', 100, '(~$14.81)');
console.log(`usd bad: ${usdBad.detail} ok=${usdBad.ok}`);
console.log(fails === 0 ? '\nALL PARSER CASES PASS' : `\n${fails} PARSER CASES FAILED`);
process.exit(fails === 0 ? 0 : 1);
