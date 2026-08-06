import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface ActionRecord {
  'action' : PlayerAction,
  'seat' : number,
  'timestamp' : bigint,
}
export interface ActionTimer {
  'player_seat' : number,
  'using_time_bank' : boolean,
  'expires_at' : bigint,
  'started_at' : bigint,
}
export interface Card { 'rank' : Rank, 'suit' : Suit }
export type Currency = { 'BTC' : null } |
  { 'ICP' : null };
export type GamePhase = { 'Flop' : null } |
  { 'Turn' : null } |
  { 'River' : null } |
  { 'Showdown' : null } |
  { 'HandComplete' : null } |
  { 'WaitingForPlayers' : null } |
  { 'PreFlop' : null };
export interface HandHistory {
  'showdown_players' : Array<ShowdownPlayer>,
  'hand_number' : bigint,
  'actions' : Array<ActionRecord>,
  'community_cards' : Array<Card>,
  'shuffle_proof' : ShuffleProof,
  'winners' : Array<Winner>,
}
export type HandRank = { 'StraightFlush' : number } |
  { 'Straight' : number } |
  { 'Pair' : [number, Uint8Array | number[]] } |
  { 'FullHouse' : [number, number] } |
  { 'TwoPair' : [number, number, number] } |
  { 'HighCard' : Uint8Array | number[] } |
  { 'ThreeOfAKind' : [number, Uint8Array | number[]] } |
  { 'Flush' : Uint8Array | number[] } |
  { 'RoyalFlush' : null } |
  { 'FourOfAKind' : [number, number] };
export type LastAction = { 'Bet' : { 'amount' : bigint } } |
  { 'PostBlind' : { 'amount' : bigint } } |
  { 'Call' : { 'amount' : bigint } } |
  { 'Fold' : null } |
  { 'Raise' : { 'amount' : bigint } } |
  { 'AllIn' : { 'amount' : bigint } } |
  { 'Check' : null };
export interface LastActionInfo {
  'action' : LastAction,
  'seat' : number,
  'timestamp' : bigint,
}
export interface Player {
  'status' : PlayerStatus,
  'timeout_count' : number,
  'principal' : Principal,
  'is_sitting_out_next_hand' : boolean,
  'has_folded' : boolean,
  'chips' : bigint,
  'seat' : number,
  'hole_cards' : [] | [[Card, Card]],
  'total_bet_this_hand' : bigint,
  'current_bet' : bigint,
  'has_acted_this_round' : boolean,
  'time_bank_remaining' : bigint,
  'broke_at' : [] | [bigint],
  'last_seen' : bigint,
  'is_all_in' : boolean,
}
export type PlayerAction = { 'Bet' : bigint } |
  { 'Call' : null } |
  { 'Fold' : null } |
  { 'Raise' : bigint } |
  { 'AllIn' : null } |
  { 'Check' : null };
export type PlayerStatus = { 'SittingOut' : null } |
  { 'Active' : null } |
  { 'Disconnected' : null };
export interface PlayerView {
  'status' : PlayerStatus,
  'is_self' : boolean,
  'principal' : Principal,
  'has_folded' : boolean,
  'chips' : bigint,
  'seat' : number,
  'hole_cards' : [] | [[Card, Card]],
  'display_name' : [] | [string],
  'current_bet' : bigint,
  'is_all_in' : boolean,
}
export type Rank = { 'Ace' : null } |
  { 'Six' : null } |
  { 'Ten' : null } |
  { 'Two' : null } |
  { 'Eight' : null } |
  { 'Seven' : null } |
  { 'Five' : null } |
  { 'Four' : null } |
  { 'Jack' : null } |
  { 'King' : null } |
  { 'Nine' : null } |
  { 'Three' : null } |
  { 'Queen' : null };
export type Result = { 'Ok' : null } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : bigint } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : TableState } |
  { 'Err' : string };
export type Result_3 = { 'Ok' : [Card, Card] } |
  { 'Err' : string };
export type Result_4 = { 'Ok' : ShuffleProof } |
  { 'Err' : string };
export type Result_5 = { 'Ok' : TableConfig } |
  { 'Err' : string };
export interface ShowdownPlayer {
  'principal' : Principal,
  'hand_rank' : [] | [HandRank],
  'cards' : [] | [[Card, Card]],
  'seat' : number,
  'amount_won' : bigint,
}
export interface ShuffleProof {
  'timestamp' : bigint,
  'seed_hash' : string,
  'revealed_seed' : [] | [string],
}
export interface SidePot {
  'eligible_players' : Uint8Array | number[],
  'amount' : bigint,
}
export type Suit = { 'Diamonds' : null } |
  { 'Hearts' : null } |
  { 'Clubs' : null } |
  { 'Spades' : null };
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
export interface TableState {
  'id' : bigint,
  'pot' : bigint,
  'min_raise' : bigint,
  'bb_has_option' : boolean,
  'action_on' : number,
  'action_timer' : [] | [ActionTimer],
  'dealer_seat' : number,
  'hand_number' : bigint,
  'deck' : Array<Card>,
  'big_blind_seat' : number,
  'auto_deal_at' : [] | [bigint],
  'players' : Array<[] | [Player]>,
  'current_bet' : bigint,
  'last_aggressor' : [] | [number],
  'deck_index' : bigint,
  'community_cards' : Array<Card>,
  'small_blind_seat' : number,
  'first_hand' : boolean,
  'phase' : GamePhase,
  'config' : TableConfig,
  'side_pots' : Array<SidePot>,
  'shuffle_proof' : [] | [ShuffleProof],
}
export interface TableView {
  'id' : bigint,
  'pot' : bigint,
  'min_raise' : bigint,
  'time_bank_remaining_secs' : [] | [bigint],
  'action_on' : number,
  'call_amount' : bigint,
  'my_seat' : [] | [number],
  'dealer_seat' : number,
  'hand_number' : bigint,
  'min_bet' : bigint,
  'big_blind_seat' : number,
  'can_check' : boolean,
  'last_action' : [] | [LastActionInfo],
  'last_hand_winners' : Array<Winner>,
  'time_remaining_secs' : [] | [bigint],
  'players' : Array<[] | [PlayerView]>,
  'current_bet' : bigint,
  'community_cards' : Array<Card>,
  'small_blind_seat' : number,
  'using_time_bank' : boolean,
  'phase' : GamePhase,
  'config' : TableConfig,
  'side_pots' : Array<SidePot>,
  'shuffle_proof' : [] | [ShuffleProof],
  'is_my_turn' : boolean,
  'can_raise' : boolean,
}
export type TimeoutCheckResult = { 'AutoDealReady' : null } |
  { 'PlayerTimedOut' : number } |
  { 'NoAction' : null };
export interface Utxo {
  'height' : number,
  'value' : bigint,
  'outpoint' : UtxoOutpoint,
}
export interface UtxoOutpoint {
  'txid' : Uint8Array | number[],
  'vout' : number,
}
/**
 * ckBTC minter types for BTC deposits
 */
export type UtxoStatus = { 'ValueTooSmall' : Utxo } |
  { 'Tainted' : Utxo } |
  {
    'Minted' : {
      'minted_amount' : bigint,
      'block_index' : bigint,
      'utxo' : Utxo,
    }
  } |
  { 'Checked' : Utxo };
export interface Winner {
  'principal' : Principal,
  'hand_rank' : [] | [HandRank],
  'cards' : [] | [[Card, Card]],
  'seat' : number,
  'amount' : bigint,
}
export interface CommitmentMatch {
  'committed_hash' : string,
  'revealed_seed' : string,
  'computed_hash' : string,
  'this_proves' : string,
  'this_does_not_prove' : string,
}
export type CommitmentCheck = { 'Match' : CommitmentMatch } |
  { 'FieldsSwapped' : CommitmentMatch } |
  {
    'NoMatch' : {
      'seed_hash_field' : string,
      'revealed_seed_field' : string,
      'computed_from_revealed_seed' : string,
      'computed_from_seed_hash' : string,
      'meaning' : string,
    }
  } |
  {
    'Malformed' : {
      'field' : string,
      'reason' : string,
      'character_length' : bigint,
    }
  };
export interface CommitmentCheckArgs {
  'seed_hash' : string,
  'revealed_seed' : string,
}
export interface FairnessRetention {
  'table_keeps_last_n_hands' : bigint,
  'table_copy_is_destructible_by_controller' : boolean,
  'archive_canister' : [] | [Principal],
  'summary' : string,
}
export interface HistoryStatus {
  'history_canister' : [] | [Principal],
  'recorded_ok_since_start' : bigint,
  'failed_since_start' : bigint,
  'in_flight' : bigint,
  'unrecorded_backlog' : bigint,
  'unrecorded_dropped' : bigint,
  'last_recorded_hand' : [] | [bigint],
  'last_error' : [] | [string],
  'local_history_cap' : bigint,
  'local_history_len' : bigint,
}
export interface _SERVICE {
  /**
   * Add a controller (controller only)
   */
  'add_controller' : ActorMethod<[Principal], Result>,
  /**
   * Admin: Get all balances (for auditing/recovery)
   * Returns (total_assigned, list of (principal, balance))
   */
  'admin_get_all_balances' : ActorMethod<
    [],
    { 'Ok' : [bigint, Array<[Principal, bigint]>] } |
      { 'Err' : string }
  >,
  /**
   * Admin: Get balance for a specific player (controller only, read-only)
   */
  'admin_get_balance' : ActorMethod<[Principal], Result_1>,
  /**
   * Admin: Get total chips at table (seated players)
   */
  'admin_get_table_chips' : ActorMethod<[], Result_1>,
  /**
   * Admin: Re-initialize the table (for recovery after upgrade issues)
   */
  'admin_reinit_table' : ActorMethod<[TableConfig], Result>,
  /**
   * Admin: Restore balance for a player (TEMPORARY - for recovery only)
   */
  'admin_restore_balance' : ActorMethod<[Principal, bigint], Result_1>,
  /**
   * Admin: Update table configuration (controller only, not during a hand)
   */
  'admin_update_config' : ActorMethod<[TableConfig], Result_5>,
  /**
   * Buy into the table using your escrow balance
   */
  'buy_in' : ActorMethod<[number, bigint], Result>,
  /**
   * Cash out and leave the table
   */
  'cash_out' : ActorMethod<[], Result_1>,
  /**
   * Check for timeouts, auto-fold, and auto-deal
   * This should be called periodically or before each action
   */
  'check_timeouts' : ActorMethod<[], TimeoutCheckResult>,
  /**
   * Deposit ICP to your escrow balance using ICRC-2 transfer_from
   * You must first approve this canister to spend your ICP via icrc2_approve
   */
  'deposit' : ActorMethod<[bigint], Result_1>,
  /**
   * Deposit from an external wallet (like OISY) using ICRC-2 transfer_from
   * The external wallet must first approve this canister, then pass their principal here
   * Balance is credited to the caller's account (not the external wallet)
   */
  'deposit_from_external' : ActorMethod<[Principal, bigint], Result_1>,
  /**
   * DEV ONLY: Get free test chips for local development
   * Disabled when dev_mode is false (production)
   */
  'dev_faucet' : ActorMethod<[bigint], Result_1>,
  /**
   * Check if a player voluntarily showed their cards this hand
   */
  'did_player_show' : ActorMethod<[number], boolean>,
  'get_action_timer' : ActorMethod<[], [] | [ActionTimer]>,
  /**
   * Get your current escrow balance
   */
  'get_balance' : ActorMethod<[], bigint>,
  /**
   * Get a BTC deposit address for native Bitcoin deposits
   * Users can send real BTC to this address, then call update_btc_balance
   * Only available for BTC tables
   */
  'get_btc_deposit_address' : ActorMethod<
    [],
    { 'Ok' : string } |
      { 'Err' : string }
  >,
  'get_community_cards' : ActorMethod<[], Array<Card>>,
  /**
   * Get all controllers
   */
  'get_controllers' : ActorMethod<[], Array<Principal>>,
  /**
   * Get the canister's account for deposits
   */
  'get_deposit_address' : ActorMethod<[], string>,
  /**
   * Get a player's display name
   */
  'get_display_name' : ActorMethod<[Principal], [] | [string]>,
  'get_hand_history' : ActorMethod<[bigint], [] | [HandHistory]>,
  /**
   * Get the history canister ID
   */
  'get_history_canister' : ActorMethod<[], [] | [Principal]>,
  /**
   * Get max players (for lobby display)
   */
  'get_max_players' : ActorMethod<[], number>,
  'get_my_cards' : ActorMethod<[], [] | [[Card, Card]]>,
  /**
   * Get current player count (for lobby display)
   */
  'get_player_count' : ActorMethod<[], number>,
  'get_pot' : ActorMethod<[], bigint>,
  /**
   * Get cards for a player who voluntarily showed them
   */
  'get_shown_cards' : ActorMethod<[number], [] | [[Card, Card]]>,
  'get_shuffle_proof' : ActorMethod<[], [] | [ShuffleProof]>,
  /**
   * Get the raw table state (admin/debug use - exposes all data)
   * RESTRICTED: Only controllers can access this to prevent cheating
   */
  'get_table_state' : ActorMethod<[], Result_2>,
  /**
   * Get the table view from the caller's perspective
   * This properly hides opponent hole cards unless at showdown
   */
  'get_table_view' : ActorMethod<[], [] | [TableView]>,
  'get_time_remaining' : ActorMethod<[], [] | [bigint]>,
  /**
   * Player heartbeat to show they're connected
   */
  'heartbeat' : ActorMethod<[], Result>,
  /**
   * Check if dev mode is enabled (always returns false now)
   */
  'is_dev_mode' : ActorMethod<[], boolean>,
  /**
   * Join table with minimum buy-in from escrow balance
   * Requires sufficient ICP deposited first via notify_deposit
   */
  'join_table' : ActorMethod<[number], Result>,
  /**
   * Leave table and return chips to escrow balance
   * If mid-hand, this acts as a fold - pot contributions stay in the pot
   */
  'leave_table' : ActorMethod<[], Result_1>,
  /**
   * Verify and credit a deposit by checking the ledger transaction
   * Players should first transfer ICP to the canister's account, then call this with the block index
   */
  'notify_deposit' : ActorMethod<[bigint], Result_1>,
  'player_action' : ActorMethod<[PlayerAction], Result>,
  /**
   * Reload chips from escrow (for players already seated who need more chips)
   * Can only be done between hands, not during active play
   */
  'reload' : ActorMethod<[bigint], Result_1>,
  /**
   * Remove a controller (controller only)
   */
  'remove_controller' : ActorMethod<[Principal], Result>,
  /**
   * Reset the table (controller only) - CAUTION: destroys all state
   */
  'reset_table' : ActorMethod<[TableConfig], Result>,
  /**
   * Set dev mode (controller only)
   */
  'set_dev_mode' : ActorMethod<[boolean], Result>,
  /**
   * Set a custom display name (visible to all players)
   * Name must be 1-12 characters, alphanumeric with some symbols allowed
   * Pass null to clear the name
   */
  'set_display_name' : ActorMethod<[[] | [string]], Result>,
  /**
   * Set the history canister ID (controller only)
   * Pass None to clear/disable history recording
   */
  'set_history_canister' : ActorMethod<[[] | [Principal]], Result>,
  /**
   * Voluntarily show your hole cards to the table
   * Only allowed after you've folded or at the end of the hand
   */
  'show_cards' : ActorMethod<[], Result_3>,
  /**
   * Sit back in
   */
  'sit_in' : ActorMethod<[], Result>,
  /**
   * Sit out (voluntarily)
   */
  'sit_out' : ActorMethod<[], Result>,
  /**
   * Request to sit out at the end of the current hand
   */
  'sit_out_next_hand' : ActorMethod<[], Result>,
  'start_new_hand' : ActorMethod<[], Result_4>,
  /**
   * Update BTC balance after sending Bitcoin to the deposit address
   * Checks for new UTXOs and mints ckBTC if confirmed
   * Only available for BTC tables
   */
  'update_btc_balance' : ActorMethod<
    [],
    { 'Ok' : Array<UtxoStatus> } |
      { 'Err' : string }
  >,
  /**
   * Use time bank to extend action time
   * Returns remaining time bank seconds
   */
  'use_time_bank' : ActorMethod<[], Result_1>,
  'check_shuffle_commitment' : ActorMethod<[CommitmentCheckArgs], CommitmentCheck>,
  'get_fairness_retention' : ActorMethod<[], FairnessRetention>,
  'get_history_status' : ActorMethod<[], HistoryStatus>,
  'flush_unrecorded_hands' : ActorMethod<[], Result_1>,
  'verify_shuffle' : ActorMethod<[string, string], boolean>,
  /**
   * Withdraw your balance from the table
   */
  'withdraw' : ActorMethod<[bigint], Result_1>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
