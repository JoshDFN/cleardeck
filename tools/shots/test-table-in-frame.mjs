// node tools/shots/test-table-in-frame.mjs   (`./scripts/dev.sh shots-selftest`)
//
// THE GATE ON THE GATE — docs/DEFECTS.md H-51.
//
// `lib/table-in-frame.mjs` landed in wave 12 and convicted three times, twice on
// defects no other instrument in this repository can see. It was also the only
// new gate of that wave with NOTHING running it: `grep -rn table-in-frame tools
// scripts Makefile` returned exactly one hit, `run.mjs:41`, and `run.mjs` needs a
// live replica and a browser. That is precisely the condition
// `artifacts/screens/acknowledged-reds.json` blames for E-63 sitting red through a
// whole wave.
//
// Worse than unrun: its own headline conviction was unprotected. `.seat` is
// `width: 0; height: 0` — a point on the ring — so measuring it directly reports
// boxes that are inside the frame by construction. The module measures the UNION
// of each seat's painted descendants instead. Restore the naive version and the
// verdict came back "9/9 seat pods, all in frame" over nine zero-size boxes, with
// every named target green. The module was written to fix exactly that failure and
// could suffer it again silently.
//
// WHAT THIS FILE TESTS, AND WHY IT NEEDS NEITHER REPLICA NOR BROWSER.
// `measureTableInFrame` is the half that needs a page; `foldTableInFrame` is a
// pure function from a measurement to a verdict, and every judgement the module
// makes lives in it. So the cases here are measurement objects whose right answer
// is known by construction — a healthy table, and then one planted defect at a
// time. A case that fails to go RED is a hole in the gate.

import { foldTableInFrame } from './lib/table-in-frame.mjs';

let failures = 0;
const results = [];

function check(name, cond, detail = '') {
    results.push({ name, ok: Boolean(cond), detail });
    if (!cond) failures += 1;
}

const WHERE = { viewport: 'mobile', scene: 'test-table' };
const VW = 390;
const VH = 844;

/** A box that is fully inside the frame. */
const inBox = (x = 10, y = 10, w = 60, h = 30) => ({
    x, y, w, h, right: x + w, bottom: y + h,
});
/** The same box pushed past the right edge. */
const outBox = (w = 60, h = 30) => ({
    x: VW - 10, y: 10, w, h, right: VW - 10 + w, bottom: 10 + h,
});

/** A measurement of a healthy 6-max table mid-hand. */
function healthy() {
    const seat = (i, occupied) => ({
        path: `div.seat.seat-${i}`,
        occupied,
        box: inBox(20 + i * 40, 100, 60, 44),
        inFrame: true,
        parts: 5,
    });
    return {
        viewport: { w: VW, h: VH },
        felt: { x: 40, y: 130, w: 310, h: 560, right: 350, bottom: 690 },
        pot: {
            label: 'pot readout',
            selector: '.pot-display, .winner-display',
            via: null,
            count: 1,
            items: [{ path: 'div.pot-display', box: inBox(150, 300, 90, 40), inFrame: true }],
        },
        board: {
            rendered: true,
            slotCount: 5,
            items: [0, 1, 2, 3, 4].map((i) => ({
                index: i,
                box: inBox(60 + i * 45, 360, 40, 56),
                inFrame: true,
                painted: true,
            })),
        },
        seatCards: 12,
        seats: {
            label: 'seat pod',
            selector: '.seat (union of its painted descendants)',
            seatElements: 6,
            renderingNothing: 0,
            count: 6,
            items: [0, 1, 2, 3, 4, 5].map((i) => seat(i, i < 4)),
        },
        actions: {
            label: 'dock button',
            selector: '.action-dock button',
            via: null,
            count: 3,
            items: [0, 1, 2].map((i) => ({
                path: 'button.action-btn',
                box: inBox(10 + i * 120, 760, 110, 48),
                inFrame: true,
            })),
        },
        primaryActions: {
            label: 'primary action button',
            selector: '.actions .action-btn',
            via: null,
            count: 3,
            items: [0, 1, 2].map((i) => ({
                path: 'button.action-btn',
                box: inBox(10 + i * 120, 760, 110, 48),
                inFrame: true,
            })),
        },
        budget: { aboveFeltPx: 130, belowFeltPx: 154, blocks: [] },
        thumb: {
            buttons: 3,
            actionRowTopPx: 760,
            actionRowTopFraction: 0.9,
            actionRowBottomGapPx: 36,
            smallestButton: { w: 110, h: 48 },
            meets44px: true,
            inThumbZone: true,
        },
        scrollY: 0,
        documentScrollWidth: VW,
    };
}

/** Deep-clone so a planted defect cannot leak into the next case. */
const clone = (o) => JSON.parse(JSON.stringify(o));

/** Plant one defect and require the verdict to go RED. */
function convicts(name, mutate, expectSubstring) {
    const m = clone(healthy());
    mutate(m);
    const v = foldTableInFrame(m, WHERE);
    const hit = v.problems.some((p) => p.includes(expectSubstring));
    check(
        `RED: ${name}`,
        !v.ok && hit,
        v.ok
            ? 'verdict was GREEN'
            : `no problem contained "${expectSubstring}"; got: ${JSON.stringify(v.problems)}`,
    );
}

// --- 1. the baseline must be GREEN, or every case below proves nothing --------
{
    const v = foldTableInFrame(healthy(), WHERE);
    check('GREEN: a healthy 6-max table', v.ok, JSON.stringify(v.problems));
    check(
        'the healthy verdict states what it saw',
        /pot 1, board 5 slots, 6\/6 seat pods, 3 action buttons/.test(v.notes),
        v.notes,
    );
}

// --- 2. a view with no felt is a SKIP, and says so ---------------------------
{
    const v = foldTableInFrame(null, { viewport: 'desktop', scene: 'lobby' });
    check('GREEN: a view with no .felt is not a table', v.ok && v.measurement === null, v.notes);
    check('and it says why', /not a table/.test(v.notes), v.notes);
}

// --- 3. the four objects a player reads, each off frame ----------------------
convicts('the pot readout is off frame', (m) => {
    m.pot.items[0].box = outBox(90, 40);
    m.pot.items[0].inFrame = false;
}, 'the pot readout is NOT fully inside');

convicts('a board slot is off frame', (m) => {
    m.board.items[4].box = outBox(40, 56);
    m.board.items[4].inFrame = false;
}, 'board slot 5 of 5 is NOT fully inside');

convicts('a seat pod is off frame', (m) => {
    m.seats.items[3].box = outBox(60, 44);
    m.seats.items[3].inFrame = false;
}, 'seat pod 4 of 6 (occupied) is NOT fully inside');

// The real desktop conviction: a dock control 6 px past the right edge, with
// `documentScrollWidth` still equal to the frame so it cannot even be scrolled to.
convicts('a dock button is off frame', (m) => {
    m.actions.items[2].box = { x: 1394.8, y: 821.8, w: 51.2, h: 28, right: 1446, bottom: 849.8 };
    m.actions.items[2].inFrame = false;
}, 'a dock button is NOT fully inside');

// --- 4. THE EMPTY-SET CASES. A probe that matches nothing reports no problems -
convicts('no pot readout at all on a table view', (m) => {
    m.pot.count = 0;
    m.pot.items = [];
}, 'no pot readout on a table view');

convicts('the ring matched nothing', (m) => {
    m.seats.seatElements = 0;
    m.seats.count = 0;
    m.seats.items = [];
}, 'no seat ring on a table view');

convicts('every seat paints nothing', (m) => {
    m.seats.renderingNothing = 6;
    m.seats.count = 0;
    m.seats.items = [];
}, 'seats on the ring paint nothing');

convicts('a board with four slots', (m) => {
    m.board.slotCount = 4;
    m.board.items.pop();
}, '.community-cards holds 4 slot(s), not 5');

// --- 5. THE CONVICTION OF THE INSTRUMENT ITSELF ------------------------------
//
// This is the case the module existed for and did not have. It reproduces what a
// regression back to the naive `.seat` measurement produces: `group('.seat', ...)`
// filters by `painted()`, so nine zero-size ring points yield ZERO items, and the
// block carries `count`/`items` but neither `seatElements` nor `renderingNothing`.
// Before wave 12's hardening this verdict was GREEN with the note
// "0/undefined seat pods, all in frame".
convicts('the seats block is a `.seat` box measurement, not the pod union', (m) => {
    m.seats = {
        label: 'seat pod',
        selector: '.seat',
        via: null,
        count: 0,
        items: [],
    };
}, 'the seats measurement is not the seat-pod union');

// And the same regression where the ring points DO report a box (a 0x0 rect at the
// ring centre is trivially "in frame"), so the shape survives and the count does
// not.
convicts('nine seats on the ring and not one measurable pod', (m) => {
    m.seats.seatElements = 9;
    m.seats.renderingNothing = 0;
    m.seats.count = 0;
    m.seats.items = [];
}, 'NOT ONE produced a measurable pod');

// --- 6. THE SILENT BOARD SKIP ------------------------------------------------
//
// `if (m.board.rendered)` meant a missing board was never judged either way. It is
// legitimate with no hand in progress and a defect with cards on the table.
convicts('cards are dealt and the board is not rendered', (m) => {
    m.board = { rendered: false, slotCount: 0, items: [] };
    m.seatCards = 12;
}, 'hole card(s) are painted at the seats and .community-cards is NOT rendered');

{
    const m = clone(healthy());
    m.board = { rendered: false, slotCount: 0, items: [] };
    m.seatCards = 0;
    const v = foldTableInFrame(m, WHERE);
    check('GREEN: no board and no cards dealt (table-empty, deposit)', v.ok, JSON.stringify(v.problems));
    check(
        'and the skip is RECORDED, not silent',
        /not rendered and 0 hole cards painted \(checked, not skipped\)/.test(v.notes),
        v.notes,
    );
}

// --- report -----------------------------------------------------------------
for (const r of results) {
    console.log(`  ${r.ok ? 'ok  ' : 'FAIL'} ${r.name}${r.ok || !r.detail ? '' : `\n         ${r.detail}`}`);
}
console.log(`\n  ${results.length - failures}/${results.length} table-in-frame cases pass`);
if (failures) {
    console.error(`\n  ${failures} case(s) FAILED: lib/table-in-frame.mjs would be green about them`);
    process.exit(1);
}
process.exit(0);
