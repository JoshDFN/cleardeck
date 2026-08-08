// The ONLY networked file in tools/archive.
//
// It pulls hand records out of an archive canister and writes a plain JSON
// bundle. Everything else in this tool reads the bundle and never opens a
// socket, so the analysis can be repeated by anyone, from data they fetched
// themselves, without trusting ClearDeck's server or this fetcher.
//
//   node tools/archive/fetch/fetch-archive.mjs --out artifacts/archive/bundle.json
//
// LOCAL ONLY by default. The mainnet canisters custody real funds; this file
// refuses any canister id listed in .icp/data/mappings/ic.ids.json, in any
// argument, and refuses the `ic` environment outright.

import { Actor, HttpAgent } from '@dfinity/agent';
import { Principal } from '@dfinity/principal';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { historyIdl, assertDealtInVisible } from './history-wire.mjs';
import { cardFromCandid } from '../lib/cards.mjs';
import { BUNDLE_VERSION } from '../lib/bundle.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const REPO_ROOT = path.resolve(HERE, '..', '..', '..');

/** Every ClearDeck MAINNET canister id, read from the file the CLI itself uses for `-e ic`. */
export function mainnetIds() {
  const f = path.join(REPO_ROOT, '.icp', 'data', 'mappings', 'ic.ids.json');
  if (!fs.existsSync(f)) return [];
  return Object.values(JSON.parse(fs.readFileSync(f, 'utf8'))).map(String);
}

/** Throws if any argument names a mainnet canister, anywhere inside it. */
export function assertNotMainnet(...args) {
  const ids = mainnetIds();
  for (const arg of args) {
    const s = String(arg ?? '');
    for (const id of ids) {
      if (id && s.includes(id)) {
        throw new Error(`REFUSING: "${s}" names ClearDeck MAINNET canister ${id}, which holds real funds`);
      }
    }
    if (/^ic$/i.test(s)) throw new Error('REFUSING: the `ic` environment is off limits to this tool');
  }
}

/** Local canister ids the CLI wrote for the running replica. */
export function readLocalIds() {
  for (const rel of [
    ['.icp', 'cache', 'mappings', 'local.ids.json'],
    ['.icp', 'data', 'mappings', 'local.ids.json'],
  ]) {
    const f = path.join(REPO_ROOT, ...rel);
    if (fs.existsSync(f)) return JSON.parse(fs.readFileSync(f, 'utf8'));
  }
  throw new Error('no local.ids.json found: is the local replica up? (./scripts/dev.sh local-up)');
}

const opt = (v) => (Array.isArray(v) ? (v.length ? v[0] : null) : (v ?? null));
const u64 = (v) => String(v);
const variantKey = (v) => (v && typeof v === 'object' ? Object.keys(v)[0] : null);

const ACTION_AMOUNTS = new Set(['Call', 'Bet', 'Raise', 'AllIn', 'PostBlind']);

function normalizeAction(a) {
  const kind = variantKey(a.action);
  const payload = a.action[kind];
  return {
    seat: Number(a.seat),
    principal: a.principal.toText(),
    kind,
    amount: ACTION_AMOUNTS.has(kind) ? u64(payload) : '0',
    phase: a.phase,
    timestamp: u64(a.timestamp),
  };
}

function normalizeHandRank(hr) {
  const v = opt(hr);
  if (v === null) return null;
  const kind = variantKey(v);
  const payload = v[kind];
  const plain = (x) => (typeof x === 'bigint' ? Number(x)
    : Array.isArray(x) ? x.map(plain)
      : x instanceof Uint8Array ? Array.from(x)
        : x === null ? null : x);
  return { kind, payload: plain(payload) };
}

function normalizePlayer(p) {
  const hole = opt(p.hole_cards);
  return {
    seat: Number(p.seat),
    principal: p.principal.toText(),
    starting_chips: u64(p.starting_chips),
    ending_chips: u64(p.ending_chips),
    hole_cards: hole ? [cardFromCandid(hole[0]), cardFromCandid(hole[1])] : null,
    final_hand_rank: normalizeHandRank(p.final_hand_rank),
    amount_won: u64(p.amount_won),
    position: p.position,
    dealt_in: opt(p.dealt_in),
    contributed: p.contributed && p.contributed.length ? u64(p.contributed[0]) : null,
    left_mid_hand: opt(p.left_mid_hand),
  };
}

/** Decoded Candid record -> the plain JSON this tool's offline half reads. */
export function normalizeRecord(r) {
  const flop = opt(r.flop);
  const turn = opt(r.turn);
  const river = opt(r.river);
  const dealtIn = opt(r.dealt_in);
  return {
    hand_id: u64(r.hand_id),
    table_id: r.table_id.toText(),
    hand_number: u64(r.hand_number),
    timestamp: u64(r.timestamp),
    small_blind: u64(r.small_blind),
    big_blind: u64(r.big_blind),
    ante: u64(r.ante),
    shuffle_proof: {
      seed_hash: r.shuffle_proof.seed_hash,
      revealed_seed: r.shuffle_proof.revealed_seed,
      timestamp: u64(r.shuffle_proof.timestamp),
    },
    dealt_in: dealtIn ? dealtIn.map((d) => ({ seat: Number(d.seat), principal: d.principal.toText() })) : null,
    dealer_seat: Number(r.dealer_seat),
    players: r.players.map(normalizePlayer),
    flop: flop ? [cardFromCandid(flop[0]), cardFromCandid(flop[1]), cardFromCandid(flop[2])] : null,
    turn: turn ? cardFromCandid(turn) : null,
    river: river ? cardFromCandid(river) : null,
    actions: r.actions.map(normalizeAction),
    total_pot: u64(r.total_pot),
    rake: u64(r.rake),
    winners: r.winners.map((w) => ({
      seat: Number(w.seat),
      principal: w.principal.toText(),
      amount: u64(w.amount),
      hand_rank: normalizeHandRank(w.hand_rank),
      pot_type: w.pot_type,
    })),
    went_to_showdown: r.went_to_showdown,
  };
}

export async function fetchBundle({ canisterId, gateway, network = 'local', limit = 0 }) {
  assertNotMainnet(canisterId, gateway, network);
  assertDealtInVisible();

  const agent = await HttpAgent.create({
    host: gateway, verifyQuerySignatures: false, shouldFetchRootKey: true,
  });
  const archive = Actor.createActor(historyIdl, { agent, canisterId });

  const total = Number(await archive.get_total_hands());
  const policy = await archive.get_retention_policy();
  const tables = await archive.get_authorized_tables();

  const want = limit > 0 ? Math.min(limit, total) : total;
  const hands = [];
  const missing = [];
  // Hand ids are assigned 1..N by the archive. Walk them; a gap is reported, not skipped silently.
  const firstId = total > 0 ? Math.max(1, total - want + 1) : 1;
  for (let id = firstId; id <= total; id += 1) {
    const rec = opt(await archive.get_hand(BigInt(id)));
    if (rec === null) { missing.push(id); continue; }
    hands.push(normalizeRecord(rec));
  }

  return {
    cleardeck_archive_bundle: BUNDLE_VERSION,
    fetched_at: new Date().toISOString(),
    source: {
      kind: 'history_canister',
      canister_id: String(canisterId),
      gateway,
      network,
      authorized_tables: tables.map((t) => t.toText()),
      retention_summary: policy.summary,
      records_are_append_only: policy.records_are_append_only,
      admin: opt(policy.admin) ? opt(policy.admin).toText() : null,
      hand_ids_missing: missing,
    },
    total_hands_reported: total,
    hands,
  };
}

// --- CLI -------------------------------------------------------------------

function parseArgs(argv) {
  const out = { out: null, canister: null, gateway: null, limit: 0 };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--out') out.out = argv[++i];
    else if (a === '--canister') out.canister = argv[++i];
    else if (a === '--gateway') out.gateway = argv[++i];
    else if (a === '--limit') out.limit = Number(argv[++i]);
    else throw new Error(`unknown argument ${a}`);
  }
  return out;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const args = parseArgs(process.argv.slice(2));
  const gateway = args.gateway || process.env.CLEARDECK_GATEWAY || 'http://localhost:8077';
  const canisterId = args.canister || readLocalIds().history;
  // `--out` is REQUIRED, with no default inside the repository. A bundle is DATA
  // -- a few thousand hands is several megabytes of machine-generated JSON -- and
  // a default that dropped one into the working tree would put it in front of
  // every `git add .` forever. docs/DEFECTS.md E-60 is what that costs.
  const out = args.out;
  if (!out) {
    console.error(
      'give --out <path>. A bundle is several MB of generated JSON and there is deliberately no\n'
      + 'default inside the repository: write it somewhere untracked, e.g.\n'
      + '  node tools/archive/fetch/fetch-archive.mjs --out /tmp/cleardeck-bundle.json',
    );
    process.exit(2);
  }
  assertNotMainnet(canisterId, gateway);

  const bundle = await fetchBundle({ canisterId, gateway, limit: args.limit });
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, `${JSON.stringify(bundle, null, 2)}\n`);
  console.log(
    `wrote ${bundle.hands.length} hand record(s) from archive ${canisterId} (${gateway}) to ${out}`,
  );
  if (bundle.source.hand_ids_missing.length > 0) {
    console.log(`  NOTE: ${bundle.source.hand_ids_missing.length} hand id(s) returned nothing: `
      + bundle.source.hand_ids_missing.slice(0, 20).join(', '));
  }
}
