import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface ActionRecord {
  'action' : PlayerAction,
  'seat' : number,
  'timestamp' : bigint,
  'phase' : string,
  'amount' : bigint,
}
export interface ActionTimer {
  'player_seat' : number,
  'using_time_bank' : boolean,
  'expires_at' : bigint,
  'started_at' : bigint,
}
export interface Card { 'rank' : Rank, 'suit' : Suit }
export type CommitmentCheck = { 'FieldsSwapped' : CommitmentMatch } |
  { 'Match' : CommitmentMatch } |
  { 'NoMatch' : CommitmentNoMatch } |
  { 'Malformed' : CommitmentMalformed };
export interface CommitmentCheckArgs {
  'seed_hash' : string,
  'revealed_seed' : string,
}
export interface CommitmentMalformed {
  'field' : string,
  'character_length' : bigint,
  'reason' : string,
}
export interface CommitmentMatch {
  'committed_hash' : string,
  'computed_hash' : string,
  'revealed_seed' : string,
  'this_does_not_prove' : string,
  'this_proves' : string,
}
export interface CommitmentNoMatch {
  'meaning' : string,
  'seed_hash_field' : string,
  'revealed_seed_field' : string,
  'computed_from_seed_hash' : string,
  'computed_from_revealed_seed' : string,
}
export type Currency = { 'BTC' : null } |
  { 'ICP' : null };
export interface CustodyStatus {
  'total' : bigint,
  'chips_at_table' : bigint,
  'committed_is_stuck' : boolean,
  'committed_in_pot' : bigint,
  'canister_shortfall_e8s' : [] | [bigint],
  'unswept_deposit_observed_at_ns' : [] | [bigint],
  'advice' : string,
  'unswept_deposit' : bigint,
  'escrow' : bigint,
  'unfinished_ledger_ops' : bigint,
  'canister_solvency' : SolvencyVerdict,
  'abandonable_in_ns' : [] | [bigint],
}
export interface CycleStatus {
  'reserved_for_freezing' : bigint,
  'next_wake_at' : [] | [bigint],
  'observed_burn_per_day' : bigint,
  'balance' : bigint,
  'sample_window_secs' : bigint,
  'liquid_balance' : bigint,
  'measurement_is_meaningful' : boolean,
  'clock_last_tick_at' : bigint,
  'runway_days' : [] | [bigint],
  'clock_watchdog_armed' : boolean,
  'recent_window_secs' : [] | [bigint],
  'recent_burn_per_day' : [] | [bigint],
  'clock_ticks' : bigint,
}
export interface DealtInSeat { 'principal' : Principal, 'seat' : number }
export interface DepartedStake {
  'principal' : Principal,
  'hand_number' : bigint,
  'seat' : number,
  'contributed' : bigint,
}
export interface DepositAddressCustody {
  'transfer_fee' : bigint,
  'observed_amount' : bigint,
  'note' : string,
  'observed_at_ns' : [] | [bigint],
  'subaccount' : Uint8Array | number[],
  'minimum_deposit' : bigint,
  'refundable' : boolean,
  'ledger' : Principal,
  'canister' : Principal,
  'address' : string,
  'sweepable' : boolean,
}
export interface FairnessRetention {
  'table_keeps_last_n_hands' : bigint,
  'summary' : string,
  'archive_canister' : [] | [Principal],
  'table_copy_is_destructible_by_controller' : boolean,
}
export type GamePhase = { 'Flop' : null } |
  { 'Turn' : null } |
  { 'River' : null } |
  { 'Showdown' : null } |
  { 'HandComplete' : null } |
  { 'WaitingForPlayers' : null } |
  { 'PreFlop' : null };
export interface HandHistory {
  'dealt_in' : [] | [Array<DealtInSeat>],
  'participants' : [] | [Array<HistoryPlayerHandRecord>],
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
export interface HistoryPlayerHandRecord {
  'dealt_in' : [] | [boolean],
  'final_hand_rank' : [] | [HandRank],
  'principal' : Principal,
  'seat' : number,
  'hole_cards' : [] | [[Card, Card]],
  'left_mid_hand' : [] | [boolean],
  'amount_won' : bigint,
  'ending_chips' : bigint,
  'starting_chips' : bigint,
  'position' : string,
  'contributed' : [] | [bigint],
}
export interface HistoryStatus {
  'last_error' : [] | [string],
  'recorded_ok_since_start' : bigint,
  'local_history_cap' : bigint,
  'local_history_len' : bigint,
  'failed_since_start' : bigint,
  'last_recorded_hand' : [] | [bigint],
  'history_canister' : [] | [Principal],
  'in_flight' : bigint,
  'unrecorded_backlog' : bigint,
  'unrecorded_dropped' : bigint,
}
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
export interface LedgerIntentView {
  'id' : bigint,
  'who' : Principal,
  'retry_deadline_ns' : bigint,
  'kind' : string,
  'attempts' : number,
  'opened_at_ns' : bigint,
  'amount' : bigint,
  'leased_until_ns' : [] | [bigint],
}
export interface MainAccountObservation {
  'debited_since' : bigint,
  'observed_at_ns' : bigint,
  'credited_since' : bigint,
  'ledger' : Principal,
  'amount' : bigint,
}
export interface Player {
  'status' : PlayerStatus,
  'timeout_count' : number,
  'principal' : Principal,
  'is_sitting_out_next_hand' : boolean,
  'has_folded' : boolean,
  'sitting_out_since' : [] | [bigint],
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
export type Result_DepositAudit = { 'Ok' : [bigint, bigint, bigint] } |
  { 'Err' : string };
export type Result_DepositCensus = {
    'Ok' : [bigint, Array<[Principal, bigint, bigint]>, Array<Principal>]
  } |
  { 'Err' : string };
export type Result_DepositCustody = { 'Ok' : DepositAddressCustody } |
  { 'Err' : string };
export type Result_IntentLine = { 'Ok' : string } |
  { 'Err' : string };
export type Result_IntentReport = { 'Ok' : Array<string> } |
  { 'Err' : string };
export type Result_MainCustody = { 'Ok' : MainAccountObservation } |
  { 'Err' : string };
export type Result_Solvency = { 'Ok' : SolvencyReport } |
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
export interface SolvencyReport {
  'pot' : bigint,
  'main_observed_at_ns' : [] | [bigint],
  'chips_at_table' : bigint,
  'main_account' : [] | [bigint],
  'held' : [] | [bigint],
  'payouts_in_flight' : bigint,
  'owed' : bigint,
  'shortfall_e8s' : [] | [bigint],
  'pulls_in_flight' : bigint,
  'unfinished_incoming' : bigint,
  'unswept_deposits' : bigint,
  'difference_e8s' : [] | [bigint],
  'verdict' : SolvencyVerdict,
  'guard_liability' : bigint,
  'unattributed_at_main' : [] | [bigint],
  'summary' : string,
  'as_of_ns' : bigint,
  'main_credited_since_reading' : bigint,
  'ledger' : Principal,
  'committed_stake' : bigint,
  'currency' : string,
  'deposit_accounts_observed' : bigint,
  'sweep_fees_in_flight' : bigint,
  'deposit_subaccounts' : bigint,
  'deposit_accounts_never_observed' : Array<Principal>,
  'main_ledger' : [] | [Principal],
  'escrow' : bigint,
  'main_debited_since_reading' : bigint,
  'deposit_accounts_never_observed_count' : bigint,
  'deposit_oldest_observed_at_ns' : [] | [bigint],
}
export type SolvencyVerdict = { 'CannotPayEveryone' : null } |
  { 'CanPayEveryone' : null } |
  { 'Unknown' : null };
export interface StuckHandStatus {
  'refundable_pot' : bigint,
  'is_stuck' : boolean,
  'hand_in_progress' : boolean,
  'abandonable_in_ns' : [] | [bigint],
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
  'last_action' : [] | [LastActionInfo],
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
  'departed_stakes' : [] | [Array<DepartedStake>],
}
export interface TableView {
  'id' : bigint,
  'pot' : bigint,
  'min_raise' : bigint,
  'time_bank_remaining_secs' : [] | [bigint],
  'action_on' : number,
  'call_amount' : bigint,
  'my_seat' : [] | [number],
  'hand_is_unmovable' : boolean,
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
  'my_committed_in_pot' : bigint,
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
export interface _SERVICE {
  'abandon_stuck_hand' : ActorMethod<[], Result_1>,
  'add_controller' : ActorMethod<[Principal], Result>,
  'admin_audit_deposit_custody' : ActorMethod<
    [Array<Principal>],
    Result_DepositAudit
  >,
  'admin_get_all_balances' : ActorMethod<
    [],
    { 'Ok' : [bigint, Array<[Principal, bigint]>] } |
      { 'Err' : string }
  >,
  'admin_get_balance' : ActorMethod<[Principal], Result_1>,
  'admin_get_deposit_custody' : ActorMethod<[], Result_DepositCensus>,
  'admin_get_table_chips' : ActorMethod<[], Result_1>,
  'admin_reinit_table' : ActorMethod<[TableConfig], Result>,
  'admin_return_all_chips_to_escrow' : ActorMethod<[], Result_1>,
  'admin_update_config' : ActorMethod<[TableConfig], Result_5>,
  'buy_in' : ActorMethod<[number, bigint], Result>,
  'cash_out' : ActorMethod<[], Result_1>,
  'check_shuffle_commitment' : ActorMethod<
    [CommitmentCheckArgs],
    CommitmentCheck
  >,
  'check_timeouts' : ActorMethod<[], TimeoutCheckResult>,
  'claim_external_deposit' : ActorMethod<[], Result_1>,
  'deposit' : ActorMethod<[bigint], Result_1>,
  'dev_faucet' : ActorMethod<[bigint], Result_1>,
  'did_player_show' : ActorMethod<[number], boolean>,
  'flush_unrecorded_hands' : ActorMethod<[], Result_1>,
  'get_action_timer' : ActorMethod<[], [] | [ActionTimer]>,
  'get_all_ledger_intents' : ActorMethod<[], Array<LedgerIntentView>>,
  'get_balance' : ActorMethod<[], bigint>,
  'get_btc_deposit_address' : ActorMethod<
    [],
    { 'Ok' : string } |
      { 'Err' : string }
  >,
  'get_community_cards' : ActorMethod<[], Array<Card>>,
  'get_controllers' : ActorMethod<[], Array<Principal>>,
  'get_custody_status' : ActorMethod<[], CustodyStatus>,
  'get_cycle_status' : ActorMethod<[], CycleStatus>,
  'get_deposit_address' : ActorMethod<[], string>,
  'get_deposit_custody' : ActorMethod<[], DepositAddressCustody>,
  'get_deposit_replay_state' : ActorMethod<[], [bigint, bigint]>,
  'get_deposit_subaccount' : ActorMethod<[], Uint8Array | number[]>,
  'get_display_name' : ActorMethod<[Principal], [] | [string]>,
  'get_fairness_retention' : ActorMethod<[], FairnessRetention>,
  'get_hand_history' : ActorMethod<[bigint], [] | [HandHistory]>,
  'get_history_canister' : ActorMethod<[], [] | [Principal]>,
  'get_history_status' : ActorMethod<[], HistoryStatus>,
  'get_max_players' : ActorMethod<[], number>,
  'get_my_cards' : ActorMethod<[], [] | [[Card, Card]]>,
  'get_my_ledger_intents' : ActorMethod<[], Array<LedgerIntentView>>,
  'get_player_count' : ActorMethod<[], number>,
  'get_pot' : ActorMethod<[], bigint>,
  'get_shown_cards' : ActorMethod<[number], [] | [[Card, Card]]>,
  'get_shuffle_proof' : ActorMethod<[], [] | [ShuffleProof]>,
  'get_solvency' : ActorMethod<[], SolvencyReport>,
  'get_stuck_hand_status' : ActorMethod<[], StuckHandStatus>,
  'get_table_state' : ActorMethod<[], Result_2>,
  'get_table_view' : ActorMethod<[], [] | [TableView]>,
  'get_time_remaining' : ActorMethod<[], [] | [bigint]>,
  'heartbeat' : ActorMethod<[], Result>,
  'is_dev_mode' : ActorMethod<[], boolean>,
  'join_table' : ActorMethod<[number], Result>,
  'leave_table' : ActorMethod<[], Result_1>,
  'notify_deposit' : ActorMethod<[bigint], Result_1>,
  'player_action' : ActorMethod<[PlayerAction], Result>,
  'refresh_deposit_custody' : ActorMethod<[], Result_DepositCustody>,
  'refresh_main_account_custody' : ActorMethod<[], Result_MainCustody>,
  'refresh_solvency' : ActorMethod<[], Result_Solvency>,
  'refund_external_deposit' : ActorMethod<[], Result_1>,
  'reload' : ActorMethod<[bigint], Result_1>,
  'remove_controller' : ActorMethod<[Principal], Result>,
  'reset_table' : ActorMethod<[TableConfig], Result>,
  'resolve_ledger_intent' : ActorMethod<[bigint], Result_IntentLine>,
  'resolve_my_ledger_intents' : ActorMethod<[], Result_IntentReport>,
  'set_dev_mode' : ActorMethod<[boolean], Result>,
  'set_display_name' : ActorMethod<[[] | [string]], Result>,
  'set_history_canister' : ActorMethod<[[] | [Principal]], Result>,
  'show_cards' : ActorMethod<[], Result_3>,
  'sit_in' : ActorMethod<[], Result>,
  'sit_out' : ActorMethod<[], Result>,
  'sit_out_next_hand' : ActorMethod<[], Result>,
  'start_new_hand' : ActorMethod<[], Result_4>,
  'update_btc_balance' : ActorMethod<
    [],
    { 'Ok' : Array<UtxoStatus> } |
      { 'Err' : string }
  >,
  'use_time_bank' : ActorMethod<[], Result_1>,
  'verify_shuffle' : ActorMethod<[string, string], boolean>,
  'withdraw' : ActorMethod<[bigint], Result_1>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
