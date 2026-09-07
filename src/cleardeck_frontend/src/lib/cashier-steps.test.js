import { describe, expect, it } from 'vitest';
import {
  EXPECTED_COMMIT_MS, FLOW, OISY_POPUP_TIMEOUT_MS, cooldownRemainingSecs, depositSteps,
  formatCountdown, progressAt, secondsLeft, stepStatus, withdrawSteps,
} from './cashier-steps.js';

describe('depositSteps', () => {
  it('the II path is approve, move, credited, each commit at the measured tempo', () => {
    const steps = depositSteps({ source: 'ii', amountText: '2.0000 ICP' });
    expect(steps.map((s) => s.id)).toEqual(['approve', 'move', 'credited']);
    expect(steps[0].title).toContain('2.0000 ICP');
    expect(steps[0].hint).toMatch(/No popup/);
    expect(steps[0].expectedMs).toBe(EXPECTED_COMMIT_MS);
    expect(steps[1].expectedMs).toBe(EXPECTED_COMMIT_MS);
    expect(steps[2].expectedMs).toBeNull();
  });

  it('the OISY path waits on the popup with its timeout as a deadline', () => {
    const steps = depositSteps({ source: 'oisy', amountText: '2.0000 ICP' });
    expect(steps.map((s) => s.id)).toEqual(['derive', 'popup', 'claim', 'credited']);
    expect(steps[1].deadlineMs).toBe(OISY_POPUP_TIMEOUT_MS);
    expect(steps[1].expectedMs).toBeNull();
  });
});

describe('withdrawSteps', () => {
  it('is one call and a terminal step', () => {
    const steps = withdrawSteps({ amountText: '0.5000 ICP' });
    expect(steps.map((s) => s.id)).toEqual(['withdraw', 'sent']);
    expect(steps[0].title).toContain('0.5000 ICP');
  });
});

describe('stepStatus', () => {
  it('marks before, on and after the current step', () => {
    expect(stepStatus(0, 1, FLOW.RUNNING)).toBe('done');
    expect(stepStatus(1, 1, FLOW.RUNNING)).toBe('active');
    expect(stepStatus(2, 1, FLOW.RUNNING)).toBe('pending');
  });

  it('a failed flow fails the step it was on and keeps the earlier ones done', () => {
    expect(stepStatus(0, 1, FLOW.FAILED)).toBe('done');
    expect(stepStatus(1, 1, FLOW.FAILED)).toBe('failed');
    expect(stepStatus(2, 1, FLOW.FAILED)).toBe('pending');
  });

  it('a finished flow is done everywhere; an idle one is pending everywhere', () => {
    expect(stepStatus(2, 0, FLOW.DONE)).toBe('done');
    expect(stepStatus(0, 0, FLOW.IDLE)).toBe('pending');
  });
});

describe('progressAt', () => {
  it('reaches nine tenths at the expected duration and never completes alone', () => {
    expect(progressAt({ startedAt: 0, now: 0, expectedMs: 1000 })).toBe(0);
    expect(progressAt({ startedAt: 0, now: 500, expectedMs: 1000 })).toBeCloseTo(0.45, 6);
    expect(progressAt({ startedAt: 0, now: 1000, expectedMs: 1000 })).toBeCloseTo(0.9, 6);
    const late = progressAt({ startedAt: 0, now: 10_000, expectedMs: 1000 });
    expect(late).toBeGreaterThan(0.9);
    expect(late).toBeLessThan(1);
  });

  it('is zero without a start or an expectation', () => {
    expect(progressAt({ startedAt: null, now: 100, expectedMs: 1000 })).toBe(0);
    expect(progressAt({ startedAt: 0, now: 100, expectedMs: null })).toBe(0);
  });
});

describe('secondsLeft / formatCountdown', () => {
  it('counts down from the deadline and floors at zero', () => {
    expect(secondsLeft({ startedAt: 0, now: 1000, deadlineMs: 300_000 })).toBe(299);
    expect(secondsLeft({ startedAt: 0, now: 400_000, deadlineMs: 300_000 })).toBe(0);
    expect(secondsLeft({ startedAt: null, now: 1, deadlineMs: 300_000 })).toBe(0);
  });

  it('formats minutes and zero-padded seconds', () => {
    expect(formatCountdown(299)).toBe('4:59');
    expect(formatCountdown(7)).toBe('0:07');
    expect(formatCountdown(0)).toBe('0:00');
    expect(formatCountdown(-3)).toBe('0:00');
    expect(formatCountdown(NaN)).toBe('0:00');
  });
});

describe('cooldownRemainingSecs', () => {
  it('is the seconds left of the cooldown since the last withdrawal', () => {
    expect(cooldownRemainingSecs({ lastAt: 0, now: 1000, cooldownSecs: 60 })).toBe(59);
    expect(cooldownRemainingSecs({ lastAt: 0, now: 61_000, cooldownSecs: 60 })).toBe(0);
    expect(cooldownRemainingSecs({ lastAt: null, now: 1000, cooldownSecs: 60 })).toBe(0);
  });
});
