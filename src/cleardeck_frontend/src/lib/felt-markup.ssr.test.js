// The felt's markup in its DEFAULT state and in big-blind mode, rendered by
// Svelte's server build (no browser; `render` from svelte/server).
//
// Why this file exists: the screenshot harness (tools/shots) reads the plate
// by class (`.chips`, `.bet-amount`, `.fold-word`, `.pot-amount`) and asserts
// every figure against the canister in the table's currency. The big-blind
// mode ($lib/bb-display.js) and the street word ($lib/street-actions.js) add
// elements to that plate, so the contract to pin is: with the defaults the
// markup the harness reads is byte-for-byte what it was, and with the mode on
// the unit tag and the exact currency title appear where the module says.
import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import SeatPod from './components/SeatPod.svelte';
import PotModule from './components/PotModule.svelte';
import ActionBar from './components/ActionBar.svelte';

const fmt = (v) => (Number(v) / 100_000_000).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const withUnit = (v) => `${fmt(v)} ICP`;
const player = (over = {}) => ({
  principal: '2vxsx-fae', chips: 5_000_000_000, current_bet: 0, has_folded: false, is_all_in: false,
  status: { Active: null }, hole_cards: [], ...over,
});
const podBase = { player: player(), seatIndex: 1, seatLabel: 'Seat 2', name: 'Nakamoto', fmt, bigBlind: 10_000_000 };

/** The server build's output without its hydration markers and scoped class hashes. */
function clean(html) {
  return String(html)
    .replace(/<!--.*?-->/gs, '')
    .replace(/ class="svelte-[a-z0-9]+"/g, '')
    .replace(/ svelte-[a-z0-9]+(?=")/g, '');
}
const markup = (component, props) => clean(render(component, { props }).body);
/** The whole `.chips` element, its nested spans balanced (a lookahead regex stopped at the wrong close). */
function chipsSpan(html) {
  const start = html.indexOf('<span class="chips cd-money"');
  if (start < 0) return '';
  const tokens = /<span\b|<\/span>/g;
  tokens.lastIndex = start;
  let depth = 0;
  for (let m = tokens.exec(html); m; m = tokens.exec(html)) {
    depth += m[0] === '</span>' ? -1 : 1;
    if (depth === 0) return html.slice(start, m.index + m[0].length);
  }
  return '';
}

describe('the plate in the default state is what the harness reads', () => {
  it('writes the stack in the currency with no unit tag, no title and no street word', () => {
    const body = markup(SeatPod, podBase);
    expect(chipsSpan(body)).toBe('<span class="chips cd-money">50.00</span>');
    expect(body).not.toContain('unit-tag');
    expect(body).not.toContain('acted-word');
    expect(body).not.toContain('fold-word');
  });

  it('a folded seat keeps the fold word, and never a street word beside it', () => {
    const body = markup(SeatPod, { ...podBase, player: player({ has_folded: true }), folded: true, actedWord: 'Call' });
    expect(body).toContain('<span class="fold-word">Fold</span>');
    expect(body).not.toContain('acted-word');
  });

  it('a seat that acted this street says so beside the stack, in the fold word\'s slot', () => {
    const body = markup(SeatPod, { ...podBase, actedWord: 'Check' });
    expect(body).toContain('<span class="chips cd-money">50.00</span>');
    expect(body).toContain('<span class="acted-word">Check</span>');
    expect(body.indexOf('class="chips')).toBeLessThan(body.indexOf('acted-word'));
  });
});

describe('the plate in big-blind mode', () => {
  const bb = (v) => (Number(v) / 10_000_000).toFixed(1);
  const props = { ...podBase, fmt: bb, feltUnit: 'BB', exactFmt: withUnit };

  it('paints the multiple, the BB tag, and the exact currency figure on the title', () => {
    const body = markup(SeatPod, props);
    expect(chipsSpan(body)).toBe('<span class="chips cd-money" title="50.00 ICP">500.0<span class="unit-tag">BB</span></span>');
  });

  it('does the same for the bet disc', () => {
    const body = markup(SeatPod, { ...props, betAmount: 30_000_000, player: player({ current_bet: 30_000_000 }) });
    expect(body).toContain('<span class="bet-amount" title="0.30 ICP">3.0<span class="unit-tag">BB</span></span>');
  });

  it('with the unit but no exact formatter there is no title to paint', () => {
    const body = markup(SeatPod, { ...props, exactFmt: null });
    expect(chipsSpan(body)).toBe('<span class="chips cd-money">500.0<span class="unit-tag">BB</span></span>');
  });
});

describe('the pot module keeps the winner line in the currency', () => {
  const bb = (v) => (Number(v) / 10_000_000).toFixed(1);

  it('default: the pot figure as before, no unit tag', () => {
    const body = markup(PotModule, { totalPot: 40_000_000, streetLabel: 'Flop', fmt });
    expect(body).toContain('<span class="pot-amount cd-money">0.40</span>');
    expect(body).not.toContain('unit-tag');
  });

  it('big-blind mode: the pot reads in BB with the exact figure on its title', () => {
    const body = markup(PotModule, { totalPot: 40_000_000, sidePots: [{ amount: 30_000_000 }, { amount: 10_000_000 }], streetLabel: 'Flop', fmt: bb, fmtSettled: fmt, unit: 'BB', exactFmt: withUnit });
    expect(body).toContain('<span class="pot-amount cd-money" title="0.40 ICP">4.0<span class="unit-tag">BB</span></span>');
    expect(body).toContain('<span class="side-pot-amount cd-money">3.0<span class="unit-tag">BB</span></span>');
  });

  it('the winner line is a settlement: written by fmtSettled, whatever fmt reads in', () => {
    const winners = [{ seat: 1, amount: 40_000_000, hand_rank: [] }];
    const body = markup(PotModule, { isHandComplete: true, winners, streetLabel: 'Complete', fmt: bb, fmtSettled: fmt, unit: 'BB', exactFmt: withUnit, currencySymbol: 'ICP' });
    expect(body).toContain('wins 0.40 ICP');
    expect(body).not.toContain('wins 4.0');
    // and with no fmtSettled the module falls back to fmt, as every older caller expects
    const fallback = markup(PotModule, { isHandComplete: true, winners, streetLabel: 'Complete', fmt, currencySymbol: 'ICP' });
    expect(fallback).toContain('wins 0.40 ICP');
  });
});

describe('the fold cell says when folding takes two presses', () => {
  const live = { isMyTurn: true, gameInProgress: true, canRaise: true, callAmount: 10_000_000, fmt };

  it('facing a bet: one press, the title says so', () => {
    const body = markup(ActionBar, { ...live, canCheck: false });
    expect(body).toContain('title="Fold (F)"');
    expect(body).toContain('<u>F</u>old');
    expect(body).not.toContain('Fold anyway?');
  });

  it('with a free check: the resting cell reads Fold, the title names the two presses', () => {
    const body = markup(ActionBar, { ...live, canCheck: true });
    expect(body).toContain('title="Check is free. Fold takes two presses (F F)"');
    expect(body).toContain('<u>F</u>old');
    expect(body).not.toContain('Fold anyway?');
    expect(body).not.toContain('aria-pressed="true"');
  });
});
