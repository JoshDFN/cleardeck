/**
 * Pure helpers behind the table's visual objects: avatars, chip stacks, the
 * action-clock ring and generated player names. No DOM, no state, no chain
 * access: every function here is a total function of its arguments so it can
 * be unit-tested and so the components that use it stay declarative.
 */

/** djb2-style string hash, kept identical to the one the table has always used
 *  for generated names so a returning player keeps the name they had. */
export function hashString(s) {
  let hash = 0;
  const str = String(s ?? '');
  for (let i = 0; i < str.length; i += 1) {
    hash = ((hash << 5) - hash) + str.charCodeAt(i);
    hash &= hash;
  }
  return Math.abs(hash);
}

const ADJECTIVES = ['Lucky', 'Wild', 'Cool', 'Sly', 'Bold', 'Swift', 'Clever', 'Daring', 'Epic', 'Mystic', 'Royal', 'Shadow', 'Golden', 'Silver', 'Cosmic'];
const NOUNS = ['Ace', 'King', 'Queen', 'Jack', 'Joker', 'Shark', 'Whale', 'Fox', 'Wolf', 'Tiger', 'Eagle', 'Hawk', 'Viper', 'Dragon', 'Phoenix'];

/** The generated display name for a principal, e.g. "ShadowDragon71". */
export function generatedName(principalText) {
  if (!principalText) return 'Unknown';
  const hash = hashString(principalText);
  return `${ADJECTIVES[hash % ADJECTIVES.length]}${NOUNS[(hash >> 8) % NOUNS.length]}${(hash % 100).toString().padStart(2, '0')}`;
}

/**
 * The monogram on an avatar tile: the first letter of each capitalised word of
 * the name, at most two ("ShadowDragon71" -> "SD", "Ada" -> "A", "You" -> "Y").
 */
export function monogram(name) {
  const words = String(name ?? '').replace(/[^A-Za-z]/g, ' ').replace(/([a-z])([A-Z])/g, '$1 $2').trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return '?';
  return words.slice(0, 2).map((w) => w[0].toUpperCase()).join('');
}

/** Eight hues for the avatar tiles, spaced round the wheel, none of them the
 *  felt's green or the money gold so a tile never reads as a chip or the felt. */
export const AVATAR_HUES = [206, 262, 318, 12, 32, 180, 228, 288];

/**
 * A deterministic local avatar for a principal: a hue and a monogram, rendered
 * as an inline SVG data URI so no seat ever fetches a face from a third party.
 * Same principal, same tile, on every device.
 *
 * @param {string} principalText
 * @param {string} displayName the name shown on the plate
 * @returns {{ hue: number, initials: string, src: string }}
 */
export function avatarFor(principalText, displayName) {
  const hue = AVATAR_HUES[hashString(principalText) % AVATAR_HUES.length];
  const initials = monogram(displayName);
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'>`
    + `<defs><linearGradient id='g' x1='0' y1='0' x2='0' y2='1'>`
    + `<stop offset='0' stop-color='hsl(${hue} 42% 46%)'/>`
    + `<stop offset='1' stop-color='hsl(${hue} 46% 26%)'/></linearGradient></defs>`
    + `<rect width='64' height='64' fill='url(%23g)'/>`
    + `<text x='32' y='40' text-anchor='middle' font-family='Inter,Helvetica,Arial,sans-serif' `
    + `font-size='${initials.length > 1 ? 24 : 28}' font-weight='700' fill='white' fill-opacity='0.92'>${initials}</text>`
    + `</svg>`;
  return { hue, initials, src: `data:image/svg+xml;utf8,${encodeURIComponent(svg).replace(/%23g/g, '%23g')}` };
}

/**
 * Chip colour by size band, in big blinds. Casinos colour chips by
 * denomination; a bet's colour therefore tells the eye how big it is before the
 * figure is read. Bands are in BB multiples of the bet, not absolute money, so
 * a 0.10 bet at 0.05/0.10 and a 2 bet at 1/2 are the same chip.
 *
 * @param {number} amount   e8s (or sats)
 * @param {number} bigBlind e8s (or sats); <= 0 means "unknown", one white chip
 * @returns {'white'|'red'|'blue'|'green'|'black'|'gold'}
 */
export function chipBand(amount, bigBlind) {
  const a = Number(amount) || 0;
  const bb = Number(bigBlind) || 0;
  if (a <= 0) return 'white';
  if (bb <= 0) return 'white';
  const x = a / bb;
  if (x < 2) return 'white';
  if (x < 5) return 'red';
  if (x < 12) return 'blue';
  if (x < 30) return 'green';
  if (x < 100) return 'black';
  return 'gold';
}

/**
 * How many discs to draw in a stack: 1 for a single blind, up to 5 for a large
 * bet. A pure log scale on the BB multiple so the stack grows with the bet
 * without ever being a count of anything the chain reports.
 */
export function chipStackCount(amount, bigBlind) {
  const a = Number(amount) || 0;
  const bb = Number(bigBlind) || 0;
  if (a <= 0) return 0;
  if (bb <= 0) return 1;
  const x = a / bb;
  if (x < 1.5) return 1;
  if (x < 4) return 2;
  if (x < 12) return 3;
  if (x < 40) return 4;
  return 5;
}

/**
 * The action clock as an arc, in degrees, for a conic gradient. fraction 1 is a
 * full ring, 0 is none; anything outside [0,1] is clamped, so a clock that
 * overshoots because a poll arrived late can never draw more than a circle.
 */
export function clockArcDegrees(fraction) {
  const f = Number(fraction);
  if (!Number.isFinite(f)) return 0;
  return Math.round(Math.max(0, Math.min(1, f)) * 360);
}

/** The short name a plate shows: 11 characters at most, never wrapped. */
export function shortName(name, max = 11) {
  const s = String(name ?? '');
  return s.length > max ? s.slice(0, max) : s;
}
