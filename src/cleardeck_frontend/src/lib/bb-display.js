/**
 * Stacks, bets and the pot in BIG BLINDS: the arithmetic and the preference.
 *
 * Every serious client (PokerStars, GGPoker, WPT Global) lets a player read
 * the live money on the felt as a multiple of the big blind, because that is
 * the unit poker decisions are made in: a 25 BB stack plays the same at
 * 0.05/0.10 as at 5/10, and "raise to 3 BB" is the same decision whatever
 * the stakes. ClearDeck's felt read only in the table's currency.
 *
 * WHAT THE MODE COVERS, AND WHAT IT NEVER TOUCHES. The mode re-labels the
 * LIVE OBJECTS ON THE FELT: the stacks on the plates, the bet discs, the
 * pot and its side pots, and the chip flights between them. It never touches
 * a figure that is SENT or SETTLED: the action buttons, the sizer, the
 * pot-odds line, the wallet panel, the winner line and the award chip all
 * stay in the currency, because the figure on the primary button must be the
 * figure sent, digit for digit (bet-sizing.js), and a settlement is money
 * that landed in a balance. The exact currency figure of every re-labelled
 * object rides its `title`, so nothing is hidden, only re-expressed.
 *
 * OFF BY DEFAULT, per viewer, in localStorage, exactly like the keyboard
 * shortcuts (hotkeys-pref.js): the screenshot harness photographs the
 * default state and asserts every felt figure against the canister in the
 * currency, so the default must be what the harness reads. Only the literal
 * 'true' turns it on; a missing key, a stale value or a storage that throws
 * all mean off.
 *
 * THE FIGURE. `amount / bigBlind`, one decimal below 100 BB (a 12.5 BB stack
 * and a 0.5 BB bet are both ordinary), none from 100 BB up (a 250 BB stack
 * is not read to the tenth), thousands separated. Rounded, not truncated:
 * this is a reading of a figure, not a proposal that will be sent. With no
 * big blind (a table whose config has not arrived, or a zero) there is no
 * honest multiple and the caller falls back to the currency.
 *
 * Pure: the storage is handed in. Nothing here mutates its inputs.
 */

/** localStorage key of the preference; absent means OFF. */
export const BB_DISPLAY_PREF_KEY = 'poker_bb_display';

/** The unit as painted beside a re-labelled figure. */
export const BB_UNIT = 'BB';

/**
 * Whether a big-blind reading is possible at all.
 * @param {number} bigBlind the table's big blind in the smallest unit
 */
export function canShowBB(bigBlind) {
  const bb = Number(bigBlind);
  return Number.isFinite(bb) && bb > 0;
}

/**
 * A smallest-unit amount as a multiple of the big blind, as text: "12.5",
 * "0.5", "250", "1,250". Null when there is no big blind to divide by, so a
 * caller can never paint a figure this module did not compute.
 * @param {number} amount   in the smallest unit (e8s or sats)
 * @param {number} bigBlind in the same unit
 * @returns {string|null}
 */
export function formatBB(amount, bigBlind) {
  if (!canShowBB(bigBlind)) return null;
  const n = Number(amount);
  if (!Number.isFinite(n)) return null;
  const multiple = n / Number(bigBlind);
  // Decided on the figure as it will READ: 99.95 rounds to 100, not "100.0".
  const toTenth = Math.round(multiple * 10) / 10;
  const decimals = Math.abs(toTenth) >= 100 ? 0 : 1;
  return multiple.toLocaleString('en-US', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

/**
 * Read the preference. OFF by default: only the literal 'true' enables it.
 * @param {{ getItem: (k: string) => string | null } | null | undefined} storage
 * @returns {boolean}
 */
export function readBbDisplayPref(storage) {
  try {
    const raw = storage ? storage.getItem(BB_DISPLAY_PREF_KEY) : null;
    return raw === 'true';
  } catch {
    return false;
  }
}

/**
 * Persist the preference. Returns whether the write succeeded.
 * @param {{ setItem: (k: string, v: string) => void } | null | undefined} storage
 * @param {boolean} enabled
 * @returns {boolean}
 */
export function writeBbDisplayPref(storage, enabled) {
  try {
    if (!storage) return false;
    storage.setItem(BB_DISPLAY_PREF_KEY, enabled === true ? 'true' : 'false');
    return true;
  } catch {
    return false;
  }
}
