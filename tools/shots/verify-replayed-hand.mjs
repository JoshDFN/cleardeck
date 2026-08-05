// Reads back the hand the `handreplay` scene last photographed and prints the
// canister's own record beside the log the replayer rendered, action by action.
//
// The record is decoded through lib/hand-record-wire.mjs, the harness's own
// Candid mirror, NOT through src/declarations (which is missing ActionRecord's
// `phase` and `amount`; docs/DEFECTS.md E-08 / H-32).
import { readLocalIds } from './lib/ids.mjs';
import { handRecordActor } from './lib/hand-record-wire.mjs';
import { optional } from './lib/agent.mjs';
import { HERO_PLAYER } from './lib/config.mjs';

const WORD = { Fold: 'folds', Check: 'checks', Call: 'calls', Bet: 'bets', Raise: 'raises to', AllIn: 'is all in for' };
const STREET = { preflop: 'Pre-flop', flop: 'Flop', turn: 'Turn', river: 'River' };
const icp = (e8s) => (Number(e8s) / 1e8).toFixed(2);
const face = (c) => `${Object.keys(c.rank)[0]}${Object.keys(c.suit)[0]}`;

const ids = readLocalIds();
const tableId = ids.table_1;
const t = await handRecordActor(HERO_PLAYER, tableId);
const v = optional(await t.get_table_view());
const n = Number(process.argv[2] || v.hand_number);
const rec = optional(await t.get_hand_history(BigInt(n)));

console.log(`table_1 ${tableId}  hand #${n}  (live view says hand ${v.hand_number}, phase ${Object.keys(v.phase)[0]})`);
console.log(`blinds from get_table_view().config: small ${v.config.small_blind} e8s (${icp(v.config.small_blind)} ICP), `
  + `big ${v.config.big_blind} e8s (${icp(v.config.big_blind)} ICP), ante ${v.config.ante}`);
console.log(`dealer seat ${v.dealer_seat}, small-blind seat ${v.small_blind_seat}, big-blind seat ${v.big_blind_seat}`);
console.log(`commitment  ${rec.shuffle_proof.seed_hash}`);
console.log(`seed        ${optional(rec.shuffle_proof.revealed_seed)}`);
console.log(`board       ${rec.community_cards.map(face).join(' ')}`);
console.log('');
console.log('  #  street     seat  action        record.amount   rendered as       what the log line must read');
console.log('  -  ------     ----  ------        -------------   ---------------   ---------------------------');
let line = 1;
console.log(`  ${String(line++).padStart(2)}  ${'Blinds'.padEnd(10)} ${String(v.small_blind_seat + 1).padEnd(5)} ${'PostBlind'.padEnd(13)} ${String(v.config.small_blind).padEnd(15)} ${(icp(v.config.small_blind) + ' ICP').padEnd(17)} Seat ${v.small_blind_seat + 1} posts the small blind ${icp(v.config.small_blind)} ICP`);
console.log(`  ${String(line++).padStart(2)}  ${'Blinds'.padEnd(10)} ${String(v.big_blind_seat + 1).padEnd(5)} ${'PostBlind'.padEnd(13)} ${String(v.config.big_blind).padEnd(15)} ${(icp(v.config.big_blind) + ' ICP').padEnd(17)} Seat ${v.big_blind_seat + 1} posts the big blind ${icp(v.config.big_blind)} ICP`);
let sum = Number(v.config.small_blind) + Number(v.config.big_blind);
for (const a of rec.actions) {
  const kind = Object.keys(a.action)[0];
  const amount = Number(a.amount);
  sum += amount;
  const shown = amount > 0 ? `${icp(amount)} ICP` : '(none)';
  console.log(`  ${String(line++).padStart(2)}  ${(STREET[a.phase] || `!${a.phase}!`).padEnd(10)} ${String(a.seat + 1).padEnd(5)} ${kind.padEnd(13)} ${String(amount).padEnd(15)} ${shown.padEnd(17)} Seat ${a.seat + 1} ${WORD[kind]}${amount > 0 ? ` ${icp(amount)} ICP` : ''}`);
}
const paid = rec.winners.reduce((s, w) => s + Number(w.amount), 0);
console.log('');
console.log(`blinds + every recorded amount = ${sum} e8s (${icp(sum)} ICP)`);
console.log(`winners were paid              = ${paid} e8s (${icp(paid)} ICP)`);
console.log(`the log's own audit line holds  : ${sum === paid ? 'YES, to the e8' : 'NO'}`);
console.log('');
for (const p of rec.showdown_players) {
  console.log(`showdown seat ${p.seat + 1} ${p.principal.toText().slice(0, 12)}… cards ${(optional(p.cards) || []).map(face).join(' ')} won ${p.amount_won} e8s`);
}
for (const w of rec.winners) console.log(`winner seat ${w.seat + 1} paid ${w.amount} e8s (${icp(w.amount)} ICP)`);
