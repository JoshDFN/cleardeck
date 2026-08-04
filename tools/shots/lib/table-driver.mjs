// Scenario driver: reaches a real on-chain game state by making real calls.
//
// Every state a screenshot shows was produced by the table canister's own poker
// engine. The driver never writes game state directly, it only plays.

import { Principal } from '@dfinity/principal';
import {
  ICP_LEDGER_CANISTER_ID,
  ICP_TRANSFER_FEE_E8S,
  CONTROLLER_IDENTITY,
  FUNDER_IDENTITIES,
  ENV,
  STATE_POLL_MS,
  STATE_TIMEOUT_MS,
  TABLE_CONFIGS,
} from './config.mjs';
import { icp, resolveControllerIdentity } from './ids.mjs';

/**
 * Identity used for controller-only calls. Resolved once per process against the
 * real controller list of a real local canister (see resolveControllerIdentity),
 * because `local-up` deploys as the machine's current default identity, not
 * necessarily CONTROLLER_IDENTITY.
 *
 * `null` means "use the current default identity" (pass no --identity).
 * @type {{identity: string|null}|null}
 */
let controllerCache = null;

/**
 * @param {string} canisterId a local canister the caller must control
 * @returns {string[]} the `--identity <name>` argv fragment, possibly empty
 */
function controllerArgs(canisterId) {
  if (!controllerCache) {
    const found = resolveControllerIdentity(canisterId, {
      preferred: CONTROLLER_IDENTITY,
      candidates: FUNDER_IDENTITIES,
    });
    if (found.identity === undefined) {
      throw new Error(
        `No local icp identity controls ${canisterId}.\n` +
          `  controllers on the canister: ${found.controllers.join(', ') || '(none reported)'}\n` +
          `  identities tried: ${found.checked.map((c) => `${c.identity}=${c.principal}`).join(', ')}\n` +
          'Controller-only calls (reset_table) cannot be made, so scenes cannot be reset.\n' +
          `Redeploy the local backend as ${CONTROLLER_IDENTITY}, or add it as a controller.`,
      );
    }
    controllerCache = { identity: found.identity };
  }
  return controllerCache.identity ? ['--identity', controllerCache.identity] : [];
}

/** @returns {string} the controller identity in use, for the run manifest. */
export function controllerIdentityInUse() {
  return controllerCache ? (controllerCache.identity ?? '(default)') : '(not resolved yet)';
}
import { ledgerActor, optional, tableActor, unwrap, variantKey } from './agent.mjs';
import { devPlayer } from './identities.mjs';

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Candid literal for a TableConfig record. */
function configLiteral(cfg) {
  return (
    '(record { ' +
    `small_blind = ${cfg.small_blind} : nat64; ` +
    `big_blind = ${cfg.big_blind} : nat64; ` +
    `min_buy_in = ${cfg.min_buy_in} : nat64; ` +
    `max_buy_in = ${cfg.max_buy_in} : nat64; ` +
    `max_players = ${cfg.max_players} : nat8; ` +
    `action_timeout_secs = ${cfg.action_timeout_secs} : nat64; ` +
    `ante = ${cfg.ante} : nat64; ` +
    `time_bank_secs = ${cfg.time_bank_secs} : nat64; ` +
    `currency = variant { ${cfg.currency} } })`
  );
}

/**
 * Puts a local table back to a known-empty state so runs are idempotent.
 * `reset_table` is controller-only and only touches TABLE/TABLE_CONFIG — escrow
 * balances (BALANCES) survive, so re-runs do not need to re-fund from scratch.
 * Chips that were sitting at the table when it is reset are destroyed; that is
 * fine for local play money, and it is the documented behaviour (see
 * docs/FINDING-01-chip-destruction.md).
 */
export function resetTable(tableName, canisterId) {
  const cfg = TABLE_CONFIGS[tableName];
  if (!cfg) throw new Error(`No config literal known for table "${tableName}"`);
  const out = icp([
    'canister', 'call', canisterId, 'reset_table', configLiteral(cfg),
    '-e', ENV, ...controllerArgs(canisterId),
  ]);
  if (!/Ok/.test(out)) throw new Error(`reset_table(${tableName}) did not return Ok: ${out}`);
  return out.trim();
}

/**
 * Returns every dev player's chips to their escrow balance before a reset, so a
 * long screenshot session does not burn through the local funders.
 * `leave_table` mid-hand acts as a fold, which is what we want between scenes.
 */
export async function releaseSeats(tableCanisterId, playerNums) {
  const released = [];
  for (const n of playerNums) {
    const table = await tableActor(tableCanisterId, devPlayer(n));
    try {
      const returned = unwrap(await table.leave_table(), 'leave_table');
      released.push({ player: n, returned });
    } catch (e) {
      // "Not at table" / "Player not found" are the canister's own strings for a
      // seat we never occupied — expected on a fresh run, not an error.
      const msg = String(e.message || e);
      if (!/not at table|player not found|not seated/i.test(msg)) {
        released.push({ player: n, error: msg.split('\n')[0] });
      }
    }
  }
  return released;
}

/**
 * Transfers local ICP to a principal via the real ledger, trying each funder
 * identity in turn so one drained throwaway does not stop a run.
 */
export function fundPrincipalFromFunder(principalText, amountE8s) {
  const arg =
    `(record { to = record { owner = principal "${principalText}" }; ` +
    `amount = ${amountE8s} : nat })`;
  const failures = [];
  for (const identity of FUNDER_IDENTITIES) {
    try {
      const out = icp([
        'canister', 'call', ICP_LEDGER_CANISTER_ID, 'icrc1_transfer', arg,
        '-e', ENV, '--identity', identity,
      ]);
      if (/Err/.test(out)) {
        failures.push(`${identity}: ${out.trim()}`);
        continue;
      }
      return { identity, output: out.trim() };
    } catch (e) {
      failures.push(`${identity}: ${String(e.message || e).split('\n')[0]}`);
    }
  }
  throw new Error(
    `Could not fund ${principalText} with ${amountE8s} e8s from any local funder.\n` +
      failures.join('\n'),
  );
}

/** Ledger balance of a principal, read from the real ledger. */
export async function ledgerBalance(principalText) {
  const ledger = await ledgerActor(ICP_LEDGER_CANISTER_ID);
  return ledger.icrc1_balance_of({
    owner: Principal.fromText(principalText),
    subaccount: [],
  });
}

/**
 * Makes sure a dev player has at least `minE8s` in the ledger, topping up from
 * the funder identity when short.
 */
export async function ensureLedgerFunds(playerNum, minE8s) {
  const principalText = devPlayer(playerNum).getPrincipal().toText();
  const balance = await ledgerBalance(principalText);
  if (balance >= minE8s) return { principalText, balance, topped: 0n };
  const need = minE8s - balance + ICP_TRANSFER_FEE_E8S * 4n;
  fundPrincipalFromFunder(principalText, need);
  const after = await ledgerBalance(principalText);
  return { principalText, balance: after, topped: need };
}

/**
 * Real deposit path: icrc2_approve then table.deposit (ICRC-2 transfer_from).
 * This is the exact flow a real user goes through — no test faucet.
 */
export async function ensureTableBalance(playerNum, tableCanisterId, targetE8s) {
  const identity = devPlayer(playerNum);
  const table = await tableActor(tableCanisterId, identity);
  const have = await table.get_balance();
  if (have >= targetE8s) return { balance: have, deposited: 0n };

  const shortfall = targetE8s - have;
  await ensureLedgerFunds(playerNum, shortfall + ICP_TRANSFER_FEE_E8S * 4n);

  const ledger = await ledgerActor(ICP_LEDGER_CANISTER_ID, identity);
  unwrap(
    await ledger.icrc2_approve({
      amount: shortfall + ICP_TRANSFER_FEE_E8S * 2n,
      spender: { owner: Principal.fromText(tableCanisterId), subaccount: [] },
      fee: [], memo: [], from_subaccount: [], created_at_time: [],
      expected_allowance: [], expires_at: [],
    }),
    'icrc2_approve',
  );
  unwrap(await table.deposit(shortfall), 'table.deposit');
  const after = await table.get_balance();
  return { balance: after, deposited: shortfall };
}

/** Seats a dev player with an exact chip stack (buy_in, not join_table's minimum). */
export async function seat(playerNum, tableCanisterId, seatIndex, chipsE8s) {
  await ensureTableBalance(playerNum, tableCanisterId, chipsE8s);
  const table = await tableActor(tableCanisterId, devPlayer(playerNum));
  unwrap(await table.buy_in(seatIndex, chipsE8s), `buy_in(seat ${seatIndex})`);
  return table;
}

/** Sets the display name the table shows for a player (real canister call). */
export async function setDisplayName(playerNum, tableCanisterId, name) {
  const table = await tableActor(tableCanisterId, devPlayer(playerNum));
  try {
    unwrap(await table.set_display_name(name === null ? [] : [name]), 'set_display_name');
  } catch (e) {
    // Names are cosmetic; a rejected name must not fail a whole scene.
    return { ok: false, error: String(e.message || e) };
  }
  return { ok: true };
}

export async function startHand(playerNum, tableCanisterId) {
  const table = await tableActor(tableCanisterId, devPlayer(playerNum));
  return withRateLimitRetry(
    async () => unwrap(await table.start_new_hand(), 'start_new_hand'),
    'start_new_hand',
  );
}

/** A table actor authenticated as a given dev player. */
export function tableActorFor(playerNum, tableCanisterId) {
  return tableActor(tableCanisterId, devPlayer(playerNum));
}

/** Reads the table view as a given dev player (or anonymously with playerNum=null). */
export async function view(playerNum, tableCanisterId) {
  const identity = playerNum === null ? undefined : devPlayer(playerNum);
  const table = await tableActor(tableCanisterId, identity);
  return optional(await table.get_table_view());
}

export function phaseOf(v) {
  return v ? variantKey(v.phase) : null;
}

/** Seat indexes of players that are currently all in. */
export function allInSeats(v) {
  if (!v) return [];
  return v.players
    .map((p) => optional(p))
    .filter((p) => p && p.is_all_in)
    .map((p) => Number(p.seat));
}

export function seatedSeats(v) {
  if (!v) return [];
  return v.players.map((p) => optional(p)).filter(Boolean).map((p) => Number(p.seat));
}

/**
 * Polls the real table view until `predicate` is satisfied.
 * @param {number|null} asPlayer
 * @param {string} tableCanisterId
 * @param {(v:any)=>boolean} predicate
 * @param {string} label
 */
export async function waitForState(asPlayer, tableCanisterId, predicate, label, {
  timeoutMs = STATE_TIMEOUT_MS, pollMs = STATE_POLL_MS,
} = {}) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    last = await view(asPlayer, tableCanisterId);
    if (last && predicate(last)) return last;
    await sleep(pollMs);
  }
  throw new Error(
    `Timed out waiting for on-chain state: ${label}. ` +
      `Last observed: phase=${phaseOf(last)} action_on=${last?.action_on} ` +
      `pot=${last?.pot} side_pots=${last?.side_pots?.length} seated=${seatedSeats(last)}`,
  );
}

/**
 * Plays the hand with a per-seat action plan until `stopWhen` is satisfied.
 *
 * `plan` maps a seat index to a function that receives the current view and
 * performs the action for that seat (or returns 'hold' to leave the action
 * pending there). Only seats owned by dev players can act.
 *
 * @param {string} tableCanisterId
 * @param {Record<number, {player:number, act:(table:any,v:any)=>Promise<any>|'hold'}>} plan
 * @param {(v:any)=>boolean} stopWhen
 */
export async function playUntil(tableCanisterId, plan, stopWhen, label, {
  timeoutMs = STATE_TIMEOUT_MS,
} = {}) {
  const deadline = Date.now() + timeoutMs;
  let v = null;
  while (Date.now() < deadline) {
    // Read as the seat that is on the clock so `is_my_turn` is authoritative.
    const anyPlayer = Object.values(plan)[0]?.player ?? 1;
    v = await view(anyPlayer, tableCanisterId);
    if (v && stopWhen(v)) return v;
    if (!v) { await sleep(STATE_POLL_MS); continue; }

    const phase = phaseOf(v);
    if (phase === 'WaitingForPlayers' || phase === 'HandComplete') {
      await sleep(STATE_POLL_MS);
      continue;
    }
    const onSeat = Number(v.action_on);
    const entry = plan[onSeat];
    if (!entry || entry.hold) {
      await sleep(STATE_POLL_MS);
      continue;
    }
    const table = await tableActor(tableCanisterId, devPlayer(entry.player));
    const seatView = await view(entry.player, tableCanisterId);
    if (!seatView?.is_my_turn) { await sleep(STATE_POLL_MS); continue; }
    await entry.act(table, seatView);
    // Breathe between actions: keeps us far under the canister's 10 calls/second
    // rate limit and gives an act() that deliberately did nothing a chance to
    // let stopWhen fire instead of spinning.
    await sleep(STATE_POLL_MS);
  }
  throw new Error(
    `Timed out playing to target state: ${label}. Last: phase=${phaseOf(v)} ` +
      `action_on=${v?.action_on} pot=${v?.pot} side_pots=${v?.side_pots?.length}`,
  );
}

/**
 * The table canister rate-limits player_action / start_new_hand to 10 calls per
 * caller per second. The driver stays well under that, but a retry keeps a run
 * from dying on a burst.
 */
async function withRateLimitRetry(fn, label, attempts = 4) {
  let lastError;
  for (let i = 0; i < attempts; i += 1) {
    try {
      return await fn();
    } catch (e) {
      lastError = e;
      if (!/rate limit/i.test(String(e.message || e))) throw e;
      await sleep(1100);
    }
  }
  throw new Error(`${label} kept hitting the rate limit: ${lastError?.message}`);
}

/** Player actions, each awaiting the real update call. */
export const doAct = {
  fold: (t) => withRateLimitRetry(async () => unwrap(await t.player_action({ Fold: null }), 'fold'), 'fold'),
  check: (t) => withRateLimitRetry(async () => unwrap(await t.player_action({ Check: null }), 'check'), 'check'),
  call: (t) => withRateLimitRetry(async () => unwrap(await t.player_action({ Call: null }), 'call'), 'call'),
  allIn: (t) => withRateLimitRetry(async () => unwrap(await t.player_action({ AllIn: null }), 'allIn'), 'allIn'),
  raise: (amount) => (t) =>
    withRateLimitRetry(async () => unwrap(await t.player_action({ Raise: BigInt(amount) }), 'raise'), 'raise'),
  bet: (amount) => (t) =>
    withRateLimitRetry(async () => unwrap(await t.player_action({ Bet: BigInt(amount) }), 'bet'), 'bet'),
  checkOrCall: (t, v) =>
    v.can_check ? doAct.check(t) : doAct.call(t),
};

/**
 * Asks every seat except `keepSeats` to sit out after the current hand. Once
 * fewer than two players are active the canister's auto-deal can no longer
 * start a new hand, which freezes a completed hand on screen for as long as we
 * need. This is a real engine feature (sit_out_next_hand), not a stub.
 */
export async function sitOutAfterHand(playerNums, tableCanisterId) {
  for (const n of playerNums) {
    const table = await tableActor(tableCanisterId, devPlayer(n));
    try {
      unwrap(await table.sit_out_next_hand(), 'sit_out_next_hand');
    } catch (e) {
      // "Not at table" (the canister's own string) is not fatal here.
      if (!/not at table|already/i.test(String(e.message || e))) throw e;
    }
  }
}
