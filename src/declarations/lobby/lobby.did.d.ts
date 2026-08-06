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
  'currency' : [] | [Currency],
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
  'currency' : [] | [Currency],
  'config' : TableConfig,
}
export type TableStatus = { 'Paused' : null } |
  { 'Closed' : null } |
  { 'InProgress' : null } |
  { 'WaitingForPlayers' : null };
export interface _SERVICE {
  /**
   * Add an authorized table canister (admin only)
   */
  'add_authorized_table' : ActorMethod<[Principal], Result>,
  /**
   * Add a single BTC heads-up table (admin only)
   */
  'add_btc_headsup_table' : ActorMethod<[Principal], Result>,
  /**
   * Get admin principal
   */
  'get_admin' : ActorMethod<[], [] | [Principal]>,
  /**
   * Get all authorized tables
   */
  'get_authorized_tables' : ActorMethod<[], Array<Principal>>,
  /**
   * Get tables with available seats
   */
  'get_available_tables' : ActorMethod<[], Array<TableInfo>>,
  /**
   * Get leaderboard (top players by winnings)
   */
  'get_leaderboard' : ActorMethod<[bigint], Array<PlayerProfile>>,
  /**
   * Get my profile
   */
  'get_my_profile' : ActorMethod<[], [] | [PlayerProfile]>,
  /**
   * Get player profile
   */
  'get_player' : ActorMethod<[Principal], [] | [PlayerProfile]>,
  /**
   * Get total stats
   */
  'get_stats' : ActorMethod<[], [bigint, bigint, bigint]>,
  /**
   * Get a specific table
   */
  'get_table' : ActorMethod<[bigint], [] | [TableInfo]>,
  /**
   * Get all active tables
   */
  'get_tables' : ActorMethod<[], Array<TableInfo>>,
  /**
   * Get tables by currency
   */
  'get_tables_by_currency' : ActorMethod<[Currency], Array<TableInfo>>,
  /**
   * Get tables by stake level
   */
  'get_tables_by_stake' : ActorMethod<[StakeLevel], Array<TableInfo>>,
  /**
   * Initialize 3 BTC tables (admin only)
   */
  'init_btc_tables' : ActorMethod<[Principal, Principal, Principal], Result>,
  /**
   * Initialize default tables - call this after deploying table canisters
   */
  'init_default_tables' : ActorMethod<[Principal, Principal], Result>,
  /**
   * Initialize microstakes ICP tables
   */
  'init_microstakes_tables' : ActorMethod<
    [Principal, Principal, Principal],
    Result
  >,
  /**
   * Check if caller is admin
   */
  'is_caller_admin' : ActorMethod<[], boolean>,
  /**
   * Check if tables are initialized
   */
  'is_initialized' : ActorMethod<[], boolean>,
  /**
   * Register or update player profile
   */
  'register_player' : ActorMethod<[string], Result_1>,
  /**
   * Remove an authorized table canister (admin only)
   */
  'remove_authorized_table' : ActorMethod<[Principal], Result>,
  /**
   * Set admin - recovery function when no admin is set
   */
  'set_admin' : ActorMethod<[Principal], Result>,
  /**
   * Update player count for a table (called by table canister)
   * SECURITY: Only authorized table canisters or admin can update
   */
  'update_player_count' : ActorMethod<[bigint, number], Result>,
  /**
   * Update player stats (called by table canister after hand completes)
   * SECURITY: Only authorized table canisters or admin can update
   */
  'update_player_stats' : ActorMethod<[Principal, bigint, bigint], Result>,
  /**
   * Update a table's name (admin only)
   */
  'update_table_name' : ActorMethod<[bigint, string], Result>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
