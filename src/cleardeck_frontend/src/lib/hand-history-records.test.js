import { describe, it, expect } from 'vitest';
import {
  codeOf, factsFrom, fromHistoryRecord, fromTableRecord, participatedIn, playersOf, principalForSeat,
  rankName, seatNameFor, seatOfPrincipal,
} from './hand-history-records.js';

const card = (rank, suit) => ({ rank: { [rank]: null }, suit: { [suit]: null } });
const P = { toString: () => 'me-principal' };

describe('fromTableRecord', () => {
  const record = {
    hand_number: 4n,
    shuffle_proof: { seed_hash: 'ab', revealed_seed: ['cd'], timestamp: 5n },
    community_cards: [card('Ace', 'Spades')],
    actions: [{ seat: 1n, action: { Call: null }, phase: 'flop', amount: 7n, timestamp: 1n }],
    showdown_players: [{ seat: 0n, principal: P, cards: [[card('Two', 'Clubs'), card('Three', 'Clubs')]], hand_rank: [{ Pair: null }], amount_won: 9n }],
    winners: [{ seat: 0n, principal: P, amount: 9n, hand_rank: [], cards: [] }],
  };
  const hand = fromTableRecord(record, 4);
  it('normalises the optionals and sums the pot from what was paid', () => {
    expect(hand.handNumber).toBe(4);
    expect(hand.proof).toEqual({ seedHash: 'ab', revealedSeed: 'cd' });
    expect(hand.potTotal).toBe(9);
    expect(hand.seats).toEqual([0, 1]);
    expect(hand.showdown[0].cards).toHaveLength(2);
    expect(hand.blinds).toBeNull();
    expect(hand.source).toBe('table canister');
  });
  it('answers who was in it and where', () => {
    expect(participatedIn(hand, 'me-principal')).toBe(true);
    expect(participatedIn(hand, 'nobody')).toBe(false);
    expect(seatOfPrincipal(hand, 'me-principal')).toBe(0);
    expect(seatOfPrincipal(hand, null)).toBeNull();
  });
  it('reads the dealt-in list when the canister recorded it, folds included', () => {
    const F = { toString: () => 'folded-principal' };
    const withDealtIn = fromTableRecord({
      ...record,
      dealt_in: [[{ seat: 0n, principal: P }, { seat: 3n, principal: F }]],
      participants: [],
    }, 4);
    expect(withDealtIn.seats).toEqual([0, 1, 3]);
    expect(withDealtIn.players).toEqual([{ seat: 0, principal: 'me-principal' }, { seat: 3, principal: 'folded-principal' }]);
    expect(participatedIn(withDealtIn, 'folded-principal')).toBe(true);
    expect(seatOfPrincipal(withDealtIn, 'folded-principal')).toBe(3);
    expect(principalForSeat(withDealtIn, 3)).toBe('folded-principal');
    // absent lists (`opt` null) change nothing
    expect(hand.players).toEqual([{ seat: 0, principal: 'me-principal' }]);
    expect(participatedIn(hand, 'folded-principal')).toBe(false);
  });
});

describe('fromHistoryRecord', () => {
  it('builds the board from flop, turn and river and keeps the level', () => {
    const hand = fromHistoryRecord({
      hand_number: 2n, hand_id: 11n, timestamp: 3n,
      shuffle_proof: { seed_hash: 'aa', revealed_seed: 'bb' },
      flop: [[card('Two', 'Clubs'), card('Three', 'Clubs'), card('Four', 'Clubs')]],
      turn: [card('Five', 'Clubs')], river: [],
      players: [{ seat: 2n, principal: P, hole_cards: [], final_hand_rank: [], amount_won: 0n }],
      winners: [], actions: [], total_pot: 30n, small_blind: 1n, big_blind: 2n, ante: 0n,
    });
    expect(hand.community).toHaveLength(4);
    expect(hand.blinds).toEqual({ small: 1, big: 2, ante: 0 });
    expect(hand.requestedNumber).toBe(11);
    expect(hand.showdown).toEqual([]);
    expect(hand.seats).toEqual([2]);
    // a player with no hole cards (folded before showdown) is still a player
    expect(hand.players).toEqual([{ seat: 2, principal: 'me-principal' }]);
    expect(participatedIn(hand, 'me-principal')).toBe(true);
  });
  it('a fold before showdown is participation: from the archive\'s player list or its actions', () => {
    const F = { toString: () => 'folded-principal' };
    const hand = fromHistoryRecord({
      hand_number: 3n, hand_id: 12n, timestamp: 3n,
      shuffle_proof: { seed_hash: 'aa', revealed_seed: 'bb' },
      flop: [], turn: [], river: [],
      players: [{ seat: 0n, principal: P, hole_cards: [[card('Ace', 'Spades'), card('King', 'Hearts')]], final_hand_rank: [], amount_won: 5n }],
      winners: [{ seat: 0n, principal: P, amount: 5n, hand_rank: [] }],
      actions: [
        { seat: 4n, principal: F, action: { Fold: null }, phase: 'preflop', timestamp: 1n },
        { seat: 0n, principal: P, action: { Check: null }, phase: 'preflop', timestamp: 2n },
      ],
      total_pot: 5n, small_blind: 1n, big_blind: 2n, ante: 0n,
    });
    expect(hand.actions[0].principal).toBe('folded-principal');
    expect(participatedIn(hand, 'folded-principal')).toBe(true);
    expect(seatOfPrincipal(hand, 'folded-principal')).toBe(4);
    expect(playersOf(hand).map((p) => p.principal)).toEqual(['me-principal', 'folded-principal']);
    expect(participatedIn(hand, 'nobody')).toBe(false);
  });
});

describe('factsFrom / seatNameFor', () => {
  const view = {
    config: { small_blind: 1n, big_blind: 2n, ante: 0n },
    small_blind_seat: 0n, big_blind_seat: 1n, dealer_seat: 0n, hand_number: 4n,
    phase: { HandComplete: null },
    players: [[{ seat: 0n, principal: P, display_name: ['Ada'] }], []],
  };
  const facts = factsFrom(view);
  it('reads the level, the button and the seated names', () => {
    expect(facts.config).toEqual({ smallBlind: 1, bigBlind: 2, ante: 0 });
    expect(facts.blindSeats).toEqual({ sb: 0, bb: 1, dealer: 0 });
    expect(facts.settled).toBe(true);
    expect(facts.seated).toEqual([{ seat: 0, principal: 'me-principal', name: 'Ada' }]);
  });
  it('names a seat only when the same principal still sits there', () => {
    const hand = { showdown: [{ seat: 0, principal: 'me-principal' }], winners: [] };
    expect(seatNameFor(hand, 0, facts)).toBe('Ada');
    expect(seatNameFor({ showdown: [{ seat: 0, principal: 'someone-else' }], winners: [] }, 0, facts)).toBeNull();
    expect(seatNameFor(hand, 0, null)).toBeNull();
  });
});

describe('codeOf / rankName', () => {
  it('speaks the verifier\'s two-character codes', () => {
    expect(codeOf(card('Ten', 'Hearts'))).toBe('Th');
    expect(codeOf(null)).toBeNull();
    expect(rankName([{ FullHouse: null }])).toBe('Full House');
    expect(rankName(null)).toBeNull();
  });
});
