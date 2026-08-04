export const idlFactory = ({ IDL }) => {
  const Result = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  const Rank = IDL.Variant({
    'Ace' : IDL.Null,
    'Six' : IDL.Null,
    'Ten' : IDL.Null,
    'Two' : IDL.Null,
    'Eight' : IDL.Null,
    'Seven' : IDL.Null,
    'Five' : IDL.Null,
    'Four' : IDL.Null,
    'Jack' : IDL.Null,
    'King' : IDL.Null,
    'Nine' : IDL.Null,
    'Three' : IDL.Null,
    'Queen' : IDL.Null,
  });
  const Suit = IDL.Variant({
    'Diamonds' : IDL.Null,
    'Hearts' : IDL.Null,
    'Clubs' : IDL.Null,
    'Spades' : IDL.Null,
  });
  const Card = IDL.Record({ 'rank' : Rank, 'suit' : Suit });
  const PlayerAction = IDL.Variant({
    'Bet' : IDL.Nat64,
    'PostBlind' : IDL.Nat64,
    'Call' : IDL.Nat64,
    'Fold' : IDL.Null,
    'Raise' : IDL.Nat64,
    'AllIn' : IDL.Nat64,
    'Check' : IDL.Null,
  });
  const ActionRecord = IDL.Record({
    'principal' : IDL.Principal,
    'action' : PlayerAction,
    'seat' : IDL.Nat8,
    'timestamp' : IDL.Nat64,
    'phase' : IDL.Text,
  });
  const HandRank = IDL.Variant({
    'StraightFlush' : IDL.Nat8,
    'Straight' : IDL.Nat8,
    'Pair' : IDL.Tuple(IDL.Nat8, IDL.Vec(IDL.Nat8)),
    'FullHouse' : IDL.Tuple(IDL.Nat8, IDL.Nat8),
    'TwoPair' : IDL.Tuple(IDL.Nat8, IDL.Nat8, IDL.Nat8),
    'HighCard' : IDL.Vec(IDL.Nat8),
    'ThreeOfAKind' : IDL.Tuple(IDL.Nat8, IDL.Vec(IDL.Nat8)),
    'Flush' : IDL.Vec(IDL.Nat8),
    'RoyalFlush' : IDL.Null,
    'FourOfAKind' : IDL.Tuple(IDL.Nat8, IDL.Nat8),
  });
  const PlayerHandRecord = IDL.Record({
    'final_hand_rank' : IDL.Opt(HandRank),
    'principal' : IDL.Principal,
    'seat' : IDL.Nat8,
    'hole_cards' : IDL.Opt(IDL.Tuple(Card, Card)),
    'amount_won' : IDL.Nat64,
    'ending_chips' : IDL.Nat64,
    'starting_chips' : IDL.Nat64,
    'position' : IDL.Text,
  });
  const ShuffleProofRecord = IDL.Record({
    'timestamp' : IDL.Nat64,
    'seed_hash' : IDL.Text,
    'revealed_seed' : IDL.Text,
  });
  const WinnerRecord = IDL.Record({
    'principal' : IDL.Principal,
    'hand_rank' : IDL.Opt(HandRank),
    'seat' : IDL.Nat8,
    'pot_type' : IDL.Text,
    'amount' : IDL.Nat64,
  });
  const HandHistoryRecord = IDL.Record({
    'small_blind' : IDL.Nat64,
    'dealer_seat' : IDL.Nat8,
    'ante' : IDL.Nat64,
    'hand_number' : IDL.Nat64,
    'flop' : IDL.Opt(IDL.Tuple(Card, Card, Card)),
    'hand_id' : IDL.Nat64,
    'rake' : IDL.Nat64,
    'turn' : IDL.Opt(Card),
    'table_id' : IDL.Principal,
    'actions' : IDL.Vec(ActionRecord),
    'total_pot' : IDL.Nat64,
    'players' : IDL.Vec(PlayerHandRecord),
    'big_blind' : IDL.Nat64,
    'timestamp' : IDL.Nat64,
    'went_to_showdown' : IDL.Bool,
    'shuffle_proof' : ShuffleProofRecord,
    'river' : IDL.Opt(Card),
    'winners' : IDL.Vec(WinnerRecord),
  });
  const HandSummary = IDL.Record({
    'player_count' : IDL.Nat8,
    'hand_number' : IDL.Nat64,
    'hand_id' : IDL.Nat64,
    'table_id' : IDL.Principal,
    'total_pot' : IDL.Nat64,
    'timestamp' : IDL.Nat64,
    'went_to_showdown' : IDL.Bool,
    'winners' : IDL.Vec(WinnerRecord),
  });
  const PlayerStats = IDL.Record({
    'biggest_pot_won' : IDL.Nat64,
    'principal' : IDL.Principal,
    'showdowns_won' : IDL.Nat64,
    'hands_won' : IDL.Nat64,
    'total_winnings' : IDL.Int64,
    'hands_played' : IDL.Nat64,
    'showdowns_total' : IDL.Nat64,
  });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Nat64, 'Err' : IDL.Text });
  const Result_2 = IDL.Variant({ 'Ok' : IDL.Bool, 'Err' : IDL.Text });
  return IDL.Service({
    'authorize_table' : IDL.Func([IDL.Principal], [Result], []),
    'get_authorized_tables' : IDL.Func([], [IDL.Vec(IDL.Principal)], ['query']),
    'get_hand' : IDL.Func([IDL.Nat64], [IDL.Opt(HandHistoryRecord)], ['query']),
    'get_hands_by_player' : IDL.Func(
        [IDL.Principal, IDL.Nat64, IDL.Nat64],
        [IDL.Vec(HandSummary)],
        ['query'],
      ),
    'get_hands_by_table' : IDL.Func(
        [IDL.Principal, IDL.Nat64, IDL.Nat64],
        [IDL.Vec(HandSummary)],
        ['query'],
      ),
    'get_player_stats' : IDL.Func(
        [IDL.Principal],
        [IDL.Opt(PlayerStats)],
        ['query'],
      ),
    'get_recent_hands' : IDL.Func(
        [IDL.Nat64],
        [IDL.Vec(HandSummary)],
        ['query'],
      ),
    'get_table_hand_count' : IDL.Func([IDL.Principal], [IDL.Nat64], ['query']),
    'get_total_hands' : IDL.Func([], [IDL.Nat64], ['query']),
    'record_hand' : IDL.Func([HandHistoryRecord], [Result_1], []),
    'revoke_table' : IDL.Func([IDL.Principal], [Result], []),
    'verify_hand_shuffle' : IDL.Func([IDL.Nat64], [Result_2], ['query']),
  });
};
export const init = ({ IDL }) => { return []; };
