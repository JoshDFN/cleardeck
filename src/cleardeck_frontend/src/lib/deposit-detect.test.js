import { describe, expect, it } from 'vitest';
import { DETECT, DETECT_POLL_MS, classifyDetected, detectionCopy, shouldAutoClaim } from './deposit-detect.js';

const FEE = 10_000n;
const MIN_EXTERNAL = 30_000n;

describe('classifyDetected', () => {
  it('is empty on nothing, an unread balance or zero', () => {
    expect(classifyDetected({ balance: null, fee: FEE, minExternal: MIN_EXTERNAL })).toBe(DETECT.EMPTY);
    expect(classifyDetected({ balance: undefined, fee: FEE, minExternal: MIN_EXTERNAL })).toBe(DETECT.EMPTY);
    expect(classifyDetected({ balance: 0n, fee: FEE, minExternal: MIN_EXTERNAL })).toBe(DETECT.EMPTY);
  });

  it('is stuck at or below one ledger fee', () => {
    expect(classifyDetected({ balance: 1n, fee: FEE, minExternal: MIN_EXTERNAL })).toBe(DETECT.STUCK);
    expect(classifyDetected({ balance: FEE, fee: FEE, minExternal: MIN_EXTERNAL })).toBe(DETECT.STUCK);
  });

  it('is short above the fee and below the address floor', () => {
    expect(classifyDetected({ balance: FEE + 1n, fee: FEE, minExternal: MIN_EXTERNAL })).toBe(DETECT.SHORT);
    expect(classifyDetected({ balance: MIN_EXTERNAL - 1n, fee: FEE, minExternal: MIN_EXTERNAL })).toBe(DETECT.SHORT);
  });

  it('is ready at the floor and above', () => {
    expect(classifyDetected({ balance: MIN_EXTERNAL, fee: FEE, minExternal: MIN_EXTERNAL })).toBe(DETECT.READY);
    expect(classifyDetected({ balance: 50_000n, fee: FEE, minExternal: MIN_EXTERNAL })).toBe(DETECT.READY);
  });
});

describe('shouldAutoClaim', () => {
  it('sweeps a ready reading once', () => {
    expect(shouldAutoClaim({ status: DETECT.READY, claiming: false, balance: 50_000n, attemptedFor: null })).toBe(true);
    expect(shouldAutoClaim({ status: DETECT.READY, claiming: false, balance: 50_000n, attemptedFor: 50_000n })).toBe(false);
  });

  it('sweeps again when the reading changed (a top-up after a failed sweep)', () => {
    expect(shouldAutoClaim({ status: DETECT.READY, claiming: false, balance: 80_000n, attemptedFor: 50_000n })).toBe(true);
  });

  it('never sweeps while a claim runs, or anything not ready', () => {
    expect(shouldAutoClaim({ status: DETECT.READY, claiming: true, balance: 50_000n, attemptedFor: null })).toBe(false);
    expect(shouldAutoClaim({ status: DETECT.SHORT, claiming: false, balance: 20_000n, attemptedFor: null })).toBe(false);
    expect(shouldAutoClaim({ status: DETECT.EMPTY, claiming: false, balance: 0n, attemptedFor: null })).toBe(false);
    expect(shouldAutoClaim({ status: DETECT.READY, claiming: false, balance: null, attemptedFor: null })).toBe(false);
  });
});

describe('detectionCopy', () => {
  it('carries no money figure of its own', () => {
    for (const status of Object.values(DETECT)) {
      expect(detectionCopy(status)).not.toMatch(/\d/);
      expect(detectionCopy(status, { claiming: true })).not.toMatch(/\d/);
      expect(detectionCopy(status, { failed: true })).not.toMatch(/\d/);
    }
  });

  it('says what is happening per status', () => {
    expect(detectionCopy(DETECT.EMPTY)).toMatch(/Nothing has arrived yet/);
    expect(detectionCopy(DETECT.STUCK)).toMatch(/no transfer can move it/);
    expect(detectionCopy(DETECT.SHORT)).toMatch(/below the minimum/);
    expect(detectionCopy(DETECT.READY, { claiming: true })).toMatch(/Sweeping it into your table balance now/);
    expect(detectionCopy(DETECT.READY, { failed: true })).toMatch(/failed/);
  });

  it('polls every five seconds', () => {
    expect(DETECT_POLL_MS).toBe(5_000);
  });
});
