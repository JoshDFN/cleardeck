// The two money doors that re-enable their own button: the address sweep
// (lib/deposit-flow.svelte.js claim) and the BTC check (lib/deposit-btc.svelte.js
// check). Round 1 of the review left both clearing their busy flag right after
// calling onFailure, so on a THROWN reply the modal's button came back before
// any balance had been re-read. The contract now: onFailure is awaited, and
// the flag stays up until it resolves.
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { logger } from './logger.js';
import { createAddressWatch } from './deposit-flow.svelte.js';
import { createBtcDeposit } from './deposit-btc.svelte.js';
import { createCashierFlow } from './deposit-flow.svelte.js';

// The doors log the throw through $lib/logger.js; keep the run's output clean.
beforeEach(() => { vi.spyOn(logger, 'error').mockImplementation(() => {}); });

/** A promise the test resolves by hand. */
function gate() {
  let resolve;
  const promise = new Promise((r) => { resolve = r; });
  return { promise, release: () => resolve() };
}

function addressWatchWith({ tableActor, onFailure }) {
  return createAddressWatch({
    tableActor,
    readBalance: async () => 0n,
    isWatching: () => false,
    isTrusted: () => true,
    untrustedReason: () => null,
    flow: createCashierFlow(),
    setError: () => {},
    onFailure,
    receiptFor: () => ({}),
    onCredited: async () => {},
    fee: 10_000n,
    minExternal: 1_000_000n,
  });
}

describe('the address sweep keeps `claiming` up until onFailure resolves', () => {
  it('a throwing claim_external_deposit: claiming stays true while the handler refreshes', async () => {
    const refresh = gate();
    const seen = [];
    const onFailure = vi.fn(async (e, opts) => {
      seen.push({ message: e.message, thrown: opts?.thrown === true });
      await refresh.promise;
    });
    const tableActor = { claim_external_deposit: vi.fn(async () => { throw new Error('reply leg lost'); }) };
    const watch = addressWatchWith({ tableActor, onFailure });

    const done = watch.claim();
    // let the actor's rejection propagate into the catch
    await new Promise((r) => setTimeout(r, 0));
    expect(onFailure).toHaveBeenCalledTimes(1);
    expect(seen).toEqual([{ message: 'reply leg lost', thrown: true }]);
    // the handler has not resolved: the button must still be disabled
    expect(watch.claiming).toBe(true);
    expect(watch.claimFailed).toBe(true);

    refresh.release();
    await done;
    expect(watch.claiming).toBe(false);
  });

  it('a canister refusal (Err) is handed over without `thrown` and the flag still waits', async () => {
    const refresh = gate();
    const onFailure = vi.fn(async () => { await refresh.promise; });
    const tableActor = { claim_external_deposit: vi.fn(async () => ({ Err: { NothingToClaim: null } })) };
    const watch = addressWatchWith({ tableActor, onFailure });

    const done = watch.claim();
    await new Promise((r) => setTimeout(r, 0));
    expect(onFailure).toHaveBeenCalledWith({ NothingToClaim: null });
    expect(onFailure.mock.calls[0][1]).toBeUndefined();
    expect(watch.claiming).toBe(true);

    refresh.release();
    await done;
    expect(watch.claiming).toBe(false);
  });

  it('a handler that throws itself still lets go of `claiming`', async () => {
    const onFailure = vi.fn(async () => { throw new Error('refresh blew up'); });
    const tableActor = { claim_external_deposit: vi.fn(async () => { throw new Error('lost'); }) };
    const watch = addressWatchWith({ tableActor, onFailure });
    await expect(watch.claim()).rejects.toThrow('refresh blew up');
    expect(watch.claiming).toBe(false);
  });
});

describe('the BTC check keeps `updating` up until onFailure resolves', () => {
  it('a throwing update_btc_balance: updating stays true while the handler refreshes', async () => {
    const refresh = gate();
    const onFailure = vi.fn(async () => { await refresh.promise; });
    const tableActor = { update_btc_balance: vi.fn(async () => { throw new Error('reply leg lost'); }) };
    const btc = createBtcDeposit({
      tableActor, isBTC: true,
      isTrusted: () => true, untrustedReason: () => null, isAuthenticated: () => true,
      format: (v) => String(v), onFailure, onMinted: async () => {},
    });

    const done = btc.check();
    await new Promise((r) => setTimeout(r, 0));
    expect(onFailure).toHaveBeenCalledTimes(1);
    expect(onFailure.mock.calls[0][1]).toEqual({ thrown: true });
    expect(btc.updating).toBe(true);

    refresh.release();
    await done;
    expect(btc.updating).toBe(false);
  });
});
