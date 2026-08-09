import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export type Currency = { 'BTC' : null } |
  { 'ICP' : null };
export interface PlayerProfile {
  'principal' : Principal,
  'username' : string,
  'created_at' : bigint,
  'total_winnings' : bigint,
  'hands_played' : bigint,
}
export type Result = { 'Ok' : null } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : PlayerProfile } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : BigUint64Array | bigint[] } |
  { 'Err' : string };
export type StakeLevel = { 'Low' : null } |
  { 'VIP' : null } |
  { 'High' : null } |
  { 'Medium' : null } |
  { 'Micro' : null };
export interface TableConfig {
  'small_blind' : bigint,
  'time_bank_secs' : bigint,
  'action_timeout_secs' : bigint,
  'ante' : bigint,
  'min_buy_in' : bigint,
  'max_players' : number,
  'currency' : Currency,
  'big_blind' : bigint,
  'max_buy_in' : bigint,
}
export interface TableInfo {
  'id' : bigint,
  'status' : TableStatus,
  'player_count' : number,
  'name' : string,
  'canister_id' : [] | [Principal],
  'created_at' : bigint,
  'created_by' : Principal,
  'currency' : Currency,
  'config' : TableConfig,
}
export type TableStatus = { 'Paused' : null } |
  { 'Closed' : null } |
  { 'InProgress' : null } |
  { 'WaitingForPlayers' : null };
export interface _SERVICE {
  'add_authorized_table' : ActorMethod<[Principal], Result>,
  'add_btc_headsup_table' : ActorMethod<[Principal], Result>,
  'get_admin' : ActorMethod<[], [] | [Principal]>,
  'get_authorized_tables' : ActorMethod<[], Array<Principal>>,
  'get_available_tables' : ActorMethod<[], Array<TableInfo>>,
  'get_leaderboard' : ActorMethod<[bigint], Array<PlayerProfile>>,
  'get_my_profile' : ActorMethod<[], [] | [PlayerProfile]>,
  'get_player' : ActorMethod<[Principal], [] | [PlayerProfile]>,
  'get_stats' : ActorMethod<[], [bigint, bigint, bigint]>,
  'get_table' : ActorMethod<[bigint], [] | [TableInfo]>,
  'get_tables' : ActorMethod<[], Array<TableInfo>>,
  'get_tables_by_currency' : ActorMethod<[Currency], Array<TableInfo>>,
  'get_tables_by_stake' : ActorMethod<[StakeLevel], Array<TableInfo>>,
  'init_btc_tables' : ActorMethod<[Principal, Principal, Principal], Result>,
  'init_default_tables' : ActorMethod<[Principal, Principal], Result>,
  'init_microstakes_tables' : ActorMethod<
    [Principal, Principal, Principal],
    Result
  >,
  'is_caller_admin' : ActorMethod<[], boolean>,
  'is_initialized' : ActorMethod<[], boolean>,
  'refresh_all_table_configs' : ActorMethod<[], Result_2>,
  'refresh_table_config' : ActorMethod<[bigint], Result>,
  'register_player' : ActorMethod<[string], Result_1>,
  'remove_authorized_table' : ActorMethod<[Principal], Result>,
  'set_admin' : ActorMethod<[Principal], Result>,
  'update_player_count' : ActorMethod<[bigint, number], Result>,
  'update_player_stats' : ActorMethod<[Principal, bigint, bigint], Result>,
  'update_table_name' : ActorMethod<[bigint, string], Result>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
