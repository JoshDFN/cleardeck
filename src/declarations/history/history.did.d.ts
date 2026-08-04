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
export interface Card { 'rank' : Rank, 'suit' : Suit }
export interface HandHistoryRecord {
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
  'went_to_showdown' : boolean,
  'shuffle_proof' : ShuffleProofRecord,
  'river' : [] | [Card],
  'winners' : Array<WinnerRecord>,
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
  'went_to_showdown' : boolean,
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
  'final_hand_rank' : [] | [HandRank],
  'principal' : Principal,
  'seat' : number,
  'hole_cards' : [] | [[Card, Card]],
  'amount_won' : bigint,
  'ending_chips' : bigint,
  'starting_chips' : bigint,
  'position' : string,
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
export type Result = { 'Ok' : null } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : bigint } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : boolean } |
  { 'Err' : string };
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
  'get_authorized_tables' : ActorMethod<[], Array<Principal>>,
  'get_hand' : ActorMethod<[bigint], [] | [HandHistoryRecord]>,
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
  'get_table_hand_count' : ActorMethod<[Principal], bigint>,
  'get_total_hands' : ActorMethod<[], bigint>,
  'record_hand' : ActorMethod<[HandHistoryRecord], Result_1>,
  'revoke_table' : ActorMethod<[Principal], Result>,
  'verify_hand_shuffle' : ActorMethod<[bigint], Result_2>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
