// Plain-text output. No colour, no spinner, nothing that would look different
// in a pipe than on a terminal: this output gets pasted into disputes.

export const pct = (x) => (x === null || x === undefined || Number.isNaN(x) ? '   -  ' : `${(x * 100).toFixed(1).padStart(5)}%`);
export const shortP = (p) => (typeof p === 'string' && p.length > 14 ? `${p.slice(0, 5)}…${p.slice(-3)}` : String(p));

/** e8s -> a decimal string. The archive is denominated in e8s; players think in ICP. */
export const icp = (e8s) => (e8s / 1e8).toFixed(4);
export const signed = (n) => (n > 0 ? `+${n}` : String(n));
export const signedIcp = (e8s) => (e8s > 0 ? `+${icp(e8s)}` : icp(e8s));

/** "40.1% [34.2, 46.3] n=250" -- a rate that cannot be mistaken for a certainty. */
export function rate(w, { minN = 100 } = {}) {
  if (!w || w.n === 0) return 'n=0';
  const body = `${pct(w.p)} [${(w.lo * 100).toFixed(1)}, ${(w.hi * 100).toFixed(1)}] n=${w.n}`;
  return w.n < minN ? `${body}  (below ${minN}: noise)` : body;
}

export function table(rows, columns) {
  const widths = columns.map((c) => Math.max(c.header.length, ...rows.map((r) => String(c.get(r) ?? '').length)));
  const line = (cells) => cells.map((c, i) => (columns[i].right ? String(c).padStart(widths[i]) : String(c).padEnd(widths[i]))).join('  ');
  const out = [line(columns.map((c) => c.header)), line(widths.map((w) => '-'.repeat(w)))];
  for (const r of rows) out.push(line(columns.map((c) => c.get(r) ?? '')));
  return out.join('\n');
}

export const h1 = (s) => `\n${s}\n${'='.repeat(s.length)}`;
export const h2 = (s) => `\n${s}\n${'-'.repeat(s.length)}`;
