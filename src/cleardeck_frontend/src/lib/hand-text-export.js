/**
 * A hand as the text dialect every study tool imports.
 *
 * PokerTracker, Hold'em Manager and GTO Wizard all read the PokerStars text
 * format; a room whose history they cannot ingest is judged by serious players
 * on that alone. This writes one hand in that shape from the same normalised
 * record the replayer renders (hand-record.js gives every line its street, verb
 * and amount), and appends the shuffle proof as comment lines so the proof
 * travels with the hand.
 *
 * Nothing is invented: a seat the record does not name is "Seat N"; a stack
 * the record does not carry is not printed; a street with no actions is still
 * headed so the board reads in order. Amounts are formatted by the caller's
 * money formatter so a ckBTC hand prints in sats.
 */

import { BOARD_AFTER, STREET_ORDER, STREET_UNKNOWN } from './hand-record.js';
import { codeOf, rankName } from './hand-history-records.js';

const STREET_HEADER = { 'Pre-flop': 'HOLE CARDS', Flop: 'FLOP', Turn: 'TURN', River: 'RIVER' };

/** "[7c 4h Jh]" for the cards a street shows, "" when none. */
function boardTag(community, upTo) {
  const codes = (community || []).slice(0, upTo).map(codeOf).filter(Boolean);
  return codes.length ? `[${codes.join(' ')}]` : '';
}

const VERB_TEXT = {
  Fold: 'folds', Check: 'checks', Call: 'calls', Bet: 'bets', Raise: 'raises to', AllIn: 'is all in for',
};

/**
 * @param {object} args
 * @param {object} args.hand the normalised hand (hand-history-records.js)
 * @param {ReturnType<import('./hand-record.js').blindsForHand>|null} args.blinds
 * @param {Array<{street:string, lines:object[]}>} args.logGroups
 * @param {(amount:number)=>string} args.money
 * @param {(seat:number)=>string} [args.nameOf] the label for a seat
 * @param {string} [args.tableName]
 * @param {string} [args.currency]
 * @param {Date|null} [args.playedAt]
 */
export function handToText({
  hand, blinds, logGroups, money, nameOf = (s) => `Seat ${s + 1}`, tableName = 'ClearDeck table',
  currency = 'ICP', playedAt = null,
}) {
  const out = [];
  const level = blinds?.level || null;
  const stakes = level ? ` (${money(level.small)}/${money(level.big)} ${currency})` : '';
  const when = playedAt ? ` - ${playedAt.toISOString().replace('T', ' ').slice(0, 19)} UTC` : '';
  out.push(`ClearDeck Hand #${hand.handNumber}: Hold'em No Limit${stakes}${when}`);
  out.push(`Table '${tableName}' ${Math.max(2, hand.seats.length)}-max`);
  for (const seat of hand.seats) out.push(`Seat ${seat + 1}: ${nameOf(seat)}`);

  const groups = logGroups || [];
  const blindGroup = groups.find((g) => g.street === 'Blinds');
  for (const line of blindGroup?.lines || []) {
    const who = line.seat === null || line.seat === undefined ? 'A seat' : nameOf(line.seat);
    const which = line.kind === 'PostAnte' ? 'posts the ante' : line.word.replace('posts the ', 'posts ');
    out.push(`${who}: ${which} ${money(line.amount)}`);
  }

  for (const street of STREET_ORDER) {
    const group = groups.find((g) => g.street === street);
    const board = BOARD_AFTER[street];
    if (street !== 'Pre-flop' && board > (hand.community || []).length) break;
    const tag = street === 'Pre-flop' ? '' : ` ${boardTag(hand.community, board)}`;
    out.push(`*** ${STREET_HEADER[street]} ***${tag}`);
    for (const line of group?.lines || []) out.push(actionLine(line, nameOf, money));
  }
  const unknown = groups.find((g) => g.street === STREET_UNKNOWN);
  if (unknown) {
    out.push('*** STREET NOT RECORDED ***');
    for (const line of unknown.lines) out.push(actionLine(line, nameOf, money));
  }

  if ((hand.showdown || []).length) {
    out.push('*** SHOW DOWN ***');
    for (const p of hand.showdown) {
      const cards = p.cards ? ` [${[p.cards[0], p.cards[1]].map(codeOf).join(' ')}]` : '';
      const rank = rankName(p.rank);
      out.push(`${nameOf(p.seat)}: shows${cards}${rank ? ` (${rank})` : ''}`);
    }
  }
  for (const w of hand.winners || []) {
    out.push(`${nameOf(w.seat)} collected ${money(w.amount)} from pot`);
  }
  out.push('*** SUMMARY ***');
  out.push(`Total pot ${money(hand.potTotal)} | Rake ${money(0)}`);
  const fullBoard = boardTag(hand.community, 5);
  if (fullBoard) out.push(`Board ${fullBoard}`);
  out.push(`# seed_hash ${hand.proof?.seedHash || 'n/a'}`);
  out.push(`# revealed_seed ${hand.proof?.revealedSeed || 'not revealed'}`);
  out.push('# Re-derive the deck from the revealed seed with docs/SHUFFLE-SPEC.md.');
  return out.join('\n');
}

function actionLine(line, nameOf, money) {
  const who = line.seat === null || line.seat === undefined ? 'A seat' : nameOf(line.seat);
  const verb = VERB_TEXT[line.kind] || line.word || String(line.kind).toLowerCase();
  const amount = line.amount ? ` ${money(line.amount)}` : '';
  return `${who}: ${verb}${amount}`;
}
