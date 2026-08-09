import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export interface ActionRecord {
  'principal' : Principal,
  'action' : PlayerAction,
  'seat' : number,
  'timestamp' : bigint,
  'phase' : string,
}
export interface ArchiveIntegrity {
  'distinct_hand_number_citations' : bigint,
  'records_without_a_usable_commitment' : bigint,
  'index_disagrees_with_records' : boolean,
  'name_collisions' : bigint,
  'rake_recorded_total' : bigint,
  'worst_hand_number_citation' : [] | [string],
  'records_under_a_colliding_name' : bigint,
  'records_with_a_nonzero_rake' : bigint,
  'records_with_a_name' : bigint,
  'summary' : string,
  'ambiguous_hand_number_citations' : bigint,
  'records_held' : bigint,
  'distinct_names' : bigint,
  'records_under_an_ambiguous_hand_number' : bigint,
  'first_record_with_a_rake' : [] | [string],
}
export interface Card { 'rank' : Rank, 'suit' : Suit }
export interface DealtInSeat { 'principal' : Principal, 'seat' : number }
export interface HandHistoryRecord {
  'dealt_in' : [] | [Array<DealtInSeat>],
  'small_blind' : bigint,
  'dealer_seat' : number,
  'ante' : bigint,
  'hand_number' : bigint,
  'flop' : [] | [[Card, Card, Card]],
  'hand_id' : bigint,
  'rake' : bigint,
  'turn' : [] | [Card],
  'table_id' : Principal,
  'actions' : Array<ActionRecord>,
  'total_pot' : bigint,
  'players' : Array<PlayerHandRecord>,
  'big_blind' : bigint,
  'timestamp' : bigint,
  'hand_uid' : [] | [string],
  'went_to_showdown' : boolean,
  'shuffle_proof' : ShuffleProofRecord,
  'river' : [] | [Card],
  'winners' : Array<WinnerRecord>,
}
export interface HandNumberCitation {
  'is_unique' : boolean,
  'hand_number' : bigint,
  'table_id' : Principal,
  'matches' : Array<HandSummary>,
  'match_count' : bigint,
  'advice' : string,
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
export interface HandSummary {
  'player_count' : number,
  'hand_number' : bigint,
  'hand_id' : bigint,
  'table_id' : Principal,
  'total_pot' : bigint,
  'timestamp' : bigint,
  'hand_uid' : string,
  'went_to_showdown' : boolean,
  'dealt_in_count' : [] | [number],
  'winners' : Array<WinnerRecord>,
}
export type PlayerAction = { 'Bet' : bigint } |
  { 'PostBlind' : bigint } |
  { 'Call' : bigint } |
  { 'Fold' : null } |
  { 'Raise' : bigint } |
  { 'AllIn' : bigint } |
  { 'Check' : null };
export interface PlayerHandRecord {
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
export interface PlayerStats {
  'biggest_pot_won' : bigint,
  'principal' : Principal,
  'showdowns_won' : bigint,
  'hands_won' : bigint,
  'total_winnings' : bigint,
  'hands_played' : bigint,
  'showdowns_total' : bigint,
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
export interface RecordedHandCheck {
  'computed_hash' : string,
  'hand_number' : bigint,
  'hand_id' : bigint,
  'table_id' : Principal,
  'commitment_matches' : boolean,
  'seed_hash' : string,
  'revealed_seed' : string,
  'this_does_not_prove' : string,
  'hand_uid' : string,
  'this_proves' : string,
  'verify_it_yourself' : string,
  'problem' : [] | [string],
}
export type Result = { 'Ok' : null } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : RecordedHandCheck } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : HandHistoryRecord } |
  { 'Err' : string };
export type Result_3 = { 'Ok' : bigint } |
  { 'Err' : string };
export type Result_4 = { 'Ok' : boolean } |
  { 'Err' : string };
export interface RetentionPolicy {
  'admin' : [] | [Principal],
  'max_age_before_discard' : [] | [bigint],
  'summary' : string,
  'records_are_append_only' : boolean,
  'records_held' : bigint,
}
export interface ShuffleProofRecord {
  'timestamp' : bigint,
  'seed_hash' : string,
  'revealed_seed' : string,
}
export type Suit = { 'Diamonds' : null } |
  { 'Hearts' : null } |
  { 'Clubs' : null } |
  { 'Spades' : null };
export interface WinnerRecord {
  'principal' : Principal,
  'hand_rank' : [] | [HandRank],
  'seat' : number,
  'pot_type' : string,
  'amount' : bigint,
}
export interface _SERVICE {
  'authorize_table' : ActorMethod<[Principal], Result>,
  'check_recorded_hand' : ActorMethod<[bigint], Result_1>,
  'get_admin' : ActorMethod<[], [] | [Principal]>,
  'get_archive_integrity' : ActorMethod<[], ArchiveIntegrity>,
  'get_authorized_tables' : ActorMethod<[], Array<Principal>>,
  'get_hand' : ActorMethod<[bigint], [] | [HandHistoryRecord]>,
  'get_hand_by_uid' : ActorMethod<[string], Result_2>,
  'get_hands_by_player' : ActorMethod<
    [Principal, bigint, bigint],
    Array<HandSummary>
  >,
  'get_hands_by_table' : ActorMethod<
    [Principal, bigint, bigint],
    Array<HandSummary>
  >,
  'get_player_stats' : ActorMethod<[Principal], [] | [PlayerStats]>,
  'get_recent_hands' : ActorMethod<[bigint], Array<HandSummary>>,
  'get_retention_policy' : ActorMethod<[], RetentionPolicy>,
  'get_table_hand_count' : ActorMethod<[Principal], bigint>,
  'get_total_hands' : ActorMethod<[], bigint>,
  'record_hand' : ActorMethod<[HandHistoryRecord], Result_3>,
  'resolve_hand_number' : ActorMethod<[Principal, bigint], HandNumberCitation>,
  'revoke_table' : ActorMethod<[Principal], Result>,
  'verify_hand_shuffle' : ActorMethod<[bigint], Result_4>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
