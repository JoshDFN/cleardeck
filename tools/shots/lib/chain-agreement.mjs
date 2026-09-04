// "Is the number on the screen the number in the canister?"
//
// Before this file, a scene was verified by finding a DOM element. That is
// PRESENCE, not truth, and it is how the screenshot run that shipped with wave 2
// recorded `verified: true` for a table whose headline pot was twice the real pot
// (docs/DEFECTS.md T-08) and whose villain's hand rendered as two blank cards
// (T-09). An artifact that says "verified" while quoting a wrong number is worse
// than no artifact.
//
// Every table scene now asserts AGREEMENT:
//
//   * the headline pot            == get_pot()
//   * the pot breakdown           sums to get_pot(), and its "betting" leg is the
//                                 real sum of current_bet
//   * every seat's stack          == that seat's chips in get_table_view()
//   * every seat's bet            == that seat's current_bet (absent iff zero)
//   * every side pot              == side_pots[i].amount, in order
//   * the board                   == get_community_cards(), rank and suit, in order
//   * the table balance           == get_balance() for the signed-in principal
//   * the pot-odds strip          == call_amount and get_pot()
//   * the winner banner           == last_hand_winners[..].amount
//
// plus a mapping check (dealer/SB/BB badges, "me" highlight, all-in and fold
// overlays) whose only job is to prove the DOM-order-is-seat-order assumption
// this file relies on is still true after a redesign.
//
// RACE HANDLING. The page polls every 500 ms, so a screen can legitimately lag
// the chain by one poll. A single read cannot tell lag from a lie. So each
// attempt reads the chain, scrapes the DOM, and reads the chain AGAIN; if the two
// chain reads differ the state moved under us and the attempt is discarded. A
// disagreement has to survive several attempts spanning more than a poll interval
// before it is reported. `attempts` is recorded so a reader can see how hard the
// verdict was won.

import { Principal } from '@dfinity/principal';
import { ledgerBalance, ledgerTransferFee, tableActorFor } from './table-driver.mjs';
import { BTC_MIN_DEPOSIT_SATS as BTC_MIN_DEPOSIT, ICP_MIN_DEPOSIT_E8S as ICP_MIN_DEPOSIT } from './config.mjs';
import { lobbyActor, optional, variantKey } from './agent.mjs';
// The archive and the table's hand record are read through the HARNESS's own
// Candid mirrors, not through `src/declarations`. See the headers of both files;
// the app's declarations are stale (docs/DEFECTS.md E-08, E-67) and Candid
// subtyping drops undeclared fields in silence, which is how a gate ends up
// asserting against a field it cannot see.
import { archiveActor } from './archive-wire.mjs';
import { handRecordActor } from './hand-record-wire.mjs';
import {
  checkFigure, checkPlainNumber, foldFigures, parseDisplayedAmount,
  RANK_BY_GLYPH, SUIT_BY_SYMBOL, cardToText, fmt,
} from './money.mjs';
import {
  closeBetPresets, readBetPreset, scrapeDeposit, scrapeHandHistory, scrapeLobby, scrapeTable,
} from './dom-scrape.mjs';
import { devPlayerPrincipal } from './identities.mjs';
import { thirdPartyObservations } from './browser.mjs';
import {
  bestCategoryName, exactEquity, heroEquityVsRandom, MC_TOLERANCE_POINTS, ORACLE_TRIALS,
  toCard as toOracleCard,
} from './equity-oracle.mjs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Attempts before a disagreement is believed, and the gap between them. */
const MAX_ATTEMPTS = Number(process.env.SHOTS_AGREEMENT_ATTEMPTS || 3);
const ATTEMPT_GAP_MS = 700; // > the app's 500 ms poll, so one lagging frame cannot fail a scene

/**
 * Everything the canister says, read as the principal the browser is logged in as.
 *
 * @param {string} tableId
 * @param {number} playerNum dev player number the browser is signed in as
 */
export async function readTableTruth(tableId, playerNum) {
    const table = await tableActorFor(playerNum, tableId);
    const [viewOpt, pot, board, balance] = await Promise.all([
        table.get_table_view(),
        table.get_pot(),
        table.get_community_cards(),
        table.get_balance(),
    ]);
    const view = optional(viewOpt);
    if (!view) throw new Error(`get_table_view() returned null for ${tableId}`);
    // ABSENCE OF A FIELD IS A STRUCTURAL FAILURE, NEVER A SILENT NaN
    // (docs/DEFECTS.md E-61, and the same rule applied here rather than only
    // written down here). Every one of these is read below with `Number(...)`,
    // and `Number(undefined)` is NaN, which makes every comparison against it
    // vacuously true and the gate silently vacuous with it.
    for (const field of ['pot', 'hand_number', 'my_committed_in_pot', 'hand_is_unmovable']) {
        if (!(field in view)) {
            throw new Error(
                `get_table_view() has no '${field}' field, so the checks that read it cannot run. `
                + `Fields present: ${Object.keys(view).join(', ')}`,
            );
        }
    }

    const seats = view.players.map((p, i) => {
        const player = optional(p);
        if (!player) return { index: i, occupied: false };
        // `hole_cards : opt record { Card; Card }` crosses the wire as
        // `[] | [[Card, Card]]`, so `[0]` is the WHOLE PAIR (docs/DEFECTS.md
        // T-09). Absent for anyone the canister will not show this viewer, which
        // is exactly the information the equity oracle is allowed to use.
        const pair = optional(player.hole_cards);
        return {
            index: i,
            occupied: true,
            chips: Number(player.chips),
            currentBet: Number(player.current_bet),
            folded: player.has_folded,
            allIn: player.is_all_in,
            isSelf: player.is_self,
            status: variantKey(player.status),
            holeCards: Array.isArray(pair) && pair.length >= 2 ? [pair[0], pair[1]] : null,
            displayName: optional(player.display_name),
        };
    });

    return {
        tableId,
        pot: Number(pot),
        viewPot: Number(view.pot),
        currency: variantKey(view.config.currency) === 'BTC' ? 'BTC' : 'ICP',
        maxPlayers: Number(view.config.max_players),
        phase: variantKey(view.phase),
        seats,
        sidePots: view.side_pots.map((sp) => ({ amount: Number(sp.amount) })),
        board: board.map((c) => ({
            rank: Object.keys(c.rank)[0],
            suit: Object.keys(c.suit)[0],
            text: cardToText(c),
        })),
        viewBoardLength: view.community_cards.length,
        balance: Number(balance),
        mySeat: optional(view.my_seat) === null ? null : Number(optional(view.my_seat)),
        callAmount: Number(view.call_amount),
        isMyTurn: view.is_my_turn,
        dealerSeat: Number(view.dealer_seat),
        smallBlindSeat: Number(view.small_blind_seat),
        bigBlindSeat: Number(view.big_blind_seat),
        smallBlind: Number(view.config.small_blind),
        bigBlind: Number(view.config.big_blind),
        handNumber: Number(view.hand_number),
        // THE CALLER'S OWN STAKE IN THE MIDDLE (docs/SECURITY-FINDINGS.md
        // FINDING 18). The dock renders this as ICP; until E-64 nothing read the
        // canister's own figure for it, so the number on the screen was checked
        // against nothing at all.
        myCommittedInPot: Number(view.my_committed_in_pot),
        handIsUnmovable: Boolean(view.hand_is_unmovable),
        // The Candid board, kept alongside the display strings so the equity
        // oracle can rank real cards rather than re-parse glyphs off the screen.
        boardCards: view.community_cards,
        winners: view.last_hand_winners.map((w) => ({
            seat: Number(w.seat),
            amount: Number(w.amount),
        })),
    };
}

/** Fields that must not move between the two chain reads bracketing a scrape. */
function truthDigest(t) {
    return JSON.stringify([
        t.pot, t.viewPot, t.phase, t.balance, t.callAmount, t.mySeat,
        t.dealerSeat, t.smallBlindSeat, t.bigBlindSeat, t.handNumber,
        t.seats.map((s) => [s.occupied, s.chips ?? null, s.currentBet ?? null, s.folded ?? null, s.allIn ?? null]),
        t.sidePots.map((s) => s.amount),
        t.board.map((c) => c.text),
        t.winners.map((w) => [w.seat, w.amount]),
    ]);
}

/** Sum of every seated player's live bet, which is what "betting" means on screen. */
const sumBets = (t) => t.seats.reduce((n, s) => n + (s.occupied ? s.currentBet : 0), 0);

/**
 * Two displayed numbers must add up to one chain number.
 * The windows add, so the combined resolution is the sum of both resolutions.
 */
function checkSum(label, chainValue, texts, currency) {
    const parsed = texts.map((t) => parseDisplayedAmount(t, { currency }));
    if (parsed.some((p) => !p)) {
        return {
            label, chain: Number(chainValue), domText: texts.join(' + '),
            agrees: false, discriminates2x: false, ok: false,
            detail: `could not read two numbers out of ${JSON.stringify(texts)}`,
        };
    }
    const low = parsed.reduce((n, p) => n + p.low, 0);
    const high = parsed.reduce((n, p) => n + p.high, 0);
    const chain = Number(chainValue);
    const agrees = chain >= low - 1e-6 && chain <= high + 1e-6;
    // Same overstatement-direction test as checkFigure: could a doubled chain
    // value have produced this pair of on-screen numbers?
    const resolution = high - low;
    const discriminates2x = chain === 0 ? true : Math.abs(chain) > resolution / 2;
    return {
        label,
        chain,
        domText: parsed.map((p) => p.text).join(' + '),
        agrees,
        discriminates2x,
        ok: agrees && (chain === 0 || discriminates2x),
        detail: agrees
            ? `screen ${parsed.map((p) => p.text).join(' + ')} sums to ${fmt(chain)} (${fmt(low)}..${fmt(high)})`
            : `DISAGREES: screen ${parsed.map((p) => p.text).join(' + ')} sums to ${fmt(low)}..${fmt(high)}, `
              + `canister says ${fmt(chain)}`,
        window: [low, high],
    };
}

/**
 * A displayed ODDS RATIO ("3.5:1" or "1:2.0") against two chain amounts.
 *
 * Not a money figure, but it is derived from one, and it is the number a player
 * uses to decide whether to put money in. Compared at the precision the client
 * printed, so a client that rounds harder is judged more leniently and says so.
 */
function checkRatio(label, potE8s, callE8s, domText) {
    const text = String(domText ?? '').trim();
    const m = /(-?[\d.]+)\s*:\s*(-?[\d.]+)/.exec(text);
    if (call0(callE8s)) {
        return {
            label, chain: null, domText: text, agrees: false, discriminates2x: false, ok: false,
            detail: `a pot-odds ratio is on screen ("${text}") but call_amount is 0 — nothing to call`,
        };
    }
    if (!m) {
        return {
            label, chain: null, domText: text || null, agrees: false, discriminates2x: false, ok: false,
            detail: `could not read an "a:b" ratio out of ${JSON.stringify(domText ?? null)}`,
        };
    }
    const chainRatio = potE8s / callE8s;
    const shownRatio = Number(m[1]) / Number(m[2]);
    // Both legs are printed to one decimal by the client; the coarser leg sets
    // the window. Derive it from the text rather than assuming.
    const dp = (s) => (s.includes('.') ? s.split('.')[1].length : 0);
    const relSlack = 0.5 * Math.pow(10, -Math.min(dp(m[1]), dp(m[2]))) / Math.max(Number(m[1]), Number(m[2]));
    const agrees = Math.abs(shownRatio - chainRatio) <= chainRatio * relSlack + 1e-9;
    const render = (r) => (r >= 1 ? `${r.toFixed(1)}:1` : `1:${(1 / r).toFixed(1)}`);
    const discriminates2x = render(chainRatio * 2) !== render(chainRatio);
    return {
        label,
        chain: Math.round(chainRatio * 1000) / 1000,
        domText: text,
        agrees,
        discriminates2x,
        ok: agrees && discriminates2x,
        detail: agrees
            ? `screen "${text}" == pot ${potE8s} / call ${callE8s} = ${render(chainRatio)}`
            : `DISAGREES: screen "${text}" (${shownRatio.toFixed(3)}:1) but pot ${potE8s} / call `
              + `${callE8s} = ${chainRatio.toFixed(3)}:1 (screen is ${(shownRatio / chainRatio).toFixed(3)}x)`,
    };
}

const call0 = (x) => !Number.isFinite(Number(x)) || Number(x) === 0;

/**
 * THE EQUITY BADGES, RECOMPUTED FROM THE CANISTER'S OWN CARDS.
 *
 * The client puts a percentage next to a player at the all-in and the showdown.
 * The canister exposes no equity, so there is nothing to compare it with — but
 * "nothing to compare it with" is not a licence to allowlist a decimal on the
 * felt. It is recomputed here from `get_table_view()`'s cards, by a second
 * lineage (lib/equity-oracle.mjs ranks all 21 five-card subsets; the client does
 * one direct 7-card pass), and the screen has to match it.
 *
 * WHICH MODE IS LEGAL IS ALSO CHECKED, and that is the security-relevant half:
 * a per-opponent equity may only appear when the CANISTER revealed every live
 * hand. If the screen shows a solid (non-modelled) badge while `hole_cards` is
 * `None` for a live opponent, the client is claiming to know something the
 * canister did not tell it, and the scene fails.
 *
 * @param {object} truth @param {object} dom @param {object[]} figures out-param
 * @returns {string[]} structural problems
 */
function equityProblems(truth, dom, figures) {
    const problems = [];
    const shown = dom.seats.filter((s) => s.equityText);
    if (shown.length === 0 && !dom.equityMethodText && !dom.heroHandText) return problems;

    const board = (truth.boardCards || []).map(toOracleCard);
    if (board.some((c) => c === null)) return ['equity check: a board card did not decode'];

    // Live = the canister's own deal predicate, in the same two phases the
    // client uses (see PokerTable.svelte `isInHand`). Mid-hand that is
    // `Active && !folded`; at showdown `status` may already have been moved on
    // by an auto-deal that refused to deal, so the cards decide instead —
    // `hole_cards` is non-null at showdown exactly for the players this hand was
    // dealt to and who did not fold.
    const showdownPhase = truth.phase === 'Showdown' || truth.phase === 'HandComplete';
    const live = truth.seats.filter((s) => s.occupied && !s.folded
        && (showdownPhase ? Array.isArray(s.holeCards) : s.status === 'Active'));
    const revealed = live.filter((s) => Array.isArray(s.holeCards));
    const heroSeat = truth.mySeat;
    const hero = heroSeat === null ? null : truth.seats[heroSeat];
    const everyLiveHandRevealed = live.length >= 2 && revealed.length === live.length;

    // ---- the mode the client chose has to be the mode it is entitled to ----
    for (const s of shown) {
        if (!s.equityModelled && !everyLiveHandRevealed) {
            problems.push(
                `seat ${s.index} shows a NON-modelled equity "${s.equityText}" but the canister has `
                + `revealed only ${revealed.length} of ${live.length} live hands — the client is claiming `
                + 'knowledge it was not given',
            );
        }
        if (!everyLiveHandRevealed && s.index !== heroSeat) {
            problems.push(
                `seat ${s.index} is not the viewer and its cards are hidden, yet an equity is shown for it`,
            );
        }
    }

    if (everyLiveHandRevealed) {
        // EXACT, both sides. Post-flop this is at most C(45,2) = 990 runouts and
        // with a complete board it is one, so the comparison is on the nose.
        const hands = revealed.map((s) => s.holeCards.map(toOracleCard));
        if (hands.some((h) => h.some((c) => c === null))) return ['equity check: a hole card did not decode'];
        const oracle = exactEquity(hands, board);
        revealed.forEach((s, i) => {
            const domSeat = dom.seats[s.index];
            if (!domSeat?.equityText) {
                problems.push(`seat ${s.index} is live with cards up but shows no equity badge`);
                return;
            }
            figures.push(checkPlainNumber(
                `seat ${s.index} equity vs independent exact enumeration (${oracle.trials} runouts)`,
                Math.round(oracle.share[i] * 10000) / 100, domSeat.equityText, { unit: '%' },
            ));
        });
        const total = oracle.share.reduce((a, b) => a + b, 0);
        if (Math.abs(total - 1) > 1e-9) {
            problems.push(`equity oracle does not sum to 1 (${total}) — the split is wrong`);
        }
    } else if (hero && Array.isArray(hero.holeCards) && shown.length > 0) {
        // The MODEL. Two independent Monte Carlos, compared inside a stated
        // tolerance rather than for equality — see MC_TOLERANCE_POINTS.
        const heroCards = hero.holeCards.map(toOracleCard);
        if (heroCards.some((c) => c === null)) return ['equity check: a hero card did not decode'];
        const opponents = live.length - 1;
        const oracle = heroEquityVsRandom(heroCards, board, opponents, ORACLE_TRIALS);
        const domSeat = dom.seats[heroSeat];
        const screenPct = Number(String(domSeat.equityText).replace(/[^\d.]/g, ''));
        const oraclePct = oracle ? oracle.equity * 100 : null;
        const delta = oraclePct === null ? null : Math.abs(screenPct - oraclePct);
        const ok = delta !== null && delta <= MC_TOLERANCE_POINTS;
        figures.push({
            label: `seat ${heroSeat} modelled equity vs independent Monte Carlo (${ORACLE_TRIALS} trials)`,
            chain: oraclePct === null ? null : Math.round(oraclePct * 100) / 100,
            domText: domSeat.equityText,
            agrees: ok,
            discriminates2x: true,
            ok,
            detail: ok
                ? `screen ${screenPct}% vs independent ${oraclePct.toFixed(2)}% `
                  + `(|d| ${delta.toFixed(3)} <= ${MC_TOLERANCE_POINTS.toFixed(3)} points, 5 sigma)`
                : `DISAGREES: screen ${screenPct}% vs independent `
                  + `${oraclePct === null ? 'n/a' : oraclePct.toFixed(2)}% (tolerance `
                  + `${MC_TOLERANCE_POINTS.toFixed(3)} points)`,
        });
        if (!/\bvs\s+\d+\s+random\b/i.test(dom.equityMethodText || '')) {
            problems.push(
                'a MODELLED equity is on screen but the method line does not say "vs N random": '
                + `"${dom.equityMethodText}"`,
            );
        }
        if (!new RegExp(`vs ${opponents} random`, 'i').test(dom.equityMethodText || '')) {
            problems.push(
                `the method line does not name ${opponents} random opponent(s) but ${live.length} `
                + `player(s) are live: "${dom.equityMethodText}"`,
            );
        }
    }

    // ---- the method line has to state a real method ----------------------
    if (dom.equityMethodText && !/\b(exact|monte carlo)\b/i.test(dom.equityMethodText)) {
        problems.push(`equity is on screen but no method is stated: "${dom.equityMethodText}"`);
    }

    // ---- the hand-strength readout ---------------------------------------
    problems.push(...heroHandProblems(truth, dom, board, hero));

    return problems;
}

/**
 * The shape the client must print for each category.
 *
 * A `includes(category)` test would be worthless here: "Straight Flush, Ten
 * high" contains "Flush" AND "Straight", and "Two Pair, Aces and Nines"
 * contains "Pair". Each category therefore has an ANCHORED pattern, so naming a
 * two pair "Pair of Aces" fails instead of passing on a substring.
 */
const RANK_WORDS = '(?:two|three|four|five|six|seven|eight|nine|ten|jack|queen|king|ace)';
const RANK_PLURALS = '(?:twos|threes|fours|fives|sixes|sevens|eights|nines|tens|jacks|queens|kings|aces)';
// The plate row prints the COMPACT form of the same name (src/lib/hand-names.js
// deletes the kicker clause and, where the ranks already say the category,
// the category prefix: "Aces full of Eights", "Aces and Eights", "Three
// Sevens"). Each alternate is anchored on the rank words of ITS category, so
// "aces and eights" still cannot pass as a pair and "three sevens" cannot pass
// as a straight.
const HAND_PHRASE = {
    'Royal Flush': /^royal flush$/,
    'Straight Flush': /^straight flush(?:, .+ high)?$/,
    'Four of a Kind': new RegExp(`^(?:four of a kind, |four )${RANK_PLURALS}$`),
    'Full House': new RegExp(`^(?:full house, )?${RANK_PLURALS} full of ${RANK_PLURALS}$`),
    Flush: /^flush, .+ high$/,
    Straight: /^straight, .+ high$/,
    'Three of a Kind': new RegExp(`^(?:three of a kind, |three )${RANK_PLURALS}$`),
    'Two Pair': new RegExp(`^(?:two pair, )?${RANK_PLURALS} and ${RANK_PLURALS}$`),
    Pair: new RegExp(`^pair of ${RANK_PLURALS}(?:, ${RANK_WORDS} kicker)?$`),
    'High Card': new RegExp(`^${RANK_WORDS} high$`),
};

/**
 * "Your hand · Two Pair, Nines and Twos", re-derived from the canister's cards.
 *
 * Pre-flop there is no five-card hand, so the client names the HOLDING instead
 * and that is checked too: the two rank words and suited/offsuit all come
 * straight out of `hole_cards`, so a client that mislabels its own hand is
 * caught on the one scene where the board is empty.
 */
function heroHandProblems(truth, dom, board, hero) {
    if (!dom.heroHandText) return [];
    if (!hero || !Array.isArray(hero.holeCards)) {
        return [`a "your hand" readout is on screen ("${dom.heroHandText}") but the canister sent `
            + 'no hole cards for the viewer'];
    }
    const phrase = dom.heroHandText.replace(/^.*?·\s*/, '').trim().toLowerCase();
    const hole = hero.holeCards.map(toOracleCard);
    if (hole.some((c) => c === null)) return ['equity check: a hero hole card did not decode'];

    if (board.length < 3) {
        // Pre-flop: the holding, named. "Pocket Queens" or "Queen-Six offsuit".
        const [hi, lo] = [...hole].sort((a, b) => b.rank - a.rank);
        const word = (r) => Object.entries({
            Two: 2, Three: 3, Four: 4, Five: 5, Six: 6, Seven: 7, Eight: 8, Nine: 9,
            Ten: 10, Jack: 11, Queen: 12, King: 13, Ace: 14,
        }).find(([, v]) => v === r)?.[0].toLowerCase();
        const plural = { six: 'sixes' };
        const expected = hi.rank === lo.rank
            ? `pocket ${plural[word(hi.rank)] ?? `${word(hi.rank)}s`}`
            : `${word(hi.rank)}-${word(lo.rank)} ${hi.suit === lo.suit ? 'suited' : 'offsuit'}`;
        return phrase === expected ? []
            : [`"your hand" says "${phrase}" but the canister dealt the viewer ${expected}`];
    }

    const category = bestCategoryName([...hole, ...board]);
    const pattern = HAND_PHRASE[category];
    if (!pattern) return [`equity check: no expected phrase for category ${category}`];
    return pattern.test(phrase) ? []
        : [`"your hand" says "${phrase}" but the canister's cards rank as ${category}`];
}

/** Non-money structural facts that keep the seat mapping honest. */
function mappingChecks(truth, dom) {
    const problems = [];
    const seatCount = Math.min(dom.seats.length, truth.seats.length);

    if (dom.seats.length === 0) problems.push('no .seat elements found on the table page');

    for (let i = 0; i < seatCount; i += 1) {
        const c = truth.seats[i];
        const d = dom.seats[i];
        if (c.occupied !== d.occupied) {
            problems.push(`seat ${i}: canister says ${c.occupied ? 'occupied' : 'empty'}, screen says ${d.occupied ? 'occupied' : 'empty'}`);
            continue;
        }
        if (!c.occupied) continue;
        if (c.allIn !== d.allIn) problems.push(`seat ${i}: all-in badge ${d.allIn} but canister is_all_in=${c.allIn}`);
        if (c.folded !== d.folded) problems.push(`seat ${i}: fold badge ${d.folded} but canister has_folded=${c.folded}`);
        if (truth.mySeat !== null && (i === truth.mySeat) !== d.isMe) {
            problems.push(`seat ${i}: "me" highlight ${d.isMe} but my_seat=${truth.mySeat}`);
        }
        if (c.occupied && (i === truth.dealerSeat) !== d.dealerBadge) {
            problems.push(`seat ${i}: dealer badge ${d.dealerBadge} but dealer_seat=${truth.dealerSeat}`);
        }
    }
    return problems;
}

/** Compares one DOM snapshot with one chain snapshot. */
function compare(truth, dom, opts) {
    const currency = truth.currency;
    const figures = [];
    const structural = [];

    if (!dom.found.felt) structural.push('SCRAPE FAILED: no .felt / .poker-table on the page');
    if (dom.found.seats === 0) structural.push('SCRAPE FAILED: no .seat elements');

    // ---- the header stakes pill -----------------------------------------
    // The largest teal string on a table screen, and until this pass nothing
    // looked at it: it rendered the LOBBY canister's stale row name, which
    // quotes blinds that are 5x and 10x below what table_2 and table_3 charge,
    // while the scene was filed "agrees with chain: yes". A pill that quotes
    // blinds must quote THIS table's blinds. A pill that quotes none (the
    // format alone, which is what the client shows before the view lands) is
    // fine and is not a figure. docs/DEFECTS.md T-11.
    if (dom.headerStakesText) {
        const pill = /(\d[\d.,]*)\s*\/\s*(\d[\d.,]*)/.exec(dom.headerStakesText);
        if (pill) {
            figures.push(checkFigure(
                `table header pill "${dom.headerStakesText}" small blind`,
                truth.smallBlind, pill[1], { currency },
            ));
            figures.push(checkFigure(
                `table header pill "${dom.headerStakesText}" big blind`,
                truth.bigBlind, pill[2], { currency },
            ));
        }
    }

    // ---- the pot ---------------------------------------------------------
    // At HandComplete the winner banner replaces the pot display, so the pot is
    // checked only where the app renders it.
    const handComplete = truth.phase === 'HandComplete';
    if (!handComplete) {
        if (!dom.found.potAmount) {
            structural.push('SCRAPE FAILED: no .pot-amount while a hand is live');
        } else {
            figures.push(checkFigure('pot (headline) vs get_pot()', truth.pot, dom.potAmountText, {
                currency, allowAbsentWhenZero: true,
            }));
        }
        // "(A + B betting)" — design-agnostic: whatever the two legs are, they
        // must add up to the real pot, and the second must be the real live bets.
        if (dom.potBreakdownText) {
            const nums = dom.potBreakdownText.match(/-?\d[\d.,]*\s*[KM]?/g) || [];
            if (nums.length >= 2) {
                figures.push(checkSum('pot breakdown sums to get_pot()', truth.pot, [nums[0], nums[1]], currency));
                figures.push(checkFigure('pot breakdown "betting" leg vs sum(current_bet)', sumBets(truth), nums[1], { currency }));
            } else {
                structural.push(`SCRAPE FAILED: .pot-breakdown "${dom.potBreakdownText}" has fewer than two numbers`);
            }
        }
    }

    // ---- side pots -------------------------------------------------------
    if (dom.sidePots.length !== truth.sidePots.length) {
        structural.push(
            `side pot COUNT disagrees: screen ${dom.sidePots.length}, canister ${truth.sidePots.length}`,
        );
    }
    for (let i = 0; i < Math.min(dom.sidePots.length, truth.sidePots.length); i += 1) {
        figures.push(checkFigure(`side pot ${i + 1}`, truth.sidePots[i].amount, dom.sidePots[i].amountText, { currency }));
    }
    // The canister's own invariant (docs/DEFECTS.md E-03): `state.side_pots` is
    // display-only state refreshed after every action, and it is a DECOMPOSITION
    // of `pot`, not an addition to it. Checked here because the client renders the
    // headline pot and the side pots side by side, so if the invariant ever broke
    // the screen would be adding up to something that does not exist.
    if (truth.sidePots.length > 0) {
        const sum = truth.sidePots.reduce((n, s) => n + s.amount, 0);
        if (sum !== truth.pot) {
            structural.push(
                `canister invariant broken: side pots sum to ${sum} e8s but get_pot() is ${truth.pot} e8s`,
            );
        }
    }

    // ---- per-seat stacks and bets ---------------------------------------
    for (let i = 0; i < Math.min(dom.seats.length, truth.seats.length); i += 1) {
        const c = truth.seats[i];
        const d = dom.seats[i];
        if (!c.occupied || !d.occupied) continue;
        figures.push(checkFigure(`seat ${i} stack`, c.chips, d.chipsText, { currency }));
        figures.push(checkFigure(`seat ${i} bet`, c.currentBet, d.betText, {
            currency, allowAbsentWhenZero: true,
        }));
    }

    // ---- the board -------------------------------------------------------
    const boardProblems = [];
    for (let i = 0; i < truth.board.length; i += 1) {
        const slot = dom.board[i];
        if (!slot) { boardProblems.push(`board slot ${i}: nothing rendered, canister dealt ${truth.board[i].text}`); continue; }
        if (slot.unreadable) {
            boardProblems.push(
                `SCRAPE CONTRACT BROKEN at board slot ${i}: the card is face UP (.card-front is `
                + 'present) but no known rank/suit selector matched, so this slot was NOT compared '
                + `with the canister's ${truth.board[i].text}. Update readCard() in `
                + `lib/dom-scrape.mjs. Markup: ${slot.markup}`,
            );
            continue;
        }
        if (slot.faceDown || slot.empty || !slot.rank || !slot.suit) {
            boardProblems.push(`board slot ${i}: rendered ${slot.faceDown ? 'FACE DOWN' : slot.empty ? 'EMPTY' : 'blank'}, canister dealt ${truth.board[i].text}`);
            continue;
        }
        const rank = RANK_BY_GLYPH[slot.rank];
        const suit = SUIT_BY_SYMBOL[slot.suit];
        if (rank !== truth.board[i].rank || suit !== truth.board[i].suit) {
            boardProblems.push(`board slot ${i}: screen ${slot.rank}${slot.suit}, canister ${truth.board[i].text}`);
        }
    }
    for (let i = truth.board.length; i < dom.board.length; i += 1) {
        const slot = dom.board[i];
        if (slot && !slot.faceDown && !slot.empty && slot.rank) {
            boardProblems.push(`board slot ${i}: screen shows ${slot.rank}${slot.suit} but the canister has dealt only ${truth.board.length} cards`);
        }
    }
    if (truth.viewBoardLength !== truth.board.length) {
        boardProblems.push(
            `canister disagrees with itself: get_community_cards()=${truth.board.length} `
            + `but get_table_view().community_cards=${truth.viewBoardLength}`,
        );
    }

    // ---- table balance ---------------------------------------------------
    if (dom.tableBalanceText !== null && dom.tableBalanceText !== undefined) {
        figures.push(checkFigure('table balance vs get_balance()', truth.balance, dom.tableBalanceText, { currency }));
    } else if (opts.requireBalance) {
        structural.push('SCRAPE FAILED: no wallet balance on screen');
    }

    // ---- the caller's own stake in the middle ----------------------------
    //
    // docs/DEFECTS.md E-64. This block only renders when `my_committed_in_pot`
    // is non-zero, so its ABSENCE is checked as hard as its value: a canister
    // that says a player has money in the pot while the dock says nothing is the
    // FINDING 18 silence coming back, and it would otherwise look identical to
    // "the block is not on this scene".
    if (dom.committedValueText !== null && dom.committedValueText !== undefined) {
        figures.push(checkFigure(
            'committed stake vs get_table_view().my_committed_in_pot',
            truth.myCommittedInPot, dom.committedValueText, { currency },
        ));
        // The sentence carries a hand number, which is the other numeric token
        // the census found unasserted at this site.
        const notedHand = (dom.committedNoteText || '').match(/hand\s+(\d[\d,]*)/i);
        if (notedHand) {
            figures.push(checkPlainNumber(
                'the hand named in the committed note vs get_table_view().hand_number',
                truth.handNumber, notedHand[1],
            ));
        }
        // The panel turns red on a hand nothing can move. The canister decides
        // that, not the client.
        if (dom.committedIsStuck !== truth.handIsUnmovable) {
            structural.push(
                `the committed-stake panel says the hand is ${dom.committedIsStuck ? '' : 'not '}`
                + `unmovable while get_table_view().hand_is_unmovable is ${truth.handIsUnmovable}`,
            );
        }
    } else if (truth.myCommittedInPot > 0) {
        structural.push(
            `THE PLAYER IS NOT TOLD: get_table_view().my_committed_in_pot is `
            + `${truth.myCommittedInPot} e8s of this caller's money in the pot and no `
            + '.committed-value is on screen. That silence is docs/SECURITY-FINDINGS.md '
            + 'FINDING 18 and this readout is the fix for it.',
        );
    }

    // ---- pot odds strip --------------------------------------------------
    if (dom.potOddsText) {
        const nums = dom.potOddsText.match(/-?\d[\d.,]*\s*[KM]?/g) || [];
        if (nums.length >= 2) {
            figures.push(checkFigure('pot-odds "Call X" vs call_amount', truth.callAmount, nums[0], { currency }));
            figures.push(checkFigure('pot-odds "to win Y" vs get_pot()', truth.pot, nums[1], { currency }));
        }
    }

    // The redesigned dock renders pot odds as a RATIO and a required-equity
    // percentage instead of "Call X to win Y". Neither is a chain field, but both
    // are computed from the pot, so a pot that is wrong on the felt is wrong here
    // too — and these two are worse than the headline, because a player reads
    // them to decide whether calling is +EV. They are checked against the
    // canister's own pot and call_amount, at the precision the client chose.
    if (dom.potOddsValue) {
        figures.push(checkRatio(
            'pot-odds ratio vs get_pot()/call_amount',
            truth.pot, truth.callAmount, dom.potOddsValue,
        ));
    }
    if (dom.equityHint) {
        const pct = truth.pot + truth.callAmount === 0
            ? null
            : (truth.callAmount / (truth.pot + truth.callAmount)) * 100;
        figures.push(checkPlainNumber(
            'required-equity % vs call_amount/(get_pot()+call_amount)',
            pct === null ? null : Math.round(pct * 10) / 10, dom.equityHint, { unit: '%' },
        ));
    }

    // ---- "Call X" wherever the client writes it --------------------------
    // The call amount appears on the primary action button and in the turn hint.
    // Both are money the player is about to commit, and both come from the same
    // `call_amount` field, so both are asserted against it.
    for (const label of dom.actionButtons || []) {
        if (!/^call\b/i.test(label)) continue;
        figures.push(checkFigure('action button "Call X" vs call_amount', truth.callAmount, label.replace(/^call/i, ''), { currency }));
    }
    if (dom.turnHint && /call/i.test(dom.turnHint) && /\d/.test(dom.turnHint)) {
        figures.push(checkFigure('turn hint "Call X" vs call_amount', truth.callAmount, dom.turnHint.replace(/^[^\d-]*/, ''), { currency }));
    }

    // ---- winner banner ---------------------------------------------------
    if (dom.winnerText) {
        if (truth.winners.length === 0) {
            structural.push(`winner banner on screen ("${dom.winnerText}") but last_hand_winners is empty`);
        } else {
            const mine = truth.mySeat === null ? undefined : truth.winners.find((w) => w.seat === truth.mySeat);
            const shown = mine ?? truth.winners[0];
            const nums = dom.winnerText.match(/-?\d[\d.,]*\s*[KM]?/g) || [];
            // "Seat N wins X" leads with the seat number; "You won X" does not.
            const amountText = mine ? nums[0] : nums[1];
            if (!mine && nums.length >= 1 && Number(nums[0]) !== truth.winners[0].seat + 1) {
                structural.push(`winner banner names seat ${nums[0]}, canister says seat ${truth.winners[0].seat + 1}`);
            }
            figures.push(checkFigure('winner amount vs last_hand_winners', shown.amount, amountText, { currency }));
        }
    } else if (handComplete && truth.winners.length > 0 && opts.requireWinnerBanner) {
        structural.push(`canister has ${truth.winners.length} winner(s) for hand ${truth.handNumber} but no winner banner is on screen`);
    }

    // ---- the delta chip on the winner's pod ------------------------------
    // `+24.00` under a winner's stack is the amount that seat just received. It
    // is MONEY, at a pod, and it is the one figure from which a player recovers
    // what changed -- so it is asserted per seat against that seat's own entry
    // in last_hand_winners, not against "some winner".
    for (const seat of dom.seats) {
        if (!seat.awardText) continue;
        const won = truth.winners.find((w) => w.seat === seat.index);
        if (!won) {
            structural.push(
                `seat ${seat.index} shows an award chip "${seat.awardText}" but is not in last_hand_winners`,
            );
            continue;
        }
        figures.push(checkFigure(
            `seat ${seat.index} award chip vs last_hand_winners`,
            won.amount, seat.awardText.replace(/^\+/, ''), { currency },
        ));
    }
    for (const w of truth.winners) {
        const seat = dom.seats[w.seat];
        if (handComplete && seat && seat.occupied && !seat.awardText) {
            structural.push(`canister paid seat ${w.seat} ${w.amount} but that pod shows no award chip`);
        }
    }

    // ---- equity, recomputed ----------------------------------------------
    structural.push(...equityProblems(truth, dom, figures));

    // ---- structure that keeps the mapping honest -------------------------
    structural.push(...mappingChecks(truth, dom));
    structural.push(...boardProblems);

    const folded = foldFigures(figures);
    return {
        ok: folded.ok && structural.length === 0,
        moneyFiguresChecked: folded.checked,
        moneyMismatches: folded.mismatches,
        structuralProblems: structural,
        figures,
    };
}

/**
 * The assertion every table scene runs before its screenshot is allowed the
 * canonical filename.
 *
 * @param {object} ctx run context (ctx.tableIds)
 * @param {import('playwright').Page} page
 * @param {{table:string, asPlayer:number, requireBalance?:boolean, requireWinnerBanner?:boolean}} opts
 * @returns {Promise<{ok:boolean, checks:object, notes:string}>}
 */
export async function assertChainAgreement(ctx, page, opts) {
    const tableId = ctx.tableIds[opts.table];
    if (!tableId) throw new Error(`No local canister id for ${opts.table}`);

    let last = null;
    let truth = null;
    let dom = null;
    let attempts = 0;
    let discarded = 0;

    for (let i = 0; i < MAX_ATTEMPTS; i += 1) {
        const before = await readTableTruth(tableId, opts.asPlayer);
        const snapshot = await scrapeTable(page);
        const after = await readTableTruth(tableId, opts.asPlayer);
        if (truthDigest(before) !== truthDigest(after)) {
            // The chain moved while we were looking. Not evidence either way.
            discarded += 1;
            await sleep(ATTEMPT_GAP_MS);
            continue;
        }
        attempts += 1;
        truth = before;
        dom = snapshot;
        last = compare(before, snapshot, opts);
        if (last.ok) break;
        await sleep(ATTEMPT_GAP_MS);
    }

    if (!last) {
        return {
            ok: false,
            checks: {
                chainAgreement: 'INCONCLUSIVE',
                reason: `on-chain state never held still across ${MAX_ATTEMPTS} attempts, so the screen could not be compared with it`,
                discardedAttempts: discarded,
            },
            notes: 'chain agreement INCONCLUSIVE: state kept moving',
        };
    }

    const checks = {
        chainAgreement: last.ok ? 'AGREES' : 'DISAGREES',
        attempts,
        discardedAttempts: discarded,
        moneyFiguresChecked: last.moneyFiguresChecked,
        moneyMismatches: last.moneyMismatches,
        structuralProblems: last.structuralProblems,
        // The canister's own numbers, so a reader of manifest.json can redo the
        // comparison by hand against the PNG without re-running anything.
        onChain: {
            getPot: truth.pot,
            viewPot: truth.viewPot,
            sumOfLiveBets: sumBets(truth),
            sidePots: truth.sidePots.map((s) => s.amount),
            seats: truth.seats.filter((s) => s.occupied)
                .map((s) => ({ seat: s.index, chips: s.chips, bet: s.currentBet, allIn: s.allIn, folded: s.folded })),
            board: truth.board.map((c) => c.text),
            balance: truth.balance,
            callAmount: truth.callAmount,
            phase: truth.phase,
            handNumber: truth.handNumber,
            winners: truth.winners,
        },
        onScreen: {
            pot: dom.potAmountText,
            potBreakdown: dom.potBreakdownText,
            sidePots: dom.sidePots.map((s) => s.amountText),
            seats: dom.seats.filter((s) => s.occupied)
                .map((s) => ({ seat: s.index, name: s.name, chips: s.chipsText, bet: s.betText })),
            board: dom.board.map((c) => (c.faceDown ? 'down' : c.empty ? 'empty' : `${c.rank}${c.suit}`)),
            balance: dom.tableBalanceText,
            potOdds: dom.potOddsText,
            winner: dom.winnerText,
        },
        figures: last.figures.map((f) => ({ label: f.label, chain: f.chain, screen: f.domText, ok: f.ok, detail: f.detail })),
    };

    const problems = [...last.moneyMismatches, ...last.structuralProblems];
    const notes = last.ok
        ? `chain agreement: ${last.moneyFiguresChecked} money figures on screen all equal the canister's`
        : `CHAIN DISAGREEMENT (${problems.length}): ${problems.slice(0, 4).join(' | ')}`;

    return { ok: last.ok, checks, notes };
}

/**
 * THE BET-SIZING PRESETS: what the client would WAGER, not what it displays.
 *
 * `½ Pot` and `Pot` write an amount into the raise field that the next click
 * sends to the canister. If the client's idea of "the pot" is wrong, this is not
 * a cosmetic defect: the player commits real chips at a size they did not
 * intend. So the presets are read off the live UI and compared with the
 * canister's own `get_pot()`, using the poker definition the labels promise:
 *
 *   Pot   = raise TO (pot + amount_to_call)   — the pot after you call
 *   ½ Pot = raise TO (pot/2), floored at the legal minimum raise
 *
 * WHAT "POT-SIZED" MEANS, WRITTEN DOWN ONCE.
 *
 * The value in the raise field is a RAISE TO: the player's total commitment for
 * the round, not the chips added. A pot-sized raise is call first, then raise by
 * the pot as it stands after that call, so with `B` = table `current_bet`,
 * `m` = my `current_bet`, `P` = `get_pot()` and `c = B - m = call_amount`:
 *
 *     Pot    raise TO  B + P + c        ( = P + 2B - m )
 *     ½ Pot  raise TO  B + floor(P/2 + c/2)
 *
 * `P` already contains every live bet, which is exactly the fact T-08 got wrong.
 *
 * Getting this arithmetic wrong in the HARNESS is as bad as getting it wrong in
 * the client, and the first draft of this check did: it used `P + c` as a
 * ceiling, which is a chips-added quantity compared against a raise-to figure,
 * and it convicted a correct client. So the observed value is also inverted back
 * into an IMPLIED POT, `implied = observed - B - c`, and reported next to
 * `get_pot()`. That number is definition-free: if the client is sizing off twice
 * the pot, `impliedPot / get_pot()` reads 2.0 and says so, whatever formula
 * either side prefers.
 *
 * Both presets are floored at the legal minimum (`current_bet + min_raise`, or
 * `min_bet` when there is no bet) and capped at the player's stack, so a preset
 * that lands on the floor or the cap is reported as UNCONSTRAINING rather than as
 * a pass — a preset pinned to the floor proves nothing about the pot behind it.
 *
 * Nothing is committed: the popover is opened, read and closed again.
 *
 * @param {object} ctx
 * @param {import('playwright').Page} page
 * @param {{table:string, asPlayer:number}} opts
 */
export async function assertBetPresetsAgree(ctx, page, opts) {
    const tableId = ctx.tableIds[opts.table];
    if (!tableId) throw new Error(`No local canister id for ${opts.table}`);
    const truth = await readTableTruth(tableId, opts.asPlayer);
    const raw = await readRaiseContext(tableId, opts.asPlayer);

    if (!truth.isMyTurn) {
        return {
            ok: true,
            checks: { betPresets: 'NOT APPLICABLE', reason: 'the hero is not on the clock, so no preset is reachable' },
            notes: 'bet presets not checked (not the hero\'s turn)',
        };
    }

    const results = [];
    const figures = [];
    const advisory = [];

    const floor = raw.currentBet === 0 ? raw.minBet : raw.currentBet + raw.minRaise;
    const cap = raw.myChips + raw.myCurrentBet;
    const B = raw.currentBet;
    const c = truth.callAmount;
    const expectedRaiseTo = {
        Pot: Math.min(cap, Math.max(floor, B + truth.pot + c)),
        '½ Pot': Math.min(cap, Math.max(floor, B + Math.floor((truth.pot + c) / 2))),
    };
    /** The pot the client must have been sizing from, given what it proposed. */
    const impliedPotFrom = (raiseTo, label) =>
        (label === 'Pot' ? raiseTo - B - c : (raiseTo - B) * 2 - c);

    for (const label of ['½ Pot', 'Pot']) {
        const read = await readBetPreset(page, label);
        const sliderValue = read.sliderValue === null || read.sliderValue === undefined
            ? null : Number(read.sliderValue);
        const impliedPot = sliderValue === null ? null : impliedPotFrom(sliderValue, label);
        const entry = {
            label,
            ...read,
            sliderValueE8s: sliderValue,
            expectedRaiseToE8s: expectedRaiseTo[label],
            impliedPotE8s: impliedPot,
            impliedPotMultiple: impliedPot === null || truth.pot === 0
                ? null : Math.round((impliedPot / truth.pot) * 1000) / 1000,
        };
        results.push(entry);
        if (!read.available) {
            advisory.push(`preset "${label}" is not reachable on this screen`);
            continue;
        }
        if (sliderValue === null) {
            advisory.push(`preset "${label}": no slider value to read`);
            continue;
        }
        // The two places the client SHOWS the amount must agree with the amount
        // it would SEND. A popover that displays one number and posts another is
        // its own defect, so this is checked before anything about the pot.
        for (const [what, text] of [['slider readout', read.amountText], ['confirm button', read.confirmText]]) {
            if (!text) continue;
            figures.push(checkFigure(
                `bet preset "${label}" ${what} vs the value that would be SENT`,
                sliderValue, text, { currency: truth.currency },
            ));
        }

        const pinned = sliderValue === floor ? 'the legal FLOOR'
            : sliderValue === cap ? 'the stack CAP' : null;
        if (pinned) {
            advisory.push(
                `preset "${label}" landed on ${pinned} (${sliderValue} e8s), so this sample `
                + 'cannot discriminate a wrong pot behind it',
            );
            continue;
        }
        const want = expectedRaiseTo[label];
        const ok = sliderValue === want;
        figures.push({
            label: `bet preset "${label}" is sized from get_pot()`,
            chain: want,
            domText: String(sliderValue),
            agrees: ok,
            // Would a doubled pot have produced a different number here? It moves
            // the target by exactly get_pot(), so yes whenever the pot is non-zero
            // and the result is not pinned by the floor or the cap.
            discriminates2x: truth.pot > 0,
            ok,
            detail: ok
                ? `raise-to ${sliderValue} e8s == current_bet ${B} + get_pot() ${truth.pot} `
                  + `${label === 'Pot' ? '+' : '/2 +'} call ${c} (implied pot ${impliedPot} e8s, `
                  + `${entry.impliedPotMultiple}x get_pot())`
                : `WOULD WAGER a raise-to of ${sliderValue} e8s where a ${label} raise is ${want} e8s. `
                  + `The client is sizing from a pot of ${impliedPot} e8s — ${entry.impliedPotMultiple}x `
                  + `get_pot() (${truth.pot}). This number is SENT to the canister, not merely displayed.`,
        });
    }
    await closeBetPresets(page);

    const folded = foldFigures(figures);
    return {
        ok: folded.ok,
        checks: {
            betPresets: folded.ok ? 'AGREE' : 'DISAGREE',
            moneyFiguresChecked: folded.checked,
            moneyMismatches: folded.mismatches,
            advisory,
            onChain: {
                getPot: truth.pot, callAmount: truth.callAmount,
                currentBet: raw.currentBet, minRaise: raw.minRaise, minBet: raw.minBet,
                legalFloor: floor, cap,
            },
            onScreen: results,
            figures: folded.figures.map((f) => ({ label: f.label, chain: f.chain, screen: f.domText, ok: f.ok, detail: f.detail })),
        },
        notes: folded.ok
            ? `bet presets: ${folded.checked} sizing figure(s) match get_pot()`
            : `BET SIZING DISAGREEMENT: ${folded.mismatches.slice(0, 3).join(' | ')}`,
    };
}

/** The raise-legality fields the presets are floored and capped by. */
async function readRaiseContext(tableId, playerNum) {
    const table = await tableActorFor(playerNum, tableId);
    const view = optional(await table.get_table_view());
    if (!view) throw new Error(`get_table_view() returned null for ${tableId}`);
    const mySeat = optional(view.my_seat);
    const me = mySeat === null ? null : optional(view.players[Number(mySeat)]);
    return {
        currentBet: Number(view.current_bet),
        minRaise: Number(view.min_raise ?? view.config.big_blind),
        minBet: Number(view.min_bet ?? view.config.big_blind),
        myChips: Number(me?.chips ?? 0),
        myCurrentBet: Number(me?.current_bet ?? 0),
    };
}

/**
 * The lobby's money: every row's blinds and buy-in range against the canisters.
 *
 * Two sources are compared, because they can disagree and the player only ever
 * sees one of them: the LOBBY canister's registration (what the row renders) and
 * the TABLE canister's own `config` (what the table will actually charge). A row
 * advertising blinds the table does not enforce is a money lie even though both
 * numbers are "on chain".
 *
 * @param {object} ctx
 * @param {import('playwright').Page} page
 */
export async function assertLobbyAgreement(ctx, page) {
    const lobby = await lobbyActor(ctx.ids.lobby);
    const registered = await lobby.get_tables();

    const byName = new Map();
    for (const t of registered) {
        const cid = optional(t.canister_id);
        byName.set(t.name, {
            name: t.name,
            canisterId: cid ? cid.toText() : null,
            currency: variantKey(optional(t.currency) ?? t.config.currency) === 'BTC' ? 'BTC' : 'ICP',
            lobbyConfig: {
                smallBlind: Number(t.config.small_blind),
                bigBlind: Number(t.config.big_blind),
                minBuyIn: Number(t.config.min_buy_in),
                maxBuyIn: Number(t.config.max_buy_in),
                maxPlayers: Number(t.config.max_players),
            },
            playerCount: Number(t.player_count),
        });
    }

    // The table canister's own view of the same numbers.
    const readTableSide = async (entry) => {
        if (!entry.canisterId) return;
        try {
            const table = await tableActorFor(1, entry.canisterId);
            const [viewOpt, count, pot] = await Promise.all([
                table.get_table_view(), table.get_player_count(), table.get_pot(),
            ]);
            const v = optional(viewOpt);
            entry.tableConfig = v
                ? {
                    smallBlind: Number(v.config.small_blind),
                    bigBlind: Number(v.config.big_blind),
                    minBuyIn: Number(v.config.min_buy_in),
                    maxBuyIn: Number(v.config.max_buy_in),
                    maxPlayers: Number(v.config.max_players),
                    ante: Number(v.config.ante),
                    actionTimeoutSecs: Number(v.config.action_timeout_secs),
                    timeBankSecs: Number(v.config.time_bank_secs),
                }
                : null;
            entry.livePlayerCount = Number(count);
            entry.livePot = Number(pot);
            entry.livePhase = v ? variantKey(v.phase) : null;
            // Everything the PREVIEW PANE renders, straight from the same view.
            entry.handNumber = v ? Number(v.hand_number) : null;
            entry.lastPot = v && v.last_hand_winners.length
                ? v.last_hand_winners.reduce((n, w) => n + Number(w.amount), 0)
                : null;
            entry.seated = v
                ? v.players
                    .map((p, i) => ({ seat: i, player: optional(p) }))
                    .filter((s) => s.player)
                    .map((s) => ({
                        seat: s.seat,
                        name: optional(s.player.display_name),
                        chips: Number(s.player.chips),
                    }))
                : [];
            entry.board = v ? v.community_cards.map((c) => cardToText(c)) : [];
        } catch (e) {
            entry.tableError = String(e.message || e).split('\n')[0];
        }
    };

    for (const entry of byName.values()) await readTableSide(entry);
    const potsBefore = new Map([...byName.values()].map((e) => [e.name, e.livePot]));

    const dom = await scrapeLobby(page);

    // Second read, so a live pot that moved under us is reported as unstable
    // rather than as a lie. Blinds and buy-ins are static and need no bracket.
    for (const entry of byName.values()) await readTableSide(entry);
    const figures = [];
    const structural = [];
    const advisory = [];

    if (dom.rowCount === 0) structural.push('SCRAPE FAILED: no lobby rows on the page');

    for (const row of dom.rows) {
        const entry = byName.get(row.name);
        if (!entry) {
            structural.push(`lobby row "${row.name}" is not a table the lobby canister registered`);
            continue;
        }
        const cur = entry.currency;
        // WHICH NUMBER IS THE TRUTH.
        // The row renders the LOBBY canister's registration. The money a player
        // actually pays is set by the TABLE canister's own config. When those
        // differ, rendering the lobby's copy faithfully still leaves a false
        // number on screen, so the TABLE's config is what the gate compares
        // against, and "the client rendered its own source correctly" is recorded
        // separately so the diagnosis is never ambiguous.
        const cfg = entry.tableConfig ?? entry.lobbyConfig;

        // "0.05/0.10"
        const stakeNums = (row.stakesText || '').match(/-?\d[\d.,]*\s*[KM]?/g) || [];
        if (stakeNums.length >= 2) {
            figures.push(checkFigure(`lobby "${row.name}" small blind (vs TABLE canister config)`, cfg.smallBlind, stakeNums[0], { currency: cur }));
            figures.push(checkFigure(`lobby "${row.name}" big blind (vs TABLE canister config)`, cfg.bigBlind, stakeNums[1], { currency: cur }));
            entry.renderedFaithfullyFromLobbyRecord =
                checkFigure('sb', entry.lobbyConfig.smallBlind, stakeNums[0], { currency: cur }).agrees
                && checkFigure('bb', entry.lobbyConfig.bigBlind, stakeNums[1], { currency: cur }).agrees;
        } else {
            structural.push(`lobby row "${row.name}": could not read two blinds out of "${row.stakesText}"`);
        }

        // "10.00 - 50.00"
        const buyInNums = (row.buyInText || '').match(/-?\d[\d.,]*\s*[KM]?/g) || [];
        if (buyInNums.length >= 2) {
            figures.push(checkFigure(`lobby "${row.name}" min buy-in (vs TABLE canister config)`, cfg.minBuyIn, buyInNums[0], { currency: cur }));
            figures.push(checkFigure(`lobby "${row.name}" max buy-in (vs TABLE canister config)`, cfg.maxBuyIn, buyInNums[1], { currency: cur }));
        } else {
            structural.push(`lobby row "${row.name}": could not read a buy-in range out of "${row.buyInText}"`);
        }

        // THE ROW NAME IS A MONEY FIGURE TOO.
        // Every row is named "<shape> - <sb>/<bb>", written once into the LOBBY
        // canister by `init_microstakes_tables` and never revised. When the
        // table's real blinds move, the name keeps quoting the old ones, and the
        // name is the largest, first thing a player reads on the row — larger
        // than the Stakes cell it contradicts. So it is asserted against the
        // TABLE canister's blinds like any other figure on screen.
        const nameStakes = /(\d[\d.,]*)\s*\/\s*(\d[\d.,]*)/.exec(row.name || '');
        if (nameStakes && entry.tableConfig) {
            figures.push(checkFigure(
                `lobby row NAME "${row.name}" quotes a small blind`,
                entry.tableConfig.smallBlind, nameStakes[1], { currency: cur },
            ));
            figures.push(checkFigure(
                `lobby row NAME "${row.name}" quotes a big blind`,
                entry.tableConfig.bigBlind, nameStakes[2], { currency: cur },
            ));
        }

        // The lobby canister's copy vs the table canister's own config.
        //
        // WHOSE DEFECT IS THIS. Two different failures wear the same symptom and
        // must never be collapsed:
        //   * the CLIENT rendered a number that is not the table's  -> a client
        //     defect, and it is already caught by the figure checks above;
        //   * the two ON-CHAIN sources disagree while the client renders the
        //     table's live config faithfully (and says so) -> a canister-data
        //     defect that the client is handling correctly.
        // The message names which one it is, so nobody fixes the wrong file.
        if (entry.tableConfig) {
            const drifted = ['smallBlind', 'bigBlind', 'minBuyIn', 'maxBuyIn', 'maxPlayers']
                .filter((key) => entry.tableConfig[key] !== entry.lobbyConfig[key]);
            if (drifted.length) {
                entry.configDrift = drifted.map((key) => ({
                    field: key, lobbyRecord: entry.lobbyConfig[key], tableEnforces: entry.tableConfig[key],
                }));
                structural.push(
                    `ON-CHAIN DATA DEFECT for "${row.name}": the LOBBY canister's stored record says `
                    + drifted.map((k) => `${k}=${entry.lobbyConfig[k]}`).join(', ')
                    + ' but the TABLE canister enforces '
                    + drifted.map((k) => `${k}=${entry.tableConfig[k]}`).join(', ')
                    + `. That is ${drifted.map((k) => `${(entry.tableConfig[k] / (entry.lobbyConfig[k] || 1)).toFixed(0)}x`).join('/')} `
                    + 'the advertised figure. Fix is in src/lobby_canister (init_microstakes_tables), '
                    + 'not in the client: the rendered cells were compared against the TABLE canister above.',
                );
            }
        }

        // The redesigned lobby renders a LIVE pot on any row with a hand in
        // progress. That is a money figure in the lobby and it gets the same
        // treatment as the pot on the felt — gated when it held still across the
        // scrape, advisory when it did not.
        if (row.livePotText) {
            const stable = potsBefore.get(row.name) === entry.livePot;
            const check = checkFigure(
                `lobby "${row.name}" live pot vs get_pot()`, entry.livePot, row.livePotText, { currency: cur },
            );
            if (stable) figures.push(check);
            else advisory.push(`${check.label}: pot moved during the scrape (${potsBefore.get(row.name)} -> ${entry.livePot}); ${check.detail}`);
        } else if (entry.livePot > 0 && entry.livePhase && entry.livePhase !== 'WaitingForPlayers' && entry.livePhase !== 'HandComplete') {
            advisory.push(
                `"${row.name}" has a live pot of ${entry.livePot} e8s at ${entry.livePhase} but the row shows no pot`,
            );
        }

        // Player counts move whenever any agent seats somebody on this shared
        // replica, so they are recorded and NOT gated. Saying so is the honest
        // alternative to a check that fails for reasons unrelated to the client.
        const countNums = (row.playersText || '').match(/\d+/g) || [];
        if (countNums.length >= 2 && entry.livePlayerCount !== undefined) {
            if (Number(countNums[0]) !== entry.livePlayerCount) {
                advisory.push(
                    `"${row.name}" shows ${countNums[0]}/${countNums[1]} players, get_player_count() says `
                    + `${entry.livePlayerCount} (not gated: other agents seat players on this replica concurrently)`,
                );
            }
        }
    }

    // ---- THE PREVIEW PANE -------------------------------------------------
    // A second money surface, beside the list and larger than it: a live pot, the
    // seated players' stacks, a facts list quoting blinds/buy-in/ante/last pot,
    // and a sentence repeating that pot. The token census (lib/token-census.mjs)
    // is what proved none of it was asserted — 16 numbers on the lobby screen
    // that no check had ever looked at, including a 155.00 ICP pot figure.
    const pv = dom.preview;
    if (pv && pv.present) {
        const entry = byName.get(pv.heading) ?? byName.get(pv.selectedRowName);
        if (!entry) {
            structural.push(
                `the preview pane is showing "${pv.heading}", which is not a table the lobby `
                + 'canister registered, so nothing on it could be compared with a canister',
            );
        } else {
            const cur = entry.currency;
            const cfg = entry.tableConfig ?? entry.lobbyConfig;
            const label = (what) => `lobby preview ${what}`;

            // The heading is `selectedTable.name` — the same stale lobby string
            // that priced the table header 5x and 10x wrong (T-11), rendered here
            // at 15px beside a facts list quoting the real blinds.
            const headStakes = /(\d[\d.,]*)\s*\/\s*(\d[\d.,]*)/.exec(pv.heading || '');
            if (headStakes && entry.tableConfig) {
                figures.push(checkFigure(
                    `lobby preview heading "${pv.heading}" quotes a small blind`,
                    entry.tableConfig.smallBlind, headStakes[1], { currency: cur },
                ));
                figures.push(checkFigure(
                    `lobby preview heading "${pv.heading}" quotes a big blind`,
                    entry.tableConfig.bigBlind, headStakes[2], { currency: cur },
                ));
            }

            // The mini-felt's live pot, bracketed the same way the row's is.
            if (pv.feltPotText) {
                const stable = potsBefore.get(entry.name) === entry.livePot;
                const check = checkFigure(
                    'lobby preview live pot vs get_pot()', entry.livePot, pv.feltPotText, { currency: cur },
                );
                if (stable) figures.push(check);
                else advisory.push(`${check.label}: pot moved during the scrape; ${check.detail}`);
            }

            // Every seated player's stack, in the order the client renders them
            // (occupied seats, ascending).
            for (let i = 0; i < pv.seated.length; i += 1) {
                const chain = entry.seated[i];
                if (!chain) {
                    structural.push(
                        `the preview lists ${pv.seated.length} seated player(s) but the table `
                        + `canister reports ${entry.seated.length}`,
                    );
                    break;
                }
                figures.push(checkFigure(
                    `lobby preview seat ${chain.seat} stack`, chain.chips, pv.seated[i].stackText,
                    { currency: cur },
                ));
            }

            // The facts list, read by its own <dt> labels rather than by position.
            const fact = (name) => pv.factByLabel?.[name] ?? null;
            const twoNumbers = (text) => (String(text ?? '').match(/-?\d[\d.,]*\s*[KM]?/g) || []);

            const blinds = twoNumbers(fact('Blinds'));
            if (blinds.length >= 2) {
                figures.push(checkFigure(label('fact "Blinds" small blind'), cfg.smallBlind, blinds[0], { currency: cur }));
                figures.push(checkFigure(label('fact "Blinds" big blind'), cfg.bigBlind, blinds[1], { currency: cur }));
            } else if (fact('Blinds') !== null) {
                structural.push(`preview fact "Blinds" has no two numbers: "${fact('Blinds')}"`);
            }

            const buyIn = twoNumbers(fact('Buy-in'));
            if (buyIn.length >= 2) {
                figures.push(checkFigure(label('fact "Buy-in" minimum'), cfg.minBuyIn, buyIn[0], { currency: cur }));
                figures.push(checkFigure(label('fact "Buy-in" maximum'), cfg.maxBuyIn, buyIn[1], { currency: cur }));
            } else if (fact('Buy-in') !== null) {
                structural.push(`preview fact "Buy-in" has no range: "${fact('Buy-in')}"`);
            }

            if (fact('Ante') !== null) {
                figures.push(checkFigure(label('fact "Ante"'), cfg.ante ?? 0, fact('Ante'), {
                    currency: cur, allowAbsentWhenZero: true,
                }));
            }

            // The clock is seconds, not money — but it is a promise about how long
            // a player has to act on money, so it is compared like everything else.
            const clock = twoNumbers(fact('Clock'));
            if (clock.length >= 2 && entry.tableConfig) {
                figures.push(checkPlainNumber(label('fact "Clock" action timeout'), entry.tableConfig.actionTimeoutSecs, clock[0], { unit: 's' }));
                figures.push(checkPlainNumber(label('fact "Clock" time bank'), entry.tableConfig.timeBankSecs, clock[1], { unit: 's' }));
            }

            if (fact('Hands dealt') !== null && /\d/.test(fact('Hands dealt')) && entry.handNumber !== null) {
                figures.push(checkPlainNumber(label('fact "Hands dealt"'), entry.handNumber, fact('Hands dealt')));
            }

            // The last pot, quoted twice: once in the facts list and once in the
            // rake line. Both are asserted, because both are read.
            if (fact('Last pot') !== null && /\d/.test(fact('Last pot'))) {
                figures.push(checkFigure(label('fact "Last pot"'), entry.lastPot, fact('Last pot'), { currency: cur }));
            }
            if (pv.rakeLineText && /\d/.test(pv.rakeLineText.replace(/0%/, ''))) {
                const amount = (pv.rakeLineText.replace(/0%/, '').match(/-?\d[\d.,]*\s*[KM]?/g) || [])[0];
                if (amount !== undefined) {
                    figures.push(checkFigure('lobby preview rake line last pot', entry.lastPot, amount, { currency: cur }));
                }
            }
        }
    } else if (dom.rowCount > 0) {
        advisory.push('no preview pane on screen; its money figures were not asserted this run');
    }

    const folded = foldFigures(figures);
    const ok = folded.ok && structural.length === 0;
    return {
        ok,
        checks: {
            chainAgreement: ok ? 'AGREES' : 'DISAGREES',
            moneyFiguresChecked: folded.checked,
            moneyMismatches: folded.mismatches,
            structuralProblems: structural,
            advisory,
            figures: folded.figures.map((f) => ({ label: f.label, chain: f.chain, screen: f.domText, ok: f.ok, detail: f.detail })),
            onChain: [...byName.values()].map((e) => ({
                name: e.name, lobbyConfig: e.lobbyConfig, tableConfig: e.tableConfig ?? null,
                livePlayerCount: e.livePlayerCount ?? null,
                renderedFaithfullyFromLobbyRecord: e.renderedFaithfullyFromLobbyRecord ?? null,
                handNumber: e.handNumber ?? null,
                livePot: e.livePot ?? null,
                lastPot: e.lastPot ?? null,
                seated: e.seated ?? [],
            })),
            onScreen: { rows: dom.rows, preview: dom.preview ?? null },
        },
        notes: ok
            ? `lobby: ${folded.checked} money figures (rows + preview pane) all equal the canisters'`
            : `LOBBY CHAIN DISAGREEMENT: ${[...folded.mismatches, ...structural].slice(0, 4).join(' | ')}`,
    };
}

/** The USD price the harness actually served this run, or null. */
function servedIcpUsd() {
    for (const o of thirdPartyObservations().slice().reverse()) {
        if (!o.body) continue;
        try {
            const parsed = JSON.parse(o.body);
            const usd = parsed?.['internet-computer']?.usd;
            if (typeof usd === 'number') return { usd, mode: o.mode, observedAt: o.observedAt };
        } catch { /* a truncated body is not a quote */ }
    }
    return null;
}

/**
 * The deposit modal's money: the ledger balance it quotes, the escrow balance
 * behind it, and the fiat conversion.
 *
 * @param {object} ctx
 * @param {import('playwright').Page} page
 * @param {{table:string, asPlayer:number}} opts
 */
export async function assertDepositAgreement(ctx, page, opts) {
    const tableId = ctx.tableIds[opts.table];
    const heroPrincipal = devPlayerPrincipal(opts.asPlayer);
    const [truth, walletE8s] = await Promise.all([
        readTableTruth(tableId, opts.asPlayer),
        ledgerBalance(heroPrincipal),
    ]);
    const wallet = Number(walletE8s);

    // THE MODAL FILLS ITSELF IN ASYNCHRONOUSLY, SO ONE SCRAPE IS A COIN FLIP.
    // `loadPrices()` is an un-awaited fetch fired from onMount, and the fiat span
    // only renders once `walletBalance` is non-zero AND `priceLoading` is false.
    // A single early scrape therefore saw no `.usd-value`, reported "a live quote
    // was served but the modal shows no fiat figure", and left the figure that
    // WAS on screen a moment later compared with nothing — which is exactly the
    // hole this pass exists to close, so it is closed here too. The scrape is
    // retried until the modal has settled (balance present, and a fiat figure
    // present whenever a quote was served), and the last snapshot is the one
    // asserted. The token census then re-reads the page independently, so a
    // figure that appears after even this loop still cannot slip through: it
    // would be counted as UNASSERTED.
    const priceServed = servedIcpUsd();
    let dom = await scrapeDeposit(page);
    for (let attempt = 0; attempt < 4; attempt += 1) {
        const settled = dom.found.modal
            && dom.cryptoBalances.length > 0
            && (!priceServed || priceServed.mode === 'unavailable' || dom.usdValues.length > 0);
        if (settled) break;
        await sleep(ATTEMPT_GAP_MS);
        dom = await scrapeDeposit(page);
    }

    const figures = [];
    const structural = [];

    if (!dom.found.modal) {
        return {
            ok: false,
            checks: { chainAgreement: 'NOT CHECKED', reason: 'no deposit modal on screen' },
            notes: 'deposit money figures NOT CHECKED: no modal on screen',
        };
    }

    if (dom.cryptoBalances.length === 0) {
        structural.push('SCRAPE FAILED: the deposit modal shows no .balance-crypto figure');
    }
    for (const text of dom.cryptoBalances) {
        figures.push(checkFigure(
            'deposit modal wallet balance vs ledger icrc1_balance_of',
            wallet, text, { currency: truth.currency },
        ));
    }

    // THE FEE AND THE MINIMUMS ARE MONEY FIGURES TOO, AND THERE ARE FOUR OF THEM.
    //
    // A player uses these to decide how much to send, and one of them (the address
    // minimum) is the number that FINDING 31 was about: send less and the sweep fee
    // eats the deposit and `withdraw` refuses what is left, a silent 100% loss at
    // the number the product prints. So each has to be compared with the quantity
    // it actually claims to be.
    //
    // READ BY LABEL, NOT BY POSITION. docs/DEFECTS.md E-86. This check used to take
    // `.minimum-notice`'s numbers in DOM order and assign them (minimum, fee,
    // minimum + 2 fee). At 134550e the copy grew a FOURTH figure and reordered the
    // rest, so nums[0] became the ADDRESS minimum (30 000 e8s) and was compared
    // against the canister's `min_deposit` (20 000), and nums[1] became the wallet
    // -route minimum (20 000) and was compared against `icrc1_fee()` (10 000). Both
    // deposit shots went red as "DEPOSIT CHAIN DISAGREEMENT ... screen is 1.500x the
    // chain", the screen was right and every quoted figure was correct: the
    // comparison had simply been re-pointed at the wrong quantity by a copy edit.
    // Correct totals, wrong recipients, and the fourth number asserted by nothing at
    // all. Anchoring on the labels makes a copy edit that moves a figure a MISSING
    // LABEL (a structural failure) instead of a silent re-pointing.
    if (dom.minimumNotice && /\d/.test(dom.minimumNotice)) {
        const fee = await ledgerTransferFee();
        const minimum = truth.currency === 'BTC' ? BTC_MIN_DEPOSIT : ICP_MIN_DEPOSIT;
        // The sweep route's floor: what arrives has one ledger fee taken out of it
        // before it is credited, so the address minimum is the withdrawal floor plus
        // one fee (src/table_canister/src/lib.rs ICP_MIN_EXTERNAL_DEPOSIT).
        const externalMinimum = minimum + Number(fee);
        const N = '(-?\\d[\\d.,]*)';
        const claims = [
            {
                label: 'deposit modal "Minimum deposit" (to the address) vs the canister\'s external-deposit floor',
                re: new RegExp(`Minimum to this address:\\s*${N}`, 'i'),
                expected: externalMinimum,
            },
            {
                label: 'deposit modal "Minimum deposit" (from a connected wallet) vs the minimum the table canister enforces',
                re: new RegExp(`lower minimum of\\s*${N}`, 'i'),
                expected: minimum,
            },
            {
                label: 'deposit modal "Network fee" vs the ledger\'s own icrc1_fee()',
                re: new RegExp(`network fee\\s*${N}`, 'i'),
                expected: Number(fee),
            },
            {
                label: 'deposit modal "you need N in your wallet" vs minimum + 2 x icrc1_fee()',
                re: new RegExp(`you need\\s*${N}`, 'i'),
                expected: minimum + 2 * Number(fee),
            },
        ];
        for (const claim of claims) {
            const m = claim.re.exec(dom.minimumNotice);
            if (!m) {
                // NOT a silent skip. A label that stopped matching means the copy
                // moved and this figure is now compared with nothing, which is the
                // exact state that let the fourth number ship unasserted.
                structural.push(
                    `the deposit modal's minimum/fee notice no longer carries the label for `
                    + `${claim.label}; the figure it names is now checked by nothing. `
                    + `Notice text: "${dom.minimumNotice}"`,
                );
                continue;
            }
            figures.push(checkFigure(claim.label, claim.expected, m[1], { currency: truth.currency }));
        }
    }

    // Re-read after the settle loop: a quote can land between the two.
    const quote = servedIcpUsd();
    if (dom.usdValues.length === 0) {
        if (quote && quote.mode !== 'unavailable') {
            structural.push(
                `a live quote (${quote.usd} USD/ICP) was served but the modal shows no fiat figure`,
            );
        }
    } else if (!quote) {
        structural.push(`the modal shows a fiat figure (${dom.usdValues[0]}) but no price was served this run`);
    } else {
        // formatUsd(): `~$X.XX`, or `~$X.XXXX` below a cent.
        const expected = (wallet / 100_000_000) * quote.usd;
        for (const text of dom.usdValues) {
            figures.push(checkPlainNumber(
                `deposit modal fiat value vs (balance x ${quote.usd} USD/ICP, ${quote.mode})`,
                expected, text, { unit: ' USD' },
            ));
        }
    }

    const folded = foldFigures(figures);
    const ok = folded.ok && structural.length === 0;
    return {
        ok,
        checks: {
            chainAgreement: ok ? 'AGREES' : 'DISAGREES',
            moneyFiguresChecked: folded.checked,
            moneyMismatches: folded.mismatches,
            structuralProblems: structural,
            figures: folded.figures.map((f) => ({ label: f.label, chain: f.chain, screen: f.domText, ok: f.ok, detail: f.detail })),
            onChain: {
                heroPrincipal,
                ledgerBalanceE8s: wallet,
                tableEscrowBalanceE8s: truth.balance,
                icpUsdQuote: quote,
                // HOW OLD THE PRICE IN THE PNG WAS. `loadPrices()` fires once from
                // the modal's onMount and is never refreshed, and the client prints
                // no "as of" anywhere, so the fiat figure is presented as current
                // however long the modal has been open. The age is recorded here so
                // the artifact states it even though the screen does not. See the
                // DepositModal finding in docs/RESPONSIVENESS.md §8.
                fiatQuoteAgeMsAtAssertion: quote
                    ? Date.now() - Date.parse(quote.observedAt)
                    : null,
                fiatIsRefreshedWhileOpen: false,
                fiatShowsItsOwnAge: false,
            },
            onScreen: {
                cryptoBalances: dom.cryptoBalances,
                usdValues: dom.usdValues,
                minimumNotice: dom.minimumNotice ?? null,
                // Always null today: DepositModal.svelte sets `priceError` and never
                // renders it, so a failed quote shows the player an empty "()" rather
                // than saying the price could not be read.
                priceError: dom.priceError,
            },
        },
        notes: ok
            ? `deposit modal: ${folded.checked} money figures equal the ledger + the served quote`
            : `DEPOSIT CHAIN DISAGREEMENT: ${[...folded.mismatches, ...structural].slice(0, 4).join(' | ')}`,
    };
}

/**
 * The hand-history modal's money: each row's pot against the canister's own
 * recorded hand.
 *
 * @param {object} ctx
 * @param {import('playwright').Page} page
 * @param {{table:string, asPlayer:number}} opts
 */
/**
 * One number off one Candid record, refusing to become `NaN`.
 *
 * @param {any} record
 * @param {string} field
 * @param {string} where human-readable name of the record, for the message
 * @param {string[]} problems appended to when the field cannot be read
 * @returns {number|null}
 */
function strictNumber(record, field, where, problems) {
    if (!record || typeof record !== 'object' || !(field in record)) {
        const fields = record && typeof record === 'object' ? Object.keys(record).join(', ') : 'none';
        problems.push(
            `STRUCTURAL: ${where} has no '${field}' field, so the assertion that reads it cannot `
            + `run. Fields present: ${fields}`,
        );
        return null;
    }
    const n = Number(record[field]);
    if (!Number.isFinite(n)) {
        problems.push(`STRUCTURAL: ${where}.${field} is not a finite number: ${String(record[field])}`);
        return null;
    }
    return n;
}

/** The winners' total off a record that has a `winners` vec. */
function awardedTotal(record, where, problems) {
    if (!record || !Array.isArray(record.winners)) {
        problems.push(`STRUCTURAL: ${where} has no 'winners' vec`);
        return null;
    }
    let sum = 0;
    for (const w of record.winners) {
        const a = strictNumber(w, 'amount', `${where}.winners[]`, problems);
        if (a === null) return null;
        sum += a;
    }
    return sum;
}

/**
 * ONE ARCHIVED HAND, READ FROM BOTH OF THE HISTORY CANISTER'S SHAPES.
 *
 * THE DEFECT THIS FUNCTION EXISTS TO END (docs/DEFECTS.md E-61). The caller used
 * to build this record from `get_hands_by_table` alone and read `Number(r.rake)`
 * off it. `get_hands_by_table` returns `vec HandSummary`, and `HandSummary` has
 * no `rake` field — only `HandHistoryRecord`, from `get_hand(hand_id)`, does. So
 * `rake` was `undefined`, `Number(undefined)` was `NaN`, `NaN !== 0` was true and
 * `totalPot !== awarded + NaN` was true. **The one gate that asserts ClearDeck's
 * no-rake property against the permanent archive reported a rake being taken on
 * every hand ever archived**, which is the most reliable way there is to make an
 * alarm ignored. It was red for a reason that had nothing to do with rake, in a
 * function whose own comment thirty lines above says *"absence of a field is now
 * a structural failure, never a silent NaN"* — the author fixed that in one field
 * and left it in the adjacent one.
 *
 * So: every field is read through {@link strictNumber}, a missing one is a
 * structural failure naming the field, and `rake` is read from the record that
 * actually carries it. The two shapes are also cross-checked against each other,
 * because the archive having two read paths that disagree is itself a finding.
 *
 * Pure and exported so `tools/shots/test-rake.mjs` can prove it goes red when a
 * rake appears — the sweep it normally runs in needs a replica, and a gate whose
 * failure mode nobody has ever seen is not a gate.
 *
 * @param {any} summary a `HandSummary` from `get_hands_by_table`
 * @param {any} full a `HandHistoryRecord` from `get_hand(hand_id)`, or null
 * @returns {{handId:number|null, totalPot:number|null, rake:number|null,
 *            awarded:number|null, structural:string[]}}
 */
export function foldArchivedHand(summary, full) {
    const structural = [];
    const handId = strictNumber(summary, 'hand_id', 'HandSummary', structural);
    // THE NAME OF THE HAND THIS RECORD IS OF (docs/DEFECTS.md E-71). Read off the
    // summary, cross-checked against the full record below, and carried through
    // so the caller's join can be ASSERTED rather than assumed.
    const handUid = typeof summary?.hand_uid === 'string' ? summary.hand_uid : null;
    if (handUid === null) {
        structural.push(
            'STRUCTURAL: HandSummary has no `hand_uid`, so the archived record cannot be matched '
            + 'to the hand it is a record OF. Either this archive predates docs/DEFECTS.md E-71 '
            + 'or the harness is decoding through a declaration that drops the field.',
        );
    } else if (handUid === '') {
        structural.push(
            'STRUCTURAL: this archived record has an EMPTY name, which means its shuffle '
            + 'commitment is not a SHA-256 digest. `record_hand` refuses such a record, so one '
            + 'that is stored got in some other way.',
        );
    }

    if (!full) {
        structural.push(
            `STRUCTURAL: get_hand(${handId ?? '?'}) returned nothing, so the archived record for a `
            + 'hand the archive itself lists cannot be read. The no-rake assertion needs '
            + '`HandHistoryRecord.rake`, which is the only place a rake is recorded.',
        );
        return { handId, handUid, handNumber: null, totalPot: null, rake: null, awarded: null, structural };
    }

    // The name must be the same on both read paths, and it must be the name the
    // record's own contents produce. `hand_uid` is derived by the canister from
    // `table_id` and `shuffle_proof.seed_hash`; recomputing it here means a
    // canister that ever starts publishing a name that does not follow from the
    // hand is caught by the gate rather than trusted by it.
    const fullUid = Array.isArray(full.hand_uid)
        ? (full.hand_uid.length ? full.hand_uid[0] : null)
        : (full.hand_uid ?? null);
    if (handUid && fullUid !== null && fullUid !== handUid) {
        structural.push(
            `the archive disagrees with itself about this hand's NAME: get_hands_by_table says `
            + `${handUid} and get_hand says ${fullUid}`,
        );
    }
    const seedHash = full?.shuffle_proof?.seed_hash;
    if (handUid && typeof seedHash === 'string' && full.table_id?.toText) {
        const derived = `${full.table_id.toText()}:${seedHash.trim().toLowerCase()}`;
        if (derived !== handUid) {
            structural.push(
                `the archive's published name for this hand does not follow from the hand: it `
                + `publishes ${handUid} and its own table_id and commitment give ${derived}`,
            );
        }
    }

    const totalPot = strictNumber(full, 'total_pot', 'HandHistoryRecord', structural);
    const rake = strictNumber(full, 'rake', 'HandHistoryRecord', structural);
    const awarded = awardedTotal(full, 'HandHistoryRecord', structural);
    // Carried for the MESSAGES, never for the join: "the archive calls this hand
    // number 3" is useful context and is not an identity.
    const handNumber = 'hand_number' in full ? Number(full.hand_number) : null;

    // The archive's two read paths must agree. A summary that says one pot and a
    // full record that says another means the modal's number depends on which
    // call the client happened to make.
    const summaryPot = strictNumber(summary, 'total_pot', 'HandSummary', structural);
    const summaryAwarded = awardedTotal(summary, 'HandSummary', structural);
    if (summaryPot !== null && totalPot !== null && summaryPot !== totalPot) {
        structural.push(
            `the archive disagrees with itself: get_hands_by_table says total_pot=${summaryPot} `
            + `and get_hand says total_pot=${totalPot}`,
        );
    }
    if (summaryAwarded !== null && awarded !== null && summaryAwarded !== awarded) {
        structural.push(
            `the archive disagrees with itself: get_hands_by_table's winners sum to `
            + `${summaryAwarded} and get_hand's sum to ${awarded}`,
        );
    }

    return { handId, handUid, handNumber, totalPot, rake, awarded, structural };
}

// ---------------------------------------------------------------------------
// JOINING THE TABLE'S OWN RECORD TO THE PERMANENT ARCHIVE  (docs/DEFECTS.md E-71)
// ---------------------------------------------------------------------------
//
// WHAT THIS GATE USED TO DO, AND WHY IT WAS RED FOR THE WRONG REASON.
// `assertHandHistoryAgreement` built `byHandNumber` with
// `byHandNumber.set(Number(summary.hand_number), …)` over a NEWEST-FIRST window of
// twenty archived records, then compared the table's hand *n* against whatever
// survived under key *n*. `hand_number` restarts at 1 on every `reset_table`, so
// that map is a collision: measured on the local archive on 2026-08-08, the
// twenty-record window for `table_2` collapsed to TEN keys, and the record left
// under "hand 1" was `hand_id 3173` — a hand from an earlier life of the table,
// with a pot of 0.2 ICP — while the hand the table had just played was
// `hand_id 3218`, with a pot of 24 ICP. The gate then reported
//
//     hand 1: the TABLE canister paid 2400000000 e8s but the HISTORY canister
//             recorded 20000000 e8s paid
//
// which reads as "the archive lost 22 ICP" and is really "the gate compared two
// different hands". Both numbers were true; they were true of different hands.
//
// TWO SEPARATE DEFECTS, BOTH FIXED HERE.
//
//   1. THE JOIN. Hands are matched on the hand's NAME — `table_id:seed_hash`, the
//      shuffle commitment, which is unique per hand and identical on both sides
//      because both sides got it from the same deal. The join is then ASSERTED:
//      whatever record comes back must carry the name of the hand it is being
//      compared against, so a future edit that reverts the lookup to
//      `hand_number` fails on the join instead of producing a plausible-looking
//      money mismatch. That assertion is what `tools/shots/test-hand-identity.mjs`
//      exercises with no replica at all.
//
//   2. THE RAKE HOLE. The no-rake loop iterated the COLLAPSED map, so on that same
//      window it checked ten of the twenty archived records for a rake and the
//      other ten were never looked at — and which ten was decided by insertion
//      order. The product's headline property was asserted over an arbitrary half
//      of the evidence. It now iterates the records themselves.

/**
 * The hand's permanent name, from either side of the comparison.
 *
 * @param {string} tableId
 * @param {string|null|undefined} seedHash
 * @returns {string|null} `table_id:seed_hash`, or null if that is not a commitment
 */
export function handUidFrom(tableId, seedHash) {
    if (typeof tableId !== 'string' || !tableId) return null;
    if (typeof seedHash !== 'string') return null;
    const c = seedHash.trim().toLowerCase();
    if (!/^[0-9a-f]{64}$/.test(c)) return null;
    return `${tableId}:${c}`;
}

/**
 * Match the table's own hand records to the archive's, BY NAME, and check the
 * match.
 *
 * Pure, and exported, for the reason `foldArchivedHand` is: the gate it belongs to
 * only runs inside a full screenshot sweep against a live replica with archived
 * hands on it, and a gate whose failure mode nobody has ever seen is not a gate.
 *
 * @param {object} opts
 * @param {string} opts.tableId the table canister id, as text
 * @param {Array<{handNumber:number, seedHash:string|null, awardedByTable:number}>} opts.tableHands
 * @param {Array<{handId:number|null, handUid:string|null, totalPot:number|null,
 *                rake:number|null, awarded:number|null, structural:string[]}>} opts.archived
 * @param {(hand:object, index:object) => object|null} [opts.lookup] how a record is
 *        found for a table hand. Defaults to lookup by NAME. Injectable so the
 *        self-test can hand in the defective by-`hand_number` lookup and require
 *        the join assertion to catch it.
 * @returns {{pairs: Array<{hand:object, hist:object|null, uid:string|null}>,
 *            structural: string[]}}
 */
export function joinTableHandsToArchive({ tableId, tableHands, archived, lookup }) {
    const structural = [];

    const byUid = new Map();
    const byHandNumber = new Map();
    for (const rec of archived) {
        if (rec.handUid) {
            if (byUid.has(rec.handUid)) {
                structural.push(
                    `THE ARCHIVE HAS TWO RECORDS NAMED ${rec.handUid}. A name identifies exactly `
                    + 'one hand or it is not a name; see the archive\'s own get_archive_integrity.',
                );
            }
            byUid.set(rec.handUid, rec);
        }
        // Kept ONLY so the self-test can inject the defective lookup, and built
        // exactly the way the defect built it -- an unconditional `set` over a
        // newest-first list, so the OLDEST colliding record wins. Nothing in the
        // default path reads it.
        byHandNumber.set(rec.handNumber, rec);
    }
    const index = { byUid, byHandNumber };
    const find = lookup || ((hand) => (hand.uid ? byUid.get(hand.uid) || null : null));

    const pairs = [];
    for (const hand of tableHands) {
        const uid = handUidFrom(tableId, hand.seedHash);
        if (!uid) {
            structural.push(
                `STRUCTURAL: the table's own record of hand ${hand.handNumber} carries no usable `
                + `shuffle commitment (${JSON.stringify(hand.seedHash)}), so this hand cannot be `
                + 'matched to its archived record by anything except its hand number, and hand '
                + 'numbers restart at 1 on every reset_table.',
            );
            pairs.push({ hand, hist: null, uid: null });
            continue;
        }

        const hist = find({ ...hand, uid }, index) || null;

        // THE JOIN, ASSERTED. This is the line that makes the gate go red when
        // the join is reverted rather than merely produce a wrong-looking number.
        if (hist && hist.handUid !== uid) {
            structural.push(
                `JOIN BROKEN: hand ${hand.handNumber} of this table committed to ${uid}, but the `
                + `archived record being compared against it is ${hist.handUid || '(unnamed)'} `
                + `(hand_id ${hist.handId}, pot ${hist.totalPot}). Those are two different hands. `
                + 'A hand is identified by its commitment, never by its hand number: hand numbers '
                + 'restart at 1 on every reset_table and one of them answers to 70 records on the '
                + 'local archive. docs/DEFECTS.md E-71.',
            );
        }

        pairs.push({ hand, hist, uid });
    }

    return { pairs, structural };
}

export async function assertHandHistoryAgreement(ctx, page, opts) {
    const tableId = ctx.tableIds[opts.table];
    // THE HAND RECORD IS READ THROUGH THE HARNESS'S OWN MIRROR, not through
    // `src/declarations`. `lib/hand-record-wire.mjs` explains why at length; the
    // short version is that the app's declaration of `HandHistory` is stale
    // (docs/DEFECTS.md E-08) and Candid subtyping drops what it does not declare.
    // This gate now depends on `shuffle_proof.seed_hash` off the table's own
    // record — it is the hand's identity, and a decoder that dropped it would
    // send the join silently back to `hand_number`.
    const table = await handRecordActor(opts.asPlayer, tableId);
    const truth = await readTableTruth(tableId, opts.asPlayer);
    const structuralPre = [];

    // TWO SOURCES, DELIBERATELY. `table.get_hand_history` (Candid type
    // `HandHistory`) carries NO pot field at all — only `winners` — so "the pot
    // of hand n" from the table canister is the sum of what it PAID. The HISTORY
    // canister's `HandHistoryRecord` does carry `total_pot` (and `rake`), and it
    // is the record the client actually renders. Both are read, and they are
    // cross-checked against each other, because a modal that agrees with one
    // while the two disagree is still showing the player a number the money did
    // not follow.
    //
    // An earlier version of this function read `rec.total_pot` off the TABLE
    // record. That field does not exist, so `Number(undefined)` was NaN and every
    // comparison was vacuous — reported as a disagreement only by luck. Absence
    // of a field is now a structural failure, never a silent NaN.
    const hands = [];
    for (let n = truth.handNumber; n >= 1 && hands.length < 10; n -= 1) {
        const rec = optional(await table.get_hand_history(BigInt(n)));
        if (!rec) continue;
        if (!('winners' in rec)) {
            structuralPre.push(`get_hand_history(${n}) has no 'winners' field: ${Object.keys(rec).join(', ')}`);
            continue;
        }
        const winners = rec.winners.map((w) => ({ seat: Number(w.seat), amount: Number(w.amount) }));
        hands.push({
            handNumber: Number(rec.hand_number),
            // THE HAND'S IDENTITY, from the table's own copy of the proof. This
            // is what the archive is joined on. docs/DEFECTS.md E-71.
            seedHash: rec.shuffle_proof?.seed_hash ?? null,
            awardedByTable: winners.reduce((n2, w) => n2 + w.amount, 0),
            winners,
        });
    }

    // The history canister's own copy of the same hands.
    //
    // TWO CALLS, NOT ONE. `get_hands_by_table` returns `vec HandSummary`, which
    // has `total_pot` and `winners` and NO `rake`. `rake` lives only on
    // `HandHistoryRecord`, which `get_hand` returns. See `foldArchivedHand` for
    // what reading the wrong one cost.
    //
    // AND EVERY WAY OF NOT READING IT IS A FAILURE, NOT A SKIP. Both branches
    // below used to leave the archive side empty, which made `hist` undefined for
    // every row, which made every assertion in the loop below -- including the
    // no-rake one -- silently not run. A scene could therefore go GREEN with the
    // product's headline property asserted by nothing, and the only trace was an
    // `error` key in the manifest that no code ever read. That is the same class
    // of hole as E-61 itself, one level up: not a wrong answer, an absent one.
    //
    // A LIST, NOT A MAP KEYED ON `hand_number`. That map was docs/DEFECTS.md
    // E-71: on the local archive the twenty-record window for `table_2` collapsed
    // to ten entries, so ten archived records went unchecked for a rake and the
    // ten that survived were compared against the wrong hands.
    const archived = [];
    let integrity = null;
    if (!ctx.ids?.history) {
        structuralPre.push(
            'STRUCTURAL: no history canister id in this run, so the archive was never read and '
            + 'the no-rake property is asserted by nothing on this scene.',
        );
    } else {
        try {
            const history = await archiveActor(ctx.ids.history);

            // THE ARCHIVE'S IDENTITY, ASSERTED AGAINST THE ARCHIVE ITSELF.
            // `get_archive_integrity` recounts, from the records rather than from
            // the index, whether a name identifies exactly one hand. If it does
            // not, every join below is meaningless and the scene must say so
            // rather than compare things.
            integrity = await history.get_archive_integrity();
            if (Number(integrity.name_collisions) !== 0) {
                structuralPre.push(
                    `THE ARCHIVE'S NAMES ARE NOT UNIQUE: ${integrity.name_collisions} name(s) are `
                    + `carried by ${integrity.records_under_a_colliding_name} record(s). A hand `
                    + 'that cannot be uniquely named cannot be cited. docs/DEFECTS.md E-71.',
                );
            }
            if (Number(integrity.records_without_a_usable_commitment) !== 0) {
                structuralPre.push(
                    `${integrity.records_without_a_usable_commitment} archived record(s) have no `
                    + 'usable shuffle commitment, so they have no name and cannot be verified by '
                    + 'anyone. record_hand refuses such a record; one that is stored got in some '
                    + 'other way.',
                );
            }
            if (integrity.index_disagrees_with_records) {
                structuralPre.push(
                    'THE ARCHIVE\'S NAME INDEX DISAGREES WITH ITS OWN RECORDS: it holds a '
                    + 'different number of names than the records themselves produce, so a lookup '
                    + 'by name can miss a record that is present.',
                );
            }
            // NO RAKE, OVER EVERY RECORD THE ARCHIVE HOLDS, not over the window
            // this gate can afford to fetch. The loop further down still checks
            // each fetched record -- it names the offending hand and does not
            // trust the canister's own arithmetic -- but a rake on a record older
            // than the newest twenty was checked by NOTHING before this line, so
            // a rake only had to wait twenty hands to become invisible.
            if (Number(integrity.records_with_a_nonzero_rake) !== 0
                || Number(integrity.rake_recorded_total) !== 0) {
                structuralPre.push(
                    `RAKE TAKEN, ARCHIVE-WIDE: ${integrity.records_with_a_nonzero_rake} of `
                    + `${integrity.records_held} archived record(s) record a rake, totalling `
                    + `${integrity.rake_recorded_total} e8s`
                    + `${optional(integrity.first_record_with_a_rake)
                        ? ` (first: ${optional(integrity.first_record_with_a_rake)})` : ''}`
                    + '. ClearDeck publishes a no-rake property; a non-zero rake contradicts it.',
                );
            }

            const recs = await history.get_hands_by_table(
                Principal.fromText(tableId), BigInt(0), BigInt(20),
            );
            for (const summary of recs) {
                const full = optional(await history.get_hand(BigInt(summary.hand_id)));
                archived.push(foldArchivedHand(summary, full));
            }

            // A hand the table played is not necessarily inside the newest-twenty
            // window, so anything the window did not cover is fetched BY NAME
            // before it is called missing. This is also the only thing in the
            // sweep that exercises `get_hand_by_uid`, which is the call a verifier
            // makes.
            const known = new Set(archived.map((a) => a.handUid).filter(Boolean));
            for (const hand of hands) {
                const uid = handUidFrom(tableId, hand.seedHash);
                if (!uid || known.has(uid)) continue;
                const answer = await history.get_hand_by_uid(uid);
                if ('Ok' in answer) {
                    const full = answer.Ok;
                    archived.push(foldArchivedHand(
                        {
                            hand_id: full.hand_id,
                            hand_uid: optional(full.hand_uid) ?? '',
                            hand_number: full.hand_number,
                            total_pot: full.total_pot,
                            winners: full.winners,
                        },
                        full,
                    ));
                    known.add(uid);
                }
            }
        } catch (e) {
            structuralPre.push(
                'STRUCTURAL: the history canister could not be read, so no archived hand was '
                + `checked for a rake: ${String(e.message || e).split('\n')[0]}`,
            );
        }
    }

    // THE JOIN. By the hand's name, and checked. See `joinTableHandsToArchive`.
    // `joined.pairs[i]` is `hands[i]`, positionally: nothing downstream is keyed
    // on a hand number, which is the whole point.
    const joined = joinTableHandsToArchive({ tableId, tableHands: hands, archived });
    structuralPre.push(...joined.structural);

    const dom = await scrapeHandHistory(page);
    const figures = [];
    const structural = [...structuralPre];

    if (!dom.found.modal) structural.push('SCRAPE FAILED: no .hand-history-modal on screen');
    if (dom.found.rows === 0) structural.push('SCRAPE FAILED: the history modal lists no hands');
    if (dom.found.rows > hands.length) {
        structural.push(`history lists ${dom.found.rows} hand(s) but the canister has recorded ${hands.length}`);
    }

    // NO RAKE IS A PROPERTY OF THE ARCHIVE, NOT OF THE MODAL, so it is asserted
    // over EVERY archived record the gate fetched rather than inside the row loop
    // below.
    //
    // The row loop walks `min(dom.rows.length, hands.length)` — at most the ten
    // most recent hands, and only those the modal is currently showing. Asserting
    // no-rake there means an archived hand outside that window is unchecked
    // forever, which is a rake that only has to wait ten hands to become
    // invisible. `rake` comes from `HandHistoryRecord` (`get_hand`), the only
    // shape that carries it; reading it off `HandSummary` is docs/DEFECTS.md
    // E-61.
    //
    // IT ITERATES THE RECORDS, NOT A MAP KEYED ON `hand_number`. It used to
    // iterate the map, and on the local archive that map held ten entries for
    // twenty fetched records — so half the evidence for the product's headline
    // property was never looked at, and which half was decided by insertion
    // order. docs/DEFECTS.md E-71.
    for (const hist of archived) {
        if (hist.rake !== null && hist.rake !== 0) {
            structural.push(
                `RAKE TAKEN: hand ${hist.handUid || `id ${hist.handId}`} recorded `
                + `rake=${hist.rake} e8s. ClearDeck publishes a no-rake property; a non-zero rake `
                + 'contradicts it.',
            );
        }
    }

    // Rows are newest-first in HandHistory.svelte, which is the order `hands` is
    // built in above.
    for (let i = 0; i < Math.min(dom.rows.length, hands.length); i += 1) {
        const hand = hands[i];
        const hist = joined.pairs[i]?.hist ?? null;
        const uid = joined.pairs[i]?.uid ?? null;

        // A hand the TABLE recorded and the ARCHIVE does not have is the archive
        // being incomplete, which is the thing "provably fair" rests on. It is
        // also the state in which every assertion below quietly does not run.
        //
        // NAMED BY ITS NAME. "hand 1 is missing" was never a checkable claim on a
        // table that has been reset: it named a set. This names the deal.
        if (!hist) {
            structural.push(
                `STRUCTURAL: the hand this table calls number ${hand.handNumber}, which committed `
                + `to ${uid || '(no usable commitment)'}, is in the table canister's own record `
                + 'but NOT in the history canister\'s archive -- not in the newest twenty and not '
                + 'under get_hand_by_uid. Nothing about it, rake included, was checked.',
            );
        }

        // A field the archive would not give up is reported as itself, once, and
        // the assertions that needed it are then SKIPPED rather than run against
        // `undefined`. Skipping is safe here only because the skip is itself a
        // structural failure: the scene cannot go green while one is present.
        if (hist?.structural?.length) {
            for (const problem of hist.structural) {
                structural.push(`hand ${uid || hand.handNumber}: ${problem}`);
            }
        }

        // ONE DEAL, ONE NUMBER. The join no longer uses `hand_number`, which is
        // exactly why the two sides' hand numbers can now be COMPARED instead of
        // assumed equal. Both were written from the same deal, so a deal the
        // table calls hand 1 and the archive calls hand 7 is the two surfaces
        // disagreeing about the record they each hold -- invisible to every gate
        // while the number was the join key, because the join made it true by
        // construction.
        if (hist && hist.handNumber !== null && hist.handNumber !== hand.handNumber) {
            structural.push(
                `hand ${uid}: the TABLE calls this deal hand number ${hand.handNumber} and the `
                + `ARCHIVE calls it hand number ${hist.handNumber} (hand_id ${hist.handId}). One `
                + 'deal, two numbers: the two surfaces disagree about the record they hold.',
            );
        }

        // Money in equals money out, hand by hand. Named by the hand's NAME: a
        // money disagreement reported against a number that answers to seventy
        // records is a report nobody can act on.
        if (hist && hist.totalPot !== null && hist.awarded !== null && hist.rake !== null
            && hist.totalPot !== hist.awarded + hist.rake) {
            structural.push(
                `hand ${uid}: history says total_pot=${hist.totalPot} but the winners were `
                + `paid ${hist.awarded} with rake ${hist.rake} (difference `
                + `${hist.totalPot - hist.awarded - hist.rake} e8s)`,
            );
        }
        if (hist && hist.awarded !== null && hist.awarded !== hand.awardedByTable) {
            structural.push(
                `hand ${uid}: the TABLE canister paid ${hand.awardedByTable} e8s but the `
                + `HISTORY canister recorded ${hist.awarded} e8s paid (archived as hand_id `
                + `${hist.handId}, which that table calls hand number ${hist.handNumber})`,
            );
        }

        // What the row must equal: the history record's own pot when there is
        // one (that is the field the client renders), otherwise the amount the
        // table actually paid out.
        const expected = hist && hist.totalPot !== null ? hist.totalPot : hand.awardedByTable;
        const named = uid ? `${uid.slice(0, 12)}…${uid.slice(-6)}` : `hand ${hand.handNumber}`;
        figures.push(checkFigure(
            `history row ${i + 1} pot vs ${hist && hist.totalPot !== null ? `history total_pot (${named})` : `sum of winners paid (${named})`}`,
            expected, dom.rows[i].potText, { currency: truth.currency },
        ));
    }

    const folded = foldFigures(figures);
    const ok = folded.ok && structural.length === 0;
    return {
        ok,
        checks: {
            chainAgreement: ok ? 'AGREES' : 'DISAGREES',
            moneyFiguresChecked: folded.checked,
            moneyMismatches: folded.mismatches,
            structuralProblems: structural,
            figures: folded.figures.map((f) => ({ label: f.label, chain: f.chain, screen: f.domText, ok: f.ok, detail: f.detail })),
            onChain: {
                fromTableCanister: hands,
                // Keyed by the hand's NAME. It used to be keyed by hand number,
                // which silently discarded every colliding record before anything
                // downstream could see it (docs/DEFECTS.md E-71).
                fromHistoryCanister: Object.fromEntries(
                    archived.map((a) => [a.handUid || `unnamed:hand_id ${a.handId}`, a]),
                ),
                joinedBy: 'hand_uid (table_id:seed_hash)',
                archivedRecordsCheckedForRake: archived.length,
                archiveIntegrity: integrity
                    ? {
                        recordsHeld: Number(integrity.records_held),
                        distinctNames: Number(integrity.distinct_names),
                        nameCollisions: Number(integrity.name_collisions),
                        recordsWithoutAUsableCommitment:
                            Number(integrity.records_without_a_usable_commitment),
                        indexDisagreesWithRecords: integrity.index_disagrees_with_records,
                        ambiguousHandNumberCitations:
                            Number(integrity.ambiguous_hand_number_citations),
                        recordsUnderAnAmbiguousHandNumber:
                            Number(integrity.records_under_an_ambiguous_hand_number),
                        worstHandNumberCitation: optional(integrity.worst_hand_number_citation),
                    }
                    : null,
                noRakeAsserted: true,
            },
            onScreen: dom.rows,
        },
        notes: ok
            ? `hand history: ${folded.checked} pot figure(s) equal get_hand_history(), joined by `
              + `hand name over ${archived.length} archived record(s); `
              + `${integrity ? `${integrity.distinct_names} names for ${integrity.records_held} `
                  + 'records, 0 collisions' : 'archive integrity NOT read'}`
            : `HISTORY CHAIN DISAGREEMENT: ${[...folded.mismatches, ...structural].slice(0, 4).join(' | ')}`,
    };
}

/**
 * Folds a chain-agreement result into a scene's own verdict.
 *
 * Presence checks stay — they are what tells a reader the scene photographed the
 * thing it claims to photograph — but they can no longer carry a scene on their
 * own.
 */
export function withAgreement(base, ...agreements) {
    let checks = { ...base.checks };
    let verified = base.verified;
    const good = [];
    const bad = [];

    for (const a of agreements) {
        if (!a) continue;
        verified = verified && a.ok;
        // A scene can assert against more than one surface (the table behind a
        // modal, and the modal). Namespace so nothing is silently overwritten.
        const key = a.name || 'chain';
        checks[key === 'chain' ? 'chain' : `chain_${key}`] = a.checks;
        (a.ok ? good : bad).push(a.notes);
    }

    return {
        verified,
        checks,
        notes: bad.length
            ? `${bad.join(' || ')} || scene notes: ${base.notes}`
            : [base.notes, ...good].filter(Boolean).join('; '),
    };
}

/** Tags an agreement result so withAgreement can namespace it. */
export const named = (name, agreement) => ({ ...agreement, name });
