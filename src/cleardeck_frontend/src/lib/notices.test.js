import { describe, expect, it } from 'vitest';
import {
  NOTICE_LEAD, NOTICE_NO_RAKE, NOTICE_STRIP, NOTICE_STRIP_TEXT, NOTICE_TERMS,
} from './notices.js';

// The five phrases tools/shots/lib/protected-notices.mjs hit-tests and
// scripts/dev.sh greps for. Byte-for-byte; a rewording here is a product
// decision, not a copy edit.
const PROTECTED = [
  'Unaudited code with known bugs',
  'your funds are NOT safe',
  'illegal in many jurisdictions',
  '18+ only',
  'No rake is taken from any pot on any table',
];

describe('the one-line strip', () => {
  it('carries every protected phrase', () => {
    for (const phrase of PROTECTED) expect(NOTICE_STRIP_TEXT).toContain(phrase);
  });

  it('is the lead, a colon, and the rest', () => {
    expect(NOTICE_STRIP_TEXT).toBe(`${NOTICE_LEAD}: ${NOTICE_STRIP}`);
    expect(NOTICE_LEAD).toBe(PROTECTED[0]);
  });
});

describe('the full terms', () => {
  it('are a superset of the strip', () => {
    const full = `${NOTICE_LEAD}. ${NOTICE_TERMS.warningBody} ${NOTICE_TERMS.infoBefore} ${NOTICE_NO_RAKE} ${NOTICE_TERMS.infoAfter} ${NOTICE_TERMS.ai}`;
    for (const phrase of PROTECTED) expect(full).toContain(phrase);
    expect(NOTICE_NO_RAKE).toBe(`${PROTECTED[4]}.`);
  });

  it('contain no em-dash', () => {
    const all = [NOTICE_LEAD, NOTICE_STRIP, NOTICE_NO_RAKE, ...Object.values(NOTICE_TERMS)].join(' ');
    expect(all).not.toMatch(/\u2014/);
  });
});
