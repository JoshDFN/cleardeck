// Helpers shared by the scene scripts.

import { HERO_PLAYER, OPPONENT_PLAYERS, icp } from '../lib/config.mjs';
import { devPlayerPrincipal } from '../lib/identities.mjs';
import {
  doAct, ensureTableBalance, phaseOf, playUntil, releaseSeats, resetTable, seat, setDisplayName,
  sitOutAfterHand, startHand, view,
} from '../lib/table-driver.mjs';

export const ALL_DEV_PLAYERS = [HERO_PLAYER, ...OPPONENT_PLAYERS];

/** Friendly, deterministic names so screenshots do not show raw principals. */
export const DISPLAY_NAMES = { 1: 'You', 2: 'Nakamoto', 3: 'Ada', 4: 'Turing' };

/**
 * Puts a table into a known state: every dev player leaves (chips back to
 * escrow), the controller resets the table, then the requested seats buy in with
 * exact stacks. Everything here is a real canister call.
 *
 * @param {object} ctx
 * @param {string} tableName e.g. 'table_2'
 * @param {Array<{player:number, seat:number, chips:bigint}>} seats
 */
export async function prepareTable(ctx, tableName, seats) {
  const tableId = ctx.tableIds[tableName];
  if (!tableId) throw new Error(`No local canister id for ${tableName}`);

  await releaseSeats(tableId, ALL_DEV_PLAYERS);
  resetTable(tableName, tableId);

  for (const s of seats) {
    await ensureTableBalance(s.player, tableId, s.chips);
    await seat(s.player, tableId, s.seat, s.chips);
    await setDisplayName(s.player, tableId, DISPLAY_NAMES[s.player] ?? `P${s.player}`);
  }

  const v = await view(HERO_PLAYER, tableId);
  return { tableId, tableName, view: v, phase: phaseOf(v) };
}

/** Empties a table completely (no seats). */
export async function emptyTable(ctx, tableName) {
  const tableId = ctx.tableIds[tableName];
  await releaseSeats(tableId, ALL_DEV_PLAYERS);
  resetTable(tableName, tableId);
  return { tableId, tableName, view: await view(HERO_PLAYER, tableId) };
}

/** The lobby-registered display name for a table canister id. */
export function tableDisplayName(ctx, tableName) {
  const name = ctx.tableNames[tableName];
  if (!name) throw new Error(`Lobby has no registered name for ${tableName}`);
  return name;
}

export const heroPrincipal = () => devPlayerPrincipal(HERO_PLAYER);

/**
 * Plays one complete heads-up hand to a real showdown and leaves it frozen at
 * HandComplete (the opponent sits out after the hand, so the canister's 3-second
 * auto-deal cannot find two active players). Used by the scenes that need a
 * finished hand on the books: hand history and the fairness proof.
 *
 * @param {object} ctx
 * @param {string} tableName
 * @param {{heroSeat?:number, oppPlayer?:number, oppSeat?:number}} [opts]
 */
export async function playCompletedHand(ctx, tableName, {
  heroSeat = 0, oppPlayer = 2, oppSeat = 1, heroChips = icp(12), oppChips = icp(18),
} = {}) {
  const { tableId } = await prepareTable(ctx, tableName, [
    { player: HERO_PLAYER, seat: heroSeat, chips: heroChips },
    { player: oppPlayer, seat: oppSeat, chips: oppChips },
  ]);
  await startHand(oppPlayer, tableId);
  await sitOutAfterHand([oppPlayer], tableId);

  const finished = await playUntil(
    tableId,
    {
      [heroSeat]: { player: HERO_PLAYER, act: (t) => doAct.allIn(t) },
      [oppSeat]: { player: oppPlayer, act: (t, v) => doAct.checkOrCall(t, v) },
    },
    (s) => phaseOf(s) === 'HandComplete' && s.last_hand_winners.length > 0,
    'one completed hand with winners',
    { timeoutMs: 90_000 },
  );
  return { tableId, view: finished };
}
