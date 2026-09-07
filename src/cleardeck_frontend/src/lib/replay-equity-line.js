/**
 * The 4-point equity line under the replay felt: one row per seat, one cell
 * per street (pre-flop, flop, turn, river), from the same equity results the
 * pods show at each street's reveal stop. Pure: the equity engine is injected
 * (`equityAt(stop)` returns lib/equity.js's result or null), so nothing here
 * models a hand and nothing here is a check.
 *
 * A cell is a number only where the pods would show one at that stop (every
 * live hand on its face, lib/replay-stops.js equityAllowedAt); a seat that
 * folded before the street reads `folded`; an unknown hand reads `unknown`.
 * Nothing is invented for a street the hand never reached.
 */

/** The four streets of the line, in order, with the board count each carries. */
export const LINE_STREETS = Object.freeze([
  Object.freeze({ label: 'Pre-flop', board: 0 }),
  Object.freeze({ label: 'Flop', board: 3 }),
  Object.freeze({ label: 'Turn', board: 4 }),
  Object.freeze({ label: 'River', board: 5 }),
]);

/**
 * The stop each street's cell reads: the first stop whose board has that many
 * cards (the deal for pre-flop, the reveal for the rest), or null when the
 * hand ended before it.
 * @param {ReadonlyArray<{board:number}>} stops
 */
export function lineStopsFor(stops) {
  return LINE_STREETS.map((street) => {
    const stop = (stops || []).find((s) => s.board === street.board) || null;
    return { ...street, stop };
  });
}

/**
 * @param {object} args
 * @param {ReadonlyArray<object>} args.stops       lib/replay-stops.js buildStops
 * @param {ReadonlyArray<number>} args.seats       the seats in the hand
 * @param {(stop:object)=>object|null} args.equityAt   equity for a stop, or null
 * @returns {{
 *   streets: Array<{label:string, board:number, reached:boolean, method:string|null, trials:number|null}>,
 *   rows: Array<{seat:number, cells: Array<{street:string, board:number, share:number|null, method:string|null, state:'live'|'folded'|'unknown'|'unreached'}>}>,
 *   any: boolean
 * }}
 */
export function equityLineRows({ stops, seats, equityAt }) {
  const columns = lineStopsFor(stops).map((col) => {
    const result = col.stop ? equityAt(col.stop) : null;
    return { ...col, result };
  });
  const streets = columns.map((c) => ({
    label: c.label,
    board: c.board,
    reached: !!c.stop,
    method: c.result?.method ?? null,
    trials: c.result?.trials ?? null,
  }));
  const rows = (seats || []).map((seat) => ({
    seat,
    cells: columns.map((c) => cellFor(seat, c)),
  }));
  return { streets, rows, any: rows.some((r) => r.cells.some((c) => c.share !== null)) };
}

function cellFor(seat, column) {
  const base = { street: column.label, board: column.board, share: null, method: null };
  if (!column.stop) return { ...base, state: 'unreached' };
  if ((column.stop.folded || []).includes(seat)) return { ...base, state: 'folded' };
  const share = column.result?.bySeat?.get(seat);
  if (share === undefined || share === null) return { ...base, state: 'unknown' };
  return { ...base, share, method: column.result.method, state: 'live' };
}

/**
 * One sentence naming the methods the line used, for its footnote:
 * "Pre-flop Monte Carlo, 20,000 trials; flop to river exact."
 * @param {ReadonlyArray<{label:string, reached:boolean, method:string|null, trials:number|null}>} streets
 */
export function lineMethodNote(streets) {
  const shown = (streets || []).filter((s) => s.reached && s.method);
  if (shown.length === 0) return '';
  const parts = [];
  let run = null;
  for (const s of shown) {
    const key = `${s.method}|${s.method === 'monte-carlo' ? s.trials : ''}`;
    if (run && run.key === key) { run.to = s.label; continue; }
    if (run) parts.push(run);
    run = { key, from: s.label, to: s.label, method: s.method, trials: s.trials };
  }
  if (run) parts.push(run);
  return parts.map((p) => {
    const span = p.from === p.to ? p.from : `${p.from} to ${p.to}`;
    const how = p.method === 'exact' ? 'exact' : `Monte Carlo, ${Number(p.trials).toLocaleString('en-US')} trials`;
    return `${span} ${how}`;
  }).join('; ') + '.';
}
