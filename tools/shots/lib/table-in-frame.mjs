// IS THE WHOLE TABLE INSIDE THE PHONE? AND WHERE DID THE HEIGHT GO?
//
// WHY THIS FILE EXISTS. `lib/felt-area.mjs` asserts one rectangle: the felt is
// big enough and inside the frame. It says nothing about the objects a player
// actually reads, and they are not the felt:
//
//   the pot          the only figure that says what the hand is worth
//   five board slots the hand itself
//   every seat pod   who is in, for how much, and whose turn it is
//   the action row   the most-used control in the product
//
// All four can leave the frame while the felt stays inside it and every gate in
// this repo stays green, because the felt is centred in the stage and those four
// are positioned off it: the pods straddle the rail (`--ring-kx > 1` in
// landscape), the pot rides under the board on a `--cluster-dy-r` offset, and the
// action dock is the felt's SIBLING, so it is sized by what is left rather than
// by what fits. docs/DESIGN-BAR.md bar 23 was written after exactly this: the pot,
// the whole board and four of six pods were above the top of the frame at
// 390x844 while every DOM assertion passed.
//
// WHAT IT ASSERTS.
//   * the pot readout (or the winner line that replaces it) is fully in frame
//   * `.community-cards` holds EXACTLY five slots and each is fully in frame
//   * every `.seat` is fully in frame
//   * every action button is fully in frame
//   * on a table view, none of those sets is EMPTY
//
// That last clause is the one that matters most and is the easiest to leave out.
// A probe that walks a selector matching nothing reports no problems, which is
// indistinguishable from a passing table; six suites in this repo were once
// auto-discovered and named by no target and one of them sat red for a wave. So
// a view with a `.felt` and no seats, or a board with four slots instead of five,
// is a FAILURE here, not a silent skip.
//
// WHAT IT ONLY RECORDS.
//   * the VERTICAL BUDGET: every block between the top of the frame and the top
//     of the felt, and between the bottom of the felt and the bottom of the
//     frame, with its measured height. This is the arithmetic behind any claim
//     about how large the playing surface could be, and it was previously done
//     by hand in a scratchpad, once, by the author of the change.
//   * THUMB REACH: where the action row sits, as a fraction of frame height.
//
// Recording rather than asserting the budget is deliberate. A floor on chrome
// height would be a second way of saying "make the felt bigger", and this repo
// already learned what happens when the only pressure points that way: wave 4
// bought 51.9% by putting all four protected notices off the screen. The budget
// is published so a reader can check the ceiling claim; the notices are asserted
// by lib/protected-notices.mjs and are not negotiable against it.

/**
 * Thumb reach. On a 390x844 phone held one-handed, the pad of the thumb sweeps
 * an arc that comfortably reaches the bottom ~60% of the screen and strains
 * above it. The action row is the most-used control in a poker client, so it is
 * measured against the bottom edge and RECORDED as a fraction.
 */
export const THUMB_ZONE_TOP_FRACTION = 0.40;

/**
 * Measures the objects a player reads, and the vertical budget around them.
 *
 * @param {import('playwright').Page} page
 */
export function measureTableInFrame(page) {
  return page.evaluate(() => {
    const vw = window.innerWidth;
    const vh = window.innerHeight;

    const box = (el) => {
      const r = el.getBoundingClientRect();
      return {
        x: +r.x.toFixed(1), y: +r.y.toFixed(1),
        w: +r.width.toFixed(1), h: +r.height.toFixed(1),
        right: +r.right.toFixed(1), bottom: +r.bottom.toFixed(1),
      };
    };
    // "Inside the frame" is measured with a 1 px tolerance, because a rounded
    // corner or a transform can put a box at -0.4 and that is not off screen.
    const inFrame = (b) => b.y >= -1 && b.bottom <= vh + 1 && b.x >= -1 && b.right <= vw + 1;
    const path = (el) => {
      const parts = [];
      for (let n = el, d = 0; n && d < 3 && n !== document.body; n = n.parentElement, d += 1) {
        parts.unshift(`${n.tagName.toLowerCase()}${[...n.classList]
          .filter((c) => !c.startsWith('svelte-')).slice(0, 2).map((c) => `.${c}`).join('')}`);
      }
      return parts.join(' > ');
    };
    // A display:none box is 0x0 at 0,0 and would read as "in frame". Anything
    // this module reports on has to be something a player can actually see.
    const painted = (el) => {
      const r = el.getBoundingClientRect();
      if (r.width < 1 || r.height < 1) return false;
      if (typeof el.checkVisibility === 'function') {
        return el.checkVisibility({ visibilityProperty: true, opacityProperty: true });
      }
      const s = getComputedStyle(el);
      return s.display !== 'none' && s.visibility !== 'hidden' && Number(s.opacity) > 0.05;
    };

    const felt = document.querySelector('.felt');
    if (!felt) return null;
    const feltBox = box(felt);

    const group = (selector, label, { via } = {}) => {
      const els = [...document.querySelectorAll(selector)].filter(painted);
      return {
        label,
        selector,
        via: via || null,
        count: els.length,
        items: els.map((el) => {
          const b = box(el);
          return { path: path(el), box: b, inFrame: inFrame(b) };
        }),
      };
    };

    // THE POT. `.pot-display` is replaced by `.winner-display` once the hand is
    // complete, so the check is "whichever one of the two is on screen", and
    // neither being on screen on a table view is a failure rather than a skip.
    const pot = (() => {
      const g = group('.pot-display, .winner-display', 'pot readout');
      return g;
    })();

    // THE BOARD. The contract PokerTable.svelte states in its own comment is that
    // `.community-cards` is the direct parent of EXACTLY five board slots. This
    // reads that contract rather than trusting it: `slotCount` is asserted to be
    // 5 whenever the board is rendered at all.
    // WHETHER A HAND IS DEALT AT ALL, so "no board" can be CHECKED rather than
    // skipped. PokerTable.svelte renders `.community-cards` under
    // `gameInProgress || isShowdown || communityCards.length > 0`, and it draws a
    // seat's `.player-cards` under the same predicate, so a painted card at any
    // seat implies the board must exist. On `table-empty` and behind the deposit
    // dialog there is legitimately no hand and no board; on any shot where cards
    // are out, a missing board is a failure.
    const seatCards = [...document.querySelectorAll('.seat .player-cards .card')]
      .filter(painted).length;

    const boardWrap = document.querySelector('.community-cards');
    const board = (() => {
      if (!boardWrap) return { rendered: false, slotCount: 0, items: [] };
      const slots = [...boardWrap.children];
      return {
        rendered: true,
        slotCount: slots.length,
        items: slots.map((el, i) => {
          const b = box(el);
          return { index: i, box: b, inFrame: inFrame(b), painted: painted(el) };
        }),
      };
    })();

    // THE SEAT POD IS NOT THE SEAT ELEMENT.
    //
    // `.seat` is `width: 0; height: 0` — a point on the ring, with the plate,
    // the cards, the committed chips and the equity badge positioned OFF it (see
    // the "SEATS" block in PokerTable.svelte). Measuring `.seat` therefore
    // measures a zero-size box at the ring point, which is inside the frame by
    // construction and always passes. The first version of this module did
    // exactly that and reported "0 seat pods, all in frame" on all six mobile
    // scenes while pods were visibly on the edge — a gate green because it was
    // looking at nothing, which is the failure this repo keeps re-learning.
    //
    // What a player sees is the UNION of the seat's painted descendants: the
    // plate for an occupied seat, the Sit button for an open one, plus whatever
    // hangs off them. That union is what has to be inside the frame.
    const seats = (() => {
      const els = [...document.querySelectorAll('.seat')];
      const items = [];
      let renderingNothing = 0;
      for (const seat of els) {
        const parts = [...seat.querySelectorAll('*')].filter((el) => {
          if (!painted(el)) return false;
          // Only leaf-ish carriers, so a wrapper's box does not stand in for its
          // children's. Anything whose own box is bigger than its parent's still
          // counts, because that is precisely how a pod escapes the frame.
          return true;
        });
        if (!parts.length) { renderingNothing += 1; continue; }
        let x0 = Infinity; let y0 = Infinity; let x1 = -Infinity; let y1 = -Infinity;
        for (const el of parts) {
          const b = box(el);
          x0 = Math.min(x0, b.x); y0 = Math.min(y0, b.y);
          x1 = Math.max(x1, b.right); y1 = Math.max(y1, b.bottom);
        }
        const b = {
          x: +x0.toFixed(1), y: +y0.toFixed(1),
          w: +(x1 - x0).toFixed(1), h: +(y1 - y0).toFixed(1),
          right: +x1.toFixed(1), bottom: +y1.toFixed(1),
        };
        items.push({
          path: path(seat),
          occupied: seat.classList.contains('occupied'),
          box: b,
          inFrame: inFrame(b),
          parts: parts.length,
        });
      }
      return {
        label: 'seat pod',
        selector: '.seat (union of its painted descendants)',
        seatElements: els.length,
        renderingNothing,
        count: items.length,
        items,
      };
    })();
    // EVERY BUTTON IN THE DOCK has to be inside the frame — Deposit and Withdraw
    // included, because docs/SECURITY-FINDINGS.md FINDING 18 turns on Withdraw
    // being reachable while a stake is committed.
    const actions = group('.action-dock button', 'dock button');
    // THUMB REACH IS ABOUT ONE ROW, NOT ALL OF THEM. `.actions .action-btn` is
    // Fold / Check / Call / Raise / All In: the control a player presses several
    // times a hand. Measuring every dock button instead reports the Log toggle's
    // y, which sits ~85 px higher and made the reach look worse than it is.
    const primaryActions = group('.actions .action-btn', 'primary action button');

    // THE VERTICAL BUDGET. Everything between the top of the frame and the top of
    // the felt, and between the bottom of the felt and the bottom of the frame,
    // as the blocks a reader can name. Walked from the DOM rather than listed by
    // hand so a block added tomorrow appears here without anyone remembering to.
    const budget = (() => {
      const named = [
        ['.banner-strip', 'compact notice strip (portrait, table view)'],
        ['.alpha-warning-banner', 'full protected-notice banner'],
        ['.table-header', 'table header'],
        ['.header-left', 'table header, left'],
        ['.header-right', 'table header, right'],
        ['.action-dock', 'action dock (whole)'],
        ['.wallet-strip', 'wallet strip'],
        ['.pot-odds-display', 'pot odds line'],
        ['.action-buttons', 'action button row'],
        ['.bet-sizing', 'bet sizing controls'],
        ['.footer-disclaimer', 'footer disclaimer'],
      ];
      const rows = [];
      for (const [sel, label] of named) {
        for (const el of document.querySelectorAll(sel)) {
          if (!painted(el)) continue;
          const b = box(el);
          rows.push({
            label,
            selector: sel,
            box: b,
            where: b.bottom <= feltBox.y + 1 ? 'above the felt'
              : b.y >= feltBox.bottom - 1 ? 'below the felt' : 'overlapping the felt',
          });
        }
      }
      return {
        aboveFeltPx: +feltBox.y.toFixed(1),
        belowFeltPx: +(vh - feltBox.bottom).toFixed(1),
        blocks: rows,
      };
    })();

    // THUMB REACH, on the primary action row only.
    const thumb = primaryActions.items.length ? (() => {
      const top = Math.min(...primaryActions.items.map((a) => a.box.y));
      const bottom = Math.max(...primaryActions.items.map((a) => a.box.bottom));
      const minH = Math.min(...primaryActions.items.map((a) => a.box.h));
      const minW = Math.min(...primaryActions.items.map((a) => a.box.w));
      return {
        buttons: primaryActions.items.length,
        actionRowTopPx: +top.toFixed(1),
        actionRowTopFraction: +(top / vh).toFixed(3),
        actionRowBottomGapPx: +(vh - bottom).toFixed(1),
        // 44 CSS px is the smallest touch target either platform guideline
        // accepts. RECORDED, not asserted, because landscape phones legally run
        // a shorter dock.
        smallestButton: { w: +minW.toFixed(1), h: +minH.toFixed(1) },
        meets44px: minH >= 44 && minW >= 44,
        inThumbZone: top / vh >= 0.40,
      };
    })() : null;

    return {
      viewport: { w: vw, h: vh },
      felt: feltBox,
      pot,
      board,
      seatCards,
      seats,
      actions,
      primaryActions,
      budget,
      thumb,
      scrollY: Math.round(window.scrollY),
      documentScrollWidth: document.documentElement.scrollWidth,
    };
  });
}

/**
 * Folds the measurement into a scene verdict fragment.
 *
 * @param {object|null} m result of `measureTableInFrame`
 * @param {{viewport:string, scene:string}} where
 */
export function foldTableInFrame(m, { viewport, scene }) {
  if (!m) {
    return { ok: true, measurement: null, problems: [], notes: 'no .felt on this view (not a table)' };
  }
  const problems = [];
  const frame = `${m.viewport.w}x${m.viewport.h}`;
  const off = (label, item) =>
    `${label} is NOT fully inside the ${frame} frame: `
    + `${item.box.w}x${item.box.h} at (${item.box.x},${item.box.y}), `
    + `right ${item.box.right}, bottom ${item.box.bottom}`;

  // A TABLE VIEW WITH NO POT, NO BOARD OR NO SEATS IS A FAILURE, NOT A SKIP.
  if (m.pot.count === 0) {
    problems.push(
      'no pot readout on a table view: neither .pot-display nor .winner-display is painted. '
      + 'A probe that matches nothing reports no problems, which is why this is a failure',
    );
  }
  for (const item of m.pot.items) if (!item.inFrame) problems.push(off('the pot readout', item));

  if (m.board.rendered) {
    if (m.board.slotCount !== 5) {
      problems.push(
        `.community-cards holds ${m.board.slotCount} slot(s), not 5. PokerTable.svelte states `
        + 'in its own comment that this element is the direct parent of exactly the five board '
        + 'cards, and lib/dom-scrape.mjs reads the board through that contract',
      );
    }
    m.board.items.forEach((slot, i) => {
      if (!slot.inFrame) problems.push(off(`board slot ${i + 1} of ${m.board.slotCount}`, slot));
    });
  } else if (m.seatCards > 0) {
    // THE SKIP THAT WAS SILENT. This module's own header says "a board with four
    // slots instead of five is a FAILURE here, not a silent skip", and for four
    // of the twenty-four shots the whole board block was skipped with no
    // statement either way, because `table-empty` and the deposit dialog have no
    // hand in progress. Legitimate there, and indistinguishable from
    // `.community-cards` having stopped rendering on a dealt board. The seat
    // cards settle it: PokerTable.svelte draws them under the same predicate as
    // the board, so cards out with no board is a contradiction.
    problems.push(
      `${m.seatCards} hole card(s) are painted at the seats and .community-cards is NOT `
      + 'rendered. PokerTable.svelte draws both under `gameInProgress || isShowdown`, so a '
      + 'dealt hand with no board element means the board stopped rendering, not that no hand '
      + 'is in progress',
    );
  }

  // A ring with no seat elements, seat elements that paint nothing, or a seats
  // block that is not the union measurement at all, is a failure and not a skip:
  // all three look identical to "every pod is in frame".
  //
  // THE THIRD CASE IS THIS MODULE'S OWN HEADLINE CONVICTION, AND IT WAS UNGUARDED.
  // `.seat` is a 0x0 point on the ring, so measuring it directly reports boxes
  // that are inside the frame by construction. `seats` is therefore built from the
  // UNION of each seat's painted descendants. Restore the naive measurement and
  // the block loses `seatElements`/`renderingNothing` and reports zero items --
  // and every check below is written over items, so the verdict came back
  // "all in frame" on nothing at all. The shape is now asserted.
  if (typeof m.seats.seatElements !== 'number' || typeof m.seats.renderingNothing !== 'number') {
    problems.push(
      'the seats measurement is not the seat-pod union: it carries no seatElements/'
      + 'renderingNothing count. `.seat` is a 0x0 point on the ring and is inside the frame by '
      + 'construction, so a measurement of `.seat` itself cannot fail and this check would be '
      + 'green about nothing',
    );
  }
  if (m.seats.seatElements === 0) {
    problems.push('no seat ring on a table view: .seat matched nothing');
  }
  if (m.seats.seatElements > 0 && m.seats.count === 0) {
    problems.push(
      `${m.seats.seatElements} seats are on the ring and NOT ONE produced a measurable pod. `
      + 'Nothing below this line can fail when the set it iterates is empty',
    );
  }
  if (m.seats.renderingNothing > 0) {
    problems.push(
      `${m.seats.renderingNothing} of ${m.seats.seatElements} seats on the ring paint nothing at `
      + 'all — neither a plate nor a Sit button. An empty box cannot be off frame, so a seat that '
      + 'renders nothing would pass this check silently',
    );
  }
  m.seats.items.forEach((item, i) => {
    if (!item.inFrame) {
      problems.push(off(
        `seat pod ${i + 1} of ${m.seats.items.length} (${item.occupied ? 'occupied' : 'open'})`,
        item,
      ));
    }
  });

  if (m.actions.count > 0) {
    for (const item of m.actions.items) if (!item.inFrame) problems.push(off('a dock button', item));
  }

  const notes = `pot ${m.pot.count}, board ${m.board.rendered ? `${m.board.slotCount} slots`
    : `not rendered and ${m.seatCards} hole cards painted (checked, not skipped)`}, `
    + `${m.seats.count}/${m.seats.seatElements} seat pods, ${m.actions.count} action buttons: `
    + `${problems.length ? `${problems.length} OFF FRAME` : 'all in frame'}; `
    + `chrome ${m.budget.aboveFeltPx}px above the felt + ${m.budget.belowFeltPx}px below; `
    + (m.thumb
      ? `${m.thumb.buttons} action buttons, row top at `
        + `${(m.thumb.actionRowTopFraction * 100).toFixed(0)}% down the frame, `
        + `${m.thumb.actionRowBottomGapPx}px above the bottom edge, smallest `
        + `${m.thumb.smallestButton.w}x${m.thumb.smallestButton.h}`
        + `${m.thumb.meets44px ? '' : ' (UNDER the 44px touch target)'}`
      : 'no primary action row on this view (not the hero\'s turn)');

  return { ok: problems.length === 0, measurement: m, problems, notes, scene, viewport };
}
