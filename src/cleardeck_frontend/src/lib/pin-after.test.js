import { describe, expect, it } from 'vitest';
import { shouldPin } from './pin-after.js';

describe('shouldPin', () => {
  it('does not pin while a gate is still below the line the pinned row would sit on', () => {
    // 844 px sheet, 68 px row: the row's top would be at 776. Solvency ends at 1200.
    expect(shouldPin({ gateBottoms: [1200], rowHeight: 68, viewportBottom: 844 })).toBe(false);
  });

  it('does not pin while a gate would be under the pinned row', () => {
    expect(shouldPin({ gateBottoms: [800], rowHeight: 68, viewportBottom: 844 })).toBe(false);
  });

  it('pins once every gate has scrolled above the pin line', () => {
    expect(shouldPin({ gateBottoms: [700, 760], rowHeight: 68, viewportBottom: 844 })).toBe(true);
    expect(shouldPin({ gateBottoms: [776], rowHeight: 68, viewportBottom: 844 })).toBe(true);
  });

  it('never pins with no gate: an unmeasured warning is an unread one', () => {
    expect(shouldPin({ gateBottoms: [], rowHeight: 68, viewportBottom: 844 })).toBe(false);
    expect(shouldPin({ gateBottoms: null, rowHeight: 68, viewportBottom: 844 })).toBe(false);
  });

  it('treats a non-finite measurement as not pinned', () => {
    expect(shouldPin({ gateBottoms: [NaN], rowHeight: 68, viewportBottom: 844 })).toBe(false);
    expect(shouldPin({ gateBottoms: [100], rowHeight: NaN, viewportBottom: 844 })).toBe(false);
  });

  it('one gate above and one below: not pinned (all of them must be read)', () => {
    expect(shouldPin({ gateBottoms: [100, 900], rowHeight: 68, viewportBottom: 844 })).toBe(false);
  });
});
