import { describe, expect, it } from 'vitest';
import {
  DETAIL_MAX_CHARS, LOBBY_RETRY_MAX_MS, LOBBY_RETRY_MS, describeLobbyFailure, detailOf,
  describeCashierFailure, isTransportFailure, rawMessageOf, retryDelayMs,
  THROWN_CASHIER_MESSAGE, THROWN_TRANSPORT_MESSAGE,
} from './humane-errors.js';

const AGENT_TEXT =
  'Failed to fetch HTTP request: error sending request for url '
  + '(http://127.0.0.1:8077/api/v2/canister/aaaaa-aa/call): client error (Connect): '
  + 'tcp connect error: Connection refused (os error 61)';

describe('rawMessageOf', () => {
  it('reads an Error, a string, and nothing', () => {
    expect(rawMessageOf(new Error(' boom '))).toBe('boom');
    expect(rawMessageOf('plain')).toBe('plain');
    expect(rawMessageOf(null)).toBe('');
    expect(rawMessageOf(undefined)).toBe('');
  });

  it('falls back to String() for an object without a message', () => {
    expect(rawMessageOf({ toString: () => 'custom' })).toBe('custom');
  });
});

describe('isTransportFailure', () => {
  it('recognises the agent transport messages', () => {
    expect(isTransportFailure(new Error(AGENT_TEXT))).toBe(true);
    expect(isTransportFailure(new TypeError('Failed to fetch'))).toBe(true);
    expect(isTransportFailure(new Error('Request timed out after 30s'))).toBe(true);
    expect(isTransportFailure(new Error('net::ERR_CONNECTION_REFUSED'))).toBe(true);
  });

  it('does not call a canister reject a network failure', () => {
    expect(isTransportFailure(new Error('Canister trapped: assertion failed'))).toBe(false);
    expect(isTransportFailure(new Error('Reject code 5: IC0503'))).toBe(false);
    expect(isTransportFailure(null)).toBe(false);
  });
});

describe('detailOf', () => {
  it('keeps the raw text on one line and caps it', () => {
    expect(detailOf(new Error('a\n  b   c'))).toBe('a b c');
    const long = 'x'.repeat(DETAIL_MAX_CHARS + 50);
    const d = detailOf(new Error(long));
    expect(d.length).toBe(DETAIL_MAX_CHARS);
    expect(d.endsWith('…')).toBe(true);
    expect(detailOf(null)).toBeNull();
  });
});

describe('describeLobbyFailure', () => {
  it('turns the agent paragraph into a sentence with the retry interval', () => {
    const d = describeLobbyFailure(new Error(AGENT_TEXT));
    expect(d.message).toBe(`Could not reach the tables. Retrying in ${LOBBY_RETRY_MS / 1000} s.`);
    expect(d.detail).toBe(AGENT_TEXT);
    expect(d.transport).toBe(true);
  });

  it('words a canister failure differently', () => {
    const d = describeLobbyFailure(new Error('Canister trapped'), { retryMs: 5000 });
    expect(d.message).toBe('The table list could not be read. Retrying in 5 s.');
    expect(d.transport).toBe(false);
  });

  it('says what to do when there is no automatic retry', () => {
    const d = describeLobbyFailure(new Error('Failed to fetch'), { retryMs: null });
    expect(d.message).toBe('Could not reach the tables. Try again in a moment.');
  });

  it('never contains an em-dash', () => {
    const d = describeLobbyFailure(new Error(AGENT_TEXT));
    expect(d.message).not.toMatch(/\u2014/);
  });
});

describe('retryDelayMs', () => {
  it('starts at the base interval and doubles per failed attempt', () => {
    expect(retryDelayMs(0)).toBe(LOBBY_RETRY_MS);
    expect(retryDelayMs(1)).toBe(LOBBY_RETRY_MS * 2);
    expect(retryDelayMs(2)).toBe(LOBBY_RETRY_MS * 4);
  });

  it('never exceeds the cap', () => {
    expect(retryDelayMs(3)).toBe(LOBBY_RETRY_MAX_MS);
    expect(retryDelayMs(40)).toBe(LOBBY_RETRY_MAX_MS);
  });

  it('treats a bad attempt count as the first attempt', () => {
    expect(retryDelayMs(-1)).toBe(LOBBY_RETRY_MS);
    expect(retryDelayMs(NaN)).toBe(LOBBY_RETRY_MS);
    expect(retryDelayMs(1.7)).toBe(LOBBY_RETRY_MS * 2);
  });

  it('feeds the sentence on screen', () => {
    const { message } = describeLobbyFailure(new Error('Failed to fetch'), { retryMs: retryDelayMs(1) });
    expect(message).toBe('Could not reach the tables. Retrying in 24 s.');
  });
});

describe('describeCashierFailure', () => {
  it('turns the withdraw cooldown into a sentence with the seconds and keeps the raw line', () => {
    const r = describeCashierFailure('Please wait 47 seconds before withdrawing again');
    expect(r.message).toBe('You withdrew less than a minute ago. Try again in 47 s. Nothing moved.');
    expect(r.detail).toBe('Please wait 47 seconds before withdrawing again');
    expect(r.transport).toBe(false);
  });

  it('maps the ledger\'s InsufficientFunds and the table\'s insufficient balance apart', () => {
    expect(describeCashierFailure('Approval failed: InsufficientFunds').message).toMatch(/wallet does not cover/);
    expect(describeCashierFailure(new Error('Insufficient balance')).message).toMatch(/table balance does not cover/);
  });

  it('keeps a sentence that names a limit as it is', () => {
    const raw = 'Minimum withdrawal is 0.0002 ICP. Your whole remaining balance can always be withdrawn in one call.';
    const r = describeCashierFailure(raw);
    expect(r.message).toBe(raw);
    expect(r.detail).toBeNull();
  });

  it('a dead transport RETURNED as an Err is a refusal: nothing moved', () => {
    const r = describeCashierFailure(new Error('Failed to fetch HTTP request: tcp connect error (os error 61)'));
    expect(r.transport).toBe(true);
    expect(r.thrown).toBe(false);
    expect(r.moved).toBe('none');
    expect(r.message).toMatch(/Nothing moved/);
    expect(r.detail).toMatch(/os error 61/);
  });

  it('anonymous, rate limit and a pending withdrawal each get their own sentence', () => {
    expect(describeCashierFailure('Anonymous callers cannot withdraw').message).toMatch(/Sign in first/);
    expect(describeCashierFailure('Rate limit exceeded. Please wait before trying again.').message).toMatch(/Too many attempts/);
    expect(describeCashierFailure('A withdrawal is already in progress').message).toMatch(/still settling/);
  });

  it('an unknown message gets the generic sentence with the raw text as detail', () => {
    const r = describeCashierFailure('Something odd: code 42');
    expect(r.message).toBe('That did not go through. Nothing moved.');
    expect(r.detail).toBe('Something odd: code 42');
  });
});

describe('describeCashierFailure: a THROW may never say nothing moved', () => {
  it('a thrown transport failure says the transfer may have gone through and to check first', () => {
    const r = describeCashierFailure(
      new Error('Failed to fetch HTTP request: tcp connect error (os error 61)'),
      { thrown: true },
    );
    expect(r.thrown).toBe(true);
    expect(r.transport).toBe(true);
    expect(r.moved).toBe('unknown');
    expect(r.message).not.toMatch(/Nothing moved/i);
    expect(r.message).toMatch(/may still have gone through/);
    expect(r.message).toMatch(/Check your balance or the receipt before trying again/);
    expect(r.detail).toMatch(/os error 61/);
  });

  it('an unknown throw gets the same promise, with the raw text as detail', () => {
    const r = describeCashierFailure(new Error('Something odd: code 42'), { thrown: true });
    expect(r.thrown).toBe(true);
    expect(r.moved).toBe('unknown');
    expect(r.message).not.toMatch(/Nothing moved/i);
    expect(r.message).toMatch(/may still have gone through/);
    expect(r.detail).toBe('Something odd: code 42');
  });

  it('a throw whose text matches a refusal pattern is still a throw', () => {
    // The wallet may throw a ledger error AFTER the transfer request left it.
    const r = describeCashierFailure(new Error('InsufficientFunds'), { thrown: true });
    expect(r.moved).toBe('unknown');
    expect(r.message).not.toMatch(/Nothing moved/i);
  });

  it('only a canister Err says nothing moved', () => {
    for (const err of ['Something odd: code 42', 'Insufficient balance', 'Rate limit exceeded']) {
      const r = describeCashierFailure(err);
      expect(r.thrown).toBe(false);
      expect(r.moved).toBe('none');
      expect(r.message).toMatch(/Nothing moved/);
    }
    expect(THROWN_CASHIER_MESSAGE).not.toMatch(/Nothing moved/i);
    expect(THROWN_TRANSPORT_MESSAGE).not.toMatch(/Nothing moved/i);
  });
});
