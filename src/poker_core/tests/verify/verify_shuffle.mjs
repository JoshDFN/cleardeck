#!/usr/bin/env node
// Independent verifier for the ClearDeck provably-fair shuffle.
//
// Written from docs/SHUFFLE-SPEC.md, NOT from src/poker_core/src/shuffle.rs, and
// deliberately not sharing a line with the Python verifier next to it either.
// Node standard library only (`node:crypto` for SHA-256), no network, no ClearDeck
// code. Runs in Node; the arithmetic is BigInt so it is also copy-pasteable into a
// browser with `crypto.subtle` in place of `createHash`.
//
//   node verify_shuffle.mjs <seed-hex>
//   node verify_shuffle.mjs <seed-hex> --players 2
//   node verify_shuffle.mjs <seed-hex> --seed-hash <hex>
//   node verify_shuffle.mjs --vectors ../golden_vectors.txt
//
// Exit status is 0 only if everything asked for checked out.

import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

// --- section 2 of the spec: deck construction ------------------------------

const SUITS = ['h', 'd', 'c', 's']; // Hearts, Diamonds, Clubs, Spades: normative order
const RANKS = ['2', '3', '4', '5', '6', '7', '8', '9', 'T', 'J', 'Q', 'K', 'A'];

const TWO_POW_64 = 1n << 64n;

function buildDeck() {
  const deck = [];
  for (const suit of SUITS) for (const rank of RANKS) deck.push(rank + suit);
  return deck;
}

// --- section 3 of the spec: the shuffle ------------------------------------

/** floor(2^64 / n) * n: the largest multiple of n that is <= 2^64. */
function drawBound(n) {
  return (TWO_POW_64 / n) * n; // BigInt division truncates
}

function sha256(bytes) {
  return new Uint8Array(createHash('sha256').update(bytes).digest());
}

function littleEndianU64(bytes) {
  let value = 0n;
  for (let k = 7; k >= 0; k -= 1) value = (value << 8n) | BigInt(bytes[k]);
  return value;
}

function shuffle(seed) {
  const deck = buildDeck();
  let chain = seed;
  let rejections = 0;

  for (let i = deck.length - 1; i >= 1; i -= 1) {
    const n = BigInt(i + 1);
    const limit = drawBound(n);
    let draw;
    for (;;) {
      const input = new Uint8Array(chain.length + 1);
      input.set(chain, 0);
      input[chain.length] = i % 256;
      chain = sha256(input);
      draw = littleEndianU64(chain.subarray(0, 8));
      if (draw < limit) break;
      rejections += 1; // never observed in practice; see spec 3.3
    }
    const j = Number(draw % n);
    [deck[i], deck[j]] = [deck[j], deck[i]];
  }

  return { deck, rejections };
}

// --- section 4 of the spec: dealing order ---------------------------------

function deal(deck, players) {
  if (players < 2) throw new Error('a hand needs at least 2 players dealt in');
  if (2 * players + 8 > deck.length) throw new Error('not enough cards for that many players');
  const holes = [];
  for (let k = 0; k < players; k += 1) holes.push([deck[2 * k], deck[2 * k + 1]]);
  const base = 2 * players;
  return {
    holeCards: holes,
    burnBeforeFlop: deck[base],
    flop: [deck[base + 1], deck[base + 2], deck[base + 3]],
    burnBeforeTurn: deck[base + 4],
    turn: deck[base + 5],
    burnBeforeRiver: deck[base + 6],
    river: deck[base + 7],
  };
}

// --- golden-vector symbol encoding ----------------------------------------

const ALPHA = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';

const cardSymbol = (card) =>
  ALPHA[13 * SUITS.indexOf(card[1]) + RANKS.indexOf(card[0])];

const deckSymbols = (deck) => deck.map(cardSymbol).join('');

// --- CLI ------------------------------------------------------------------

const hexToBytes = (hex) => {
  if (!/^([0-9a-fA-F]{2})+$/.test(hex)) throw new Error('seed must be even-length hex');
  return Uint8Array.from(hex.match(/../g).map((h) => parseInt(h, 16)));
};

const bytesToHex = (bytes) =>
  Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');

function checkVectors(path) {
  let checked = 0;
  let failed = 0;
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (!line.startsWith('S ')) continue;
    const [, seedHex, expected] = line.trim().split(/\s+/);
    const { deck } = shuffle(hexToBytes(seedHex));
    const got = deckSymbols(deck);
    checked += 1;
    if (got !== expected) {
      failed += 1;
      console.log(`MISMATCH\n  seed     ${seedHex}\n  expected ${expected}\n  got      ${got}`);
    }
  }
  console.log(`checked ${checked} shuffle vectors, ${failed} mismatched`);
  return checked > 0 && failed === 0 ? 0 : 1;
}

/** Splits argv into `--name value` pairs, bare `--switches` and positionals. */
function parseArgs(argv) {
  const valued = new Set(['--vectors', '--players', '--seed-hash']);
  const flags = {};
  const positional = [];
  for (let at = 0; at < argv.length; at += 1) {
    const token = argv[at];
    if (valued.has(token)) {
      flags[token] = argv[at + 1];
      at += 1;
    } else if (token.startsWith('--')) {
      flags[token] = true;
    } else {
      positional.push(token);
    }
  }
  return { flags, positional };
}

function main(argv) {
  const { flags, positional } = parseArgs(argv);
  const flag = (name) => flags[name];
  const vectors = flag('--vectors');
  if (vectors) return checkVectors(vectors);

  const seedHex = positional[0];
  if (!seedHex) {
    console.error('usage: verify_shuffle.mjs <seed-hex> [--players N] [--seed-hash HEX] [--symbols]');
    return 2;
  }

  const seed = hexToBytes(seedHex);
  let status = 0;
  const computedHash = bytesToHex(sha256(seed));
  console.log(`seed        ${bytesToHex(seed)}  (${seed.length} bytes)`);
  console.log(`SHA256(seed) ${computedHash}`);
  const committed = flag('--seed-hash');
  if (committed) {
    if (computedHash.toLowerCase() === committed.trim().toLowerCase()) {
      console.log('commitment  OK: the revealed seed hashes to the committed seed_hash');
    } else {
      console.log(`commitment  MISMATCH: table committed to ${committed}`);
      status = 1;
    }
  }

  const { deck, rejections } = shuffle(seed);
  console.log(`rejections  ${rejections}`);
  console.log('deck');
  for (let row = 0; row < 4; row += 1) {
    const cards = deck.slice(row * 13, row * 13 + 13).map((c) => c.padStart(2, ' '));
    console.log(`  ${String(row * 13).padStart(2, ' ')}..${row * 13 + 12}  ${cards.join(' ')}`);
  }
  if (flags["--symbols"]) console.log(`symbols     ${deckSymbols(deck)}`);

  const players = flag('--players');
  if (players) {
    const hand = deal(deck, Number(players));
    console.log(`\nhand with ${players} players dealt in, seats in ascending order`);
    hand.holeCards.forEach(([a, b], seat) => console.log(`  seat ${seat}: ${a} ${b}`));
    console.log(`  flop:  ${hand.flop.join(' ')}   (burned ${hand.burnBeforeFlop})`);
    console.log(`  turn:  ${hand.turn}   (burned ${hand.burnBeforeTurn})`);
    console.log(`  river: ${hand.river}   (burned ${hand.burnBeforeRiver})`);
  }

  return status;
}

process.exit(main(process.argv.slice(2)));
