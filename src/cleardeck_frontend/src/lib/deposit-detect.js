// THE ADDRESS ROUTE WATCHES ITSELF. A new player who sends ICP to the derived
// deposit address should never have to find a Claim button: the sheet reads
// the address's ledger balance every few seconds while the card is up, says
// what it sees, and sweeps it in with the same `claim_external_deposit` call
// the button made. The decision of what a reading means, and whether it is
// time to sweep, is here, pure and tested; DepositModal.svelte only polls and
// renders.

/** How often the address is re-read while the card is up. */
export const DETECT_POLL_MS = 5_000;

/** What a reading of the deposit address means. */
export const DETECT = Object.freeze({
  /** Nothing has arrived. */
  EMPTY: 'empty',
  /** Something arrived, at or below one ledger fee: no transfer can move it. */
  STUCK: 'stuck',
  /** Something arrived, above the fee but below this route's minimum. */
  SHORT: 'short',
  /** Enough arrived to sweep into the table balance. */
  READY: 'ready',
});

/**
 * Classifies the balance found at the address against the ledger fee and the
 * address route's floor (lib.rs ICP_MIN_EXTERNAL_DEPOSIT, mirrored by the
 * caller). The sweep pays the fee out of what arrived, which is why a figure
 * at or below the fee is stuck and a figure below the floor is short.
 *
 * @param {{balance: bigint|null|undefined, fee: bigint, minExternal: bigint}} p
 * @returns {'empty'|'stuck'|'short'|'ready'}
 */
export function classifyDetected({ balance, fee, minExternal }) {
  if (balance === null || balance === undefined || balance <= 0n) return DETECT.EMPTY;
  if (balance <= fee) return DETECT.STUCK;
  if (balance < minExternal) return DETECT.SHORT;
  return DETECT.READY;
}

/**
 * Is it time to sweep? Only a READY reading, only while no claim is running,
 * and only once per distinct reading: a claim that failed must not be retried
 * on every poll (the player retries it), and a claim that succeeded leaves
 * the address empty, so the next reading is a different one.
 *
 * @param {{status: string, claiming: boolean, balance: bigint|null, attemptedFor: bigint|null}} p
 *   `attemptedFor`: the reading the last automatic claim was made for
 * @returns {boolean}
 */
export function shouldAutoClaim({ status, claiming, balance, attemptedFor }) {
  if (status !== DETECT.READY) return false;
  if (claiming) return false;
  if (balance === null || balance === undefined) return false;
  return attemptedFor === null || attemptedFor === undefined || attemptedFor !== balance;
}

/**
 * The one sentence under the detected figure, per status. The figure itself
 * is rendered by the caller in its own element (the harness asserts it against
 * the ledger), so no money figure is in these words.
 *
 * @param {'empty'|'stuck'|'short'|'ready'} status
 * @param {{claiming?: boolean, failed?: boolean}} [state]
 * @returns {string}
 */
export function detectionCopy(status, { claiming = false, failed = false } = {}) {
  switch (status) {
    case DETECT.STUCK:
      return 'at your address, at or below the network fee: no transfer can move it. Top the same address up to the minimum and the whole balance comes out together.';
    case DETECT.SHORT:
      return 'at your address, below the minimum for this route (in Limits below). It is not lost: top the same address up and the whole balance is swept together.';
    case DETECT.READY:
      if (failed) return 'at your address. The sweep into your balance failed; press the button to try again.';
      return claiming
        ? 'at your address. Sweeping it into your table balance now.'
        : 'at your address. Sweeping it into your table balance.';
    default:
      return 'Nothing has arrived yet. This card re-reads your address every few seconds; a transfer usually shows within a few seconds of landing.';
  }
}
