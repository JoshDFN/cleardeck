// Minimal PNG decoder, built on node:zlib.
//
// WHY THIS EXISTS AT ALL. The occlusion gate (lib/occlusion.mjs) decides whether
// one element covers another by comparing PIXELS of four clipped screenshots, so
// it has to read the bytes Playwright hands back. `tools/shots` deliberately
// depends only on playwright and @dfinity/*; adding an image library for this
// would make the pixel gate the one gate in the repo that can fail to install.
// Chromium writes 8-bit non-interlaced PNGs (colour type 6 for a screenshot with
// an alpha channel, 2 without), which is a small enough subset to decode here.
//
// Anything outside that subset THROWS with the header it saw, rather than
// returning plausible garbage: a pixel gate that silently mis-reads its own
// evidence is worse than no pixel gate.

import zlib from 'node:zlib';

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** Channels per pixel for each PNG colour type. */
const CHANNELS = { 0: 1, 2: 3, 4: 2, 6: 4 };

/**
 * Reverses one scanline's filter in place.
 *
 * @param {number} filter PNG filter type 0..4
 * @param {Buffer} line current row, unfiltered in place
 * @param {Buffer|null} prev the already-unfiltered previous row
 * @param {number} bpp bytes per pixel
 */
function unfilter(filter, line, prev, bpp) {
  const n = line.length;
  switch (filter) {
    case 0:
      return;
    case 1:
      for (let i = bpp; i < n; i += 1) line[i] = (line[i] + line[i - bpp]) & 0xff;
      return;
    case 2:
      if (!prev) return;
      for (let i = 0; i < n; i += 1) line[i] = (line[i] + prev[i]) & 0xff;
      return;
    case 3:
      for (let i = 0; i < n; i += 1) {
        const left = i >= bpp ? line[i - bpp] : 0;
        const up = prev ? prev[i] : 0;
        line[i] = (line[i] + ((left + up) >> 1)) & 0xff;
      }
      return;
    case 4:
      for (let i = 0; i < n; i += 1) {
        const a = i >= bpp ? line[i - bpp] : 0;
        const b = prev ? prev[i] : 0;
        const c = prev && i >= bpp ? prev[i - bpp] : 0;
        const p = a + b - c;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - b);
        const pc = Math.abs(p - c);
        const pred = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
        line[i] = (line[i] + pred) & 0xff;
      }
      return;
    default:
      throw new Error(`png.mjs: unknown filter type ${filter}`);
  }
}

/**
 * Decodes a PNG buffer to straight RGBA bytes.
 *
 * @param {Buffer} buf
 * @returns {{width:number, height:number, data:Uint8Array}} data is RGBA, 4 bytes per pixel
 */
export function decodePng(buf) {
  if (!Buffer.isBuffer(buf)) buf = Buffer.from(buf);
  if (buf.length < 8 || !buf.subarray(0, 8).equals(SIGNATURE)) {
    throw new Error('png.mjs: not a PNG (signature mismatch)');
  }

  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  let interlace = 0;
  const idat = [];

  for (let off = 8; off + 8 <= buf.length;) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString('ascii', off + 4, off + 8);
    const dataStart = off + 8;
    if (type === 'IHDR') {
      width = buf.readUInt32BE(dataStart);
      height = buf.readUInt32BE(dataStart + 4);
      bitDepth = buf[dataStart + 8];
      colorType = buf[dataStart + 9];
      interlace = buf[dataStart + 12];
    } else if (type === 'IDAT') {
      idat.push(buf.subarray(dataStart, dataStart + len));
    } else if (type === 'IEND') {
      break;
    }
    off = dataStart + len + 4; // + CRC
  }

  if (bitDepth !== 8 || !CHANNELS[colorType] || interlace !== 0) {
    throw new Error(
      `png.mjs: unsupported PNG (bitDepth=${bitDepth} colorType=${colorType} interlace=${interlace}). `
      + 'Only 8-bit, non-interlaced, non-palette PNGs are supported, which is what Chromium writes.',
    );
  }

  const channels = CHANNELS[colorType];
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  const expected = (stride + 1) * height;
  if (raw.length < expected) {
    throw new Error(`png.mjs: truncated image data (${raw.length} bytes, expected ${expected})`);
  }

  const out = new Uint8Array(width * height * 4);
  let prev = null;
  for (let y = 0; y < height; y += 1) {
    const rowStart = y * (stride + 1);
    const filter = raw[rowStart];
    const line = raw.subarray(rowStart + 1, rowStart + 1 + stride);
    unfilter(filter, line, prev, channels);
    prev = line;
    for (let x = 0; x < width; x += 1) {
      const s = x * channels;
      const d = (y * width + x) * 4;
      if (channels === 4) {
        out[d] = line[s]; out[d + 1] = line[s + 1]; out[d + 2] = line[s + 2]; out[d + 3] = line[s + 3];
      } else if (channels === 3) {
        out[d] = line[s]; out[d + 1] = line[s + 1]; out[d + 2] = line[s + 2]; out[d + 3] = 255;
      } else if (channels === 2) {
        out[d] = line[s]; out[d + 1] = line[s]; out[d + 2] = line[s]; out[d + 3] = line[s + 1];
      } else {
        out[d] = line[s]; out[d + 1] = line[s]; out[d + 2] = line[s]; out[d + 3] = 255;
      }
    }
  }

  return { width, height, data: out };
}

/**
 * True when two decoded images differ at pixel `i` by more than `tol` on any
 * colour channel. Alpha is ignored: a screenshot is composited onto an opaque
 * page, so alpha is 255 everywhere and comparing it only adds noise.
 */
export function pixelDiffers(a, b, i, tol) {
  const o = i * 4;
  return Math.abs(a[o] - b[o]) > tol
    || Math.abs(a[o + 1] - b[o + 1]) > tol
    || Math.abs(a[o + 2] - b[o + 2]) > tol;
}

/**
 * Encodes RGBA bytes as a PNG. Used only to write the evidence crops beside a
 * finding, so the reader can see the covered figure without re-running anything.
 *
 * @param {number} width
 * @param {number} height
 * @param {Uint8Array} rgba
 * @returns {Buffer}
 */
export function encodePng(width, height, rgba) {
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y += 1) {
    raw[y * (stride + 1)] = 0; // filter: none
    Buffer.from(rgba.buffer, rgba.byteOffset + y * stride, stride)
      .copy(raw, y * (stride + 1) + 1);
  }
  const chunk = (type, data) => {
    const out = Buffer.alloc(data.length + 12);
    out.writeUInt32BE(data.length, 0);
    out.write(type, 4, 'ascii');
    data.copy(out, 8);
    out.writeInt32BE(crc32(Buffer.concat([Buffer.from(type, 'ascii'), data])) | 0, data.length + 8);
    return out;
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; ihdr[9] = 6; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  return Buffer.concat([
    SIGNATURE,
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const CRC_TABLE = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();

function crc32(buf) {
  let c = -1;
  for (let i = 0; i < buf.length; i += 1) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return c ^ -1;
}
