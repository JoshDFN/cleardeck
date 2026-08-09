// AN INDEPENDENT CANDID MIRROR OF THE ARCHIVE, FOR THE HARNESS ONLY.
//
// WHY THE HARNESS MUST NOT SHARE THE APP'S DECLARATION
// ----------------------------------------------------
// The same reason `lib/hand-record-wire.mjs` exists, and this time it is not
// hypothetical: `src/declarations/history/history.did.js` is STALE
// (docs/DEFECTS.md E-67). It declares `PlayerHandRecord` without `dealt_in`,
// `contributed` or `left_mid_hand`, and `HandSummary` without `dealt_in_count`.
// Candid record subtyping means a decoder silently DROPS fields it does not
// declare, so a gate reading the archive through that file cannot see the fields
// FINDING 30 added, and would report them absent from a canister that is sending
// them.
//
// `hand_uid` (docs/DEFECTS.md E-71) is the field that made this file necessary
// rather than merely wise. It is the archive's only unique name for a hand, the
// gate below joins on it, and the app's declaration does not know it exists — so
// a gate reading through the app's declaration would find `undefined` on every
// record and either fail on every scene or, worse, quietly fall back to joining
// on `hand_number` again, which is the defect.
//
// WHAT IS MIRRORED
// ----------------
// Only what the assertions need, plus every tag of every variant the wire can
// carry (a decoder must declare all of them or the decode fails). `HandRank` is
// therefore complete, and `hole_cards` is declared because a verifier
// reconstructing a hand needs it.

import { Actor } from '@dfinity/agent';
import { agentFor } from './agent.mjs';
import { assertNotMainnet } from './ids.mjs';

const idlFactory = ({ IDL }) => {
  const Rank = IDL.Variant({
    Two: IDL.Null, Three: IDL.Null, Four: IDL.Null, Five: IDL.Null, Six: IDL.Null,
    Seven: IDL.Null, Eight: IDL.Null, Nine: IDL.Null, Ten: IDL.Null, Jack: IDL.Null,
    Queen: IDL.Null, King: IDL.Null, Ace: IDL.Null,
  });
  const Suit = IDL.Variant({
    Hearts: IDL.Null, Diamonds: IDL.Null, Clubs: IDL.Null, Spades: IDL.Null,
  });
  const Card = IDL.Record({ rank: Rank, suit: Suit });
  const HandRank = IDL.Variant({
    StraightFlush: IDL.Nat8,
    Straight: IDL.Nat8,
    Pair: IDL.Tuple(IDL.Nat8, IDL.Vec(IDL.Nat8)),
    FullHouse: IDL.Tuple(IDL.Nat8, IDL.Nat8),
    TwoPair: IDL.Tuple(IDL.Nat8, IDL.Nat8, IDL.Nat8),
    HighCard: IDL.Vec(IDL.Nat8),
    ThreeOfAKind: IDL.Tuple(IDL.Nat8, IDL.Vec(IDL.Nat8)),
    Flush: IDL.Vec(IDL.Nat8),
    RoyalFlush: IDL.Null,
    FourOfAKind: IDL.Tuple(IDL.Nat8, IDL.Nat8),
  });
  const PlayerAction = IDL.Variant({
    Bet: IDL.Nat64, PostBlind: IDL.Nat64, Call: IDL.Nat64, Fold: IDL.Null,
    Raise: IDL.Nat64, AllIn: IDL.Nat64, Check: IDL.Null,
  });
  const ActionRecord = IDL.Record({
    principal: IDL.Principal, action: PlayerAction, seat: IDL.Nat8,
    timestamp: IDL.Nat64, phase: IDL.Text,
  });
  const DealtInSeat = IDL.Record({ principal: IDL.Principal, seat: IDL.Nat8 });
  const WinnerRecord = IDL.Record({
    principal: IDL.Principal, hand_rank: IDL.Opt(HandRank), seat: IDL.Nat8,
    pot_type: IDL.Text, amount: IDL.Nat64,
  });
  const PlayerHandRecord = IDL.Record({
    dealt_in: IDL.Opt(IDL.Bool), final_hand_rank: IDL.Opt(HandRank), principal: IDL.Principal,
    seat: IDL.Nat8, hole_cards: IDL.Opt(IDL.Tuple(Card, Card)), left_mid_hand: IDL.Opt(IDL.Bool),
    amount_won: IDL.Nat64, ending_chips: IDL.Nat64, starting_chips: IDL.Nat64,
    position: IDL.Text, contributed: IDL.Opt(IDL.Nat64),
  });
  const ShuffleProofRecord = IDL.Record({
    timestamp: IDL.Nat64, seed_hash: IDL.Text, revealed_seed: IDL.Text,
  });
  // THE POINT OF THIS FILE: `hand_uid` is declared, on both shapes.
  const HandHistoryRecord = IDL.Record({
    dealt_in: IDL.Opt(IDL.Vec(DealtInSeat)), small_blind: IDL.Nat64, dealer_seat: IDL.Nat8,
    ante: IDL.Nat64, hand_number: IDL.Nat64, flop: IDL.Opt(IDL.Tuple(Card, Card, Card)),
    hand_id: IDL.Nat64, rake: IDL.Nat64, turn: IDL.Opt(Card), table_id: IDL.Principal,
    actions: IDL.Vec(ActionRecord), total_pot: IDL.Nat64, players: IDL.Vec(PlayerHandRecord),
    big_blind: IDL.Nat64, timestamp: IDL.Nat64, hand_uid: IDL.Opt(IDL.Text),
    went_to_showdown: IDL.Bool, shuffle_proof: ShuffleProofRecord, river: IDL.Opt(Card),
    winners: IDL.Vec(WinnerRecord),
  });
  const HandSummary = IDL.Record({
    player_count: IDL.Nat8, hand_number: IDL.Nat64, hand_id: IDL.Nat64, table_id: IDL.Principal,
    total_pot: IDL.Nat64, timestamp: IDL.Nat64, hand_uid: IDL.Text, went_to_showdown: IDL.Bool,
    dealt_in_count: IDL.Opt(IDL.Nat8), winners: IDL.Vec(WinnerRecord),
  });
  const ArchiveIntegrity = IDL.Record({
    distinct_hand_number_citations: IDL.Nat64,
    records_without_a_usable_commitment: IDL.Nat64,
    index_disagrees_with_records: IDL.Bool,
    name_collisions: IDL.Nat64,
    worst_hand_number_citation: IDL.Opt(IDL.Text),
    records_under_a_colliding_name: IDL.Nat64,
    records_with_a_name: IDL.Nat64,
    summary: IDL.Text,
    ambiguous_hand_number_citations: IDL.Nat64,
    records_held: IDL.Nat64,
    distinct_names: IDL.Nat64,
    records_under_an_ambiguous_hand_number: IDL.Nat64,
    records_with_a_nonzero_rake: IDL.Nat64,
    rake_recorded_total: IDL.Nat64,
    first_record_with_a_rake: IDL.Opt(IDL.Text),
  });
  const HandNumberCitation = IDL.Record({
    is_unique: IDL.Bool, hand_number: IDL.Nat64, table_id: IDL.Principal,
    matches: IDL.Vec(HandSummary), match_count: IDL.Nat64, advice: IDL.Text,
  });

  return IDL.Service({
    get_hand: IDL.Func([IDL.Nat64], [IDL.Opt(HandHistoryRecord)], ['query']),
    get_hand_by_uid: IDL.Func(
      [IDL.Text],
      [IDL.Variant({ Ok: HandHistoryRecord, Err: IDL.Text })],
      ['query'],
    ),
    get_hands_by_table: IDL.Func(
      [IDL.Principal, IDL.Nat64, IDL.Nat64], [IDL.Vec(HandSummary)], ['query'],
    ),
    resolve_hand_number: IDL.Func(
      [IDL.Principal, IDL.Nat64], [HandNumberCitation], ['query'],
    ),
    get_archive_integrity: IDL.Func([], [ArchiveIntegrity], ['query']),
    get_total_hands: IDL.Func([], [IDL.Nat64], ['query']),
  });
};

/**
 * An actor for the archive, decoding through the mirror above rather than
 * through `src/declarations/history`.
 *
 * @param {string} historyCanisterId
 */
export async function archiveActor(historyCanisterId) {
  assertNotMainnet(historyCanisterId, 'archive actor canister id');
  const agent = await agentFor(undefined);
  return Actor.createActor(idlFactory, { agent, canisterId: historyCanisterId });
}

export { idlFactory as archiveIdlFactory };
