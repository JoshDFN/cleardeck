// The pipeline: bundle -> reconstructed, replayed, and either USABLE or excluded
// with a reason.
//
// Nothing downstream is allowed to see a hand that did not reconstruct from its
// seed or whose money did not add up. That is not tidiness: a statistic computed
// over a set that quietly includes the hands the tool could not check is exactly
// the instrument this project keeps finding -- correct totals, wrong recipients,
// every invariant silent.

import { reconstructHand } from './reconstruct.mjs';
import { replayHand } from './betting.mjs';
import { analyseHandEV } from './ev.mjs';

/**
 * @param {object} bundle parsed by lib/bundle.mjs
 * @param {{ev?:boolean, samples?:number, hindsight?:boolean, tables?:string[]}} [opts]
 */
export function analyse(bundle, { ev = false, samples = 100_000, hindsight = false, tables = null } = {}) {
  const usable = [];
  const excluded = [];
  const hands = tables ? bundle.hands.filter((h) => tables.includes(h.tableId)) : bundle.hands;

  for (const hand of hands) {
    let recon;
    try {
      recon = reconstructHand(hand);
    } catch (e) {
      excluded.push({ handId: hand.handId, reason: `reconstruction threw: ${e.message}` });
      continue;
    }
    if (recon.status !== 'VERIFIED') {
      excluded.push({
        handId: hand.handId,
        reason: `cards ${recon.status}`,
        detail: recon.problems,
        recon,
      });
      continue;
    }
    let replay;
    try {
      replay = replayHand(hand);
    } catch (e) {
      excluded.push({ handId: hand.handId, reason: `betting replay threw: ${e.message}`, recon });
      continue;
    }
    if (!replay.ok) {
      excluded.push({ handId: hand.handId, reason: 'money does not check out', detail: replay.problems, recon, replay });
      continue;
    }
    const entry = { hand, recon, replay, ev: null };
    if (ev) entry.ev = analyseHandEV(hand, recon, replay, { samples, hindsight });
    usable.push(entry);
  }

  return { usable, excluded, total: hands.length };
}
