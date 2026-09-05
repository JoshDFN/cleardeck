import { describe, expect, it } from 'vitest';
import {
  DETAIL_MAX_CHARS, LOBBY_RETRY_MAX_MS, LOBBY_RETRY_MS, describeLobbyFailure, detailOf,
  isTransportFailure, rawMessageOf, retryDelayMs,
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
