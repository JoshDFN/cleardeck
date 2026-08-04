export const idlFactory = ({ IDL }) => {
  const Result = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  const TableStatus = IDL.Variant({
    'Paused' : IDL.Null,
    'Closed' : IDL.Null,
    'InProgress' : IDL.Null,
    'WaitingForPlayers' : IDL.Null,
  });
  const Currency = IDL.Variant({ 'BTC' : IDL.Null, 'ICP' : IDL.Null });
  const TableConfig = IDL.Record({
    'small_blind' : IDL.Nat64,
    'time_bank_secs' : IDL.Nat64,
    'action_timeout_secs' : IDL.Nat64,
    'ante' : IDL.Nat64,
    'min_buy_in' : IDL.Nat64,
    'max_players' : IDL.Nat8,
    'currency' : IDL.Opt(Currency),
    'big_blind' : IDL.Nat64,
    'max_buy_in' : IDL.Nat64,
  });
  const TableInfo = IDL.Record({
    'id' : IDL.Nat64,
    'status' : TableStatus,
    'player_count' : IDL.Nat8,
    'name' : IDL.Text,
    'canister_id' : IDL.Opt(IDL.Principal),
    'created_at' : IDL.Nat64,
    'created_by' : IDL.Principal,
    'currency' : IDL.Opt(Currency),
    'config' : TableConfig,
  });
  const PlayerProfile = IDL.Record({
    'principal' : IDL.Principal,
    'username' : IDL.Text,
    'created_at' : IDL.Nat64,
    'total_winnings' : IDL.Int64,
    'hands_played' : IDL.Nat64,
  });
  const StakeLevel = IDL.Variant({
    'Low' : IDL.Null,
    'VIP' : IDL.Null,
    'High' : IDL.Null,
    'Medium' : IDL.Null,
    'Micro' : IDL.Null,
  });
  const Result_1 = IDL.Variant({ 'Ok' : PlayerProfile, 'Err' : IDL.Text });
  return IDL.Service({
    'add_authorized_table' : IDL.Func([IDL.Principal], [Result], []),
    'add_btc_headsup_table' : IDL.Func([IDL.Principal], [Result], []),
    'get_admin' : IDL.Func([], [IDL.Opt(IDL.Principal)], ['query']),
    'get_authorized_tables' : IDL.Func([], [IDL.Vec(IDL.Principal)], ['query']),
    'get_available_tables' : IDL.Func([], [IDL.Vec(TableInfo)], ['query']),
    'get_leaderboard' : IDL.Func(
        [IDL.Nat64],
        [IDL.Vec(PlayerProfile)],
        ['query'],
      ),
    'get_my_profile' : IDL.Func([], [IDL.Opt(PlayerProfile)], ['query']),
    'get_player' : IDL.Func(
        [IDL.Principal],
        [IDL.Opt(PlayerProfile)],
        ['query'],
      ),
    'get_stats' : IDL.Func([], [IDL.Nat64, IDL.Nat64, IDL.Nat64], ['query']),
    'get_table' : IDL.Func([IDL.Nat64], [IDL.Opt(TableInfo)], ['query']),
    'get_tables' : IDL.Func([], [IDL.Vec(TableInfo)], ['query']),
    'get_tables_by_currency' : IDL.Func(
        [Currency],
        [IDL.Vec(TableInfo)],
        ['query'],
      ),
    'get_tables_by_stake' : IDL.Func(
        [StakeLevel],
        [IDL.Vec(TableInfo)],
        ['query'],
      ),
    'init_btc_tables' : IDL.Func(
        [IDL.Principal, IDL.Principal, IDL.Principal],
        [Result],
        [],
      ),
    'init_default_tables' : IDL.Func(
        [IDL.Principal, IDL.Principal],
        [Result],
        [],
      ),
    'init_microstakes_tables' : IDL.Func(
        [IDL.Principal, IDL.Principal, IDL.Principal],
        [Result],
        [],
      ),
    'is_caller_admin' : IDL.Func([], [IDL.Bool], ['query']),
    'is_initialized' : IDL.Func([], [IDL.Bool], ['query']),
    'register_player' : IDL.Func([IDL.Text], [Result_1], []),
    'remove_authorized_table' : IDL.Func([IDL.Principal], [Result], []),
    'set_admin' : IDL.Func([IDL.Principal], [Result], []),
    'update_player_count' : IDL.Func([IDL.Nat64, IDL.Nat8], [Result], []),
    'update_player_stats' : IDL.Func(
        [IDL.Principal, IDL.Int64, IDL.Nat64],
        [Result],
        [],
      ),
    'update_table_name' : IDL.Func([IDL.Nat64, IDL.Text], [Result], []),
  });
};
export const init = ({ IDL }) => { return []; };
