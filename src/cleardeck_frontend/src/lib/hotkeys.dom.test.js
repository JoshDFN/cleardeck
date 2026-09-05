// @vitest-environment happy-dom
//
// The hotkeys against a REAL document, not the abstract `dialogOpen` flag.
//
// Why this file exists: round 2 of the fairness wave gave the table's LOG
// drawer role="dialog" so the occlusion gate would treat it as an overlay, and
// DIALOG_SELECTOR then muted F / C / R / A, the presets, +/- and Escape for as
// long as the drawer was open. Every hotkeys test passed, because they all
// took `dialogOpen` as an input. These take the DOM as the input.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it, beforeEach } from 'vitest';
import { dialogIsOpen, focusKindOf, resolveHotkey } from './hotkeys.js';

const base = {
  enabled: true, live: true, modifier: false, canCheck: false, canRaise: true, raiseDisabled: false,
  allInArmed: false, compact: false, sizerOpen: false, presets: [{ id: 'pot', key: '4' }],
};

/** The table with the log open, as PokerTable.svelte renders it. */
const TABLE_WITH_LOG = `
  <div class="poker-table log-open">
    <div class="stage"><div class="table-inner"><div class="felt"></div>
      <button type="button" class="deck-seal on-felt" data-seal="sealed">Deck</button>
    </div></div>
    <div class="feed-container left" role="region" aria-label="Action log" data-surface="log">
      <div class="action-feed"><button type="button" class="feed-close">Close</button></div>
    </div>
    <div class="action-dock">
      <button type="button" class="log-toggle" aria-pressed="true">Log</button>
      <div class="actions">
        <button type="button" class="action-btn">Fold</button>
        <button type="button" class="action-btn danger">All in</button>
      </div>
    </div>
  </div>`;

function contextFrom(doc) {
  // ActionBar passes the whole dock, so its Log toggle counts as a dock button.
  const dock = doc.querySelector('.action-dock');
  return { ...base, focusKind: focusKindOf(doc.activeElement, dock), dialogOpen: dialogIsOpen(doc) };
}

describe('hotkeys with the action log open', () => {
  beforeEach(() => { document.body.innerHTML = TABLE_WITH_LOG; });

  it('the log is a region, not a dialog: nothing is muted', () => {
    expect(dialogIsOpen(document)).toBe(false);
    const ctx = contextFrom(document);
    expect(resolveHotkey('f', ctx)).toEqual({ type: 'action', action: 'fold' });
    expect(resolveHotkey('c', ctx)).toEqual({ type: 'action', action: 'call' });
    expect(resolveHotkey('r', ctx)).toEqual({ type: 'commit-raise' });
    expect(resolveHotkey('a', ctx)).toEqual({ type: 'arm-allin' });
    expect(resolveHotkey('4', ctx)).toEqual({ type: 'preset', id: 'pot' });
    expect(resolveHotkey('+', ctx)).toEqual({ type: 'step', delta: 1 });
    expect(resolveHotkey('Escape', ctx)).toEqual({ type: 'escape' });
  });

  it('with the preference off, the open log changes nothing: still muted', () => {
    expect(resolveHotkey('f', { ...contextFrom(document), enabled: false })).toBeNull();
    expect(resolveHotkey('a', { ...contextFrom(document), enabled: false })).toBeNull();
  });

  it('a real modal still mutes everything', () => {
    document.body.insertAdjacentHTML('beforeend', '<div class="modal-backdrop"></div>');
    expect(dialogIsOpen(document)).toBe(true);
    expect(resolveHotkey('f', contextFrom(document))).toBeNull();
    document.querySelector('.modal-backdrop').remove();
    document.body.insertAdjacentHTML('beforeend', '<div role="dialog" aria-label="Deposit"></div>');
    expect(resolveHotkey('f', contextFrom(document))).toBeNull();
  });

  it('a dock button holding focus does not mute letters; the log\'s Close does', () => {
    document.querySelector('.action-btn').focus();
    expect(focusKindOf(document.activeElement, document.querySelector('.action-dock'))).toBe('dock-button');
    expect(resolveHotkey('f', contextFrom(document))).toEqual({ type: 'action', action: 'fold' });
    // the Log toggle the player has just clicked is in the dock too
    document.querySelector('.log-toggle').focus();
    expect(resolveHotkey('f', contextFrom(document))).toEqual({ type: 'action', action: 'fold' });
    document.querySelector('.feed-close').focus();
    expect(resolveHotkey('f', contextFrom(document))).toBeNull();
  });

  it('the deck seal releases focus once pressed, so the hotkeys come back', () => {
    const seal = document.querySelector('.deck-seal');
    seal.focus();
    expect(resolveHotkey('f', contextFrom(document))).toBeNull();
    // DeckSeal.svelte blurs itself in its click handler; the same call here.
    seal.blur();
    expect(document.activeElement === document.body || document.activeElement === null).toBe(true);
    expect(resolveHotkey('f', contextFrom(document))).toEqual({ type: 'action', action: 'fold' });
  });
});

describe('the rendered drawer in PokerTable.svelte', () => {
  it('is not marked as a dialog (the markup that muted the hotkeys in round 2)', () => {
    const src = fs.readFileSync(path.resolve(__dirname, 'components/PokerTable.svelte'), 'utf8');
    const drawer = src.match(/<div class="feed-container left"[^>]*>/);
    expect(drawer).not.toBeNull();
    expect(drawer[0]).not.toMatch(/role="dialog"/);
    expect(drawer[0]).not.toMatch(/aria-modal/);
    expect(drawer[0]).toMatch(/role="region"/);
  });
});
