// Replays the betting of one archived hand, and checks the money.
//
// The cards come from the seed. The MONEY does not: chip flows are only in the
// record, so the record is the primary source for them and the job here is to
// re-derive the same numbers a second way and see whether the record agrees with
// itself. Four independent statements of the same fact are available --
// `contributed`, `total_pot`, `winners[].amount`, and `starting/ending_chips` --
// plus a fifth this module computes from the blinds and the action list.
//
// If they disagree, the hand is NOT usable for statistics or EV, and this module
// says so. Printing an EV number off a hand whose pot does not add up would be
// exactly the failure mode this project keeps finding: correct totals, wrong
// recipients, every invariant silent.

const STREETS = ['preflop', 'flop', 'turn', 'river'];

/** Board visible during each street. */
export function boardForStreet(hand, street) {
  const flop = hand.flop ?? [];
  switch (street) {
    case 'preflop': return [];
    case 'flop': return [...flop];
    case 'turn': return hand.turn ? [...flop, hand.turn] : [...flop];
    case 'river': return hand.turn && hand.river ? [...flop, hand.turn, hand.river] : [...flop];
    default: return [];
  }
}

/**
 * Who posted what before a card was acted on.
 *
 * Derived from `dealer_seat` and `dealt_in`, then CROSS-CHECKED against the
 * record's own `position` labels. Note the label quirk: in a heads-up hand the
 * dealer IS the small blind and the record labels that seat "BTN", so there is no
 * "SB" label to find. A derivation that expected one would be wrong on every
 * heads-up hand in the archive.
 */
export function deriveBlinds(hand) {
  const problems = [];
  const seats = hand.dealtIn.map((d) => d.seat);
  if (seats.length < 2) return { problems: ['fewer than two seats dealt in'], sbSeat: null, bbSeat: null };

  const nextAfter = (seat) => {
    const sorted = [...seats].sort((a, b) => a - b);
    const later = sorted.find((s) => s > seat);
    return later !== undefined ? later : sorted[0];
  };

  let sbSeat;
  let bbSeat;
  if (seats.length === 2) {
    sbSeat = seats.includes(hand.dealerSeat) ? hand.dealerSeat : Math.min(...seats);
    bbSeat = seats.find((s) => s !== sbSeat);
  } else {
    sbSeat = nextAfter(hand.dealerSeat);
    bbSeat = nextAfter(sbSeat);
  }

  // The record's own labels, as a second opinion.
  const labelled = new Map(hand.players.map((p) => [p.seat, p.position]));
  const bbLabel = [...labelled.entries()].find(([, v]) => v === 'BB');
  if (bbLabel && bbLabel[0] !== bbSeat) {
    problems.push(`derived big blind seat ${bbSeat} but the record labels seat ${bbLabel[0]} "BB"`);
  }
  const sbLabel = [...labelled.entries()].find(([, v]) => v === 'SB');
  if (sbLabel && sbLabel[0] !== sbSeat) {
    problems.push(`derived small blind seat ${sbSeat} but the record labels seat ${sbLabel[0]} "SB"`);
  }
  if (!sbLabel && seats.length > 2) {
    problems.push('a three-or-more-handed hand with no seat labelled "SB"');
  }
  return { sbSeat, bbSeat, problems };
}

/**
 * @param {object} hand validated record
 * @returns {object} the replay, its decision points and its money checks
 */
export function replayHand(hand) {
  const problems = [];
  const notes = [];

  if (hand.dealtIn === null) {
    return { ok: false, problems: ['dealt_in is null: who was in the hand is not stated'], notes, decisions: [] };
  }

  const dealtSeats = hand.dealtIn.map((d) => d.seat);
  const principalOfSeat = new Map(hand.dealtIn.map((d) => [d.seat, d.principal]));
  const recordOfSeat = new Map();
  for (const p of hand.players) {
    if (p.dealtIn === true || principalOfSeat.get(p.seat) === p.principal) recordOfSeat.set(p.seat, p);
  }

  // WHO WALKED OUT ON A LIVE POT.
  //
  // Leaving mid-hand acts as a fold, but it is not an ACTION, so it is nowhere in
  // the action list. A replay that only watches actions leaves that player
  // standing in the hand: still "live", still eligible for a pot layer, still in
  // the equity calculation. The totals would still add up -- their chips really
  // are in the pot -- and the SHARES would be wrong. That is the exact shape of
  // failure this repository keeps finding, so it is called out here by name and
  // the hands it affects are excluded from EV rather than quietly mis-split.
  const departed = new Set(
    hand.players.filter((p) => p.leftMidHand === true && principalOfSeat.get(p.seat) === p.principal)
      .map((p) => p.seat),
  );

  // A street cannot have been acted on unless its cards were dealt. If the record
  // shows action on the river with four board cards, either the board is truncated
  // or the phases are mislabelled, and every board-dependent number would be
  // computed against the wrong cards.
  const needBoard = { preflop: 0, flop: 3, turn: 4, river: 5 };
  const boardLen = (hand.flop ? 3 : 0) + (hand.turn ? 1 : 0) + (hand.river ? 1 : 0);
  for (const a of hand.actions) {
    const need = needBoard[a.phase];
    if (need !== undefined && boardLen < need) {
      problems.push(
        `there is action on the ${a.phase} but the record holds only ${boardLen} board card(s); ` +
        `the ${a.phase} needs ${need}`,
      );
      break;
    }
  }

  const { sbSeat, bbSeat, problems: blindProblems } = deriveBlinds(hand);
  problems.push(...blindProblems);

  // --- forced money, in the order the canister posts it ---------------------
  const posted = new Map(dealtSeats.map((s) => [s, 0]));
  const stackLeft = new Map();
  for (const s of dealtSeats) {
    const rec = recordOfSeat.get(s);
    stackLeft.set(s, rec ? rec.startingChips : 0);
    if (!rec) problems.push(`seat ${s} was dealt in but has no player record`);
  }
  if (hand.ante > 0) {
    for (const s of dealtSeats) {
      const pay = Math.min(hand.ante, stackLeft.get(s));
      posted.set(s, posted.get(s) + pay);
      stackLeft.set(s, stackLeft.get(s) - pay);
    }
  }
  const sbPosted = sbSeat === null ? 0 : Math.min(hand.smallBlind, stackLeft.get(sbSeat) ?? 0);
  if (sbSeat !== null) { posted.set(sbSeat, posted.get(sbSeat) + sbPosted); stackLeft.set(sbSeat, stackLeft.get(sbSeat) - sbPosted); }
  const bbPosted = bbSeat === null ? 0 : Math.min(hand.bigBlind, stackLeft.get(bbSeat) ?? 0);
  if (bbSeat !== null) { posted.set(bbSeat, posted.get(bbSeat) + bbPosted); stackLeft.set(bbSeat, stackLeft.get(bbSeat) - bbPosted); }

  // --- the action list, street by street ------------------------------------
  const streetTotal = new Map(dealtSeats.map((s) => [s, 0]));
  streetTotal.set(sbSeat, sbPosted);
  streetTotal.set(bbSeat, bbPosted);
  const contributed = new Map(dealtSeats.map((s) => [s, posted.get(s)]));
  const folded = new Set();
  const allIn = new Set();
  const decisions = [];

  let street = 'preflop';
  let pot = [...contributed.values()].reduce((a, b) => a + b, 0);
  const refunds = [];

  /**
   * THE UNCALLED BET COMES BACK.
   *
   * Without this the replay counts money nobody could win. A player who bets 0.18
   * and takes the pot uncontested wagered 0.18 but CONTRIBUTED only what somebody
   * matched; the rest is handed straight back (`return_uncalled_bet`, called at
   * every completed betting round and again at settlement). Omitting it inflated
   * every fold-out pot in this replay and made five of twenty-two real hands look
   * like the record did not add up -- a tool that would have accused the canister
   * of losing money it had correctly returned.
   *
   * The rule is `poker_core::uncalled_excess`: over WHOLE-HAND contributions, the
   * single largest contributor gets back the amount by which it exceeds the
   * second largest. A tie for the top means the top was fully covered.
   */
  const returnUncalled = (why) => {
    const entries = [...contributed.entries()].filter(([, v]) => v > 0).sort((a, b) => b[1] - a[1]);
    if (entries.length === 0) return;
    const [topSeat, top] = entries[0];
    const second = entries.length > 1 ? entries[1][1] : 0;
    if (top <= second) return;
    const excess = top - second;
    contributed.set(topSeat, top - excess);
    streetTotal.set(topSeat, Math.max(0, (streetTotal.get(topSeat) ?? 0) - excess));
    pot -= excess;
    refunds.push({ seat: topSeat, amount: excess, why });
  };

  const startStreet = (name) => {
    returnUncalled(`end of ${street}`);
    street = name;
    for (const s of dealtSeats) streetTotal.set(s, 0);
  };

  for (const [i, a] of hand.actions.entries()) {
    if (!a.phaseKnown) {
      problems.push(`actions[${i}] has phase "${a.phase}", which is not a street this tool knows`);
      continue;
    }
    if (a.phase !== street) {
      const from = STREETS.indexOf(street);
      const to = STREETS.indexOf(a.phase);
      if (to < from) { problems.push(`actions[${i}] goes back from ${street} to ${a.phase}`); continue; }
      startStreet(a.phase);
    }
    if (!dealtSeats.includes(a.seat)) {
      problems.push(`actions[${i}] is at seat ${a.seat}, which was never dealt in`);
      continue;
    }
    const dealtPrincipal = principalOfSeat.get(a.seat);
    if (dealtPrincipal !== a.principal) {
      problems.push(
        `actions[${i}] at seat ${a.seat} is attributed to ${a.principal} but the deal put ` +
        `${dealtPrincipal} in that chair`,
      );
    }

    const currentBet = Math.max(0, ...dealtSeats.map((s) => streetTotal.get(s)));
    const mine = streetTotal.get(a.seat);
    const toCall = Math.max(0, currentBet - mine);
    const stackBefore = (recordOfSeat.get(a.seat)?.startingChips ?? 0) - contributed.get(a.seat);

    let increment = 0;
    switch (a.kind) {
      case 'Fold': folded.add(a.seat); break;
      case 'Check': break;
      case 'Call': increment = a.amount; break;
      // Bet / Raise / AllIn record the player's TOTAL for the street, not the
      // increment. `Raise(x)` is raise-TO-x, and `AllIn(x)` is the shover's
      // current_bet AFTER the shove. Reading either as an increment inflates
      // every pot on the table.
      case 'Bet': case 'Raise': case 'AllIn': increment = Math.max(0, a.amount - mine); break;
      case 'PostBlind': increment = a.amount; break;
      default: problems.push(`actions[${i}]: unknown kind ${a.kind}`);
    }

    decisions.push({
      index: i,
      seat: a.seat,
      principal: dealtPrincipal ?? a.principal,
      street,
      kind: a.kind,
      recordedAmount: a.amount,
      increment,
      potBefore: pot,
      toCall,
      streetTotalBefore: mine,
      stackBefore,
      board: boardForStreet(hand, street),
      liveSeatsBefore: dealtSeats.filter((s) => !folded.has(s) || s === a.seat),
      allInBefore: [...allIn],
      // Settled contributions as they stood when this decision was made, so a
      // counterfactual can be built on the same money the hand was built on.
      contributedBefore: Object.fromEntries(contributed),
      streetTotalsBefore: Object.fromEntries(streetTotal),
    });

    if (increment > 0) {
      streetTotal.set(a.seat, mine + increment);
      contributed.set(a.seat, contributed.get(a.seat) + increment);
      pot += increment;
      if (increment >= stackBefore) allIn.add(a.seat);
    }
    if (a.kind === 'AllIn') allIn.add(a.seat);
  }

  returnUncalled('hand end');

  // --- money checks ---------------------------------------------------------
  const checks = [];
  const push = (name, ok, detail) => checks.push({ name, ok, detail });

  const recordContribSum = hand.players.reduce((a, p) => a + (p.contributed ?? 0), 0);
  const winnerSum = hand.winners.reduce((a, w) => a + w.amount, 0);
  const replayedSum = [...contributed.values()].reduce((a, b) => a + b, 0);

  push('sum(contributed) == total_pot', recordContribSum === hand.totalPot,
    `${recordContribSum} vs ${hand.totalPot}`);
  push('sum(winners) == total_pot', winnerSum === hand.totalPot, `${winnerSum} vs ${hand.totalPot}`);

  // WRONG RECIPIENT, RIGHT TOTAL.
  //
  // Every check above is a SUM, and a sum cannot see a pot paid to the wrong
  // person. Move one credit from one player to another and `sum(winners)`,
  // `sum(contributed)` and `total_pot` are all still exactly right. That is the
  // signature this repository keeps finding, seven waves running, so it gets its
  // own check: the winner list and the player list are two independent statements
  // of WHO WAS PAID, and they have to name the same people for the same amounts.
  const wonByPerson = new Map();
  for (const w of hand.winners) {
    const k = `${w.seat}|${w.principal}`;
    wonByPerson.set(k, (wonByPerson.get(k) ?? 0) + w.amount);
  }
  for (const p of hand.players) {
    const k = `${p.seat}|${p.principal}`;
    const fromWinners = wonByPerson.get(k) ?? 0;
    if (fromWinners !== p.amountWon) {
      push(`seat ${p.seat} paid the same in both lists`, false,
        `winners credit ${fromWinners} to ${p.principal.slice(0, 8)}… but the player record says ${p.amountWon}`);
    }
    wonByPerson.delete(k);
  }
  for (const [k, amount] of wonByPerson) {
    push('every winner is a player in this hand', false,
      `${amount} paid to ${k} who has no player record in this hand`);
  }
  push('replayed pot == total_pot', replayedSum === hand.totalPot, `${replayedSum} vs ${hand.totalPot}`);
  push('rake == 0', hand.rake === 0, `rake=${hand.rake}`);

  // Everything the RECORD says about itself, excluding this tool's own replay.
  // The replayed-pot check is the one a departure necessarily breaks, so it
  // cannot be part of the evidence that a departure is the explanation.
  const sumsBalance = checks.filter((c) => c.name !== 'replayed pot == total_pot').every((c) => c.ok);
  const departureRefunds = [];

  for (const p of hand.players) {
    if (p.contributed === null) { notes.push(`seat ${p.seat}: record predates "contributed"`); continue; }
    const replayed = contributed.get(p.seat);
    if (p.dealtIn === true && replayed !== undefined && replayed !== p.contributed) {
      // THE BIG BLIND THAT COST HALF A BIG BLIND.
      //
      // A player who walks out of a live hand is folded, and `leave_table` hands
      // back anything of theirs nobody had covered -- including part of a POSTED
      // BLIND, if the action had not reached them yet. Right after the blinds the
      // big blind is always the top contributor, so the big blind always gets
      // `big_blind - small_blind` back by leaving. Reproduced four times out of
      // four on a live local table.
      //
      // Nothing in the record says WHEN they left, so a replay from the action
      // list cannot predict the amount. It can, however, tell this apart from a
      // record that does not add up: the sums all still balance, the shortfall is
      // in one departed seat, and it is no larger than what that seat was forced
      // to post. Anything outside that stays a failure.
      const forced = posted.get(p.seat) ?? 0;
      const shortfall = replayed - p.contributed;
      if (departed.has(p.seat) && shortfall > 0 && shortfall <= forced && sumsBalance) {
        departureRefunds.push({ seat: p.seat, principal: p.principal, amount: shortfall, forced });
        notes.push(
          `seat ${p.seat} left this hand while it was live and was handed back ${shortfall} of the `
          + `${forced} it was forced to post. The pot balances; this is the canister's uncalled-bet `
          + 'rule applied to a blind, and it is why nothing here can be priced exactly.',
        );
      } else {
        push(`seat ${p.seat} replay == contributed`, false, `${replayed} vs ${p.contributed}`);
      }
    }
    const expectEnd = p.startingChips - p.contributed + p.amountWon;
    if (expectEnd !== p.endingChips) {
      push(`seat ${p.seat} chips balance`, false,
        `${p.startingChips} - ${p.contributed} + ${p.amountWon} = ${expectEnd}, record says ${p.endingChips}`);
    }
    if (p.contributed > p.startingChips) {
      push(`seat ${p.seat} contributed <= starting chips`, false,
        `${p.contributed} > ${p.startingChips}`);
    }
  }

  const failed = checks.filter((c) => !c.ok);
  for (const f of failed) problems.push(`${f.name}: ${f.detail}`);
  // The replayed-pot check is computed from the action list, which cannot see a
  // departure, so it has to be recomputed once the refund is known.
  if (departureRefunds.length > 0) {
    const back = departureRefunds.reduce((a, r) => a + r.amount, 0);
    const idx = problems.findIndex((x) => x.startsWith('replayed pot == total_pot'));
    if (idx >= 0 && replayedSum - back === hand.totalPot) {
      problems.splice(idx, 1);
      const c = checks.find((x) => x.name === 'replayed pot == total_pot');
      if (c) { c.ok = true; c.detail = `${replayedSum} - ${back} returned on departure = ${hand.totalPot}`; }
    }
  }

  return {
    ok: problems.length === 0,
    problems,
    notes,
    sbSeat,
    bbSeat,
    sbPosted,
    bbPosted,
    dealtSeats,
    principalOfSeat,
    recordOfSeat,
    contributed,
    folded,
    allIn,
    departed,
    decisions,
    refunds,
    departureRefunds,
    pot,
    checks,
  };
}
