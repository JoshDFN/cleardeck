// A QR CODE AS AN SVG PATH, FOR THE DEPOSIT ADDRESS CARD.
//
// The encoder is qrcode-generator (Kazuhiko Arase's reference implementation,
// MIT, no dependencies); this module only turns its module grid into one path
// string the card paints in the design tokens' ink. Nothing here touches the
// address itself: the text encoded is exactly the text the card shows and the
// Copy button copies.

import qrcode from 'qrcode-generator';

/**
 * The module grid for `text`, as a size and a dark(row, col) test.
 * @param {string} text
 * @param {{ecl?: 'L'|'M'|'Q'|'H'}} [opts]
 * @returns {{size: number, isDark: (row: number, col: number) => boolean}}
 */
export function qrModules(text, { ecl = 'M' } = {}) {
  const value = String(text ?? '');
  if (!value) throw new Error('a QR code needs text to encode');
  const qr = qrcode(0, ecl);
  qr.addData(value, 'Byte');
  qr.make();
  const size = qr.getModuleCount();
  return { size, isDark: (row, col) => qr.isDark(row, col) };
}

/**
 * One SVG path (in module units, so `viewBox="0 0 size size"`) painting every
 * dark module as a unit square, plus the grid size.
 * @param {string} text
 * @param {{ecl?: 'L'|'M'|'Q'|'H'}} [opts]
 * @returns {{size: number, path: string}}
 */
export function qrSvgPath(text, opts = {}) {
  const { size, isDark } = qrModules(text, opts);
  const parts = [];
  for (let row = 0; row < size; row += 1) {
    let runStart = -1;
    for (let col = 0; col <= size; col += 1) {
      const dark = col < size && isDark(row, col);
      if (dark && runStart < 0) runStart = col;
      if (!dark && runStart >= 0) {
        parts.push(`M${runStart} ${row}h${col - runStart}v1h-${col - runStart}z`);
        runStart = -1;
      }
    }
  }
  return { size, path: parts.join('') };
}
