import { describe, expect, it } from 'vitest';
import {
  COPIED_FOR_MS, COPY_FAILED_TEXT, FAILED_FOR_MS, copiedState, copyStateTtlMs, copyText, copyWord,
} from './copy-text.js';

/** A document stand-in whose execCommand answers as told. */
function fakeDoc(copyResult) {
  const body = { appended: [], appendChild(node) { this.appended.push(node); } };
  return {
    body,
    createElement: () => ({
      style: {}, setAttribute() {}, select() {}, setSelectionRange() {}, remove() {},
    }),
    execCommand: () => copyResult,
  };
}

describe('copyText', () => {
  it('uses the async clipboard when it works', async () => {
    const written = [];
    const clipboard = { writeText: async (s) => { written.push(s); } };
    expect(await copyText('abc', { clipboard, doc: undefined })).toEqual({ ok: true });
    expect(written).toEqual(['abc']);
  });

  it('falls back to the selection route when the clipboard refuses', async () => {
    const clipboard = { writeText: async () => { throw new Error('denied'); } };
    expect(await copyText('abc', { clipboard, doc: fakeDoc(true) })).toEqual({ ok: true });
  });

  it('reports a failure instead of throwing when every route fails', async () => {
    const clipboard = { writeText: async () => { throw new Error('denied'); } };
    const result = await copyText('abc', { clipboard, doc: fakeDoc(false) });
    expect(result.ok).toBe(false);
    expect(result.reason).toBe('denied');
  });

  it('reports a failure with no clipboard and no document', async () => {
    const result = await copyText('abc', { clipboard: undefined, doc: undefined });
    expect(result).toEqual({ ok: false, reason: 'no clipboard available' });
  });

  it('refuses an empty value', async () => {
    expect(await copyText('', { clipboard: { writeText: async () => {} } })).toEqual({ ok: false, reason: 'nothing to copy' });
    expect(await copyText(null, { clipboard: { writeText: async () => {} } })).toEqual({ ok: false, reason: 'nothing to copy' });
  });
});

describe('copy button words', () => {
  it('reads Copied on success, the humane sentence on failure, idle otherwise', () => {
    expect(copyWord(null, 'hash', 'Copy')).toBe('Copy');
    expect(copyWord(copiedState('hash', true), 'hash', 'Copy')).toBe('Copied');
    expect(copyWord(copiedState('hash', false), 'hash', 'Copy')).toBe(COPY_FAILED_TEXT);
    expect(copyWord(copiedState('seed', true), 'hash', 'Copy')).toBe('Copy');
    expect(copyWord(copiedState('text', true), 'text', 'Copy as text', 'Copied')).toBe('Copied');
  });

  it('keeps a failure on screen longer than a success', () => {
    expect(copyStateTtlMs(copiedState('hash', true))).toBe(COPIED_FOR_MS);
    expect(copyStateTtlMs(copiedState('hash', false))).toBe(FAILED_FOR_MS);
    expect(FAILED_FOR_MS).toBeGreaterThan(COPIED_FOR_MS);
  });
});
