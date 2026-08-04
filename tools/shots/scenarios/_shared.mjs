// Helpers shared by the scene scripts.

import { HERO_PLAYER, OPPONENT_PLAYERS, icp } from '../lib/config.mjs';
import { devPlayerPrincipal } from '../lib/identities.mjs';
import {
  doAct, ensureTableBalance, phaseOf, playUntil, releaseSeats, resetTable, seat, setDisplayName,
  sitOutAfterHand, startHand, view,
} from '../lib/table-driver.mjs';

export const ALL_DEV_PLAYERS = [HERO_PLAYER, ...OPPONENT_PLAYERS];

/**
 * Reads the lobby's own settled/unsettled signal.
 *
 * `+page.svelte` publishes `data-lobby-state` on <main>: "loading" only while
 * there is genuinely nothing to show, "ready" once the lobby list is what the
 * user sees. Before that attribute existed, the "Loading tables..." block was a
 * SIBLING of <Lobby> rather than an either/or branch, so both rendered at once
 * and ~230 px of layout was in a screenshot or not depending on shutter timing
 * (docs/DEFECTS.md H-09).
 *
 * @param {import('playwright').Page} page
 * @returns {Promise<string|null>}
 */
export function lobbyStateOf(page) {
  return page.locator('main').first().getAttribute('data-lobby-state').catch(() => null);
}

/**
 * Waits until the lobby is settled: real state, not a sleep.
 *
 * Three conditions, all read from the live DOM:
 *   1. <main data-lobby-state="ready">         the app says it has finished loading
 *   2. the loading block is gone from the DOM   no half-rendered 230 px band
 *   3. `data-lobby-tables` matches the rows on screen
 *
 * Note on the player counts: they need no separate wait. `loadTables()` resolves
 * every `get_player_count` / `get_max_players` query into a local array and only
 * then assigns `tables`, and only then sets `loading = false`. So
 * `data-lobby-state="ready"` already implies every count in every row is a real,
 * resolved canister answer — there is no placeholder state to wait out. (An
 * earlier version of this helper tried to detect placeholders by looking for a
 * bare "-" in the row text and hung forever on the buy-in cell, `2.00 - 10.00`.)
 *
 * @param {import('playwright').Page} page
 * @param {{timeoutMs?:number, requireRows?:boolean}} [opts]
 */
export async function waitForLobbySettled(page, { timeoutMs = 60_000, requireRows = true } = {}) {
  await page.waitForFunction(
    (needRows) => {
      const main = document.querySelector('main');
      if (!main || main.getAttribute('data-lobby-state') !== 'ready') return false;
      if (document.querySelector('.loading-state')) return false;
      const rows = document.querySelectorAll('tbody tr').length;
      if (needRows && rows === 0) return false;
      const claimed = Number(main.getAttribute('data-lobby-tables'));
      return Number.isFinite(claimed) && claimed === rows;
    },
    requireRows,
    { timeout: timeoutMs },
  );
}

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
