// Scene registry, in run order.
//
// Order matters only for speed: the table_2 scenes run together so the local
// replica does fewer resets.

import lobby from './lobby.mjs';
import tableEmpty from './table-empty.mjs';
import tablePreflop from './table-preflop.mjs';
import tableFacingBet from './table-facing-bet.mjs';
import tableAllin from './table-allin.mjs';
import tableShowdown from './table-showdown.mjs';
import tableSidepots from './table-sidepots.mjs';
import deposit from './deposit.mjs';
import handhistory from './handhistory.mjs';
import shuffleproof from './shuffleproof.mjs';

export const SCENES = [
  lobby,
  tableEmpty,
  tablePreflop,
  tableFacingBet,
  tableAllin,
  tableShowdown,
  tableSidepots,
  deposit,
  handhistory,
  shuffleproof,
];

export function scenesByName(names) {
  if (!names || names.length === 0) return SCENES;
  const set = new Set(names);
  const picked = SCENES.filter((s) => set.has(s.name));
  const missing = [...set].filter((n) => !SCENES.some((s) => s.name === n));
  if (missing.length) {
    throw new Error(`Unknown scene(s): ${missing.join(', ')}. Known: ${SCENES.map((s) => s.name).join(', ')}`);
  }
  return picked;
}
