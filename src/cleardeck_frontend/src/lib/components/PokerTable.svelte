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
  import { untrack } from 'svelte';
  import ActionFeed from './ActionFeed.svelte';
  import SeatPod from './SeatPod.svelte';
  import PotModule from './PotModule.svelte';
  import BoardStrip from './BoardStrip.svelte';
  import { generatedName, shortName } from '$lib/table-visuals.js';
  import {
    isFlankSeat, isBottomSeat, puckSpot, readoutSpoke, revealedCellShift, freeSpokeEnd, portraitAwardSpot
  } from '$lib/table-geometry.js';
  import { compactHandName } from '$lib/hand-names.js';
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
    // The lobby's known seat count, used for the ring until the canister's
    // config arrives so a spectator never sees nine chairs on a 6-max table
    // (docs/DEFECTS.md T-32).
    maxPlayers = null,
    // Retained for the profile menu's contract; the table now draws its own
    // local avatar tiles (lib/table-visuals.js) and fetches nothing.
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

  // ---------------------------------------------------------------------------
  // MONEY OF MINE THAT IS IN THE MIDDLE (docs/SECURITY-FINDINGS.md FINDING 18)
  // ---------------------------------------------------------------------------
  //
  // `tableBalance` is `get_balance()`: what can be withdrawn right now. It does
  // not include a stake of mine in the current pot, and it CANNOT, because a
  // stake in a pot is not withdrawable. An auditor read that number as zero and
  // walked away from 2.98 ICP.
  //
  // The view now carries the figure per caller, so it is on screen beside the
  // balance it corrects. Non-zero with `mySeat === null` is the finding itself:
  // money of mine in a hand I am no longer sitting in.
  const myCommittedInPot = $derived(Number(tableState?.my_committed_in_pot ?? 0));
  const handIsUnmovable = $derived(tableState?.hand_is_unmovable === true);
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
   * BEFORE it checks whether two active players are left (lib.rs:2644-2665),
   * and an update that returns `Err` still commits what it wrote. So an
   * auto-deal that fires and correctly refuses to deal leaves a player who is
   * sitting in the finished hand, cards face up on the felt, marked SittingOut.
   * Reading `status` there dropped that player out of the live set and the
   * showdown rendered with NO equity badges at all, nondeterministically,
   * depending on whether the auto-deal timer beat the screenshot.
   *
   * The cards themselves do not have that problem. At showdown the canister
   * reveals `hole_cards` for every non-folded player it dealt in, and leaves
   * them null for everyone it did not, so "has cards on the wire" IS "was dealt
   * in": a fact about this hand, not about what the seat intends to do next.
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

  /**
   * One line naming the method (bar 15). Not painted on the felt: it is the
   * badges' tooltip, the pot module's hidden `.equity-method` line (the
   * harness reads it by textContent) and one LOG line per computation.
   */
  const equityMethodLabel = $derived.by(() => {
    if (!equity) return null;
    if (equity.mode === 'showdown') {
      return equity.method === 'exact'
        ? `Equity · exact · ${equity.trials.toLocaleString('en-US')} runout${equity.trials === 1 ? '' : 's'}`
        : `Equity · Monte Carlo · ${equity.trials.toLocaleString('en-US')} trials`;
    }
    return `Equity vs ${equity.opponents} random · Monte Carlo · ${equity.trials.toLocaleString('en-US')} trials`;
  });

  /** The tooltip on every equity badge: the method, then the engine's note. */
  const equityTooltip = $derived(
    [equityMethodLabel, equity?.note ?? ''].filter(Boolean).join('. ')
  );

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
    // Compacted for a plate row ($lib/hand-names.js): the kicker clause and
    // the category prefix the ranks already state are dropped, nothing else.
    return compactHandName(describeHand(codes)?.name ?? null);
  });

  // ---------------------------------------------------------------------------
  // 3. The seat ring -- equal ARC LENGTH on the ellipse
  // ---------------------------------------------------------------------------

  const seatCapacity = $derived(Number(tableState?.config?.max_players ?? maxPlayers ?? 9));
  const seatCount = $derived(Math.max(2, Math.min(seatCapacity, players.length || seatCapacity)));

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
      const crowded = n >= 8;
      const sparse = n <= 3;
      // WHICH RUN OF THE RAIL the seat sits on decides everything below. The
      // classifiers are $lib/table-geometry.js (unit-tested): landscape keeps
      // the |cos| > 0.35 test; portrait reads the seat's x, because the old
      // normal-component comparison was a coin flip (0.90 vs 0.92) for the
      // 6-max lower flank seat and flipped its badge, cards and chips.
      const flank = isFlankSeat({ tall, cs });
      const bottom = isBottomSeat({ tall, cs, sn });
      const CHIP_IN = tall ? 0.255 : 0.155;
      const CHIP_SIDE = tall ? 0.150 : 0.140;
      const CHIP_SIDE_IN = tall ? 0.045 : 0.050;
      // `alongNormal` is true when pushing inward already moves the disc along
      // the roomy axis: flank seats in landscape, top/bottom seats in portrait.
      const alongNormal = tall ? !flank : flank;
      let bx = nx * CHIP_IN;
      let by = ny * CHIP_IN;
      if (tall && bottom) {
        // THE HERO'S CHIPS ON A PHONE sit BESIDE the hero's pair, at the pair's
        // lower half, on its right. Above the pair (the old spot, further up
        // the normal) is the band the lower flank plates' pucks, chips and
        // badges share. These mirror the CSS ratios --card-hero-r and
        // --off-hero-r for each ring density (PokerTable's portrait block).
        bx = crowded ? 0.25 : 0.27;
        by = -(crowded ? 0.14 : sparse ? 0.17 : 0.14);
      } else if (!alongNormal) {
        // Tangent, taken away from the axis the board sits on so the disc never
        // drifts under it. Dead centre is broken to the right (landscape) or
        // downward (portrait).
        if (tall) {
          // A PORTRAIT FLANK SEAT'S CHIPS RIDE THE RAIL UPWARD, whichever half
          // of the ring it is on: toward the board from a lower seat (the band
          // between its plate and the pot module is free; below it is the
          // hero), away from the board from an upper one (the board's first
          // card sits right under its plate).
          // On the 6-max ring (ring-kx 0.70, the plate well inside the felt)
          // a LOWER flank seat's chips also lean a little outward: measured,
          // the inward lean put the amount capsule under the pot module's
          // edge. On the nine-seat ring (ring-kx 0.90) the plate already
          // hangs off the felt, so its chips keep the inward lean.
          const lean = (sn > 0 && !crowded) ? -0.5 : 1;
          bx = nx * CHIP_SIDE_IN * lean;
          by = -CHIP_SIDE;
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
      //   bottom seat, PORTRAIT: below the plate is the action dock and above
      //     it are the hero's own cards, so the badge takes the plate's LEFT
      //     end and the award its RIGHT end (`spokeEnds`).
      // The rule itself is $lib/table-geometry.js readoutSpoke(), unit-tested.
      const { rdx, rdy, spokeEnds } = readoutSpoke({ tall, flank, bottom, cs, nx, ny });

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

      // THE DEALER PUCK'S SPOT: felt widths (px/py) plus plate sizes (pkx/pky)
      // from the seat point, so "just past the plate's inner end" stays just
      // past it whatever size the plate is. The rule is $lib/table-geometry.js
      // puckSpot(), unit-tested; measured failures it replaces: the lowest
      // landscape flank seat's puck on the rail, the portrait flank puck on
      // its own plate's top edge.
      const cyDir = (tall && flank) ? (sn >= 0 ? 1 : -1) : (sn >= 0 ? -1 : 1);
      const puck = puckSpot({ tall, flank, cs, sn, ny, crowded });

      // A LANDSCAPE FLANK SEAT'S REVEALED PAIR clears the board strip along x
      // ($lib/table-geometry.js revealedCellShift, unit-tested). The ratios
      // mirror the landscape tokens in poker-table-tokens.scss: --card-opp-r
      // per ring density, the revealed cell's plinth (0.010) and gap (0.006),
      // and the board (5 x --card-board-r 0.112 + 4 x --board-gap-r 0.009).
      const cardOppR = crowded ? 0.065 : sparse ? 0.084 : 0.076;
      const cellShift = revealedCellShift({
        tall, flank, cs, ringKx: LANDSCAPE_RING_KX,
        cellHalfW: cardOppR + 0.003 + 0.010, boardHalfW: 0.298, nudge: 0.0175
      });
      // signed for the screen: inward is +x for a left seat, -x for a right one
      const shx = cellShift === null ? null : Number((cellShift * (cs < 0 ? 1 : -1)).toFixed(5));

      // A PORTRAIT FLANK SEAT'S AWARD spot, in plate widths and the plate
      // edge it stands past ($lib/table-geometry.js portraitAwardSpot).
      const award = portraitAwardSpot({ tall, flank, cs, sn, crowded });

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
        spokeEnds,
        px: puck.px,
        py: puck.py,
        pkx: puck.pkx,
        pky: puck.pky,
        flank,
        shx,
        akx: award?.akx ?? 0,
        aky: award?.aky ?? -1,
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
        cy: cyDir,
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
  /** Mirrors --ring-kx in poker-table-tokens.scss (landscape). */
  const LANDSCAPE_RING_KX = 1.02;

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

  /**
   * WHICH END OF A LANDSCAPE TOP/BOTTOM PLATE THE EQUITY BADGE TAKES.
   *
   * The badge only exists at the all-in and the showdown, and its two
   * candidate spots are the plate's two ends at mid-height. Which one is free
   * depends on who is SEATED next door, so it is decided here, per seat, from
   * the occupied neighbours' plate rectangles ($lib/table-geometry.js
   * freeSpokeEnd, unit-tested). Measured on the nine-seat desktop ring: the
   * hero's badge at the left end sat on the corner of the next plate. The
   * ratios mirror the landscape tokens in poker-table-tokens.scss.
   */
  const LANDSCAPE_PLATE = {
    default: { podW: 0.235, podH: 0.086, avatarW: 0.066, ui: 0.021 },
    crowded: { podW: 0.202, podH: 0.074, avatarW: 0.056, ui: 0.018 },
    sparse: { podW: 0.262, podH: 0.094, avatarW: 0.072, ui: 0.0225 }
  };
  const spokeEndBySeat = $derived.by(() => {
    if (portrait) return seatPoints.map((p) => p.rdx);
    const d = seatCount >= 8 ? LANDSCAPE_PLATE.crowded : seatCount <= 3 ? LANDSCAPE_PLATE.sparse : LANDSCAPE_PLATE.default;
    const dims = {
      podW: d.podW, podH: d.podH, avatarW: d.avatarW,
      badgeW: 2.8 * d.ui, badgeH: 1.1 * d.ui, badgeGap: 2.1 * d.ui
    };
    const at = (p) => ({ x: p.cs * LANDSCAPE_RING_KX * 0.5, y: p.sn * 0.5 / LANDSCAPE_AR });
    return seatPoints.map((p, i) => {
      if (p.side !== 'center') return p.rdx;
      const neighbours = seatPoints.filter((_, j) => j !== i && players[j]).map(at);
      return freeSpokeEnd(at(p), dims, neighbours);
    });
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

  /**
   * THE EQUITY METHOD, IN THE LOG. The felt no longer states how the badges
   * were computed (that sentence is the badges' tooltip); the log keeps one
   * line per computation so the method survives the hand. Counts only (trials,
   * opponents), never a percentage or a chip figure.
   */
  $effect(() => {
    const label = equityMethodLabel;
    if (!label) return;
    if (actionFeed.some(a => a.type === 'phase' && a.text === label)) return;
    actionFeed = [...actionFeed, { type: 'phase', text: label, timestamp: Date.now() }];
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

  function getShortName(player, seatIndex) {
    if (seatIndex === mySeat) return customName || 'You';
    if (!player?.principal) return seatLabel(seatIndex);
    const name = player.display_name || generatedName(player.principal.toString());
    return shortName(name, 11);
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
   * `players`, which is a fresh array on every poll, so the timer that removes
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

  // ---------------------------------------------------------------------------
  // 11. The dealer puck travels (bar 11: 500 ms, seat to seat)
  // ---------------------------------------------------------------------------
  //
  // The puck is rendered INSIDE the dealer's `.seat` (the harness reads
  // `.position-badge.dealer` per seat), so it cannot be one element that moves.
  // Instead the new dealer's puck plays a 500 ms keyframe FROM the previous
  // dealer's ring point, expressed in ring units the CSS resolves with --rx/--ry.
  let puckFrom = $state(null);
  let prevDealer = null;
  $effect(() => {
    const d = dealerSeat;
    if (d === undefined || d === null) return;
    const dn = Number(d);
    if (!Number.isFinite(dn)) return;
    const ring = untrack(() => seatPoints);
    const from = prevDealer;
    prevDealer = dn;
    if (from === null || from === dn || !ring[from] || !ring[dn] || untrack(() => reducedMotion)) {
      puckFrom = null;
      return;
    }
    puckFrom = {
      seat: dn,
      dx: Number((ring[from].cs - ring[dn].cs).toFixed(5)),
      dy: Number((ring[from].sn - ring[dn].sn).toFixed(5))
    };
  });

</script>

<!-- `ring-<n>` is a breakpoint on SEAT COUNT, not on viewport width, so it does
     not violate bar 19: the table is still one rigid object scaled from one
     input, but nine plates share the same rail as two and cannot be the same
     size. -->
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
    <!-- ===================== the stage: a lit room ===================== -->
    <div class="stage">
      <div
        class="table-inner"
        class:all-in-moment={allInMoment}
        class:showdown={isShowdown}
        class:has-board={communityCards.length > 0}
      >
        <!-- THE RAIL: black padded leather with a lit crown, drawn as its own
             element BEHIND the felt so the felt's layout box stays the green
             surface a pixel measurer calls "the felt". It is thicker at the
             bottom than the top (the table is seen from a raised chair, not
             from the ceiling) and it never changes colour with game state. -->
        <div class="rail" aria-hidden="true"></div>

        <!-- felt: the LAYOUT BOX is the visible green surface. -->
        <div class="felt">
          <div class="felt-marks" aria-hidden="true">
            <span class="mark-1">&#9824;</span>
            <span class="mark-2">ClearDeck &middot; no rake</span>
          </div>
        </div>

        <!-- board + pot cluster, dead centre -->
        <div class="board-cluster">
          <PotModule
            {totalPot}
            {liveBets}
            {collectedPot}
            {sidePots}
            {allInMoment}
            allInCount={allInSeats.length}
            compact={portrait && seatCount >= 8}
            {streetLabel}
            {equityMethodLabel}
            equityNote={equityTooltip}
            winners={lastWinners}
            {myWinInfo}
            {isHandComplete}
            {currencySymbol}
            {fmt}
            {seatLabel}
            {handRankWords}
          />
          {#if gameInProgress || isShowdown || communityCards.length > 0}
            <BoardStrip cards={communityCards} />
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
          <!-- THE READOUT SPOKE of a landscape top or bottom seat takes the
               plate end that is clear of the seated neighbours (spokeEndBySeat);
               every other seat keeps the ring's own spoke. -->
          {@const rdx = spokeEndBySeat[i] ?? point.rdx}
          <div
            class="seat seat-{point.side}"
            class:occupied={!!player}
            class:flank={point.flank}
            class:lower={point.sn > 0.2}
            class:upper={point.sn < -0.2}
            class:award-on-spoke={point.awardOnSpoke}
            class:spoke-y={point.rdy !== 0}
            class:spoke-ends={point.spokeEnds}
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
            style:--rdx={rdx}
            style:--rdy={point.rdy}
            style:--px={point.px}
            style:--py={point.py}
            style:--pkx={point.pkx}
            style:--pky={point.pky}
            style:--cy={point.cy}
            style:--shx={point.shx ?? 0}
            style:--akx={point.akx}
            style:--aky={point.aky}
          >
            <SeatPod
              {player}
              seatIndex={i}
              seatLabel={seatLabel(i)}
              name={getShortName(player, i)}
              {isHero}
              {acting}
              {live}
              lit={live && (allInMoment || isShowdown)}
              folded={!!player?.has_folded}
              winner={!!win}
              dealer={!!player && i === Number(dealerSeat)}
              puckFrom={puckFrom && puckFrom.seat === i ? puckFrom : null}
              showCards={gameInProgress || isShowdown}
              heroCards={myCards}
              heroHandName={isHero && (gameInProgress || isShowdown) ? heroHandName : null}
              revealed={isHero ? null : revealedHole(player)}
              betAmount={Number(player?.current_bet ?? 0)}
              betAllIn={!!player?.is_all_in}
              bigBlind={bigBlindRaw}
              {timeRemaining}
              {clockFraction}
              {clockUrgent}
              {equityText}
              equityModelled={equityMode === 'hero'}
              equityNote={equityTooltip}
              awardAmount={win ? `+${fmt(Number(win.amount))}` : null}
              handTag={win ? handRankWords(win.hand_rank) : ''}
              awardOnSpoke={point.awardOnSpoke}
              spokeY={point.rdy !== 0}
              spokeEnds={point.spokeEnds}
              {fmt}
              onJoin={(seat) => onAction('join', seat)}
            />
          </div>
        {/each}

        <!-- ============ the two flights (bar 11: 500 ms, transform only) ====
             GHOSTS, not the real elements: a copy carrying the last amount the
             chain reported, so no figure on the felt ever shows a stale number
             in order to animate. -->
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

        <!-- bet-sizing popover, anchored above the dock (behaviour: phase 2) -->
        {#if showRaiseSlider && isMyTurn && gameInProgress}
          <div class="raise-slider-panel">
            <div class="slider-header">
              <span>{currentBet === 0 ? 'Bet' : 'Raise to'}</span>
              <button class="close-slider" onclick={() => showRaiseSlider = false} aria-label="Close">&times;</button>
            </div>
            <div class="slider-amount cd-money">{fmt(raiseAmount)}</div>
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
        <!-- Sit out / Leave live in the LEFT cell (docs/DESIGN-BAR.md section
             11.6): the right cell carries the wallet, and a track sized by
             symmetry cannot hold five controls and a money panel. Measured:
             the Leave button rendered 7 px past the window's right edge. -->
        {#if mySeat !== null}
          <div class="sit-controls">
            <button class="control-btn" onclick={() => onAction(isSittingOut ? 'sitIn' : 'sitOut')}>
              {isSittingOut ? 'Sit in' : 'Sit out'}
            </button>
            <button class="control-btn destructive" onclick={() => onAction('leave')}>Leave</button>
          </div>
        {/if}
      </div>

      <div class="dock-center">
        {#if isMyTurn && callAmount > 0 && potOdds()}
          <div class="pot-odds-display">
            <span class="pot-odds-label">Pot odds</span>
            <span class="pot-odds-value">{potOdds()}</span>
            <!-- Spelled out in money, so the strip and the headline pot can be
                 checked against the same get_pot() (docs/DEFECTS.md T-08). -->
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
            <!-- IN FLOW, directly under the balance it corrects. Never an overlay:
                 nothing in this app may cover the notices (HARD RULE 2). -->
            {#if myCommittedInPot > 0}
              <div class="wallet-committed" class:stuck={handIsUnmovable}>
                <span class="committed-label">
                  {handIsUnmovable ? 'Stuck in the pot' : 'In the pot'}
                </span>
                <span class="committed-value">{formatWithUnit(myCommittedInPot)}</span>
                <span class="committed-note">
                  {#if handIsUnmovable}
                    This hand cannot move, so nobody can win it. Open Withdraw to get it back.
                  {:else if mySeat === null}
                    You have left this table and this is still yours in hand {handNumber}.
                  {:else}
                    Yours, in hand {handNumber}, until the hand settles.
                  {/if}
                </span>
              </div>
            {/if}
            <div class="wallet-actions">
              {#if onShowDeposit}
                <button class="wallet-action-btn deposit" onclick={onShowDeposit}>Deposit</button>
              {/if}
              {#if onShowWithdraw}
                <!-- ALSO OPEN WHEN THE BALANCE IS ZERO AND A STAKE IS OUTSTANDING
                     (docs/SECURITY-FINDINGS.md FINDING 18). -->
                <button
                  class="wallet-action-btn withdraw"
                  onclick={onShowWithdraw}
                  disabled={tableBalance <= 0 && myCommittedInPot <= 0}
                >
                  Withdraw
                </button>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style lang="scss">
  /* The table's geometry tokens and the dock live in two partials next to
     this file, included below at the places their rules used to sit (the
     cascade is order-sensitive) and scoped by Svelte like everything else. */
  @use './poker-table-tokens' as tokens;
  @use './poker-table-dock' as dock;

  @include tokens.tokens;

  .poker-table {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--cd-space-2);
    width: 100%;
    min-width: 0;
    min-height: 0;
  }

  /* THE ROOM. One overhead pool of light centred on the felt, falling to black
     within a table-width. The page's ambient orbs are hidden on the table view
     (+page.svelte) so there is exactly one light source in the scene. */
  .stage {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    container-type: size;
    border-radius: var(--cd-radius-panel);
    background:
      radial-gradient(ellipse 70% 80% at 50% 50%, var(--cd-stage-hi) 0%, var(--cd-stage) 55%, var(--cd-stage-lo) 100%);
  }

  .table-inner {
    position: absolute;
    inset: 0;
    /* THE single scale input. Both caps are the stage box divided by what the
       RING needs (the pods hang off the felt on every side, and the avatar now
       breaks the plate's outer edge by half its width):
         width  needs  ring-kx + pod-w-r + avatar-r = 1.02 + 0.235 + 0.066 = 1.32 -> 75cqw
         height needs  ring-ky / ar + pod-h-r        = 0.476 + 0.086         = 0.562 -> 177cqh */
    --fw: min(75cqw, 177cqh);
    --fh: calc(var(--fw) / var(--ar));
    --rx: calc(var(--fw) * 0.5 * var(--ring-kx));
    --ry: calc(var(--fh) * 0.5 * var(--ring-ky));
    --pod-w: calc(var(--fw) * var(--pod-w-r));
    --pod-h: calc(var(--fw) * var(--pod-h-r));
    --avatar: calc(var(--fw) * var(--avatar-r));
    --ui: calc(var(--fw) * var(--ui-r));
    font-size: var(--ui);
  }

  /* 1.02 + 0.202 + 0.056 = 1.278 -> 78cqw ; 0.476 + 0.074 = 0.550 -> 181cqh */
  .ring-crowded .table-inner { --fw: min(78cqw, 181cqh); }
  /* 1.02 + 0.262 + 0.072 = 1.354 -> 73cqw ; 0.476 + 0.094 = 0.570 -> 175cqh */
  .ring-sparse  .table-inner { --fw: min(73cqw, 175cqh); }

  /* The all-in vignette: the room darkens around the surface. Opacity only. */
  .table-inner::after {
    content: '';
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    border-radius: var(--cd-radius-panel);
    background: radial-gradient(ellipse 62% 62% at 50% 50%, transparent 45%, var(--cd-felt-shade) 100%);
    transition: opacity var(--cd-move) var(--cd-ease);
    z-index: 3;
  }

  .table-inner.all-in-moment::after { opacity: 1; }

  /* ---------- the rail ---------- */

  .rail {
    position: absolute;
    left: 50%;
    top: 50%;
    width: calc(var(--fw) + 2 * var(--fw) * var(--rail-side-r));
    height: calc(var(--fh) + var(--fw) * (var(--rail-top-r) + var(--rail-bottom-r)));
    /* Offset down by half the difference between the near and far rims, so
       the felt sits high in the rail: the foreshortened view from a chair. */
    transform: translate(-50%, calc(-50% + var(--fw) * (var(--rail-bottom-r) - var(--rail-top-r)) * 0.5));
    border-radius: 50%;
    /* A padded roll seen from a raised chair: the far rail is a thin lit
       crown; the near rail is a broad top surface (lit, --cd-rail) that
       rounds over into a dark front face (--cd-rail-face) at the very edge. */
    background:
      linear-gradient(180deg,
        var(--cd-rail-hi) 0%, var(--cd-rail) 16%, var(--cd-rail-lo) 46%,
        var(--cd-rail) 78%, var(--cd-rail-hi) 92%, var(--cd-rail-face) 100%);
    box-shadow:
      /* the specular along the top of the padding */
      inset 0 3px 2px var(--cd-rail-crown),
      /* the roll's rim on the near edge */
      inset 0 -2px 1px var(--cd-rail-crown-lo),
      /* the padding's own roundness, darker toward the felt */
      inset 0 calc(var(--fw) * -0.012) calc(var(--fw) * 0.02) var(--cd-felt-shade),
      /* the table's contact shadow on the room */
      var(--cd-shadow-rail);
  }

  /* ---------- felt ---------- */

  .felt {
    position: absolute;
    left: 50%;
    top: 50%;
    width: var(--fw);
    height: var(--fh);
    transform: translate(-50%, -50%);
    border-radius: 50%;               /* bar 2: an ellipse, not a rectangle */
    /* Key light above centre; a fibre nap from the noise tile; the edge falls
       into the rail's shadow. */
    background:
      var(--cd-noise),
      radial-gradient(ellipse 58% 72% at 50% 32%, var(--cd-felt-hi) 0%, var(--cd-felt) 46%, var(--cd-felt-lo) 100%);
    background-blend-mode: overlay, normal;
    box-shadow:
      /* the dark seam where the felt meets the rail lip */
      0 0 0 calc(var(--fw) * 0.006) var(--cd-rail-seam),
      /* vignette: the surface darkens toward the rail, in two falloffs */
      inset 0 0 calc(var(--fw) * 0.13) var(--cd-felt-shade),
      inset 0 0 calc(var(--fw) * 0.03) var(--cd-felt-edge),
      inset 0 calc(var(--fw) * 0.012) calc(var(--fw) * 0.05) var(--cd-felt-edge);
  }

  /* The betting line: a thin ring inset from the edge, as on every casino felt. */
  .felt::before {
    content: '';
    position: absolute;
    inset: calc(var(--fh) * 0.16) calc(var(--fw) * 0.075);
    border-radius: 50%;
    border: 1px solid var(--cd-felt-line);
    pointer-events: none;
  }

  /* One centre mark, low contrast, UNDER the board. It fades out while a
     board is out: cards on top of a watermark read as a page, not a table. */
  .felt-marks {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0;
    white-space: nowrap;
    pointer-events: none;
    color: var(--cd-felt-mark);
    transition: opacity var(--cd-move) var(--cd-ease);
  }

  .table-inner.has-board .felt-marks { opacity: 0; }

  .mark-1 {
    font-size: calc(var(--fw) * 0.16);
    line-height: 1;
  }

  .mark-2 {
    font-size: var(--cd-felt-small);
    letter-spacing: 0.42em;
    text-indent: 0.42em;
    text-transform: uppercase;
    font-weight: var(--cd-weight-figure);
    margin-top: calc(var(--fw) * -0.01);
  }

  /* ---------- board + pot cluster ---------- */

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

  /* =========================================================================
     SEATS -- each seat is a zero-size point on the ring; SeatPod.svelte draws
     everything off it with the seat's own vectors.
     ========================================================================= */

  .seat {
    /* --sx / --sy MUST be declared here, not on .table-inner: custom properties
       inherit as COMPUTED values. */
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

  /* A FOLDED SEAT IS OUT, AND HAS TO LOOK OUT: dimmed here, drained of colour
     in the plate itself (SeatPod). */
  .seat.folded { opacity: 0.42; }

  /* ---- the two flights: ghost copies painted over the felt ---- */

  .chip-flight,
  .pot-flight {
    position: absolute;
    left: 50%;
    top: 50%;
    z-index: 23;
    pointer-events: none;
    padding: 0.14em 0.6em;
    border-radius: var(--cd-radius-pill);
    font-weight: var(--cd-weight-display);
    font-size: var(--cd-felt-body);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    background: var(--cd-capsule);
    color: var(--cd-money);
    border: 1px solid var(--cd-money-line);
    box-shadow: var(--cd-shadow-chip);
  }

  /* seat chip -> middle */
  .chip-flight {
    --fx: calc(var(--rx) * var(--cs, 0) + var(--bx, 0) * var(--fw));
    --fy: calc(var(--ry) * var(--sn, 0) + var(--by, 0) * var(--fw));
    animation: chip-to-pot var(--cd-move) var(--cd-ease-move) both;
  }

  @keyframes chip-to-pot {
    from { transform: translate(-50%, -50%) translate(var(--fx), var(--fy)) scale(1); opacity: 1; }
    85%  { opacity: 1; }
    to   { transform: translate(-50%, -50%) scale(0.55); opacity: 0; }
  }

  /* middle -> winner's pod. 360 ms behind the chip sweep; must equal
     POT_FLIGHT_DELAY_MS in the script block. */
  .pot-flight {
    background: linear-gradient(180deg, var(--cd-money-hi), var(--cd-money));
    color: var(--cd-money-ink);
    box-shadow: 0 0 calc(var(--fw) * 0.04) var(--cd-money-glow);
    animation: pot-to-winner var(--cd-move) var(--cd-ease-move) 0.36s both;
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
    border-radius: var(--cd-radius-pill);
    background: var(--cd-panel);
    border: 1px solid var(--cd-warn-line);
    color: var(--cd-ink-1);
    font-size: 0.85em;
  }

  .sit-in-btn {
    padding: 0.25em 0.7em;
    border-radius: var(--cd-radius-pill);
    border: none;
    background: var(--cd-money);
    color: var(--cd-money-ink);
    font-weight: var(--cd-weight-display);
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
    border-radius: var(--cd-radius-panel);
    background: var(--cd-panel);
    border: 1px solid var(--cd-line-strong);
    box-shadow: var(--cd-shadow-lift);
    font-size: var(--cd-text-md);
  }

  .slider-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 0.8em;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--cd-ink-2);
  }

  .close-slider {
    width: 22px;
    height: 22px;
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line-strong);
    background: transparent;
    color: var(--cd-ink-1);
    cursor: pointer;
    line-height: 1;
  }

  .slider-amount {
    text-align: center;
    font-size: 1.9em;
    font-weight: var(--cd-weight-display);
    color: var(--cd-money);
  }

  .raise-slider {
    width: 100%;
    accent-color: var(--cd-money);
  }

  .preset-buttons { display: flex; gap: var(--cd-space-2); }

  .preset-buttons button {
    flex: 1;
    padding: 7px 0;
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-2);
    color: var(--cd-ink-1);
    font-size: 0.82em;
    font-weight: var(--cd-weight-figure);
    cursor: pointer;
  }

  .preset-buttons button:hover { background: var(--cd-money-dim); }

  .confirm-raise {
    padding: 10px 0;
    border-radius: var(--cd-radius-chip);
    border: none;
    background: linear-gradient(180deg, var(--cd-money-hi), var(--cd-money-lo));
    color: var(--cd-money-ink);
    font-weight: var(--cd-weight-display);
    font-size: 0.95em;
    cursor: pointer;
  }

  @include dock.dock;

  /* =========================================================================
     PORTRAIT -- switched on ASPECT RATIO, not width (bar 19): a 0.555 stadium
     that takes the whole phone, pods ON the felt in two columns, the pot under
     the board, a two-row dock. The numbers are docs/DESIGN-BAR.md section 11.
     ========================================================================= */

  @media (max-aspect-ratio: 1/1) {
    /* FULL BLEED: the wrapper takes the whole viewport width back. */
    .poker-table-wrapper {
      width: 100vw;
      max-width: 100vw;
      margin-left: calc(50% - 50vw);
      margin-right: calc(50% - 50vw);
    }

    .stage { border-radius: 0; }
    .table-inner::after { border-radius: 0; }

    /* THE MIDDLE OF THE TABLE OUTRANKS AN EMPTY CHAIR on a narrow felt. */
    .board-cluster { z-index: 26; }

    /* bar 20: a tall ROUNDED RECTANGLE, not an ellipse. */
    .felt { border-radius: calc(var(--fw) * 0.28); }
    .felt::before { border-radius: calc(var(--fw) * 0.22); inset: calc(var(--fw) * 0.07); }
    .rail { border-radius: calc(var(--fw) * 0.33); }
    .mark-1 { font-size: calc(var(--fw) * 0.3); }
    .mark-2 { display: none; }

  }

  @media (prefers-reduced-motion: reduce) {
    .felt, .turn-indicator, .table-inner::after, .action-btn { transition: none; }
    .action-btn:hover { transform: none; }
    .spinner { animation: none; }
    /* THE TWO FLIGHTS STOP DEAD, AND NOTHING IS LOST: the chips are counted
       into `.pot-amount`, the award is a static chip on the winner's pod. A
       ghost that never travels would be a duplicate figure, so they are hidden
       rather than zero-length. */
    .chip-flight, .pot-flight { display: none; }
  }
</style>
