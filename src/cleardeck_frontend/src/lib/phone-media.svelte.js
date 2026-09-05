// IS THIS THE PHONE SHEET? The one media query the money sheets use
// (lib/pin-after.js PHONE_MEDIA, money-sheet-phone.scss), as reactive state,
// so a component can place one piece of markup in two different spots: the
// cashier's button row rides a footer under the scroller on a wide screen and
// sits in the scroller after the disclosures on a phone.

import { PHONE_MEDIA } from './pin-after.js';

/**
 * Call during component init. `matches` follows the media query live.
 * @returns {{ readonly matches: boolean }}
 */
export function phoneMedia() {
  const query = typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia(PHONE_MEDIA)
    : null;
  let matches = $state(query ? query.matches : false);

  $effect(() => {
    if (!query) return undefined;
    const update = () => { matches = query.matches; };
    update();
    query.addEventListener?.('change', update);
    return () => query.removeEventListener?.('change', update);
  });

  return {
    get matches() { return matches; },
  };
}
