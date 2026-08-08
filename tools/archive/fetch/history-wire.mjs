// A Candid mirror of the ARCHIVE canister, written from
// `src/history_canister/history_canister.did`.
//
// NOT `src/declarations/history/history.did.js`. That generated client is stale
// and is missing, by name:
//
//   HandHistoryRecord.dealt_in       (opt vec DealtInSeat)
//   PlayerHandRecord.dealt_in        (opt bool)
//   PlayerHandRecord.contributed     (opt nat64)
//   PlayerHandRecord.left_mid_hand   (opt bool)
//   HandSummary.dealt_in_count       (opt nat8)
//
// `dealt_in` is the field SHUFFLE-SPEC section 4 tells a verifier to READ to get
// P, added because guessing P is FINDING 30 and reproduces the wrong board from
// the right seed. A client generated from src/declarations cannot see it at all:
// Candid record subtyping silently drops the fields the decoder does not declare,
// so the fetch succeeds and the field is simply absent. This tool would then have
// to guess P -- the exact mistake the field exists to prevent. Hence its own
// mirror, and hence `assertDealtInVisible()` below, which fails loudly if a
// future edit reintroduces the blindness.

import { IDL } from '@dfinity/candid';

const Suit = IDL.Variant({
  Diamonds: IDL.Null, Hearts: IDL.Null, Clubs: IDL.Null, Spades: IDL.Null,
});
const Rank = IDL.Variant({
  Ace: IDL.Null, Six: IDL.Null, Ten: IDL.Null, Two: IDL.Null, Eight: IDL.Null,
  Seven: IDL.Null, Five: IDL.Null, Four: IDL.Null, Jack: IDL.Null, King: IDL.Null,
  Nine: IDL.Null, Three: IDL.Null, Queen: IDL.Null,
});
const Card = IDL.Record({ rank: Rank, suit: Suit });

const PlayerAction = IDL.Variant({
  Bet: IDL.Nat64,
  PostBlind: IDL.Nat64,
  Call: IDL.Nat64,
  Fold: IDL.Null,
  Raise: IDL.Nat64,
  AllIn: IDL.Nat64,
  Check: IDL.Null,
});

const ActionRecord = IDL.Record({
  principal: IDL.Principal,
  action: PlayerAction,
  seat: IDL.Nat8,
  timestamp: IDL.Nat64,
  phase: IDL.Text,
});

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

const DealtInSeat = IDL.Record({ principal: IDL.Principal, seat: IDL.Nat8 });

const PlayerHandRecord = IDL.Record({
  dealt_in: IDL.Opt(IDL.Bool),
  final_hand_rank: IDL.Opt(HandRank),
  principal: IDL.Principal,
  seat: IDL.Nat8,
  hole_cards: IDL.Opt(IDL.Tuple(Card, Card)),
  left_mid_hand: IDL.Opt(IDL.Bool),
  amount_won: IDL.Nat64,
  ending_chips: IDL.Nat64,
  starting_chips: IDL.Nat64,
  position: IDL.Text,
  contributed: IDL.Opt(IDL.Nat64),
});

const ShuffleProofRecord = IDL.Record({
  timestamp: IDL.Nat64, seed_hash: IDL.Text, revealed_seed: IDL.Text,
});

const WinnerRecord = IDL.Record({
  principal: IDL.Principal,
  hand_rank: IDL.Opt(HandRank),
  seat: IDL.Nat8,
  pot_type: IDL.Text,
  amount: IDL.Nat64,
});

const HandHistoryRecord = IDL.Record({
  dealt_in: IDL.Opt(IDL.Vec(DealtInSeat)),
  small_blind: IDL.Nat64,
  dealer_seat: IDL.Nat8,
  ante: IDL.Nat64,
  hand_number: IDL.Nat64,
  flop: IDL.Opt(IDL.Tuple(Card, Card, Card)),
  hand_id: IDL.Nat64,
  rake: IDL.Nat64,
  turn: IDL.Opt(Card),
  table_id: IDL.Principal,
  actions: IDL.Vec(ActionRecord),
  total_pot: IDL.Nat64,
  players: IDL.Vec(PlayerHandRecord),
  big_blind: IDL.Nat64,
  timestamp: IDL.Nat64,
  went_to_showdown: IDL.Bool,
  shuffle_proof: ShuffleProofRecord,
  river: IDL.Opt(Card),
  winners: IDL.Vec(WinnerRecord),
});

const HandSummary = IDL.Record({
  player_count: IDL.Nat8,
  hand_number: IDL.Nat64,
  hand_id: IDL.Nat64,
  table_id: IDL.Principal,
  total_pot: IDL.Nat64,
  timestamp: IDL.Nat64,
  went_to_showdown: IDL.Bool,
  dealt_in_count: IDL.Opt(IDL.Nat8),
  winners: IDL.Vec(WinnerRecord),
});

const RetentionPolicy = IDL.Record({
  admin: IDL.Opt(IDL.Principal),
  max_age_before_discard: IDL.Opt(IDL.Nat64),
  summary: IDL.Text,
  records_are_append_only: IDL.Bool,
  records_held: IDL.Nat64,
});

export const historyIdl = ({ IDL: I }) => I.Service({
  get_hand: I.Func([I.Nat64], [I.Opt(HandHistoryRecord)], ['query']),
  get_recent_hands: I.Func([I.Nat64], [I.Vec(HandSummary)], ['query']),
  get_hands_by_table: I.Func([I.Principal, I.Nat64, I.Nat64], [I.Vec(HandSummary)], ['query']),
  get_hands_by_player: I.Func([I.Principal, I.Nat64, I.Nat64], [I.Vec(HandSummary)], ['query']),
  get_total_hands: I.Func([], [I.Nat64], ['query']),
  get_retention_policy: I.Func([], [RetentionPolicy], ['query']),
  get_authorized_tables: I.Func([], [I.Vec(I.Principal)], ['query']),
  get_admin: I.Func([], [I.Opt(I.Principal)], ['query']),
});

/**
 * Fails loudly if this mirror ever loses the field that states P.
 * Cheap, and it turns a silent regression into a startup error.
 */
export function assertDealtInVisible() {
  const fields = HandHistoryRecord._fields.map(([name]) => name);
  if (!fields.includes('dealt_in')) {
    throw new Error(
      'the archive mirror has lost HandHistoryRecord.dealt_in. Without it P must be guessed, ' +
      'and a guessed P reproduces the wrong board from the right seed (FINDING 30). Refusing to run.',
    );
  }
  const playerFields = PlayerHandRecord._fields.map(([name]) => name);
  for (const required of ['dealt_in', 'contributed', 'left_mid_hand']) {
    if (!playerFields.includes(required)) {
      throw new Error(`the archive mirror has lost PlayerHandRecord.${required}. Refusing to run.`);
    }
  }
}
