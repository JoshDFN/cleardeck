// AN INDEPENDENT CANDID MIRROR OF THE HAND RECORD, FOR THE HARNESS ONLY.
//
// WHY THE HARNESS MUST NOT SHARE THE APP'S DECLARATION
// ----------------------------------------------------
// `lib/agent.mjs` loads `src/declarations/table_1/table_1.did.js` on purpose: the
// driver should talk to the canister through the same client the app does. For
// most things that is exactly right. For the ASSERTION SIDE of a comparison it is
// wrong, and the first run of the `handreplay` scene proved it.
//
// That declaration is generated from `src/table_canister/table_canister.did`,
// which is stale (docs/DEFECTS.md E-08): `ActionRecord` was declared with only
// `{ action; seat; timestamp }` while the canister has always put `phase: text`
// and `amount: nat64` on the wire as well. Candid record subtyping means a
// decoder silently DROPS fields it does not declare, so the app could not render
// a call's amount or an action's street however it was written.
//
// When the scene read its "chain truth" through that same declaration, reverting
// the fix produced a failure -- but the wrong one. The scene's expected street
// became `null` too, and the mismatch it reported was
// `is under street "Street not recorded", not "null"`. The gate held by accident:
// two blind sides that happened to disagree. Had the client rendered "Street not
// recorded" while claiming it was the canister's answer, the scene would have
// agreed with it.
//
// So the harness decodes the hand record from ITS OWN type, mirrored from
// `src/table_canister/src/lib.rs` -- the same rule `tests/money_safety` and
// `tests/settlement` already follow for the same reason, stated in their own
// headers. A field the app's declaration loses is still visible here, which is
// what lets the scene say "the screen shows no amount but the canister recorded
// 5,000,000 e8s".
//
// WHAT IS MIRRORED, AND WHAT IS DELIBERATELY LEFT OUT
// ---------------------------------------------------
// Only what the assertions need. Candid lets a decoder omit record fields, so
// `hand_rank` and the rest are simply absent below. VARIANTS are the exception:
// a decoder must declare every tag the wire can carry, or the decode fails, so
// `Rank`, `Suit` and `GamePhase` are complete.

import { Actor } from '@dfinity/agent';
import { agentFor } from './agent.mjs';
import { devPlayer } from './identities.mjs';
import { assertNotMainnet } from './ids.mjs';

const idlFactory = ({ IDL }) => {
  // ---- complete variants (a decoder must know every tag) ----
  const Rank = IDL.Variant({
    Two: IDL.Null, Three: IDL.Null, Four: IDL.Null, Five: IDL.Null, Six: IDL.Null,
    Seven: IDL.Null, Eight: IDL.Null, Nine: IDL.Null, Ten: IDL.Null, Jack: IDL.Null,
    Queen: IDL.Null, King: IDL.Null, Ace: IDL.Null,
  });
  const Suit = IDL.Variant({
    Hearts: IDL.Null, Diamonds: IDL.Null, Clubs: IDL.Null, Spades: IDL.Null,
  });
  const Card = IDL.Record({ rank: Rank, suit: Suit });
  const PlayerAction = IDL.Variant({
    Fold: IDL.Null, Check: IDL.Null, Call: IDL.Null,
    Bet: IDL.Nat64, Raise: IDL.Nat64, AllIn: IDL.Null,
  });
  const GamePhase = IDL.Variant({
    WaitingForPlayers: IDL.Null, PreFlop: IDL.Null, Flop: IDL.Null, Turn: IDL.Null,
    River: IDL.Null, Showdown: IDL.Null, HandComplete: IDL.Null,
  });

  // ---- records, narrowed to what is asserted ----
  // THE POINT OF THIS FILE: `phase` and `amount` are declared here.
  const ActionRecord = IDL.Record({
    seat: IDL.Nat8,
    action: PlayerAction,
    timestamp: IDL.Nat64,
    phase: IDL.Text,
    amount: IDL.Nat64,
  });
  const Winner = IDL.Record({
    principal: IDL.Principal,
    seat: IDL.Nat8,
    amount: IDL.Nat64,
  });
  const ShowdownPlayer = IDL.Record({
    principal: IDL.Principal,
    seat: IDL.Nat8,
    cards: IDL.Opt(IDL.Tuple(Card, Card)),
    amount_won: IDL.Nat64,
  });
  const ShuffleProof = IDL.Record({
    timestamp: IDL.Nat64,
    seed_hash: IDL.Text,
    revealed_seed: IDL.Opt(IDL.Text),
  });
  const HandHistory = IDL.Record({
    hand_number: IDL.Nat64,
    shuffle_proof: ShuffleProof,
    actions: IDL.Vec(ActionRecord),
    winners: IDL.Vec(Winner),
    community_cards: IDL.Vec(Card),
    showdown_players: IDL.Vec(ShowdownPlayer),
  });
  const TableConfig = IDL.Record({
    small_blind: IDL.Nat64,
    big_blind: IDL.Nat64,
    ante: IDL.Nat64,
    max_players: IDL.Nat8,
  });
  const TableView = IDL.Record({
    config: TableConfig,
    hand_number: IDL.Nat64,
    dealer_seat: IDL.Nat8,
    small_blind_seat: IDL.Nat8,
    big_blind_seat: IDL.Nat8,
    phase: GamePhase,
    community_cards: IDL.Vec(Card),
  });

  return IDL.Service({
    get_hand_history: IDL.Func([IDL.Nat64], [IDL.Opt(HandHistory)], ['query']),
    get_table_view: IDL.Func([], [IDL.Opt(TableView)], ['query']),
  });
};

/**
 * An actor for the hand-record surface, decoding through the mirror above rather
 * than through `src/declarations`.
 *
 * @param {number} playerNum dev identity to sign as
 * @param {string} tableCanisterId
 */
export async function handRecordActor(playerNum, tableCanisterId) {
  assertNotMainnet(tableCanisterId, 'hand-record actor canister id');
  const agent = await agentFor(devPlayer(playerNum));
  return Actor.createActor(idlFactory, { agent, canisterId: tableCanisterId });
}
