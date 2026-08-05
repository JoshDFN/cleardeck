<script>
  /**
   * The poker table.
   *
   * GEOMETRY IS A SINGLE RIGID OBJECT, exactly as measured on PokerNow in
   * docs/DESIGN-BAR.md §2.2 (bar 19): one scale input, everything else a ratio
   * of it, no per-element breakpoints. Here the scale input is the FELT WIDTH
   * `--fw`, resolved once in CSS from container-query units:
   *
   *     --fw: min(72cqw, 174.6cqh)
   *
   * so the surface is width-capped at 72% of the stage on wide viewports and
   * height-fitted otherwise. `--cd-avail` (measured in JS, the only measurement
   * the layout needs) tells the wrapper how much viewport is left below the
   * page chrome, so the action bar can never fall below the fold.
   *
   * Seats sit on an ELLIPSE (bar 2). Their angles are spaced by ARC LENGTH, not
   * by angle, which is why 6-max lands on the GGPoker pattern (bottom-centre,
   * bottom-left, top-left, top-centre, top-right, bottom-right) and 9-max on
   * the WPT Global pattern, without either being hand-placed.
   */
  import Card from './Card.svelte';
  import ActionFeed from './ActionFeed.svelte';
  import { playSound } from '$lib/sounds.js';
  import { computeEquity, describeHand, formatEquity, cardCodes } from '$lib/equity.js';
  // Importing this module installs the app-wide BigInt/JSON guard (see the
  // header of $lib/utils.js). This component is where the class of defect that
  // guard exists for was found (docs/DEFECTS.md T-10), so it names it.
  import { candidKey } from '$lib/utils.js';

  const {
    tableState,
    myCards,
    onAction,
    actionPending = false,
    tableBalance = 0,
    onShowDeposit = null,
    onShowWithdraw = null,
    currency = 'ICP',
    avatarStyle = 'bottts',
    customName = null,
    shuffleProof = null,
    onShowProof = null
  } = $props();

  const isBTC = $derived(currency === 'BTC');
  const currencySymbol = $derived(isBTC ? 'BTC' : 'ICP');

  // ---------------------------------------------------------------------------
  // 1. Money formatting -- ONE precision for the whole table
  // ---------------------------------------------------------------------------
  //
  // The old formatter chose its precision per value, so a 50 ICP stack rendered
  // "50.00" while a busted stack two seats away rendered "0.0000". Precision is
  // now a property of the TABLE (derived from its big blind) and every figure on
  // the felt uses it, so figures are comparable at a glance.

  const bigBlindRaw = $derived(Number(tableState?.config?.big_blind ?? 0));
  const decimals = $derived(
    isBTC ? 0 : (bigBlindRaw > 0 && bigBlindRaw < 1_000_000 ? 4 : 2)
  );

  function fmt(value) {
    const n = Number(value ?? 0);
    if (!Number.isFinite(n)) return '0';
    if (isBTC) return Math.round(n).toLocaleString('en-US');
    return (n / 100_000_000).toLocaleString('en-US', {
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals
    });
  }

  function fmtWithUnit(value) {
    return `${fmt(value)} ${currencySymbol}`;
  }

  // Names kept for readability at the call sites.
  const formatChips = fmt;
  const formatWithUnit = fmtWithUnit;

  // ---------------------------------------------------------------------------
  // 2. Table state
  // ---------------------------------------------------------------------------

  function variantKey(v) {
    if (!v) return null;
    const keys = Object.keys(v);
    return keys.length ? keys[0] : null;
  }

  function humanise(key) {
    return key ? key.replace(/([A-Z])/g, ' $1').trim() : '';
  }

  const phaseKey = $derived(variantKey(tableState?.phase) ?? 'WaitingForPlayers');
  const phase = $derived(humanise(phaseKey) || 'Waiting');
  const streetLabel = $derived(
    phaseKey === 'WaitingForPlayers' ? 'Waiting'
      : phaseKey === 'HandComplete' ? 'Complete'
      : humanise(phaseKey)
  );

  const pot = $derived(Number(tableState?.pot ?? 0));
  const sidePots = $derived(tableState?.side_pots || []);
  const communityCards = $derived(tableState?.community_cards || []);
  const players = $derived((tableState?.players || []).map(p => (p && p.length > 0) ? p[0] : null));
  const actionOn = $derived(Number(tableState?.action_on ?? 0));
  const currentBet = $derived(Number(tableState?.current_bet ?? 0));
  const minRaise = $derived(Number(tableState?.min_raise ?? tableState?.config?.big_blind ?? 0));
  const minBet = $derived(Number(tableState?.min_bet ?? tableState?.config?.big_blind ?? 10));
  const mySeat = $derived(tableState?.my_seat?.length > 0 ? tableState.my_seat[0] : null);
  const isMyTurn = $derived(tableState?.is_my_turn === true);
  const gameInProgress = $derived(phaseKey !== 'WaitingForPlayers' && phaseKey !== 'HandComplete');
  const myPlayer = $derived(mySeat !== null && mySeat < players.length ? players[mySeat] : null);
  const handNumber = $derived(Number(tableState?.hand_number ?? 0));

  const callAmount = $derived(Number(
    tableState?.call_amount ?? (myPlayer ? Math.max(0, currentBet - Number(myPlayer.current_bet ?? 0)) : 0)
  ));
  const canCheck = $derived(tableState?.can_check ?? (myPlayer !== null && callAmount === 0));
  const canRaise = $derived(tableState?.can_raise ?? (Number(myPlayer?.chips ?? 0) > callAmount));
  const myChips = $derived(Number(myPlayer?.chips ?? 0));
  const maxBetAmount = $derived(myChips + Number(myPlayer?.current_bet ?? 0));

  const dealerSeat = $derived(tableState?.dealer_seat);
  const smallBlindSeat = $derived(tableState?.small_blind_seat);
  const bigBlindSeat = $derived(tableState?.big_blind_seat);

  const lastWinners = $derived(tableState?.last_hand_winners || []);
  const isHandComplete = $derived(phaseKey === 'HandComplete');
  const isShowdown = $derived(phaseKey === 'Showdown' || phaseKey === 'HandComplete');
  const myWinInfo = $derived(mySeat !== null ? lastWinners.find(w => w.seat === mySeat) : null);
  const winnerSeats = $derived(new Set(lastWinners.map(w => Number(w.seat))));

  const isSittingOut = $derived(variantKey(myPlayer?.status) === 'SittingOut');

  /**
   * THE POT -- docs/DEFECTS.md T-08.
   *
   * `get_pot()` returns `state.pot`, and the canister raises `state.pot` in the
   * SAME statement that raises a player's `current_bet` (src/table_canister:
   * blinds ~2742/2754, Call 3270, Bet 3289, Raise 3323, AllIn 3356). So the
   * chain's pot ALREADY contains every live bet. The table used to render
   * `pot + sum(current_bet)`, which counted every live chip twice: at 0.10/0.10
   * pre-flop the screen read 0.40 over a canister pot of 0.20.
   *
   * The headline is therefore `pot`, unmodified, and it is the figure the
   * screenshot harness asserts against `get_pot()`. `liveBets` and
   * `collectedPot` are a DECOMPOSITION of it -- they must sum back to it, which
   * is what the harness checks on `.pot-breakdown` -- never an addition to it.
   */
  const liveBets = $derived(
    players.reduce((t, p) => t + (p ? Number(p.current_bet ?? 0) : 0), 0)
  );
  const totalPot = $derived(pot);
  const collectedPot = $derived(Math.max(0, pot - liveBets));

  /**
   * THE ALL-IN MOMENT, built from chain state only.
   *
   * docs/DESIGN-BAR.md bar 15: the reference clients dramatise all-in with
   * INFORMATION -- an All-In tag, live per-player EQUITY, and the pot called
   * out. Two of the four (PokerStars, GGPoker) show the equity. What is on chain
   * is used first: who is all in, how much each of them has committed, and the
   * pot at risk. The equity is COMPUTED, in $lib/equity.js, from cards this
   * viewer can already see -- see `equity` below for the information rule.
   */
  const allInSeats = $derived(
    players.map((p, i) => (p && p.is_all_in && !p.has_folded ? i : -1)).filter(i => i >= 0)
  );
  const allInMoment = $derived(gameInProgress && allInSeats.length > 0);

  /**
   * WHO IS IN THIS HAND, by the canister's own rule, in both of its phases.
   *
   * MID-HAND. `start_hand` clears every seat's `hole_cards` and then deals only
   * to `status == Active` seats (src/table_canister/src/lib.rs:2714, 6765-6777),
   * and folding sets `has_folded`. So "dealt in and still live" is exactly
   * `Active && !has_folded`.
   *
   * AT SHOWDOWN, `status` is no longer safe to read, and this cost a whole
   * capture before it was found. `sit_out_next_hand` only sets a FLAG; the flag
   * is consumed by the next `start_hand`, which writes `status = SittingOut`
   * BEFORE it checks whether two active players are left (lib.rs:2644-2665) —
   * and an update that returns `Err` still commits what it wrote. So an
   * auto-deal that fires and correctly refuses to deal leaves a player who is
   * sitting in the finished hand, cards face up on the felt, marked SittingOut.
   * Reading `status` there dropped that player out of the live set and the
   * showdown rendered with NO equity badges at all — nondeterministically,
   * depending on whether the auto-deal timer beat the screenshot.
   *
   * The cards themselves do not have that problem. At showdown the canister
   * reveals `hole_cards` for every non-folded player it dealt in, and leaves
   * them null for everyone it did not, so "has cards on the wire" IS "was dealt
   * in" — a fact about this hand, not about what the seat intends to do next.
   */
  function isInHand(p) {
    if (!p || p.has_folded) return false;
    if (isShowdown) return !!revealedHole(p);
    return variantKey(p.status) === 'Active';
  }

  const liveSeats = $derived(
    (gameInProgress || isShowdown)
      ? players.map((p, i) => (isInHand(p) ? i : -1)).filter(i => i >= 0)
      : []
  );

  /**
   * EQUITY -- computed, never invented, and never from a card you may not see.
   *
   * `get_table_view` nulls `hole_cards` for every player the caller is not
   * entitled to (self, or showdown-and-not-folded, or a voluntary show), so the
   * opponents' cards are simply ABSENT from `tableState` until the engine
   * reveals them. Two modes fall out of that, and $lib/equity.js implements only
   * those two:
   *
   *   showdown -- every live hand is face up because the ENGINE turned it up.
   *               Exact enumeration of the remaining runouts; with a complete
   *               board that is one runout, i.e. 100.00% / 0.00%.
   *   hero     -- only your own two cards are known. No per-opponent number is
   *               produced. Yours is computed against opponents drawn uniformly
   *               at random, and the badge SAYS "vs N random" so it can never be
   *               read as a read on anybody.
   *
   * MEMOISED ON THE CARDS. `tableState` is re-polled about once a second and the
   * action clock ticks inside it, so a naive $derived would re-run a 200,000
   * trial Monte Carlo every second and the badge would flicker. The key is the
   * exact card set plus the mode; anything else moving on the table is not an
   * input to equity and must not restart it.
   */
  const equityInput = $derived.by(() => {
    if (!allInMoment && !isShowdown) return null;
    const board = communityCards.slice(0, 5);
    const boardKey = (cardCodes(board) ?? []).join('.');
    const revealed = liveSeats
      .map(seat => ({ seat, cards: seat === mySeat ? (myCards || []) : revealedHole(players[seat]) }))
      .filter(r => Array.isArray(r.cards) && r.cards.length === 2);
    const hero = (mySeat !== null && myCards && myCards.length === 2 && liveSeats.includes(mySeat))
      ? { seat: mySeat, cards: myCards }
      : null;
    if (!hero && revealed.length < 2) return null;
    const key = [
      revealed.length === liveSeats.length ? 'sd' : 'hero',
      boardKey,
      liveSeats.length,
      hero ? (cardCodes(hero.cards) ?? []).join('-') : '',
      revealed.map(r => `${r.seat}:${(cardCodes(r.cards) ?? []).join('-')}`).join(','),
    ].join('|');
    return { key, revealed, liveCount: liveSeats.length, hero, board };
  });

  // A plain (non-reactive) instance-local memo: the cache must NOT be a signal,
  // or writing it would re-invalidate the derived that wrote it.
  let equityMemo = { key: null, value: null };

  const equity = $derived.by(() => {
    const input = equityInput;
    if (!input) return null;
    if (equityMemo.key === input.key) return equityMemo.value;
    let value = null;
    try {
      value = computeEquity(input);
    } catch (err) {
      // A wrong picture of the table must degrade to NO number, never a wrong
      // one. The felt simply stops claiming an equity.
      console.error('[cleardeck] equity computation failed; no equity will be shown', err);
      value = null;
    }
    equityMemo = { key: input.key, value };
    return value;
  });

  const equityMode = $derived(equity?.mode ?? null);

  /** What the badge says, or null if this seat has no honest number. */
  function equityFor(seat) {
    if (!equity) return null;
    const share = equity.bySeat.get(seat);
    if (share === undefined) return null;
    return formatEquity(share, equity.method);
  }

  /** One line naming the method, shown on the felt beside the pot (bar 15). */
  const equityMethodLabel = $derived.by(() => {
    if (!equity) return null;
    if (equity.mode === 'showdown') {
      return equity.method === 'exact'
        ? `Equity · exact · ${equity.trials.toLocaleString('en-US')} runout${equity.trials === 1 ? '' : 's'}`
        : `Equity · Monte Carlo · ${equity.trials.toLocaleString('en-US')} trials`;
    }
    return `Equity vs ${equity.opponents} random · Monte Carlo · ${equity.trials.toLocaleString('en-US')} trials`;
  });

  /**
   * YOUR hand, named in words -- GGPoker's hand-strength readout (bar 15/16).
   *
   * Always safe: it is a function of the two cards in your own hand and the
   * board everyone can see. Pre-flop it names the holding ("Queen-Six offsuit")
   * rather than a five-card hand that does not exist yet.
   */
  const heroHandName = $derived.by(() => {
    if (!myCards || myCards.length !== 2) return null;
    const codes = cardCodes([...myCards, ...communityCards.slice(0, 5)]);
    if (!codes) return null;
    return describeHand(codes)?.name ?? null;
  });

  // ---------------------------------------------------------------------------
  // 3. The seat ring -- equal ARC LENGTH on the ellipse
  // ---------------------------------------------------------------------------

  const maxPlayers = $derived(Number(tableState?.config?.max_players ?? 9));
  const seatCount = $derived(Math.max(2, Math.min(maxPlayers, players.length || maxPlayers)));

  /**
   * @param {number} n seat count
   * @param {number} aspect surface w:h
   * @returns {{cs:number, sn:number, nx:number, ny:number, bx:number, by:number,
   *            side:'left'|'right'|'center'}[]}
   *   cs/sn are cos/sin of the seat's angle; nx/ny is the unit vector pointing
   *   at the table centre in SCREEN pixels (used to lay cards, chips and badges
   *   on inner concentric rings without any per-seat hand placement); bx/by is
   *   where this seat's committed chips sit, in felt widths from the ring point.
   *
   *   bx/by is NOT just "inward a bit", and that is the whole point. The felt is
   *   a 2:1 stadium, so the room between a seat and the board is ~0.20 felt
   *   widths at the sides and only ~0.15 at top and bottom -- and the hole cards
   *   already eat most of the vertical one. Pushing every chip inward by a fixed
   *   amount is what parked the bet disc on top of the hero's own cards. Side
   *   seats therefore push their chips inward past the cards; top and bottom
   *   seats push theirs SIDEWAYS along the tangent, into the felt that a 2:1
   *   surface has in abundance exactly there.
   */
  function ringSeats(n, aspect) {
    const rx = 1;
    const ry = 1 / aspect;
    // Arc-length table over one full turn, starting at bottom centre (90deg).
    const STEPS = 3600;
    const pt = (deg) => {
      const r = (deg * Math.PI) / 180;
      return [rx * Math.cos(r), ry * Math.sin(r)];
    };
    const cum = [0];
    let prev = pt(90);
    for (let i = 1; i <= STEPS; i += 1) {
      const cur = pt(90 + (360 * i) / STEPS);
      cum.push(cum[i - 1] + Math.hypot(cur[0] - prev[0], cur[1] - prev[1]));
      prev = cur;
    }
    const total = cum[STEPS];

    const out = [];
    for (let k = 0; k < n; k += 1) {
      const target = (total * k) / n;
      let lo = 0;
      let hi = STEPS;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (cum[mid] < target) lo = mid + 1; else hi = mid;
      }
      const deg = 90 + (360 * lo) / STEPS;
      const r = (deg * Math.PI) / 180;
      const cs = Math.cos(r);
      const sn = Math.sin(r);
      // Screen-space position, in units of the felt half-width.
      const px = cs;
      const py = sn / aspect;
      const len = Math.hypot(px, py) || 1;
      const nx = -px / len;
      const ny = -py / len;
      const side = cs < -0.35 ? 'left' : cs > 0.35 ? 'right' : 'center';

      // Committed chips. CHIP_IN / CHIP_SIDE are felt-width fractions chosen so
      // the disc clears the hole cards (which end 0.11 fw inward) without
      // reaching the board (whose half-width is 0.30 fw).
      //
      // THE RULE IS "PUSH ALONG THE AXIS THAT HAS ROOM", AND THAT AXIS TURNS
      // WITH THE SURFACE. On the 2.1:1 landscape stadium the spare felt is
      // horizontal, so every disc moves horizontally: flank seats push inward
      // (their normal IS horizontal) and top/bottom seats push sideways along
      // the tangent. On the 0.555:1 portrait surface the spare felt is
      // vertical, and the same three constants applied unchanged put the
      // hero's disc on top of the hero's own hole cards and the flank discs
      // across the pot's collected/betting line. Portrait therefore transposes
      // the rule: top and bottom seats push inward (their normal is vertical
      // there), flank seats push along the tangent, up or down.
      const tall = aspect < 1;
      const CHIP_IN = tall ? 0.255 : 0.155;
      const CHIP_SIDE = tall ? 0.150 : 0.140;
      const CHIP_SIDE_IN = tall ? 0.045 : 0.050;
      // A seat sits on a rail segment that is either mostly horizontal (its
      // normal points up or down) or mostly vertical. `alongNormal` is true when
      // pushing inward already moves the disc along the roomy axis. The
      // LANDSCAPE test is left exactly as it was -- `side`, i.e. |cos| > 0.35 --
      // so the proven desktop chip positions do not move by a pixel.
      const alongNormal = tall ? Math.abs(ny) >= Math.abs(nx) : side !== 'center';
      let bx = nx * CHIP_IN;
      let by = ny * CHIP_IN;
      if (!alongNormal) {
        // Tangent, taken away from the axis the board sits on so the disc never
        // drifts under it. Dead centre is broken to the right (landscape) or
        // downward (portrait).
        if (tall) {
          const away = sn >= 0 ? 1 : -1;
          bx = nx * CHIP_SIDE_IN;
          by = away * CHIP_SIDE + ny * CHIP_SIDE_IN;
        } else {
          const away = cs >= 0 ? 1 : -1;
          bx = away * CHIP_SIDE + nx * CHIP_SIDE_IN;
          by = ny * CHIP_SIDE_IN;
        }
      }

      // THE AWARD SPOT (docs/DEFECTS.md T-23). The winner's `+X` chip used to be
      // the chip spot times a single multiplier -- 1.30 in landscape, 1.12 in
      // portrait -- and that multiplier means two different things depending on
      // which axis the chip spot used:
      //
      //   chip pushed along the TANGENT (top and bottom seats in landscape,
      //   flank seats in portrait): scaling it moves the award further from the
      //   board's centre line. Proven, and kept exactly as it was.
      //
      //   chip pushed along the NORMAL (flank seats in landscape, top and bottom
      //   seats in portrait): the normal points AT THE BOARD, so scaling it
      //   drives the award INTO the board. Measured on desktop: the `+24.00`
      //   chip of a winner at seat 1 or 2 overlapped `.community-cards` by 25.6%
      //   of its own area and covered the suit pip of a board card -- the cards
      //   that justify the award.
      //
      // For those seats the award therefore keeps the chip's own distance along
      // the normal and takes its extra clearance along the TANGENT, away from
      // the board: sideways on a wide surface, and away from the board's row on
      // a tall one. Nothing here moves the bet disc, which is measured
      // everywhere else.
      const AWARD_OUT = tall ? 1.12 : 1.30;
      const AWARD_TAN = 0.150;
      let ax = bx * AWARD_OUT;
      let ay = by * AWARD_OUT;
      if (alongNormal && !tall) {
        // Landscape flank seat: the board is a mid-height band, so the room is
        // vertical. The award keeps the chip's own inward distance and takes its
        // showdown clearance along the tangent, away from the board's row.
        ax = bx;
        ay = by + (sn >= 0 ? 1 : -1) * AWARD_TAN;
      }

      // THE POD-ATTACHED READOUT SPOKE (docs/DEFECTS.md T-22 and T-23).
      //
      // The equity badge -- and in portrait the award chip as well -- hangs off
      // the plate. Which side is free is not a matter of taste: every other
      // object in a seat already claims a direction, and claiming the same one
      // is how a readout ends up on top of the cards it describes.
      //
      //   flank seat (the normal is mostly HORIZONTAL): the cards ride +/-cy,
      //     which is vertical, and the bet disc rides the tangent, also
      //     vertical. The free side is therefore horizontal, and INWARD -- at a
      //     phone's left flank seat, outward is off the screen entirely
      //     (measured).
      //   top or bottom seat, PORTRAIT: the cards ride the vertical normal
      //     inward, so the free side is vertically OUTWARD.
      //   top or bottom seat, LANDSCAPE: outward is off the felt and over the
      //     action dock (the old "All In" tag died there: read at y=818 on a
      //     surface ending at y=753). The plate's two ends are free, so the
      //     readout takes the end OPPOSITE the bet disc and the award, both of
      //     which ride the tangent there.
      const flank = Math.abs(nx) > Math.abs(ny);
      let rdx = 0;
      let rdy = 0;
      if (flank) rdx = nx >= 0 ? 1 : -1;
      else if (tall) rdy = ny >= 0 ? -1 : 1;
      else rdx = cs >= 0 ? -1 : 1;

      // IN PORTRAIT THE AWARD JOINS THE BADGE ON THE SPOKE. At a portrait flank
      // seat the chip's tangent and the cards' `cy` are the SAME direction (see
      // `cy` below), so the award landed on the winner's own revealed pair --
      // measured at 68% of a card's ink on a phone. At a portrait top or bottom
      // seat the chip vector points at a board that spans nearly the whole felt
      // width, and the award then competes with the flank seats' own readouts in
      // the middle of the table (measured: the hero's award covering a flank
      // seat's `0.00%` badge). The plate's free side is free for both, so both
      // use it -- the badge at one end of it, the award at the other, which the
      // CSS lays out because the distances are functions of the pod's own size.
      // Landscape keeps the chip vector, where it is proven and has room.
      const awardOnSpoke = tall;

      out.push({
        cs: Number(cs.toFixed(5)),
        sn: Number(sn.toFixed(5)),
        nx: Number(nx.toFixed(5)),
        ny: Number(ny.toFixed(5)),
        bx: Number(bx.toFixed(5)),
        by: Number(by.toFixed(5)),
        ax: Number(ax.toFixed(5)),
        ay: Number(ay.toFixed(5)),
        rdx,
        rdy,
        awardOnSpoke,
        // Which way an OPPONENT's hole cards peek out from behind their plate.
        // Never along the inward normal: at a side seat the normal is horizontal
        // and the pod is 0.235 fw WIDE, so a horizontal nudge buries the cards
        // under the plate -- seat 1 showed 20px of a 158px pair. Always
        // perpendicular to the rail and into the felt: up for the bottom half of
        // the ring, down for the top half.
        //
        // PORTRAIT SENDS A FLANK SEAT'S PAIR THE OTHER WAY. On the 0.555
        // surface "toward the centre" is where the board tray, the pot and the
        // winner line already are, and at showdown a revealed pair landed on
        // top of the winner readout. A tall table has ~85 px of clear felt
        // between one flank plate and the next along the rail, so that is where
        // the pair goes. Top and bottom seats are unaffected: their normal IS
        // vertical, and the felt they open onto is the middle of the table.
        cy: (tall && Math.abs(nx) > Math.abs(ny)) ? (sn >= 0 ? 1 : -1) : (sn >= 0 ? -1 : 1),
        side
      });
    }
    return out;
  }

  // Landscape and portrait use different surface aspects, so the ring is
  // recomputed for each -- one ring generator, two dressings (bar 19/20).
  // These two MUST equal `--ar` in the matching CSS block, or the seats sit on
  // an ellipse the felt is not drawing.
  //
  // PORTRAIT_AR went 0.70 -> 0.555 in wave 4. 0.70 was chosen so the RING fit a
  // short stage; it left the felt itself 0.70:1, which on a 390x844 phone is a
  // 217x316 surface -- 20.6% of the screen against PokerNow's real portrait
  // capture at 52.1% (docs/DESIGN-BAR.md bar 22 wants >= 40%). 0.555 is the
  // aspect at which the WIDTH cap (the ring plus its two flanking pods must fit
  // the stage) and the HEIGHT cap (the felt itself must fit the stage) bind at
  // the same felt width on that phone, i.e. the largest surface the screen can
  // actually hold. It is also within 3% of PokerNow's measured 0.571.
  const LANDSCAPE_AR = 2.10;
  const PORTRAIT_AR = 0.555;

  let portrait = $state(false);
  const ring = $derived(ringSeats(seatCount, portrait ? PORTRAIT_AR : LANDSCAPE_AR));

  /**
   * THE HERO ALWAYS SITS AT THE BOTTOM OF THE SCREEN.
   *
   * Every reference client rotates the table so that you are at the near edge;
   * this one used to map seat i to ring point i, so a player who took seat 4 of
   * 9 sat at the TOP of their own table and read their hole cards across the
   * board from the far rail.
   *
   * The rotation is applied to the GEOMETRY only. `{#each}` still walks seats in
   * canister order, so the nth `.seat` element in the DOM is still seat n --
   * which is the contract tools/shots/lib/dom-scrape.mjs reads the table by
   * ("index: DOM order, which is the seat index"). Rotating the markup instead
   * would have silently re-pointed every stack, bet and badge assertion in the
   * screenshot harness at the wrong player.
   */
  const seatPoints = $derived.by(() => {
    const n = ring.length;
    if (n === 0) return [];
    const anchor = (mySeat !== null && mySeat >= 0 && mySeat < n) ? mySeat : 0;
    return Array.from({ length: n }, (_, i) => ring[(i - anchor + n) % n]);
  });

  // ---------------------------------------------------------------------------
  // 4. The one measurement the layout needs: how much viewport is left
  // ---------------------------------------------------------------------------

  let wrapperEl = $state(null);
  let availPx = $state(620);
  let parentSlack = $state(0);

  const MIN_AVAIL_LANDSCAPE = 460;
  const MIN_AVAIL_PORTRAIT = 340;

  function documentTop(el) {
    let top = 0;
    let node = el;
    while (node && node !== document.body) {
      top += node.offsetTop || 0;
      node = node.offsetParent;
    }
    return top;
  }

  function measureViewport() {
    if (typeof window === 'undefined') return;
    portrait = window.innerHeight >= window.innerWidth;
    if (!wrapperEl) return;
    const top = documentTop(wrapperEl);
    // A FLOOR TALLER THAN WHAT IS LEFT IS NOT A FLOOR, IT IS AN OVERFLOW.
    //
    // `Math.max(floor, room)` alone hands the wrapper a height the viewport does
    // not have whenever the chrome above it is tall, and the two floors here are
    // 340 and 460 CSS px. Measured on ONE PHONE HELD SIDEWAYS, 844x390, before
    // this clamp: the 460 floor put the felt at `y 323..625` of a 390 px
    // viewport and took the action dock (`y 677..739`), the pot readout and the
    // bottom-left seat pod off the frame with it. Four things a player acts on,
    // below the fold, on a viewport `./scripts/dev.sh shots` does not
    // photograph and docs/DESIGN-BAR.md §9.6 lists as unadjudicated -- which is
    // why a floor and bar 23 ("the felt wholly inside the viewport") had been
    // able to coexist in this function.
    //
    // Bar 23 wins. The floor may RAISE a table the page has squeezed; it may
    // never push one past the bottom edge. `usable` is what is genuinely left
    // below the chrome, and both the floor and the room are capped by it, so
    // `availPx <= usable` always holds.
    //
    // Neither canonical viewport is affected: measured `usable` is 703.4 at
    // 390x844 (floor 340) and 629.5 at 1440x900 (floor 460), so the clamp is
    // inert at both and both felt measurements are unchanged -- desktop is still
    // 961.1 x 457.7 at 6-max and 982.8 x 468.0 at 9-max, aspect 2.10.
    const rawFloor = portrait ? MIN_AVAIL_PORTRAIT : MIN_AVAIL_LANDSCAPE;
    const usable = Math.max(0, window.innerHeight - top - 8);
    const floor = Math.min(rawFloor, usable);
    // 8px of breathing room at the bottom edge; never taller than the viewport.
    const room = Math.min(usable, Math.round(window.innerHeight * 0.94));
    availPx = Math.max(floor, Math.round(room));
    // The page reserves bottom padding around the table area; cancel it so a
    // table that exactly fits does not produce a scrollbar.
    const parent = wrapperEl.parentElement;
    parentSlack = parent
      ? Math.round(parseFloat(getComputedStyle(parent).paddingBottom) || 0)
      : 0;
  }

  /**
   * THE TABLE STARTS AT THE TOP OF THE VIEWPORT.
   *
   * docs/DEFECTS.md T-16 / docs/DESIGN-BAR.md bar 23. `--cd-avail` is a
   * DOCUMENT-space arithmetic: "the
   * viewport, less everything above the table in the flow". It is only the
   * on-screen truth while the page is scrolled to the top, and entering a table
   * does not guarantee that -- the lobby's first row is BELOW the fold on a
   * phone (docs/DESIGN-BAR.md bar 25 measured it at y=1004 of 844), so tapping
   * a table scrolls the page down first and the table view inherits that offset.
   * Measured on the shipped build at 390x844: scrollY=137 on arrival, which is
   * exactly why `table-preflop-mobile.png` and `table-facing-bet-mobile.png`
   * shipped with the pot, the whole board and four of six pods ABOVE the top of
   * the frame while the DOM assertions all passed (bar 23).
   *
   * The table therefore puts the page back at the top when it opens, once, and
   * every scrollable ancestor with it. It is a mount-time correction, not a
   * scroll lock: the player can still scroll down to the notices afterwards.
   */
  function anchorToTop() {
    if (typeof window === 'undefined' || !wrapperEl) return;
    for (let node = wrapperEl.parentElement; node; node = node.parentElement) {
      if (node.scrollTop) node.scrollTop = 0;
    }
    if (window.scrollY) window.scrollTo(0, 0);
  }

  $effect(() => {
    if (typeof window === 'undefined') return undefined;
    anchorToTop();
    measureViewport();
    const onResize = () => measureViewport();
    window.addEventListener('resize', onResize);
    let ro = null;
    if (wrapperEl && typeof ResizeObserver !== 'undefined') {
      ro = new ResizeObserver(() => measureViewport());
      ro.observe(document.documentElement);
    }
    return () => {
      window.removeEventListener('resize', onResize);
      if (ro) ro.disconnect();
    };
  });

  // ---------------------------------------------------------------------------
  // 5. Action clock
  // ---------------------------------------------------------------------------

  let serverTimeRemaining = $state(null);
  let lastServerUpdate = $state(0);
  let displayedTimeRemaining = $state(null);

  $effect(() => {
    const serverTime = tableState?.time_remaining_secs?.length > 0
      ? Number(tableState.time_remaining_secs[0])
      : null;
    if (serverTime !== null) {
      const diff = Math.abs((serverTimeRemaining ?? 0) - serverTime);
      if (serverTimeRemaining === null || diff > 2 || serverTime > serverTimeRemaining) {
        serverTimeRemaining = serverTime;
        lastServerUpdate = Date.now();
        displayedTimeRemaining = serverTime;
      }
    } else {
      serverTimeRemaining = null;
      displayedTimeRemaining = null;
    }
  });

  $effect(() => {
    if (displayedTimeRemaining === null || displayedTimeRemaining <= 0) return undefined;
    const id = setInterval(() => {
      const elapsed = Math.floor((Date.now() - lastServerUpdate) / 1000);
      displayedTimeRemaining = Math.max(0, (serverTimeRemaining ?? 0) - elapsed);
    }, 1000);
    return () => clearInterval(id);
  });

  const timeRemaining = $derived(displayedTimeRemaining);
  const usingTimeBank = $derived(tableState?.using_time_bank || false);
  const timeBankRemaining = $derived(
    tableState?.time_bank_remaining_secs?.length > 0
      ? Number(tableState.time_bank_remaining_secs[0])
      : 0
  );
  const actionTimeout = $derived(
    usingTimeBank
      ? Number(tableState?.config?.time_bank_secs ?? 30)
      : Number(tableState?.config?.action_timeout_secs ?? 60)
  );
  const clockFraction = $derived(
    timeRemaining === null || !actionTimeout
      ? 0
      : Math.max(0, Math.min(1, timeRemaining / actionTimeout))
  );
  const clockUrgent = $derived(timeRemaining !== null && timeRemaining <= 10);

  $effect(() => {
    if (timeRemaining === null || !isMyTurn || timeRemaining > 10 || timeRemaining <= 0) {
      return undefined;
    }
    playSound('timer', { frequency: 800, duration: 30 });
    const id = setInterval(() => {
      if (displayedTimeRemaining > 0 && displayedTimeRemaining <= 10) {
        playSound('timer', { frequency: 800, duration: 30 });
      }
    }, 1000);
    return () => clearInterval(id);
  });

  // ---------------------------------------------------------------------------
  // 6. Action log
  // ---------------------------------------------------------------------------

  const lastAction = $derived(tableState?.last_action?.[0] || tableState?.last_action);

  const ACTION_VERBS = {
    Fold: ['folded', 'fold', null],
    Check: ['checked', 'check', null],
    Call: ['called', 'call', 'amount'],
    Bet: ['bet', 'bet', 'amount'],
    Raise: ['raised to', 'raise', 'amount'],
    AllIn: ['went ALL IN', 'allin', 'amount'],
    PostBlind: ['posted blind', 'blind', 'amount']
  };

  function formatLastAction(info) {
    if (!info?.action) return null;
    const type = Object.keys(info.action)[0];
    const spec = ACTION_VERBS[type];
    if (!spec) return null;
    const data = info.action[type];
    return {
      player: info.seat === mySeat ? 'You' : seatLabel(info.seat),
      action: spec[0],
      type: spec[1],
      amount: spec[2] ? (data?.amount ?? data) : null
    };
  }

  let actionFeed = $state([]);
  let previousActionFeed = $state([]);
  let previousHandNumber = $state(0);
  let lastTrackedAction = $state(null);
  let lastTrackedPhase = $state(null);
  let lastTrackedHandNumber = $state(0);

  $effect(() => {
    if (handNumber !== lastTrackedHandNumber && handNumber > 0) {
      if (actionFeed.length > 0) {
        previousActionFeed = [...actionFeed];
        previousHandNumber = lastTrackedHandNumber;
      }
      actionFeed = [];
      lastTrackedAction = null;
      lastTrackedPhase = null;
      lastTrackedHandNumber = handNumber;
    }
  });

  const PHASE_NAMES = {
    PreFlop: 'Pre-Flop', Flop: 'Flop', Turn: 'Turn', River: 'River',
    Showdown: 'Showdown', HandComplete: 'Hand Complete'
  };

  $effect(() => {
    if (phaseKey && phaseKey !== lastTrackedPhase && phaseKey !== 'WaitingForPlayers') {
      if (PHASE_NAMES[phaseKey] && lastTrackedPhase !== null) {
        actionFeed = [...actionFeed, {
          type: 'phase', text: PHASE_NAMES[phaseKey], timestamp: Date.now()
        }];
      }
      lastTrackedPhase = phaseKey;
    }
  });

  /**
   * Identity of a `last_action` record, as a string.
   *
   * WHY NOT `JSON.stringify(lastAction.action)`. `action` is a Candid variant
   * whose payload carries `nat64` amounts, which the agent decodes to `BigInt`.
   * `JSON.stringify` THROWS on a BigInt ("Do not know how to serialize a
   * BigInt"), and this runs inside an `$effect`, so the throw is uncaught: the
   * effect dies and every later action stops reaching `actionFeed`. Measured on
   * the real canisters, one ordinary heads-up hand at table_2: 19 uncaught
   * TypeErrors, an empty action log, and the fairness panel and hand-history
   * list starved of the same reactive pass. See docs/DEFECTS.md T-10.
   *
   * The variant tag plus its amount is all the identity this key needs. Every
   * piece of it is turned into text by `String()` or by `candidKey()`, both of
   * which are total on BigInt. Nothing here can throw, whatever shape a future
   * variant arm takes.
   */
  function actionKey(a) {
    const tag = a && a.action ? Object.keys(a.action)[0] : 'none';
    const payload = a && a.action ? a.action[tag] : null;
    let detail = '';
    if (payload && typeof payload === 'object') {
      // The common arms carry `{ amount }`; anything else falls back to the
      // BigInt-safe serialiser rather than collapsing to the empty string,
      // which would make two different payloads look like the same action.
      detail = 'amount' in payload ? String(payload.amount) : candidKey(payload);
    } else if (payload !== null && payload !== undefined) {
      detail = String(payload);
    }
    return `${a.seat}-${a.timestamp}-${tag}-${detail}`;
  }

  $effect(() => {
    if (!lastAction) return;
    const key = actionKey(lastAction);
    if (key === lastTrackedAction) return;
    const formatted = formatLastAction(lastAction);
    if (formatted) {
      actionFeed = [...actionFeed, {
        type: formatted.type,
        seat: lastAction.seat,
        text: formatted.action,
        amount: formatted.amount,
        timestamp: Date.now()
      }];
    }
    lastTrackedAction = key;
  });

  /**
   * SHOWDOWN LINES. Every hand the engine turned up, named in words, in the log
   * -- so the reason the pot went where it went survives the four seconds the
   * winner glow lasts. The names come from $lib/equity.js's `describeHand`, the
   * same function that writes the felt's own readout, so the log and the felt
   * cannot say different things about the same five cards.
   *
   * WORDS, DELIBERATELY, NO PERCENTAGES. A number in the log is a number the
   * screenshot gate has to account for, and the equity already has one home on
   * the felt with its method printed beside it. Repeating it here would put a
   * second, unasserted copy of it on the screen.
   */
  $effect(() => {
    if (!isShowdown || communityCards.length < 3) return;
    if (actionFeed.some(a => a.type === 'showdown')) return;
    const lines = liveSeats
      .map(seat => {
        const hole = seat === mySeat ? (myCards || []) : revealedHole(players[seat]);
        if (!Array.isArray(hole) || hole.length !== 2) return null;
        const codes = cardCodes([...hole, ...communityCards.slice(0, 5)]);
        const named = codes ? describeHand(codes)?.name : null;
        return named ? { type: 'showdown', seat, text: `shows ${named}`, timestamp: Date.now() } : null;
      })
      .filter(Boolean);
    if (lines.length > 0) actionFeed = [...actionFeed, ...lines];
  });

  $effect(() => {
    if (!isHandComplete || lastWinners.length === 0) return;
    if (actionFeed.some(a => a.type === 'winner')) return;
    actionFeed = [
      ...actionFeed,
      ...lastWinners.map(w => ({
        type: 'winner', seat: w.seat, text: 'won', amount: w.amount, timestamp: Date.now()
      }))
    ];
  });

  // ---------------------------------------------------------------------------
  // 7. Bet sizing
  // ---------------------------------------------------------------------------

  let raiseAmount = $state(0);
  let showRaiseSlider = $state(false);
  let initializedForTurn = $state(false);

  $effect(() => {
    if (!isMyTurn) {
      initializedForTurn = false;
      showRaiseSlider = false;
    }
  });

  $effect(() => {
    if (isMyTurn && gameInProgress && !initializedForTurn) {
      initializedForTurn = true;
      raiseAmount = canCheck ? minBet : currentBet + minRaise;
    }
  });

  const raiseFloor = $derived(currentBet === 0 ? minBet : currentBet + minRaise);

  /**
   * Pot-fraction presets. A "pot-sized raise" is not "raise TO the pot": it is
   * call, then raise by the pot as it stands AFTER that call. Writing it as one
   * formula makes the no-bet case fall out for free, because callAmount and
   * currentBet are both 0 there and it reduces to "bet half / all of the pot".
   */
  function setBetPreset(kind) {
    const floor = raiseFloor;
    const potAfterCall = totalPot + callAmount;
    let target = floor;
    if (kind === 'half') target = currentBet + Math.floor(potAfterCall / 2);
    else if (kind === 'pot') target = currentBet + potAfterCall;
    else if (kind === 'allin') target = maxBetAmount;
    raiseAmount = Math.min(Math.max(target, floor), maxBetAmount);
  }

  function commitRaise() {
    if (raiseAmount <= 0) return;
    onAction(currentBet === 0 ? 'bet' : 'raise', raiseAmount);
    showRaiseSlider = false;
  }

  function potOdds() {
    if (callAmount <= 0 || totalPot <= 0) return null;
    const ratio = totalPot / callAmount;
    return ratio >= 1 ? `${ratio.toFixed(1)}:1` : `1:${(1 / ratio).toFixed(1)}`;
  }

  function equityNeeded() {
    if (callAmount <= 0 || totalPot <= 0) return null;
    return ((callAmount / (totalPot + callAmount)) * 100).toFixed(0);
  }

  // ---------------------------------------------------------------------------
  // 8. Player identity
  // ---------------------------------------------------------------------------

  function seatLabel(i) {
    return `Seat ${Number(i) + 1}`;
  }

  function getAvatarUrl(player, seatIndex = null) {
    if (!player?.principal) return null;
    const principalStr = player.principal.toString();
    const style = (seatIndex === mySeat) ? avatarStyle : 'bottts';
    return `https://api.dicebear.com/7.x/${style}/svg?seed=${encodeURIComponent(principalStr)}&size=68&scale=110&radius=50`;
  }

  const ADJECTIVES = ['Lucky', 'Wild', 'Cool', 'Sly', 'Bold', 'Swift', 'Clever', 'Daring', 'Epic', 'Mystic', 'Royal', 'Shadow', 'Golden', 'Silver', 'Cosmic'];
  const NOUNS = ['Ace', 'King', 'Queen', 'Jack', 'Joker', 'Shark', 'Whale', 'Fox', 'Wolf', 'Tiger', 'Eagle', 'Hawk', 'Viper', 'Dragon', 'Phoenix'];

  function getPlayerName(player) {
    if (!player?.principal) return 'Unknown';
    const s = player.principal.toString();
    let hash = 0;
    for (let i = 0; i < s.length; i += 1) {
      hash = ((hash << 5) - hash) + s.charCodeAt(i);
      hash &= hash;
    }
    hash = Math.abs(hash);
    return `${ADJECTIVES[hash % ADJECTIVES.length]}${NOUNS[(hash >> 8) % NOUNS.length]}${(hash % 100).toString().padStart(2, '0')}`;
  }

  function getShortName(player, seatIndex) {
    if (seatIndex === mySeat) return customName || 'You';
    if (!player?.principal) return seatLabel(seatIndex);
    const name = player.display_name || getPlayerName(player);
    return name.length > 11 ? name.slice(0, 11) : name;
  }

  function winInfoFor(seat) {
    return lastWinners.find(w => Number(w.seat) === seat) || null;
  }

  /**
   * A revealed opponent's two cards -- docs/DEFECTS.md T-09.
   *
   * The canister declares `hole_cards : opt record { Card; Card }`
   * (`Option<(Card, Card)>` in src/table_canister/src/lib.rs:430), which crosses
   * the wire as `[] | [[Card, Card]]`. So `hole_cards[0]` is the WHOLE PAIR and
   * `hole_cards[1]` is `undefined`. Passing those two straight to <Card> is why
   * the villain's revealed hand rendered as one blank white rectangle and one
   * empty slot at every showdown -- the data was there the whole time; the
   * option was never unwrapped.
   *
   * @returns {[object, object]|null}
   */
  function revealedHole(player) {
    const opt = player?.hole_cards;
    if (!Array.isArray(opt) || opt.length === 0) return null;
    const pair = opt[0];
    if (Array.isArray(pair) && pair.length >= 2 && pair[0]?.rank) {
      return [pair[0], pair[1]];
    }
    // Tolerate a flattened `vec Card` shape too, so a Candid change degrades to
    // a correct render instead of silently going back to two blanks.
    if (opt.length >= 2 && opt[0]?.rank && opt[1]?.rank) return [opt[0], opt[1]];
    return null;
  }

  function handRankWords(handRank) {
    if (!handRank) return '';
    const raw = Array.isArray(handRank) ? handRank[0] : handRank;
    return humanise(variantKey(raw));
  }

  // ---------------------------------------------------------------------------
  // 9. Panels
  // ---------------------------------------------------------------------------

  let walletCollapsed = $state(
    typeof localStorage !== 'undefined' && localStorage.getItem('poker_wallet_collapsed') === 'true'
  );
  let logOpen = $state(false);

  function toggleWalletPanel() {
    walletCollapsed = !walletCollapsed;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('poker_wallet_collapsed', walletCollapsed.toString());
    }
  }

  // ---------------------------------------------------------------------------
  // 10. The two things that MOVE: chips to the pot, and the pot to the winner
  // ---------------------------------------------------------------------------
  //
  // docs/DESIGN-BAR.md bar 11: 500 ms for anything that moves an object, and
  // that is the whole budget here. Both flights are TRANSFORM-ONLY ghosts
  // rendered on top of the felt -- no real element ever moves, so the geometry
  // that §2.2 measures cannot be disturbed by an animation frame, and a capture
  // taken mid-flight still measures the same table.
  //
  // The ghosts also mean the client never has to lie about state: the bet discs
  // are gone from the DOM the instant the canister zeroes `current_bet`; what
  // flies to the middle is a copy of what was just there, carrying the amount
  // the chain last reported for it.

  /**
   * THE BEAT, in milliseconds. Bar 11 sets 500 ms for anything that moves an
   * object, and no single element here moves for longer than that. They are
   * SEQUENCED rather than simultaneous, because the order is the story:
   *
   *      0 -  500   the street's chips travel from the seats to the middle
   *    360 -  860   the pot travels from the middle to the winner
   *    760 - 1180   the `+X` award lands on the winner's pod
   *      0 -  440   the revealed pairs flip up (100 ms stagger per card)
   *
   * The 4 s winner glow (bar 13) runs underneath all of it and is the only
   * thing on the table allowed to last longer than a second.
   */
  const FLIGHT_MS = 500;
  /** Matches `.pot-flight`'s animation-delay; keeps the ghost alive long enough. */
  const POT_FLIGHT_DELAY_MS = 360;

  let sweepingChips = $state([]);   // [{ seat, amount }] flying to the pot
  let potFlight = $state([]);       // [{ seat, amount, cs, sn }] flying to a winner
  let lastStreetBets = [];          // plain: last non-zero per-seat bets
  /**
   * Whether this client WATCHED the current hand being played.
   *
   * The pot flight is a claim about time: "that pot just moved, in front of
   * you". Firing it for a hand that finished before the page was open is a
   * false claim, and it is not hypothetical -- the frame probe caught it. A
   * page opened on a table showing a completed hand flew that old pot on
   * connect, and because the flight was de-duplicated on `hand_number` alone
   * (which restarts at 1 after a table reset), the hand the viewer actually
   * watched end was then treated as already flown and did NOT animate. Exactly
   * backwards: an old payout replayed, the real one did not.
   */
  let sawHandInProgress = false;

  /**
   * A GHOST'S LIFETIME BELONGS TO THE GHOST.
   *
   * Both flights used to be un-mounted by a `setTimeout` returned as the
   * effect's teardown, which reads naturally and is wrong twice over: Svelte
   * runs the previous teardown before EVERY re-run, and both effects read
   * `players`, which is a fresh array on every poll — so the timer that removes
   * a ghost was liable to be cancelled by the next poll. The frame probe in
   * $SCRATCH caught it: `chipFlights: 2` still in the DOM a full second after
   * the flight had ended. Invisible (the animation fills to `opacity: 0`), but
   * a stale keyed node means the NEXT sweep reuses it and never replays.
   *
   * A flight now ends when its own animation ends, which is the only event that
   * actually knows. The timer stays as a backstop for the case where
   * `animationend` cannot fire at all, and neither is tied to an effect's life.
   */
  let sweepTimer = null;
  let potTimer = null;
  $effect(() => () => { clearTimeout(sweepTimer); clearTimeout(potTimer); });

  const dropSweep = (seat) => { sweepingChips = sweepingChips.filter(g => g.seat !== seat); };
  const dropPotFlight = (seat) => { potFlight = potFlight.filter(f => f.seat !== seat); };

  /**
   * `prefers-reduced-motion`, read from the platform.
   *
   * The CSS already hides both flights under the query, but a ghost that is
   * only hidden is still a node that has to be cleaned up by an event that will
   * never fire, because a `display: none` element does not animate. When the
   * viewer has asked for less motion the ghosts are simply never created, and
   * the CSS rule stays as the second line of defence.
   */
  let reducedMotion = $state(false);
  $effect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return undefined;
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    reducedMotion = mq.matches;
    const onChange = () => { reducedMotion = mq.matches; };
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  });

  $effect(() => {
    const bets = players.map(p => (p ? Number(p.current_bet ?? 0) : 0));
    const total = bets.reduce((a, b) => a + b, 0);
    if (total > 0) {
      lastStreetBets = bets;
      return undefined;
    }
    // The bets just went to zero. Either a street was collected mid-hand, or
    // the hand ended -- and BOTH are a sweep. Gating this on `gameInProgress`
    // alone meant the one moment the player most wants to see chips move, the
    // end of an all-in hand, was the one moment nothing moved: the canister
    // zeroes `current_bet` and sets HandComplete in the same poll, so the
    // sweep was skipped and the pot appeared to teleport. Measured with the
    // frame probe in $SCRATCH: `chipFlights: 0` across the whole transition.
    if ((!gameInProgress && !isHandComplete) || !lastStreetBets.some(b => b > 0)) return undefined;
    const swept = lastStreetBets.map((amount, seat) => ({ seat, amount })).filter(g => g.amount > 0);
    lastStreetBets = [];
    if (reducedMotion) return undefined;
    sweepingChips = swept;
    clearTimeout(sweepTimer);
    sweepTimer = setTimeout(() => { sweepingChips = []; }, FLIGHT_MS + 600);
    return undefined;
  });

  $effect(() => {
    if (gameInProgress) { sawHandInProgress = true; return undefined; }
    if (!isHandComplete || lastWinners.length === 0) return undefined;
    // Only ever once per watched hand, and never for a hand that ended before
    // this client was looking at it.
    if (!sawHandInProgress) return undefined;
    sawHandInProgress = false;
    if (reducedMotion) return undefined;
    const ring = seatPoints;
    potFlight = lastWinners.map(w => {
      const seat = Number(w.seat);
      const point = ring[seat] ?? { cs: 0, sn: 1 };
      return { seat, amount: Number(w.amount), cs: point.cs, sn: point.sn };
    });
    clearTimeout(potTimer);
    potTimer = setTimeout(() => { potFlight = []; }, POT_FLIGHT_DELAY_MS + FLIGHT_MS + 600);
    return undefined;
  });
</script>

<!-- `ring-<n>` is a breakpoint on SEAT COUNT, not on viewport width, so it does
     not violate bar 19: the table is still one rigid object scaled from one
     input, but nine plates share the same rail as two and cannot be the same
     size. Nine seats put ~0.27 felt widths of arc between neighbours, and a
     0.235-wide plate plus a pair of hole cards does not fit in it. -->
<div
  class="poker-table-wrapper"
  class:portrait
  class:ring-crowded={seatCount >= 8}
  class:ring-sparse={seatCount <= 3}
  bind:this={wrapperEl}
  style:--cd-avail="{availPx}px"
  style:--cd-slack="{parentSlack}px"
>
  <div class="poker-table">
    <!-- ===================== the stage ===================== -->
    <div class="stage">
      <div
        class="table-inner"
        class:all-in-moment={allInMoment}
        class:showdown={isShowdown}
      >
        <!-- felt: the LAYOUT BOX is the visible green surface. The rail is
             drawn outside it with box-shadow rings so it costs no layout
             height, which is what lets the surface stay wide in a short
             viewport. -->
        <div class="felt">
          <div class="felt-marks" aria-hidden="true">
            <span class="mark-1">TEXAS HOLD'EM</span>
            <span class="mark-2">100% ON-CHAIN &middot; NO RAKE</span>
          </div>
        </div>

        <!-- board + pot cluster -->
        <!-- THE METHOD TRAVELS WITH THE FIGURE (docs/DEFECTS.md T-27).
             `.equity-method` used to be rendered inside `.pot-display` only, and
             the pot display is REPLACED by the winner banner the moment the pot
             hits zero -- which is exactly the frame where the equity becomes a
             verdict. Measured at the showdown on both viewports: two solid
             `100.00% / 0.00%` badges on screen and `.equity-method` null, with the
             method surviving only in a `title` attribute a phone cannot open. An
             equity figure whose method is not stated is a number a player acts on
             without knowing what it means.

             One snippet, rendered in whichever readout is on screen, so the two
             cannot drift and the line cannot be dropped by a branch again. -->
        {#snippet equityMethodLine()}
          {#if equityMethodLabel}
            <!-- Bar 15 says show the equity; honesty says show HOW. The method and
                 the trial count are on the felt, not buried, and the full statement
                 of the model is the tooltip. -->
            <div class="equity-method" title={equity?.note ?? ''}>{equityMethodLabel}</div>
          {/if}
        {/snippet}
        <div class="board-cluster">
          {#if isHandComplete && lastWinners.length > 0}
            <div class="winner-display" class:you-won={myWinInfo}>
              {#if myWinInfo}
                <span class="winner-text">You won {fmt(Number(myWinInfo.amount))} {currencySymbol}</span>
                {#if handRankWords(myWinInfo.hand_rank)}
                  <span class="winner-hand-rank">{handRankWords(myWinInfo.hand_rank)}</span>
                {/if}
              {:else}
                <span class="winner-text">
                  {seatLabel(lastWinners[0].seat)} wins {fmt(Number(lastWinners[0].amount))} {currencySymbol}
                </span>
                {#if handRankWords(lastWinners[0].hand_rank)}
                  <span class="winner-hand-rank">{handRankWords(lastWinners[0].hand_rank)}</span>
                {/if}
              {/if}
              {#if lastWinners.length > 1}
                <span class="split-info">Split pot &middot; {lastWinners.length} winners</span>
              {/if}
              <span class="phase-indicator">{streetLabel}</span>
              {@render equityMethodLine()}
            </div>
          {:else}
            <div class="pot-display">
              <div class="main-pot" class:has-chips={totalPot > 0} class:at-risk={allInMoment}>
                <span class="pot-meta">
                  <span class="pot-label">
                    {#if allInMoment}
                      All in &middot; {allInSeats.length} at risk
                    {:else}
                      Total pot
                    {/if}
                  </span>
                  <span class="phase-indicator">{streetLabel}</span>
                </span>
                <span class="pot-amount">{totalPot > 0 ? fmt(totalPot) : '--'}</span>
              </div>
              <!-- The decomposition, shown only when it says something the
                   headline does not. Both legs are chain figures and they sum
                   back to get_pot(); the harness asserts exactly that. -->
              {#if liveBets > 0}
                <div class="pot-breakdown">
                  {fmt(collectedPot)} collected + {fmt(liveBets)} betting
                </div>
              {/if}
              {#if sidePots.length > 0}
                <div class="side-pots">
                  <!-- `build_side_pots_from_contributions` returns the MAIN pot
                       at index 0 and the side pots after it, so labelling index
                       0 "Side 1" names the main pot with the wrong poker word,
                       and on a hand with a single layer it printed "SIDE 1
                       0.40" directly under "TOTAL POT 0.40". Same numbers, right
                       word. docs/DEFECTS.md T-12. -->
                  {#each sidePots as sidePot, i}
                    <div class="side-pot">
                      <span class="side-pot-label">{i === 0 ? 'Main' : `Side ${i}`}</span>
                      <span class="side-pot-amount">{fmt(sidePot.amount)}</span>
                    </div>
                  {/each}
                </div>
              {/if}
              {@render equityMethodLine()}
            </div>
          {/if}

          <!-- An undealt board slot renders EMPTY, never face-down. A face-down
               board card says "the flop is dealt and hidden from you", which on
               a provably-fair table is a claim about chain state; the canister
               has dealt exactly `communityCards.length` cards and the felt says
               so. The five slots stay in the layout so the board keeps its
               footprint and the pot does not jump when the flop lands.

               THE TRAY IS THE FIX FOR THE SEVEN-CARD ROW. At showdown the felt
               used to show `J 4  K 9 2 2 10` in one horizontal band -- an
               opponent's revealed pair butted against the community board -- and
               the hero's own pair 14 px under the board's bottom edge. Nothing
               said which five cards were the board. It is now drawn as ONE
               OBJECT on its own tray, with its own caption, and the caption row
               physically occupies the gap between the board and the hero's pair.
               `.community-cards` is still the direct parent of exactly the five
               board cards, which is the contract tools/shots/lib/dom-scrape.mjs
               reads the board by. -->
          {#if gameInProgress || isShowdown || communityCards.length > 0}
            {@const framed = allInMoment || isShowdown}
            <div class="board-frame" class:framed>
              <div class="community-cards">
                {#each Array(5) as _, i}
                  <Card card={communityCards[i] ?? null} />
                {/each}
              </div>
              {#if framed}
                <div class="board-caption">
                  <span class="caption-tag">
                    Board
                    {#if communityCards.length < 5}
                      &middot; {5 - communityCards.length} to come
                    {/if}
                  </span>
                  {#if heroHandName}
                    <!-- GGPoker's named hand-strength readout. Yours only: it is
                         a function of the two cards in your own hand and a board
                         everyone can see, so it reveals nothing. -->
                    <span class="caption-hand">Your hand &middot; {heroHandName}</span>
                  {/if}
                </div>
              {/if}
            </div>
          {/if}
        </div>

        <!-- ===================== the seat ring =====================
             Walked in CANISTER seat order (see `seatPoints`): the nth .seat in
             the DOM is seat n, whatever the rotation does on screen. -->
        {#each seatPoints as point, i}
          {@const player = players[i] ?? null}
          {@const acting = gameInProgress && i === actionOn}
          {@const isHero = i === mySeat}
          {@const win = isHandComplete ? winInfoFor(i) : null}
          {@const equityText = (allInMoment || isShowdown) ? equityFor(i) : null}
          {@const live = isInHand(player)}
          <div
            class="seat seat-{point.side}"
            class:occupied={!!player}
            class:award-on-spoke={point.awardOnSpoke}
            class:spoke-y={point.rdy !== 0}
            class:acting
            class:is-me={isHero}
            class:folded={player?.has_folded}
            class:live-hand={live && (allInMoment || isShowdown)}
            class:winner={!!win}
            style:--cs={point.cs}
            style:--sn={point.sn}
            style:--nx={point.nx}
            style:--ny={point.ny}
            style:--bx={point.bx}
            style:--by={point.by}
            style:--ax={point.ax}
            style:--ay={point.ay}
            style:--rdx={point.rdx}
            style:--rdy={point.rdy}
            style:--cy={point.cy}
          >
            {#if player}
              <!-- Hole cards. A FACE-DOWN pair only has to say "this player has
                   cards", so it tucks behind the plate. A revealed pair has to
                   be READ, so it clears the plate entirely -- at showdown the
                   winner's hand was legible only down to the suit pips. -->
              <div
                class="player-cards"
                class:hero={isHero}
                class:shown={!isHero && !!revealedHole(player)}
              >
                {#if isHero && myCards && (gameInProgress || isShowdown)}
                  <Card card={myCards[0]} />
                  <Card card={myCards[1]} />
                {:else if revealedHole(player)}
                  {@const hole = revealedHole(player)}
                  <Card card={hole[0]} />
                  <Card card={hole[1]} />
                {:else if (gameInProgress || isShowdown) && live}
                  <!-- `live`, not `!has_folded`: `start_hand` deals only to
                       Active players, so a seated-but-sitting-out player was
                       never dealt in and must not be drawn holding cards. The
                       equity model counts opponents with the SAME predicate, so
                       the number of hands drawn and the number of hands modelled
                       are the same number by construction. -->
                  <Card faceDown={true} />
                  <Card faceDown={true} />
                {/if}
              </div>

              <!-- committed chips, between the pod and the pot -->
              {#if Number(player.current_bet ?? 0) > 0}
                <div class="bet-chip" class:all-in={player.is_all_in}>
                  <span class="chip-stack" aria-hidden="true"></span>
                  <span class="bet-amount">{fmt(player.current_bet)}</span>
                </div>
              {/if}

              <!-- the pod -->
              <div
                class="player-nameplate"
                class:highlight-me={isHero}
                class:action-on={acting}
                class:is-winner={!!win}
              >
                <div class="avatar-container" class:is-me={isHero}>
                  <img src={getAvatarUrl(player, i)} alt="" class="player-avatar" />
                  {#if player.has_folded}
                    <!-- PokerNow's treatment, measured: a folded seat is a grey
                         cross plus the WORD, and the whole pod desaturates. Ours
                         dimmed the seat to 46% opacity and nothing else, which
                         at a glance is indistinguishable from an empty chair. -->
                    <div class="avatar-overlay folded"><span class="fold-x" aria-hidden="true">&#10005;</span></div>
                  {:else if player.is_all_in}
                    <div class="avatar-overlay allin">All In</div>
                  {/if}
                </div>
                <div class="pod-text">
                  <span class="player-name" class:is-me={isHero}>{getShortName(player, i)}</span>
                  <span class="stack-row">
                    <span class="chips">{fmt(player.chips)}</span>
                    {#if player.has_folded}<span class="fold-word">Fold</span>{/if}
                  </span>
                </div>
                <div class="pod-slot">
                  {#if acting && timeRemaining !== null}
                    <span class="turn-timer" class:urgent={clockUrgent}>{timeRemaining}s</span>
                  {/if}
                  <span class="position-badges">
                    {#if i === dealerSeat}<span class="position-badge dealer">D</span>{/if}
                    {#if i === smallBlindSeat}<span class="position-badge sb">SB</span>{/if}
                    {#if i === bigBlindSeat}<span class="position-badge bb">BB</span>{/if}
                  </span>
                </div>
                {#if acting}
                  <span
                    class="pod-clock"
                    class:urgent={clockUrgent}
                    style:--clock={clockFraction}
                  ></span>
                {/if}
              </div>

              {#if equityText}
                <!-- THE EQUITY BADGE IS A SEAT-LEVEL OBJECT (docs/DEFECTS.md T-22).
                     Bar 15 puts it beside the player, as PokerStars and GGPoker do,
                     and it is ADDITIVE: the action clock keeps its slot in the
                     plate, because a player deciding whether to call needs both.

                     It used to be rendered INSIDE `.player-nameplate`, which sets
                     `z-index: 6` and therefore opens a stacking context, while the
                     hero's own cards and every revealed pair paint at `z-index: 7`
                     as siblings of that plate. From in there no z-index could win:
                     on a phone the winner's `100.00%` was measured 58.4% covered by
                     the hero's own card and read as `0%`. It is now a child of the
                     SEAT, above the cards, and placed on the one axis no card of
                     this seat ever uses -- see `.equity-badge` in the stylesheet. -->
                <span
                  class="equity-badge"
                  class:modelled={equityMode === 'hero'}
                  title={equity?.note ?? ''}
                >{equityText}</span>
              {/if}

              <!-- Showdown only: the winning hand named in words (bar 16). It
                   lands on the chip spot, which is free at showdown because the
                   street's bets have already been swept into the pot -- and
                   which is the one place on the felt already proven clear of
                   this seat's cards and of the board.

                   There is deliberately no "All In" tag here any more. It was
                   placed on the far side of the plate, which for the bottom
                   seats is OFF the felt: the hero's read at y=818 on a surface
                   ending at y=753, floating over the action bar. The state is
                   already said twice by chain data that cannot drift -- the
                   avatar overlay on the player, and "All in - N at risk" on the
                   pot. -->
              {#if win}
                <!-- THE DELTA CHIP. PokerNow puts `+450` directly under the
                     winner's stack, so the player reads WHAT CHANGED at the pod
                     that received it. Ours put the amount in a banner in the
                     middle of the felt, ~300 px away, and the pod showed only
                     the post-hand stack -- the one number from which the change
                     cannot be recovered.
                     It lands on the chip spot (`bx`/`by`), which touches the
                     pod's inner edge and is the one place on the felt already
                     proven clear of this seat's cards AND of the board, at every
                     seat on the ring. The named hand rides with it, so the award
                     is one object rather than two competing tags. -->
                <div class="winner-award">
                  <span class="stack-delta">+{fmt(Number(win.amount))}</span>
                  {#if handRankWords(win.hand_rank)}
                    <span class="hand-tag">{handRankWords(win.hand_rank)}</span>
                  {/if}
                </div>
              {/if}
            {:else}
              <button class="join-seat" onclick={() => onAction('join', i)}>
                <span class="sit-word">Sit</span>
                <span class="sit-seat">{seatLabel(i)}</span>
              </button>
            {/if}
          </div>
        {/each}

        <!-- ============ the two flights (bar 11: 500 ms, transform only) ====
             GHOSTS, not the real elements. `.bet-chip` is removed from the DOM
             the moment the canister zeroes `current_bet`; what travels is a copy
             carrying the last amount the chain reported, so no figure on the
             felt is ever showing a stale number in order to animate. -->
        {#each sweepingChips as ghost (ghost.seat)}
          {@const point = seatPoints[ghost.seat]}
          {#if point}
            <div
              class="chip-flight"
              style:--cs={point.cs}
              style:--sn={point.sn}
              style:--bx={point.bx}
              style:--by={point.by}
              onanimationend={() => dropSweep(ghost.seat)}
              aria-hidden="true"
            >{fmt(ghost.amount)}</div>
          {/if}
        {/each}

        {#each potFlight as flight (flight.seat)}
          <div
            class="pot-flight"
            style:--wcs={flight.cs}
            style:--wsn={flight.sn}
            onanimationend={() => dropPotFlight(flight.seat)}
            aria-hidden="true"
          >{fmt(flight.amount)}</div>
        {/each}

        <!-- sitting-out notice sits over the surround, never over the felt -->
        {#if isSittingOut}
          <div class="sitting-out-banner">
            <span>You are sitting out</span>
            <button class="sit-in-btn" onclick={() => onAction('sitIn')}>Sit back in</button>
          </div>
        {/if}

        <!-- bet-sizing popover, anchored above the dock -->
        {#if showRaiseSlider && isMyTurn && gameInProgress}
          <div class="raise-slider-panel">
            <div class="slider-header">
              <span>{currentBet === 0 ? 'Bet' : 'Raise to'}</span>
              <button class="close-slider" onclick={() => showRaiseSlider = false} aria-label="Close">&times;</button>
            </div>
            <div class="slider-amount">{fmt(raiseAmount)}</div>
            <input
              type="range"
              class="raise-slider"
              min={raiseFloor}
              max={maxBetAmount}
              step={Math.max(1, Math.round(minRaise / 4) || 1)}
              bind:value={raiseAmount}
              aria-label="Bet amount"
            />
            <div class="preset-buttons">
              <button onclick={() => setBetPreset('half')}>&frac12; Pot</button>
              <button onclick={() => setBetPreset('pot')}>Pot</button>
              <button onclick={() => setBetPreset('allin')}>All In</button>
            </div>
            <button class="confirm-raise" onclick={commitRaise}>
              {currentBet === 0 ? 'Bet' : 'Raise to'} {fmt(raiseAmount)}
            </button>
          </div>
        {/if}

        <!-- action log: a drawer over the surround (PokerNow LOG / WPT HANDS) -->
        {#if logOpen}
          <div class="feed-container left">
            <ActionFeed
              actions={actionFeed}
              previousActions={previousActionFeed}
              mySeat={mySeat}
              handNumber={handNumber}
              previousHandNumber={previousHandNumber}
              {shuffleProof}
              {onShowProof}
              format={fmt}
            />
          </div>
        {/if}
      </div>
    </div>

    <!-- ===================== the dock ===================== -->
    <div class="action-dock">
      <div class="dock-aux dock-left">
        <button class="log-toggle" class:active={logOpen} onclick={() => logOpen = !logOpen}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M4 6h16M4 12h16M4 18h10"/>
          </svg>
          Log
        </button>
        {#if gameInProgress && mySeat !== null}
          <div class="turn-indicator" class:my-turn={isMyTurn} class:waiting={!isMyTurn} class:time-bank={usingTimeBank}>
            <span class="turn-title">{isMyTurn ? 'Your turn' : 'Waiting'}</span>
            <span class="turn-hint">
              {#if isMyTurn}
                {#if canCheck}Check or bet{:else}Call {fmt(callAmount)} or raise{/if}
              {:else}
                {getShortName(players[actionOn], actionOn)} to act
              {/if}
            </span>
          </div>
        {:else}
          <div class="turn-indicator waiting">
            <span class="turn-title">Hand #{handNumber || '--'}</span>
            <span class="turn-hint">{streetLabel}</span>
          </div>
        {/if}
      </div>

      <div class="dock-center">
        {#if isMyTurn && callAmount > 0 && potOdds()}
          <div class="pot-odds-display">
            <span class="pot-odds-label">Pot odds</span>
            <span class="pot-odds-value">{potOdds()}</span>
            <!-- Spelled out in money, not just as a ratio, so the strip and the
                 headline pot can be checked against the same get_pot() -- the
                 disagreement between the two was half of docs/DEFECTS.md T-08. -->
            <span class="pot-odds-explanation">
              Call {fmt(callAmount)} to win {fmt(totalPot)}
            </span>
            <span class="equity-hint">need {equityNeeded()}%</span>
          </div>
        {/if}
        <div class="actions" class:disabled={!isMyTurn || !gameInProgress || actionPending}>
          {#if actionPending}
            <div class="action-pending"><span class="spinner"></span><span>Processing</span></div>
          {:else if !gameInProgress}
            <div class="no-game-message">
              {phaseKey === 'HandComplete' ? 'Hand complete' : 'Waiting for players'}
            </div>
          {:else if !isMyTurn}
            <div class="not-your-turn">Waiting for {getShortName(players[actionOn], actionOn)}</div>
          {:else}
            <button class="action-btn secondary" onclick={() => onAction('fold')}>Fold</button>
            {#if canCheck}
              <button class="action-btn primary" onclick={() => onAction('check')}>Check</button>
            {:else}
              <button class="action-btn primary" onclick={() => onAction('call')}>
                Call {fmt(callAmount)}
              </button>
            {/if}
            {#if canRaise}
              <button class="action-btn raise" onclick={() => showRaiseSlider = !showRaiseSlider}>
                {currentBet === 0 ? 'Bet' : 'Raise'}
              </button>
            {/if}
            <button class="action-btn danger" onclick={() => onAction('allin')}>All In</button>
            {#if timeBankRemaining > 0 && !usingTimeBank}
              <button class="action-btn ghost" onclick={() => onAction('useTimeBank')}>
                +{timeBankRemaining}s
              </button>
            {/if}
          {/if}
        </div>
      </div>

      <div class="dock-aux dock-right" class:collapsed={walletCollapsed}>
        <button
          class="panel-toggle"
          onclick={toggleWalletPanel}
          title={walletCollapsed ? 'Show wallet' : 'Hide wallet'}
          aria-label={walletCollapsed ? 'Show wallet' : 'Hide wallet'}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="2" y="5" width="20" height="14" rx="2"/><path d="M2 10h20"/>
          </svg>
          {#if walletCollapsed}<span class="collapsed-balance">{formatWithUnit(tableBalance)}</span>{/if}
        </button>
        {#if !walletCollapsed}
          <div class="wallet-panel">
            <div class="wallet-balance">
              <span class="balance-label">Table balance</span>
              <span class="balance-value">{formatWithUnit(tableBalance)}</span>
            </div>
            <div class="wallet-actions">
              {#if onShowDeposit}
                <button class="wallet-action-btn deposit" onclick={onShowDeposit}>Deposit</button>
              {/if}
              {#if onShowWithdraw}
                <button class="wallet-action-btn withdraw" onclick={onShowWithdraw} disabled={tableBalance <= 0}>
                  Withdraw
                </button>
              {/if}
            </div>
          </div>
        {/if}
        {#if mySeat !== null}
          <div class="sit-controls">
            <button class="control-btn" onclick={() => onAction(isSittingOut ? 'sitIn' : 'sitOut')}>
              {isSittingOut ? 'Sit in' : 'Sit out'}
            </button>
            <button class="control-btn destructive" onclick={() => onAction('leave')}>Leave</button>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  /* =========================================================================
     TOKENS
     Every geometric value on the table is a ratio of --fw, the felt width.
     --fw itself is resolved once, from the stage's own box:
        width-capped at 72% of the stage, otherwise height-fitted.
     The height divisor 0.5735 is the stage height the ring needs, expressed
     in felt widths:  felt (1/2.05) + pod overhang top and bottom.
     ========================================================================= */

  .poker-table-wrapper {
    /* Felt aspect. Declared 2.10 rather than the measured target 2.05 because
       the innermost rail ring is drawn OUTSIDE the layout box and a pixel
       measurer counts it: it adds ~2 x 0.009 fw to both axes, which pulls the
       MEASURED aspect down toward 1. 2.10 declared lands ~2.06 measured, inside
       bar 2's 1.9-2.3 and next to the reference median of 2.13. */
    --ar: 2.10;

    /* ring -- pods straddle the rail, as PokerStars, GGPoker and WPT Global all
       do. The ring is deliberately a little LARGER than the felt (kx/ky > 1) so
       the pods sit on the rail rather than biting into the playing surface, and
       so the ring's circumference grows enough to seat nine pods without them
       touching. */
    --ring-kx: 1.02;
    --ring-ky: 1.00;
    --pod-w-r: 0.235;
    --pod-h-r: 0.086;
    --avatar-r: 0.062;

    /* cards -- board 11.2% of surface width, opponents 68% of a board card
       (PokerNow's exact ratio), hero 82%. The hole-card offsets are small on
       purpose: hole cards belong AT the pod (PokerNow draws them inside it), not
       out on the felt, because the felt between pod and board is where the
       committed chips have to go. */
    --card-board-r: 0.112;
    --card-hero-r: 0.092;
    --card-opp-r: 0.076;
    --board-gap-r: 0.009;
    --card-nudge-r: 0.035;
    --off-opp-r: 0.058;
    --off-shown-r: 0.090;   /* revealed: clears the plate, half plate + half card */
    --off-hero-r: 0.085;
    --cluster-dy-r: 0.012;

    /* type */
    --ui-r: 0.0165;

    --dock-h: 78px;

    display: flex;
    justify-content: center;
    width: 100%;
    height: var(--cd-avail, 620px);
    margin-bottom: calc(-1 * var(--cd-slack, 0px));
    min-width: 0;
  }

  /* 8- and 9-max: the same rail has to carry four more plates, so the plates,
     their avatars, their cards and their type all come down together. Every
     ratio below is scaled by the same ~0.86, so the seat stays one object. */
  .poker-table-wrapper.ring-crowded {
    --pod-w-r: 0.202;
    --pod-h-r: 0.074;
    --avatar-r: 0.053;
    --card-opp-r: 0.065;
    --off-opp-r: 0.049;
    --off-shown-r: 0.077;
    --ui-r: 0.0143;
  }

  /* Heads-up and 3-handed: two plates on a rail built for nine. Bigger cards
     and bigger type, because there is nothing to collide with. */
  .poker-table-wrapper.ring-sparse {
    --pod-w-r: 0.262;
    --pod-h-r: 0.094;
    --avatar-r: 0.068;
    --card-opp-r: 0.084;
    --off-opp-r: 0.064;
    --off-shown-r: 0.099;
    --ui-r: 0.0178;
  }

  .poker-table {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    min-width: 0;
    min-height: 0;
  }

  .stage {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    container-type: size;
  }

  .table-inner {
    position: absolute;
    inset: 0;
    /* THE single scale input, and the only place a viewport number appears.
       Both caps are the stage box divided by what the RING needs, not by what
       the felt needs -- the pods hang off the felt on every side, so sizing to
       the felt is how the action bar ends up below the fold.
         width  needs  ring-kx + pod-w-r          = 1.02 + 0.235 = 1.255 -> 79cqw
         height needs  ring-ky / ar + pod-h-r     = 0.476 + 0.086 = 0.562 -> 177cqh
       Both are rounded IN so the outermost plate keeps a hair of margin against
       the stage edge, and both are restated per ring density below, because a
       ring that shrank its plates has room to grow the felt. */
    --fw: min(79cqw, 177cqh);
    --fh: calc(var(--fw) / var(--ar));
    --rx: calc(var(--fw) * 0.5 * var(--ring-kx));
    --ry: calc(var(--fh) * 0.5 * var(--ring-ky));
    --pod-w: calc(var(--fw) * var(--pod-w-r));
    --pod-h: calc(var(--fw) * var(--pod-h-r));
    --avatar: calc(var(--fw) * var(--avatar-r));
    --ui: calc(var(--fw) * var(--ui-r));
    font-size: var(--ui);
  }

  /* 1.02 + 0.202 = 1.222 -> 81cqw ; 0.476 + 0.074 = 0.550 -> 181cqh */
  .ring-crowded .table-inner { --fw: min(81cqw, 181cqh); }
  /* 1.02 + 0.262 = 1.282 -> 78cqw ; 0.476 + 0.094 = 0.570 -> 175cqh */
  .ring-sparse  .table-inner { --fw: min(78cqw, 175cqh); }

  /* The all-in vignette: the surround darkens around the surface. Transform and
     opacity only, so it can never move the geometry. */
  .table-inner::after {
    content: '';
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    background: radial-gradient(ellipse 62% 62% at 50% 50%, transparent 45%, rgba(40, 8, 0, 0.5) 100%);
    transition: opacity 0.5s ease;
    z-index: 3;
  }

  .table-inner.all-in-moment::after { opacity: 1; }

  /* ---------- felt ---------- */

  .felt {
    position: absolute;
    left: 50%;
    top: 50%;
    width: var(--fw);
    height: var(--fh);
    transform: translate(-50%, -50%);
    border-radius: 50%;               /* bar 2: an ellipse, not a rectangle */
    background:
      radial-gradient(ellipse 62% 78% at 50% 34%, #1f6b45 0%, #175537 42%, #0e3623 100%);
    box-shadow:
      /* The rail, drawn OUTSIDE the layout box so it costs no height. The
         innermost ring used to be dark GREEN, which meant the painted green
         surface was ~2 x 0.9% wider and taller than the felt element and every
         pixel measurement of the felt was measuring the rail as well. The lip is
         now rail-coloured, so what a measurer calls "the felt" is the felt. */
      0 0 0 calc(var(--fw) * 0.008) #1a120a,
      0 0 0 calc(var(--fw) * 0.026) #4a2f18,
      0 0 0 calc(var(--fw) * 0.029) #6b4726,
      0 0 0 calc(var(--fw) * 0.033) #2a1a0e,
      inset 0 0 calc(var(--fw) * 0.09) rgba(0, 0, 0, 0.55),
      0 calc(var(--fw) * 0.02) calc(var(--fw) * 0.06) rgba(0, 0, 0, 0.55);
    transition: box-shadow 0.5s ease;
  }

  /* The felt is never truly empty (PokerNow ships two watermarks on theirs).
     These sit at dead centre, UNDER the board, and are set wider than the board
     on purpose so they read past its edges instead of fighting it: at 72% they
     ran straight through the hero's own hole cards. They live inside `.felt`,
     so nothing on the table can be behind them. */
  .felt-marks {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3em;
    white-space: nowrap;
    pointer-events: none;
  }

  .mark-1, .mark-2 {
    font-size: 1.15em;
    letter-spacing: 0.5em;
    text-indent: 0.5em;               /* balance the trailing letter-space */
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.09);
    font-weight: 700;
  }

  .mark-2 {
    font-size: 0.72em;
    letter-spacing: 0.42em;
    text-indent: 0.42em;
    color: rgba(120, 240, 190, 0.14);
  }

  /* the all-in moment: the surface itself reacts (bar 15 -- information, not
     fireworks; the only motion is a slow rail glow) */
  .all-in-moment .felt {
    box-shadow:
      0 0 0 calc(var(--fw) * 0.008) #1a120a,
      0 0 0 calc(var(--fw) * 0.026) #5a2a12,
      0 0 0 calc(var(--fw) * 0.029) #b4531f,
      0 0 0 calc(var(--fw) * 0.033) #2a1a0e,
      0 0 calc(var(--fw) * 0.06) calc(var(--fw) * 0.006) rgba(233, 122, 42, 0.38),
      inset 0 0 calc(var(--fw) * 0.09) rgba(0, 0, 0, 0.6);
  }

  /* ---------- board + pot ---------- */

  .board-cluster {
    position: absolute;
    left: 50%;
    top: calc(50% + var(--fw) * var(--cluster-dy-r));
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    z-index: 6;
    pointer-events: none;
  }

  .community-cards {
    display: flex;
    gap: calc(var(--fw) * var(--board-gap-r));
    --card-w: calc(var(--fw) * var(--card-board-r));
  }

  /* ---------- the board frame: what stops a seven-card row ----------
     Only drawn at the all-in and the showdown, which are the two moments a
     revealed pair sits next to the community cards. On every other scene the
     board is unchanged, geometry included -- an unframed `.board-frame` is a
     zero-cost wrapper with no padding, no background and no caption. */

  .board-frame {
    display: flex;
    flex-direction: column;
    align-items: center;
    border-radius: calc(var(--fw) * 0.018);
  }

  .board-frame.framed {
    gap: calc(var(--fw) * 0.010);
    padding: calc(var(--fw) * 0.013) calc(var(--fw) * 0.016) calc(var(--fw) * 0.010);
    background: rgba(3, 12, 8, 0.42);
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.10),
      inset 0 calc(var(--fw) * 0.004) calc(var(--fw) * 0.02) rgba(0, 0, 0, 0.4);
    backdrop-filter: blur(2px);
  }

  /* The caption row is not decoration: it physically occupies the gap between
     the last board card and the hero's own pair, which used to be 14 px of bare
     felt, and it NAMES both sides of that gap. */
  .board-caption {
    display: flex;
    align-items: baseline;
    justify-content: center;
    gap: 0.9em;
    white-space: nowrap;
  }

  .caption-tag {
    font-size: 0.62em;
    font-weight: 800;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.42);
  }

  .caption-hand {
    font-size: 0.74em;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: #9ef0c8;
  }

  .showdown .board-frame.framed {
    background: rgba(3, 12, 8, 0.58);
    box-shadow:
      inset 0 0 0 1px rgba(233, 255, 99, 0.20),
      inset 0 calc(var(--fw) * 0.004) calc(var(--fw) * 0.02) rgba(0, 0, 0, 0.45);
  }

  .pot-display {
    position: absolute;
    bottom: calc(100% + var(--fw) * 0.012);
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: calc(var(--fw) * 0.006);
  }

  .main-pot {
    display: flex;
    align-items: baseline;
    gap: 0.55em;
    padding: 0.22em 0.85em;
    border-radius: 999px;
    background: rgba(6, 14, 11, 0.78);
    border: 1px solid rgba(255, 255, 255, 0.09);
    backdrop-filter: blur(6px);
    white-space: nowrap;
  }

  .pot-meta {
    display: flex;
    flex-direction: column;
    line-height: 1.1;
  }

  .pot-label {
    font-size: 0.62em;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.45);
  }

  .phase-indicator {
    font-size: 0.72em;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #7ee2b8;
    text-transform: uppercase;
  }

  .pot-amount {
    font-size: 1.55em;
    font-weight: 800;
    letter-spacing: -0.01em;
    color: #f8d97a;
    font-variant-numeric: tabular-nums;
  }

  .main-pot.has-chips {
    border-color: rgba(248, 217, 122, 0.28);
    box-shadow: 0 0 calc(var(--fw) * 0.03) rgba(248, 217, 122, 0.16);
  }

  /* No money in the middle yet. The readout stays on the felt -- the harness
     reads `.pot-amount` on every non-complete hand and a missing one is a
     failed scrape, not a clean table -- but it stops competing with the seats. */
  .main-pot:not(.has-chips) {
    opacity: 0.62;
    background: rgba(6, 14, 11, 0.55);
  }

  /* The all-in moment reads on the pot itself: the figure at risk grows and
     turns to the all-in accent. Transform only, so nothing reflows. */
  .main-pot {
    transition: transform 0.5s cubic-bezier(0.34, 1.3, 0.64, 1),
                border-color 0.5s ease, box-shadow 0.5s ease;
  }

  .main-pot.at-risk {
    transform: scale(1.07);
    background: rgba(32, 10, 2, 0.86);
    border-color: rgba(233, 122, 42, 0.6);
    box-shadow: 0 0 calc(var(--fw) * 0.045) rgba(233, 122, 42, 0.3);
  }

  .main-pot.at-risk .pot-label { color: #ff9f5a; letter-spacing: 0.1em; }
  .main-pot.at-risk .phase-indicator { color: rgba(255, 210, 180, 0.85); }
  .main-pot.at-risk .pot-amount { color: #ffb27a; }

  /* The decomposition of the headline, one type size down and dimmer, so it
     reads as a footnote to the pot rather than a second pot. */
  .pot-breakdown {
    font-size: 0.66em;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.5);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* THE METHOD, ON THE FELT. A percentage with no method behind it is the same
     class of claim as a pot with no chain behind it. This line says which
     computation produced the badges and over how many runouts or trials; the
     `title` carries the full statement, including the modelling assumption in
     the hero case. */
  .equity-method {
    pointer-events: auto;
    cursor: help;
    font-size: 0.6em;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.44);
    white-space: nowrap;
  }

  .side-pots {
    display: flex;
    gap: calc(var(--fw) * 0.008);
  }

  .side-pot {
    display: flex;
    align-items: center;
    gap: 0.4em;
    padding: 0.1em 0.55em;
    border-radius: 999px;
    background: rgba(6, 14, 11, 0.7);
    border: 1px solid rgba(255, 255, 255, 0.08);
    white-space: nowrap;
  }

  .side-pot-label {
    font-size: 0.6em;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.45);
  }

  .side-pot-amount {
    font-size: 0.82em;
    font-weight: 700;
    color: #e9d79f;
    font-variant-numeric: tabular-nums;
  }

  /* ---------- winner ---------- */

  .winner-display {
    position: absolute;
    bottom: calc(100% + var(--fw) * 0.012);
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.1em;
    padding: 0.3em 1.1em;
    border-radius: 999px;
    background: rgba(8, 18, 12, 0.86);
    border: 1px solid rgba(233, 255, 99, 0.35);
    box-shadow: 0 0 calc(var(--fw) * 0.05) rgba(233, 255, 99, 0.22);
    white-space: nowrap;
  }

  .winner-text {
    font-size: 1.1em;
    font-weight: 800;
    color: #E9FF63;              /* bar 13: the reference celebration accent */
  }

  .winner-display:not(.you-won) .winner-text { color: #f3f6df; }

  /* THE METHOD LINE INSIDE THE WINNER BANNER (docs/DEFECTS.md T-27).
     Under the pot it is one more row of a column, which is where it has always
     been. The winner banner is a rounded pill whose height is tuned to a 166 px
     slot in portrait, so the line hangs OFF the pill instead of growing it, on
     the side that faces away from the board: above the banner in landscape (the
     banner sits above the board there) and below it in portrait (where the
     readout is under the board). It is the same element, the same words and the
     same `title` in both cases -- only the anchor differs. */
  .winner-display .equity-method {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 0.25em);
    transform: translateX(-50%);
  }

  .winner-hand-rank {
    font-size: 0.72em;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.62);
  }

  .split-info {
    font-size: 0.66em;
    color: rgba(255, 255, 255, 0.5);
  }

  /* =========================================================================
     SEATS -- each seat is a zero-size point on the ring; pod, cards, chips and
     tag are positioned off it with the seat's own inward normal (--nx/--ny).
     ========================================================================= */

  .seat {
    /* --sx / --sy MUST be declared here, not on .table-inner: custom properties
       inherit as COMPUTED values, so a formula written on the parent would be
       frozen with --cs / --sn unset and every seat would land on the centre. */
    --sx: calc(var(--rx) * var(--cs, 0));
    --sy: calc(var(--ry) * var(--sn, 0));
    position: absolute;
    left: calc(50% + var(--sx));
    top: calc(50% + var(--sy));
    width: 0;
    height: 0;
    z-index: 10;
    user-select: none;
  }

  .seat.acting { z-index: 22; }
  .seat.winner { z-index: 24; }

  /* A FOLDED SEAT IS OUT, AND HAS TO LOOK OUT.
     PokerNow greys the whole pod, replaces the avatar with a cross and prints
     the word FOLD under the name, so that at a glance only the live hands are
     lit. Ours dropped the seat to 46% opacity and did nothing else -- which on
     a dark table is not a signal, it is a slightly dimmer identical pod, and
     the wave-3 critic read it as "ours dims nobody".
     Opacity is kept low AND the colour is drained, because either one alone is
     ambiguous with a seat that is merely far from the light. */
  .seat.folded { opacity: 0.42; }
  .seat.folded .player-nameplate {
    filter: grayscale(1) contrast(0.85);
    border-color: rgba(255, 255, 255, 0.06);
    box-shadow: none;
  }
  .seat.folded .bet-chip { filter: grayscale(1); opacity: 0.6; }

  /* ...and a live hand at the all-in or the showdown is LIT, so the contrast is
     carried by both ends and not only by the folded one. */
  .seat.live-hand .player-nameplate {
    border-color: rgba(255, 255, 255, 0.34);
    box-shadow:
      0 0 calc(var(--fw) * 0.022) rgba(255, 255, 255, 0.12),
      0 calc(var(--fw) * 0.006) calc(var(--fw) * 0.018) rgba(0, 0, 0, 0.55);
  }
  .seat.live-hand.is-me .player-nameplate { border-color: rgba(126, 226, 184, 0.85); }

  /* ---- pod ---- */

  .player-nameplate {
    position: absolute;
    left: 0;
    top: 0;
    /* ABOVE the hole cards. `.player-cards` opens a stacking context at z-index
       4, so a pod with `z-index: auto` painted UNDERNEATH its own cards: at any
       seat whose cards leaned back over the plate the name and the stack were
       simply covered up, which is why seat 1 read "Naka / 11.9" with the rest
       of the name and the last digits of the stack missing. */
    z-index: 6;
    transform: translate(-50%, -50%);
    width: var(--pod-w);
    height: var(--pod-h);
    display: flex;
    align-items: center;
    gap: 0.45em;
    padding: 0 0.5em 0 calc(var(--pod-h) * 0.09);
    border-radius: 999px;
    background: linear-gradient(180deg, rgba(38, 40, 48, 0.95), rgba(20, 21, 27, 0.96));
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 0 calc(var(--fw) * 0.006) calc(var(--fw) * 0.018) rgba(0, 0, 0, 0.55);
    overflow: hidden;
    /* bar 12: the acting indicator lands in <=400 ms */
    transition: border-color 0.4s ease, box-shadow 0.4s ease, background 0.4s ease;
  }

  /* YOUR plate is a PLATE. It used to be filled with felt green at 0.96 alpha,
     which meant your own seat dissolved into the table it was sitting on -- and
     a pixel measurer agreed: the detected "felt" ran 48px past the bottom of the
     surface, through the hero's nameplate, and reported the table as 1.92:1
     instead of 2.12:1 on every scene where the hero is not holding cards over
     the plate. Same charcoal family as every other seat, opaque so the felt
     cannot bleed through it, and the "this is you" signal carried where it
     belongs: the edge, the avatar ring and the name. */
  .player-nameplate.highlight-me {
    background: linear-gradient(180deg, #262832, #14151b);
    border-color: rgba(126, 226, 184, 0.75);
  }

  .player-nameplate.action-on {
    border-color: rgba(255, 255, 255, 0.85);
    box-shadow:
      0 0 calc(var(--fw) * 0.035) rgba(255, 255, 255, 0.42),
      0 calc(var(--fw) * 0.006) calc(var(--fw) * 0.018) rgba(0, 0, 0, 0.55);
  }

  .player-nameplate.is-winner {
    border-color: #E9FF63;
    animation: winner-glow 4s cubic-bezier(0.4, 0, 0.2, 1) 1 both;
  }

  /* bar 13: one long beat, three phases, single bright accent */
  @keyframes winner-glow {
    0%   { box-shadow: 0 0 0 rgba(233, 255, 99, 0); }
    25%  { box-shadow: 0 0 calc(var(--fw) * 0.09) rgba(233, 255, 99, 0.95); }
    50%  { box-shadow: 0 0 calc(var(--fw) * 0.035) rgba(233, 255, 99, 0.55); }
    75%  { box-shadow: 0 0 calc(var(--fw) * 0.075) rgba(233, 255, 99, 0.8); }
    100% { box-shadow: 0 0 calc(var(--fw) * 0.03) rgba(233, 255, 99, 0.45); }
  }

  .avatar-container {
    position: relative;
    flex: 0 0 auto;
    width: var(--avatar);
    height: var(--avatar);
    border-radius: 50%;
    overflow: hidden;
    background: #2a2d36;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
  }

  .avatar-container.is-me { box-shadow: inset 0 0 0 2px rgba(73, 161, 110, 0.85); }

  .player-avatar { width: 100%; height: 100%; display: block; }

  .avatar-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.52em;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    text-align: center;
    line-height: 1;
  }

  .avatar-overlay.folded { background: rgba(10, 10, 12, 0.88); color: rgba(255, 255, 255, 0.5); }

  .fold-x {
    font-size: 2.1em;
    font-weight: 400;
    line-height: 1;
    color: rgba(255, 255, 255, 0.42);
  }

  /* The WORD, on the stack line, exactly where PokerNow puts it. It is a SIBLING
     of `.chips`, never inside it: `.chips` is a money figure the screenshot gate
     reads by `textContent` and asserts against the canister, so nothing may be
     appended to that element. The row keeps the pod at two lines, which is all
     the height a portrait pod has. */
  .stack-row {
    display: flex;
    align-items: baseline;
    gap: 0.45em;
    min-width: 0;
  }

  .fold-word {
    font-size: 0.6em;
    font-weight: 800;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.45);
    line-height: 1;
  }

  .avatar-overlay.allin {
    background: rgba(178, 43, 12, 0.9);
    color: #fff;
    box-shadow: inset 0 0 0 2px rgba(255, 159, 90, 0.85);
  }

  .pod-text {
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-width: 0;
    flex: 1 1 auto;
    line-height: 1.15;
  }

  .player-name {
    font-size: 0.82em;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.78);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .player-name.is-me { color: #7ee2b8; }

  .chips {
    font-size: 0.92em;
    font-weight: 800;
    color: #fff;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .pod-slot {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.15em;
    min-width: 2.3em;
  }

  /* THE EQUITY BADGE (bar 15). Two visually distinct things, because they are
     two epistemically distinct things:
       solid mint   -- computed over hands the ENGINE revealed. A fact.
       outlined     -- computed against opponents drawn at random because the
                       canister has not revealed them. A model, and it is dashed
                       and carries the `vs N random` method line beside the pot
                       or the winner banner so it can never be mistaken for the
                       first kind.

     WHERE IT SITS, AND WHY IT IS NOT IN THE PLATE (docs/DEFECTS.md T-22).
     A badge inside `.player-nameplate` is inside a stacking context (`z-index:
     6`) that the hole cards (`z-index: 7`) beat from outside, at any z-index the
     badge picks. So it is a child of `.seat`, painted above the cards -- and to
     avoid trading one occlusion for another it hangs off the plate on this
     seat's READOUT SPOKE (`--rdx`/`--rdy`, computed per seat and per orientation
     in `ringSeats`): the one side of the plate that neither this seat's cards nor
     its bet disc nor the board nor the pot readout claims.

     The badge no longer costs the pod any width either: `.pod-slot.with-equity`
     used to reserve 3.5em inside a 116 px phone pod, which is width the STACK
     figure needed. */
  .equity-badge {
    position: absolute;
    left: 0;
    top: 0;
    /* Above `.player-cards.hero` and `.player-cards.shown` (both z-index 7) and
       above the plate (6). Below `.winner-award` (20), which never lands here. */
    z-index: 9;
    --badge-dx: calc(var(--rdx, 0) * (var(--pod-w) * 0.5 + 1.55em));
    --badge-dy: calc(var(--rdy, 0) * (var(--pod-h) * 0.5 + 0.8em));
    transform:
      translate(-50%, -50%)
      translate(var(--badge-dx), var(--badge-dy));
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.08em 0.34em;
    border-radius: 0.35em;
    font-size: 0.68em;
    font-weight: 800;
    line-height: 1.25;
    letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums;
    background: #7ee2b8;
    color: #06231a;
    white-space: nowrap;
    cursor: help;
    /* On the felt now rather than on a charcoal plate, so it carries its own
       edge. Costs nothing where it overlaps the plate. */
    box-shadow: 0 0 0 2px rgba(9, 11, 15, 0.85);
  }

  /* A seat whose spoke is VERTICAL carries both readouts on the same edge of the
     plate, so they take opposite ENDS of it: the badge on the inner end, the
     award on the outer. Only portrait produces such a seat (`rdy`, `ringSeats`). */
  .seat.spoke-y .equity-badge { --badge-dx: calc(var(--pod-w) * -0.24); }

  .equity-badge.modelled {
    background: transparent;
    border: 1px dashed rgba(126, 226, 184, 0.7);
    color: #9ef0c8;
  }

  .turn-timer {
    font-size: 0.74em;
    font-weight: 800;
    color: #fff;
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }

  .turn-timer.urgent { color: #ff8b6b; }

  .position-badges { display: flex; gap: 0.15em; }

  .position-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 1.35em;
    height: 1.35em;
    padding: 0 0.28em;
    border-radius: 999px;
    font-size: 0.56em;
    font-weight: 800;
    letter-spacing: 0.02em;
    line-height: 1;
  }

  .position-badge.dealer { background: #f4f4f5; color: #17181c; }
  .position-badge.sb { background: #2f6fd0; color: #fff; }
  .position-badge.bb { background: #d08a2f; color: #fff; }

  /* PokerNow's treatment: a thin progress line along the pod's bottom edge */
  .pod-clock {
    position: absolute;
    left: 0;
    bottom: 0;
    height: calc(var(--pod-h) * 0.075);
    width: 100%;
    background: rgba(255, 255, 255, 0.12);
  }

  .pod-clock::after {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: calc(var(--clock, 0) * 100%);
    background: #49A16E;
    transition: width 1s linear;
  }

  .pod-clock.urgent::after { background: #e8543a; }

  /* ---- hole cards, on an inner ring ---- */

  /* An opponent's pair peeks out from behind their plate, perpendicular to the
     rail (--cy), never along the inward normal -- see `cy` in ringSeats(). */
  .player-cards {
    position: absolute;
    left: 0;
    top: 0;
    display: flex;
    gap: calc(var(--fw) * 0.006);
    --card-w: calc(var(--fw) * var(--card-opp-r));
    /* Perpendicular to the rail (--cy) to clear the plate, plus a small push
       along the inward normal so the pair lands ON the felt: the plates
       themselves straddle the rail at ring-kx 1.02, and cards that simply rode
       with them sat half on the woodwork. */
    transform:
      translate(-50%, -50%)
      translate(
        calc(var(--nx, 0) * var(--fw) * var(--card-nudge-r)),
        calc(var(--cy, -1) * var(--fw) * var(--off-opp-r))
      );
    z-index: 4;                      /* behind the pod, like GGPoker */
  }

  /* A REVEALED PAIR IS A THIRD OBJECT, not more board.
     The inward nudge is HALVED here and the pair gets its own plinth. Full
     nudge put seat 1's revealed `6s 4d` 33 px INSIDE the board tray at
     showdown, so the felt read `6 4 | 10 4 J 7 9` as one seven-card row -- the
     exact complaint the tray exists to answer, arriving from the other side.
     Half the nudge clears the tray; the plinth means that even when a crowded
     ring brings them close again, the two groups are drawn on different
     surfaces and cannot merge. */
  .player-cards.shown {
    z-index: 7;
    transform:
      translate(-50%, -50%)
      translate(
        calc(var(--nx, 0) * var(--fw) * var(--card-nudge-r) * 0.5),
        calc(var(--cy, -1) * var(--fw) * var(--off-shown-r))
      );
    filter: drop-shadow(0 calc(var(--fw) * 0.004) calc(var(--fw) * 0.012) rgba(0, 0, 0, 0.55));
  }

  .player-cards.shown::before {
    content: '';
    position: absolute;
    inset: calc(var(--fw) * -0.008) calc(var(--fw) * -0.010);
    z-index: -1;
    border-radius: calc(var(--fw) * 0.014);
    background: rgba(4, 14, 9, 0.78);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
  }

  .seat.winner .player-cards.shown::before {
    background: rgba(20, 26, 4, 0.85);
    box-shadow: inset 0 0 0 1px rgba(233, 255, 99, 0.45);
  }

  /* YOUR cards are the one thing on the felt that outranks a pod, so the hero's
     pair paints ABOVE its own plate. Opponents' stay behind theirs (GGPoker's
     order): you see that they are holding cards, not what the cards are. */
  .player-cards.hero {
    z-index: 7;
    --card-w: calc(var(--fw) * var(--card-hero-r));
    transform:
      translate(-50%, -50%)
      translate(
        calc(var(--nx, 0) * var(--fw) * var(--off-hero-r)),
        calc(var(--ny, 0) * var(--fw) * var(--off-hero-r))
      );
    filter: drop-shadow(0 calc(var(--fw) * 0.004) calc(var(--fw) * 0.012) rgba(0, 0, 0, 0.5));
  }

  /* ---- bet chips ---- */

  .bet-chip {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 8;
    display: flex;
    align-items: center;
    gap: 0.3em;
    padding: 0.12em 0.5em 0.12em 0.2em;
    border-radius: 999px;
    background: #dfe86a;
    color: #23260c;
    font-weight: 800;
    white-space: nowrap;
    box-shadow: 0 calc(var(--fw) * 0.003) calc(var(--fw) * 0.01) rgba(0, 0, 0, 0.5);
    /* bar 11: 500 ms for anything that moves an object */
    transition: transform 0.5s cubic-bezier(0.4, 0, 0.2, 1);
    transform:
      translate(-50%, -50%)
      translate(calc(var(--bx, 0) * var(--fw)), calc(var(--by, 0) * var(--fw)));
  }

  .bet-chip.all-in { background: #ff9f5a; color: #2a1002; }

  .chip-stack {
    width: 0.9em;
    height: 0.9em;
    border-radius: 50%;
    background:
      repeating-conic-gradient(#23260c 0 25%, #f6ffa6 0 50%);
    box-shadow: inset 0 0 0 1.5px rgba(35, 38, 12, 0.6);
  }

  .bet-amount { font-size: 0.78em; font-variant-numeric: tabular-nums; }

  /* ---- the winner's award, on the felt side of the pod ---- */

  /* Parked on this seat's AWARD SPOT -- `ax`/`ay` in ringSeats(), which is the
     chip spot given its extra showdown clearance along whichever axis has room
     rather than along the chip's own vector. See the comment there: scaling the
     chip vector is what put the `+24.00` chip of a winner at seat 1 or 2 on top
     of the board and over the suit pip of a board card (docs/DEFECTS.md T-23).
     The award still touches the pod, which is the whole point: the delta has to
     be AT the stack it changed. */
  .winner-award {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.18em;
    white-space: nowrap;
    /* One offset, named once, so the landing animation below cannot drift from
       the resting position -- the keyframes used to restate the vector and any
       change had to be made in three places. */
    --award-dx: calc(var(--ax, 0) * var(--fw));
    --award-dy: calc(var(--ay, 0) * var(--fw));
    transform:
      translate(-50%, -50%)
      translate(var(--award-dx), var(--award-dy));
    /* The award LANDS, and it lands LAST: 760 ms in, which is where the pot
       ghost finishes its flight to this pod. */
    animation: award-land 0.42s cubic-bezier(0.2, 1.25, 0.5, 1) 0.76s both;
  }

  /* PokerNow's `+300`: an olive chip under the winner's stack, the single
     figure that says what changed. */
  .stack-delta {
    padding: 0.1em 0.5em;
    border-radius: 999px;
    background: #E9FF63;
    color: #1d2000;
    font-size: 0.86em;
    font-weight: 800;
    letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums;
    box-shadow: 0 0 calc(var(--fw) * 0.03) rgba(233, 255, 99, 0.45);
  }

  .hand-tag {
    padding: 0.08em 0.45em;
    border-radius: 0.35em;
    background: rgba(8, 18, 12, 0.9);
    border: 1px solid rgba(233, 255, 99, 0.45);
    color: #e9ff63;
    font-size: 0.58em;
    font-weight: 800;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  @keyframes award-land {
    from { opacity: 0; transform:
      translate(-50%, -50%) translate(var(--award-dx), var(--award-dy)) scale(0.6); }
    to   { opacity: 1; transform:
      translate(-50%, -50%) translate(var(--award-dx), var(--award-dy)) scale(1); }
  }

  /* ---- the two flights ----
     Ghost copies painted over the felt. Neither is in the layout, so neither can
     move a measured object; both are pure transform + opacity. */

  .chip-flight,
  .pot-flight {
    position: absolute;
    left: 50%;
    top: 50%;
    z-index: 23;
    pointer-events: none;
    padding: 0.12em 0.55em;
    border-radius: 999px;
    font-weight: 800;
    font-size: 0.8em;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* seat chip -> middle */
  .chip-flight {
    --fx: calc(var(--rx) * var(--cs, 0) + var(--bx, 0) * var(--fw));
    --fy: calc(var(--ry) * var(--sn, 0) + var(--by, 0) * var(--fw));
    background: #dfe86a;
    color: #23260c;
    animation: chip-to-pot 0.5s cubic-bezier(0.4, 0, 0.2, 1) both;
  }

  @keyframes chip-to-pot {
    from { transform: translate(-50%, -50%) translate(var(--fx), var(--fy)) scale(1); opacity: 1; }
    85%  { opacity: 1; }
    to   { transform: translate(-50%, -50%) scale(0.55); opacity: 0; }
  }

  /* middle -> winner's pod */
  .pot-flight {
    background: #E9FF63;
    color: #1d2000;
    box-shadow: 0 0 calc(var(--fw) * 0.04) rgba(233, 255, 99, 0.5);
    /* 360 ms behind the chip sweep: the pot leaves once the street's chips have
       arrived in it. Must equal POT_FLIGHT_DELAY_MS in the script block, which
       is what keeps the ghost mounted for the whole flight. */
    animation: pot-to-winner 0.5s cubic-bezier(0.4, 0, 0.2, 1) 0.36s both;
  }

  @keyframes pot-to-winner {
    from { transform: translate(-50%, -50%) scale(1); opacity: 1; }
    80%  { opacity: 1; }
    to   {
      transform:
        translate(-50%, -50%)
        translate(calc(var(--rx) * var(--wcs, 0)), calc(var(--ry) * var(--wsn, 0)))
        scale(0.7);
      opacity: 0;
    }
  }

  /* ---- cards revealing (bar 11: a flip is short) ----
     Only the pairs the ENGINE just turned up animate. The hero's own pair has
     been face up all hand and must not re-flip every poll. */
  .player-cards.shown > :global(.card) {
    animation: card-reveal 0.34s cubic-bezier(0.2, 0.7, 0.3, 1) both;
    transform-origin: 50% 50%;
  }

  .player-cards.shown > :global(.card:nth-child(2)) { animation-delay: 0.1s; }

  @keyframes card-reveal {
    0%   { transform: perspective(600px) rotateY(88deg); opacity: 0.2; }
    65%  { transform: perspective(600px) rotateY(-9deg); opacity: 1; }
    100% { transform: none; opacity: 1; }
  }

  /* ---- empty seat: a full-size pod with an explicit call to action (bar 4) ---- */

  /* An empty seat is a full-size pod carrying an explicit call to action (bar
     4) -- but it is not competing with the occupied ones. At 9-max seven of
     these used to shout as loudly as the two real players. Same footprint, a
     third of the contrast, and it lights up on hover. */
  .join-seat {
    position: absolute;
    left: 0;
    top: 0;
    z-index: 5;
    transform: translate(-50%, -50%);
    width: var(--pod-w);
    height: var(--pod-h);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.05em;
    border-radius: 999px;
    background: rgba(9, 11, 15, 0.34);
    border: 1px dashed rgba(255, 255, 255, 0.17);
    color: rgba(255, 255, 255, 0.42);
    cursor: pointer;
    transition: background 0.2s ease, border-color 0.2s ease, color 0.2s ease;
  }

  .join-seat:hover {
    background: rgba(73, 161, 110, 0.2);
    border-color: rgba(126, 226, 184, 0.8);
    color: #d8fff0;
  }

  .sit-word {
    font-size: 0.86em;
    font-weight: 800;
    letter-spacing: 0.28em;
    text-transform: uppercase;
  }

  .sit-seat {
    font-size: 0.62em;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    opacity: 0.7;
  }

  /* =========================================================================
     OVERLAYS over the surround -- never over the felt
     ========================================================================= */

  .sitting-out-banner {
    position: absolute;
    left: 50%;
    top: calc(var(--fw) * 0.012);
    transform: translateX(-50%);
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 0.7em;
    padding: 0.4em 0.9em;
    border-radius: 999px;
    background: rgba(120, 60, 10, 0.9);
    border: 1px solid rgba(255, 180, 90, 0.45);
    color: #ffe6c8;
    font-size: 0.85em;
  }

  .sit-in-btn {
    padding: 0.25em 0.7em;
    border-radius: 999px;
    border: none;
    background: #f0a02a;
    color: #241000;
    font-weight: 800;
    font-size: 0.9em;
    cursor: pointer;
  }

  .feed-container.left {
    position: absolute;
    left: 0;
    bottom: 0;
    z-index: 32;
    width: min(240px, 24cqw);
    max-height: 74cqh;
    display: flex;
  }

  .raise-slider-panel {
    position: absolute;
    left: 50%;
    bottom: calc(var(--fw) * 0.01);
    transform: translateX(-50%);
    z-index: 34;
    width: min(340px, 34cqw);
    display: flex;
    flex-direction: column;
    gap: 0.5em;
    padding: 0.8em 0.9em;
    border-radius: 14px;
    background: rgba(14, 15, 22, 0.97);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.6);
    font-size: 14px;
  }

  .slider-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 0.8em;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.5);
  }

  .close-slider {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: transparent;
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    line-height: 1;
  }

  .slider-amount {
    text-align: center;
    font-size: 1.9em;
    font-weight: 800;
    color: #f8d97a;
    font-variant-numeric: tabular-nums;
  }

  .raise-slider {
    width: 100%;
    accent-color: #49A16E;
  }

  .preset-buttons { display: flex; gap: 6px; }

  .preset-buttons button {
    flex: 1;
    padding: 7px 0;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.82);
    font-size: 0.82em;
    font-weight: 700;
    cursor: pointer;
  }

  .preset-buttons button:hover { background: rgba(73, 161, 110, 0.22); }

  .confirm-raise {
    padding: 10px 0;
    border-radius: 10px;
    border: none;
    background: linear-gradient(180deg, #d9a63a, #b9821f);
    color: #241a02;
    font-weight: 800;
    font-size: 0.95em;
    cursor: pointer;
  }

  /* =========================================================================
     DOCK
     ========================================================================= */

  /* The three columns used to be `210px | 1fr | 240px`, and the right-hand
     cluster (wallet toggle + balance + Deposit + Withdraw + Sit out + Leave)
     needs about 340. Its children were all `flex: 0 0 auto`, so they did not
     shrink -- they overflowed a 240px box and printed on top of each other:
     the shipped capture reads "SWithdrawLeave". The side columns are now equal
     free space around a content-sized centre, and everything inside them is
     allowed to shrink. */
  .action-dock {
    flex: 0 0 auto;
    height: var(--dock-h);
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 12px;
    padding: 0 4px;
  }

  .dock-aux {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
  }

  .dock-right { justify-content: flex-end; }

  .log-toggle {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex: 0 0 auto;
    white-space: nowrap;
    padding: 7px 10px;
    border-radius: 9px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.04);
    color: rgba(255, 255, 255, 0.68);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
  }

  .log-toggle.active { background: rgba(73, 161, 110, 0.22); color: #b9f0d6; }

  .turn-indicator {
    display: flex;
    flex-direction: column;
    flex: 0 1 auto;
    min-width: 0;
    padding: 6px 10px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    transition: border-color 0.4s ease, background 0.4s ease;
  }

  .turn-indicator.my-turn {
    background: rgba(73, 161, 110, 0.16);
    border-color: rgba(126, 226, 184, 0.5);
  }

  .turn-indicator.time-bank { border-color: rgba(240, 160, 42, 0.6); }

  .turn-title {
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.5);
  }

  .turn-indicator.my-turn .turn-title { color: #7ee2b8; }

  .turn-hint {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.78);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dock-center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }

  .pot-odds-display {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 11px;
  }

  .pot-odds-label {
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.45);
  }

  .pot-odds-value { font-size: 13px; font-weight: 800; color: #f8d97a; }

  .pot-odds-explanation {
    color: rgba(255, 255, 255, 0.7);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .equity-hint { color: rgba(255, 255, 255, 0.5); }

  .actions {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex-wrap: nowrap;
  }

  .actions.disabled { opacity: 0.45; pointer-events: none; }

  .no-game-message, .not-your-turn, .action-pending {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 18px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    color: rgba(255, 255, 255, 0.55);
    font-size: 13px;
    white-space: nowrap;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.18);
    border-top-color: #7ee2b8;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .action-btn {
    flex: 0 1 auto;
    min-width: 78px;
    padding: 12px 16px;
    border-radius: 11px;
    border: 1px solid transparent;
    font-size: 14px;
    font-weight: 700;
    cursor: pointer;
    white-space: nowrap;
    transition: transform 0.15s ease, filter 0.15s ease;
  }

  .action-btn:hover { transform: translateY(-1px); filter: brightness(1.1); }
  .action-btn:active { transform: translateY(0); }

  .action-btn.secondary {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.75);
  }

  .action-btn.primary {
    background: linear-gradient(180deg, #49A16E, #338054);
    color: #062015;
  }

  .action-btn.raise {
    background: linear-gradient(180deg, #d9a63a, #b9821f);
    color: #241a02;
  }

  .action-btn.danger {
    background: rgba(178, 43, 12, 0.9);
    color: #fff;
  }

  .action-btn.ghost {
    background: transparent;
    border-color: rgba(255, 255, 255, 0.16);
    color: rgba(255, 255, 255, 0.6);
    min-width: 0;
    padding: 12px 12px;
  }

  .wallet-panel {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 0 1 auto;
    padding: 6px 10px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
    min-width: 0;
    overflow: hidden;
  }

  .wallet-balance { display: flex; flex-direction: column; min-width: 0; }

  .balance-label {
    font-size: 9px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.42);
    white-space: nowrap;
  }

  .balance-value {
    font-size: 13px;
    font-weight: 800;
    color: #7ee2b8;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .wallet-actions { display: flex; gap: 6px; }

  .wallet-action-btn {
    padding: 7px 10px;
    border-radius: 8px;
    border: 1px solid transparent;
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.04em;
    cursor: pointer;
    white-space: nowrap;
  }

  .wallet-action-btn.deposit { background: linear-gradient(180deg, #49A16E, #338054); color: #062015; }

  .wallet-action-btn.withdraw {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.75);
  }

  .wallet-action-btn.withdraw:disabled { opacity: 0.4; cursor: not-allowed; }

  .panel-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: 0 0 auto;
    padding: 7px 9px;
    border-radius: 9px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.04);
    color: rgba(255, 255, 255, 0.62);
    cursor: pointer;
  }

  .collapsed-balance {
    font-size: 12px;
    font-weight: 800;
    color: #7ee2b8;
    font-variant-numeric: tabular-nums;
  }

  .sit-controls { display: flex; gap: 5px; flex: 0 1 auto; min-width: 0; }

  .control-btn {
    padding: 7px 9px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.03);
    color: rgba(255, 255, 255, 0.55);
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    white-space: nowrap;
  }

  .control-btn:hover { color: rgba(255, 255, 255, 0.9); }
  .control-btn.destructive:hover { color: #ff8b6b; border-color: rgba(255, 139, 107, 0.4); }

  /* =========================================================================
     PORTRAIT -- switched on ASPECT RATIO, not width (bar 19), and a genuinely
     different dressing of the same ring (bar 20): shorter surface, compact
     pods, bigger cards relative to the felt, two-row dock.
     ========================================================================= */

  @media (max-aspect-ratio: 1/1) {
    /* THE SURFACE TURNS THROUGH NINETY DEGREES, and then it takes the whole
       phone. Landscape is a 2.1:1 stadium; portrait is 0.555:1 -- taller than it
       is wide, which is bar 20 ("the ellipse becomes a tall rounded rectangle
       and pods move to the left and right edges in two columns, pot centred.
       Table still occupies ~75% of viewport height") and it is also what the one
       real phone reference does: PokerNow portrait measures 313x548 on a 390x844
       screen, aspect 0.571, 52.1% of the screen by area.

       WHY 0.555 AND NOT 0.70. Two caps decide the felt width:

         width   the ring plus the two pods straddling it must fit the stage
                   fw * (ring-k + pod-w-r)  <=  stage width
         height  the FELT ITSELF must fit the stage -- the old cap only sized
                 the RING, and at ring-ky 0.98 with pods hanging off the top and
                 bottom that is nearly the same number. It is not the same number
                 once the pods move INSIDE the surface.
                   fw / ar                  <=  stage height

       0.555 is where those two bind at the same felt width on a 390x844 phone,
       i.e. the largest surface the screen can hold. At 0.70 with the pods
       outboard the width cap bound alone and left the felt at 217x316 = 20.6% of
       the screen, 40% of PokerNow's, with 215 px of dead surround under it.

       WHY THE PODS MOVED ONTO THE FELT. `--ring-kx` was 1.00, so each flank pod
       hung half its width off the surface and the felt could never be wider than
       stage/1.42 = 70% of a phone. PokerNow's portrait pods sit ON the felt --
       their surface is 80.3% of the screen width and the pods overlap it, ending
       ~38 px outside the rail. `--ring-kx: 0.70` does the same thing here and
       buys 16 points of screen width. Pods no longer hang off the top and bottom
       at all, which is where the height went.

       `PORTRAIT_AR` in the script block must equal `--ar` here, and `--ring-kx`
       must equal `--ring-ky`, or the seats sit on an ellipse the felt is not
       drawing: `ringSeats()` spaces by arc length on the FELT ellipse, and only
       a uniform scale of it keeps that spacing true on the ring. */
    .poker-table-wrapper {
      --ar: 0.555;
      --ring-kx: 0.70;
      --ring-ky: 0.70;
      --pod-w-r: 0.46;
      --pod-h-r: 0.140;
      --avatar-r: 0.098;
      --card-board-r: 0.112;
      --card-hero-r: 0.150;
      --card-opp-r: 0.092;
      --board-gap-r: 0.010;
      --card-nudge-r: 0.050;
      --off-opp-r: 0.068;
      --off-shown-r: 0.112;
      --off-hero-r: 0.132;
      /* The board sits high and the pot sits under it, PokerNow's portrait
         order. Centring both put the pot readout in the same 40 px band as the
         two mid-height flank seats. */
      --cluster-dy-r: -0.066;
      /* bar 24: money glyphs. 0.044 rendered the stack figure at 8 px on a
         phone. 0.058 puts it at ~17 px, against PokerNow portrait's ~11 px. */
      --ui-r: 0.058;
      --dock-h: 90px;
    }

    /* width  0.70 + 0.46 = 1.160 -> 86cqw
       height max(0.70/0.555 + 0.140, 1/0.555) = 1.802 (the FELT, not the ring)
                                              -> 55cqh */
    .table-inner { --fw: min(86cqw, 55cqh); }

    /* Per-density again, at .poker-table-wrapper.<class> specificity. Media
       queries add NO specificity, so the landscape `.ring-crowded .table-inner`
       rule (two classes) outranks a portrait `.table-inner` rule (one class) and
       would otherwise win here: a 9-max table on a phone was being laid out with
       the LANDSCAPE felt cap, which is how it ended up 1.43:1 with unreadable
       pods. Every density override therefore has a portrait twin. */

    /* Nine pods on a phone. The ring has to open out (0.90, not 0.70) or the two
       plates either side of top centre overlap by 23 px, and the two mid-height
       flank plates land on the board. Everything else comes down with it. */
    .poker-table-wrapper.ring-crowded {
      --ring-kx: 0.90;
      /* The only place kx and ky part company in portrait. Nine seats need the
         ring TALLER than a uniform scale of the felt would make it, so the two
         mid-height plates and the two upper flank plates open a wide enough
         band for the board tray and the pot to sit between them. It costs a
         little of the equal-arc spacing (ringSeats() measures arc on the felt
         ellipse) and nothing at all of the felt: at ky 1.00 the height cap is
         1.00/0.555 + 0.116 = 1.918 -> 52cqh = 328 px, still looser than the
         78cqw = 304 px the width cap allows. */
      --ring-ky: 1.00;
      --pod-w-r: 0.38;
      --pod-h-r: 0.112;
      --avatar-r: 0.056;
      /* Nine plates means the board and the pot have to clear the two
         mid-height flank seats, which on a 9-ring sit within 30 px of the
         vertical centre. The cluster rides higher for them. */
      --cluster-dy-r: -0.180;
      --card-board-r: 0.098;
      --card-opp-r: 0.076;
      --off-opp-r: 0.056;
      --off-shown-r: 0.094;
      --ui-r: 0.056;
    }
    /* 0.90 + 0.38 = 1.280 -> 78cqw ; ring-bound at ky 1.00 -> 52cqh */
    .ring-crowded .table-inner { --fw: min(78cqw, 52cqh); }

    .poker-table-wrapper.ring-sparse {
      --ring-kx: 0.62;
      --ring-ky: 0.62;
      --pod-w-r: 0.52;
      --pod-h-r: 0.155;
      --avatar-r: 0.112;
      --card-opp-r: 0.104;
      --off-opp-r: 0.074;
      --off-shown-r: 0.124;
      --ui-r: 0.062;
    }
    /* 0.62 + 0.52 = 1.140 -> 87cqw ; felt-bound -> 55cqh */
    .ring-sparse .table-inner { --fw: min(87cqw, 55cqh); }

    /* FULL BLEED. The page reserves 8 px of padding either side of the table
       area; on a 390 px phone that is 4% of the playing surface spent on a
       gutter nothing sits in. The wrapper takes the whole viewport width back
       and the rail keeps the edge margin instead. */
    .poker-table-wrapper {
      width: 100vw;
      max-width: 100vw;
      margin-left: calc(50% - 50vw);
      margin-right: calc(50% - 50vw);
    }

    /* THE MIDDLE OF THE TABLE OUTRANKS AN EMPTY CHAIR. A portrait felt is
       narrow enough that the pot's collected/betting decomposition and the
       equity-method line run under the two mid-height plates, and those two
       lines are chain figures. The cluster therefore paints above every seat in
       portrait (z-index 26 clears .seat 10, .seat.acting 22 and .seat.winner
       24); it is `pointer-events: none`, so nothing becomes unclickable. */
    .board-cluster { z-index: 26; }

    /* bar 20: a tall ROUNDED RECTANGLE, not an ellipse. It is not decoration --
       an ellipse only offers its full width on one scanline, so a flank pod
       parked at mid-height crops the measured surface; a stadium offers it over
       most of the height. It is also what the phone reference draws. */
    .felt { border-radius: calc(var(--fw) * 0.28); }

    .mark-1, .mark-2 { display: none; }

    /* The pot reads UNDER the board in portrait (PokerNow's order), so the
       board can sit high and clear of the flank seats. */
    .pot-display, .winner-display {
      bottom: auto;
      top: calc(100% + var(--fw) * 0.016);
    }

    /* THE MIDDLE OF A PORTRAIT TABLE IS A 166 px SLOT.
       On a 0.555 surface the two mid-height flank seats sit at +/-106 px of the
       centre and their plates are 47 tall, so everything the middle carries --
       the board, its showdown frame and caption, and the pot or the winner line
       -- has to live inside the 166 px between them. Measured before this
       block: the winner line alone was 247 px wide and lay 51x47 into both
       plates. The board tray, its caption and the winner readout are therefore
       all one type step tighter in portrait than in landscape. */
    .caption-hand { font-size: 0.6em; }
    .caption-tag { font-size: 0.55em; letter-spacing: 0.12em; }
    .board-frame.framed {
      gap: calc(var(--fw) * 0.006);
      padding: calc(var(--fw) * 0.008) calc(var(--fw) * 0.012) calc(var(--fw) * 0.006);
    }
    .board-caption { gap: 0.6em; }

    /* NOTHING IN THE POD OUTRANKS THE STACK FIGURE.
       A 9-max phone pod is ~116 px wide. `min-width: 2.3em` on the clock/blind
       column reserved 39 of those pixels whether or not anything was in them --
       and the thing that lost the argument was the money. Portrait lets the
       column size to its contents. (The equity badge no longer costs the pod any
       width at ANY viewport: it is a seat-level object outside the plate, see
       `.equity-badge`.)

       `overflow: visible` is kept because the action clock's progress line now
       carries the pill's own bottom radius instead of being clipped by it. */
    .player-nameplate { overflow: visible; }
    .pod-clock { border-radius: 0 0 999px 999px; }

    .pod-slot { min-width: 0; }

    .position-badge { font-size: 0.52em; min-width: 1.2em; height: 1.2em; }
    .ring-crowded .position-badge { font-size: 0.44em; min-width: 1.05em; height: 1.05em; }
    .ring-crowded .player-nameplate { gap: 0.32em; padding: 0 0.4em 0 calc(var(--pod-h) * 0.08); }
    .ring-crowded .chips { letter-spacing: -0.015em; }

    /* Type only. The PLACEMENT is the same rule at both viewports (the `-cy`
       axis), because the reason for it -- the cards paint above the plate -- is
       the same at both. */
    .equity-badge { font-size: 0.6em; padding: 0.06em 0.3em; }

    /* The winner line spanned 247 of a 390 px screen and lay across both
       mid-height flank plates (measured: 24x39 px into each). Same words, one
       type step down, and allowed to WRAP inside a box narrower than the gap
       between the two flank pods rather than pushing through them. */
    .winner-text { font-size: 0.82em; }
    .winner-hand-rank, .split-info { font-size: 0.56em; }
    /* Every readout that lands in that 166 px slot is set on a 1.15 leading in
       portrait. The winner block was 94 px of a 127 px gap on its own. */
    .winner-display, .pot-display { line-height: 1.15; }
    /* Wide and SHORT, not narrow and tall: a pod is 47 px tall and the gap
       above it is only 127, so height is the scarce axis here and width is not
       -- the readout may run the width of the felt as long as it stays out of
       the two flank plates' rows. */
    .winner-display {
      flex-direction: row;
      flex-wrap: nowrap;
      align-items: baseline;
      column-gap: 0.5em;
      padding: 0.26em 0.7em;
      gap: 0.5em;
      white-space: nowrap;
      text-align: center;
    }
    .winner-display .phase-indicator { font-size: 0.56em; }

    /* The pot's footnote rows -- the collected/betting decomposition and the
       equity method line -- are the other thing that pushed the cluster into
       the flank plates on a 9-ring. */
    .main-pot { padding: 0.16em 0.7em; }
    .pot-label { font-size: 0.55em; }
    .pot-display .phase-indicator { font-size: 0.62em; }
    .pot-breakdown { font-size: 0.54em; }
    .equity-method { font-size: 0.5em; }
    /* Portrait puts the readout BELOW the board, so the method line hangs below
       the winner banner rather than above it (T-27). */
    .winner-display .equity-method { bottom: auto; top: calc(100% + 0.25em); }

    /* THE AWARD IS SIZED FOR THE GAP IT LANDS IN. Its POSITION is computed per
       seat and per orientation in `ringSeats` (`ax`/`ay`); a portrait board is
       the same five cards on a felt barely half as wide, so the tray reaches
       much closer to the rail and the clearance there is smaller. What is left
       here is type, one step down, the same treatment every other portrait
       readout gets. */
    .winner-award { gap: 0.1em; }

    /* PORTRAIT HAS NO ROOM ON THE CHIP VECTOR (T-23), so the award rides the
       readout spoke with the badge -- see `awardOnSpoke` in ringSeats().
       Horizontal spoke (flank seats): one step further out than the badge. The
       badge reaches ~0.15 felt widths past the plate's end at its widest
       (`100.00%`, measured 59 px on a 332 px felt), and the award's own
       half-width is ~0.09, so 0.20 was 12 px short of clearing it and the award
       took 11.8% of the badge's ink. 0.28 leaves a 12 px gap on the same
       measurement.
       Vertical spoke (top and bottom seats): the same edge as the badge, at the
       other END of it, so the two cannot meet however wide either gets. */
    .seat.award-on-spoke .winner-award {
      --award-dx: calc(var(--rdx, 0) * (var(--pod-w) * 0.5 + var(--fw) * 0.28));
      --award-dy: 0px;
    }
    .seat.award-on-spoke.spoke-y .winner-award {
      --award-dx: calc(var(--pod-w) * 0.24);
      --award-dy: calc(var(--rdy, 0) * (var(--pod-h) * 0.5 + var(--fw) * 0.05));
    }
    .stack-delta { font-size: 0.7em; padding: 0.08em 0.4em; }
    .hand-tag { font-size: 0.48em; padding: 0.06em 0.35em; }
    .side-pot-label { font-size: 0.52em; }
    .side-pot-amount { font-size: 0.7em; }

    .action-dock {
      grid-template-columns: 1fr auto;
      grid-template-rows: auto auto;
      gap: 6px 8px;
      height: var(--dock-h);
      padding: 0 8px;
    }

    /* THUMB REACH. The action row is the BOTTOM row on a phone -- it used to be
       the top one, furthest from the thumb, with the LOG button and the wallet
       occupying the reachable edge. */
    .dock-center { grid-column: 1 / -1; grid-row: 2; }
    .dock-left { grid-column: 1; grid-row: 1; }
    .dock-right { grid-column: 2; grid-row: 1; }

    .actions { flex-wrap: nowrap; gap: 6px; width: 100%; }

    /* 48 CSS px tall, which is >= the 44 px touch target on this device. */
    .action-btn {
      flex: 1 1 0;
      min-width: 0;
      min-height: 48px;
      padding: 13px 4px;
      font-size: 15px;
      border-radius: 12px;
    }

    .action-btn.ghost { flex: 0 0 auto; padding: 13px 9px; }

    .no-game-message, .not-your-turn, .action-pending {
      min-height: 48px;
      font-size: 14px;
    }

    /* The dock's second row was carrying LOG + a turn indicator + the balance +
       Deposit + Withdraw across 390px, and the indicator lost: it rendered as
       "WAIT A...". Row 1 already names whose turn it is in full, so the
       duplicate goes rather than being truncated. */
    .turn-indicator { display: none; }

    .wallet-panel { padding: 4px 8px; gap: 7px; }
    .balance-label { display: none; }
    .wallet-action-btn { padding: 7px 9px; font-size: 11px; }
    .sit-controls { display: none; }

    .feed-container.left { width: min(230px, 62cqw); max-height: 60cqh; }
    .raise-slider-panel { width: min(320px, 88cqw); font-size: 13px; }
  }

  /* Very short landscape (phone held sideways): trim the dock, keep the felt. */
  @media (min-aspect-ratio: 1/1) and (max-height: 560px) {
    .poker-table-wrapper { --dock-h: 62px; }
    .action-btn { padding: 9px 12px; font-size: 13px; min-width: 66px; }
    .turn-indicator { display: none; }
    .sit-controls { display: none; }
  }

  @media (prefers-reduced-motion: reduce) {
    .player-nameplate.is-winner { animation: none; box-shadow: 0 0 0 2px #E9FF63; }
    .bet-chip, .felt, .player-nameplate, .turn-indicator { transition: none; }
    .pod-clock::after { transition: none; }
    .spinner { animation: none; }
    /* The all-in beat is carried by colour and by the vignette, both of which
       are still there; only the movement stops. The pot keeps its emphasis
       without the spring, so the moment still reads. */
    .main-pot { transition: none; }
    .main-pot.at-risk { transform: none; }
    .table-inner::after { transition: none; }
    .action-btn { transition: none; }
    .action-btn:hover { transform: none; }

    /* THE TWO FLIGHTS AND THE REVEAL STOP DEAD, AND NOTHING IS LOST.
       Everything the motion was carrying is also stated in place: the chips are
       already counted into `.pot-amount` and its `.pot-breakdown`; the award is
       a static `+X` chip on the winner's pod; the revealed cards are simply
       there. So `animation: none` here removes the movement, not the
       information -- which is the only kind of animation this table is allowed
       to have in the first place. Both flights are also given `display: none`
       rather than a zero-length animation, because a ghost that never travels
       is a duplicate figure sitting on the felt. */
    .chip-flight, .pot-flight { display: none; }
    .winner-award { animation: none; }
    .player-cards.shown > :global(.card) { animation: none; }
  }
</style>
