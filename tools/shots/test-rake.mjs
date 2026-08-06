// Self-check for the NO-RAKE gate: node tools/shots/test-rake.mjs
//
// No-rake is one of the four properties this product says must never be
// weakened, and `assertHandHistoryAgreement` is the only thing in the tree that
// asserts it against the PERMANENT ARCHIVE. That gate had never once fired for a
// reason involving rake.
//
// WHAT IT WAS DOING (docs/DEFECTS.md E-61). It built its record from
// `history.get_hands_by_table`, which returns `vec HandSummary`, and read
// `Number(r.rake)` off it. `HandSummary` has no `rake` field. So `rake` was
// `undefined`, `Number(undefined)` was `NaN`, and both `NaN !== 0` and
// `totalPot !== awarded + NaN` were true on EVERY archived hand, forever:
//
//     RAKE TAKEN: hand 1 recorded rake=NaN e8s.
//     hand 1: history says total_pot=2400000000 but the winners were paid
//             2400000000 with rake NaN (difference NaN e8s)
//
// An alarm that fires on every hand is an alarm nobody reads, so the product's
// headline claim was guarded by nothing at all.
//
// The reason it survived is the reason this file exists: the gate can only run
// inside a full screenshot sweep, against a live replica with an archived hand on
// it. Nobody had ever seen it PASS, so nobody could tell its red from a real red,
// and two waves running the sweep could not be re-run at all. The fold is
// therefore a pure exported function and every case below is a record whose
// answer is known by construction — this runs with no replica, no browser and no
// canister.
//
// Case 3 is the regression: it feeds in exactly the shape the defect fed in, a
// `HandSummary` with no `rake`, and requires a STRUCTURAL failure that names the
// missing field rather than a rake of NaN.

import { IDL } from '@dfinity/candid';
import { foldArchivedHand } from './lib/chain-agreement.mjs';
import { idlFactory as historyIdl } from '../../src/declarations/history/history.did.js';

const cases = [];
const check = (name, condition, detail) => cases.push({ name, ok: Boolean(condition), detail });

// ---------------------------------------------------------------------------
// 0. THE DEFECT'S PREMISE, READ OFF THE LIVE CANDID DECLARATIONS.
//
//    Everything below this point is a fixture, and a fixture can be wrong in the
//    same direction as the code. This case is not: it asks the real interface
//    which shape carries `rake`, so if the two methods ever swap that field
//    around, the fixtures stop describing the wire and this fails first.
// ---------------------------------------------------------------------------
{
    const service = historyIdl({ IDL });
    const method = Object.fromEntries(service._fields);
    const fieldsOf = (t) => (t._fields ?? []).map(([name]) => name);
    // `get_hand : (nat64) -> (opt HandHistoryRecord)`
    const record = fieldsOf(method.get_hand.retTypes[0]._type);
    // `get_hands_by_table : (principal, nat64, nat64) -> (vec HandSummary)`
    const summaryFields = fieldsOf(method.get_hands_by_table.retTypes[0]._type);

    check(
        'HandSummary really has no `rake` field',
        !summaryFields.includes('rake'),
        summaryFields.join(', '),
    );
    check(
        'HandHistoryRecord really does have one',
        record.includes('rake'),
        record.join(', '),
    );
    check(
        'so the no-rake property can ONLY be asserted from get_hand',
        record.includes('rake') && !summaryFields.includes('rake'),
        `get_hand: ${record.join(', ')} // get_hands_by_table: ${summaryFields.join(', ')}`,
    );
    for (const f of ['total_pot', 'winners', 'hand_id', 'hand_number']) {
        check(
            `both shapes still carry \`${f}\`, which the cross-check reads`,
            record.includes(f) && summaryFields.includes(f),
            `record=${record.includes(f)} summary=${summaryFields.includes(f)}`,
        );
    }
}

/** A `HandSummary` as `get_hands_by_table` returns it. Note: no `rake`. */
const summary = (over = {}) => ({
    hand_id: 7n,
    hand_number: 1n,
    table_id: 'aaaaa-aa',
    player_count: 2,
    timestamp: 0n,
    went_to_showdown: true,
    total_pot: 2_400_000_000n,
    winners: [{ seat: 0, amount: 2_400_000_000n, pot_type: 'main', principal: 'aaaaa-aa', hand_rank: [] }],
    ...over,
});

/** A `HandHistoryRecord` as `get_hand` returns it. This one DOES carry `rake`. */
const full = (over = {}) => ({
    hand_id: 7n,
    hand_number: 1n,
    table_id: 'aaaaa-aa',
    total_pot: 2_400_000_000n,
    rake: 0n,
    winners: [{ seat: 0, amount: 2_400_000_000n, pot_type: 'main', principal: 'aaaaa-aa', hand_rank: [] }],
    players: [],
    actions: [],
    ...over,
});

/**
 * The gate's own verdict on one folded hand, as the sweep computes it.
 *
 * The rake clause is a copy of the loop in `assertHandHistoryAgreement` that runs
 * over EVERY archived hand rather than only the rows the modal is showing --
 * no-rake is a property of the archive, and a rake that only has to wait ten
 * hands to fall out of the visible window is not caught by a check inside the row
 * loop.
 */
function verdict(fold) {
    const problems = [...fold.structural];
    if (fold.rake !== null && fold.rake !== 0) {
        problems.push(`RAKE TAKEN: recorded rake=${fold.rake} e8s`);
    }
    if (fold.totalPot !== null && fold.awarded !== null && fold.rake !== null
        && fold.totalPot !== fold.awarded + fold.rake) {
        problems.push(`pot ${fold.totalPot} != awarded ${fold.awarded} + rake ${fold.rake}`);
    }
    return problems;
}

// ---------------------------------------------------------------------------
// 1. THE GREEN CASE. Until this exists, nobody has ever seen the gate pass, and
//    a gate whose green nobody has seen cannot convict anything with its red.
// ---------------------------------------------------------------------------
{
    const fold = foldArchivedHand(summary(), full());
    const problems = verdict(fold);
    check('an honest no-rake hand is GREEN', problems.length === 0, JSON.stringify(problems));
    check('rake reads as the number 0, not null and not NaN', fold.rake === 0, String(fold.rake));
    check('the pot is read from the full record', fold.totalPot === 2_400_000_000, String(fold.totalPot));
    check('the winners are summed', fold.awarded === 2_400_000_000, String(fold.awarded));
}

// ---------------------------------------------------------------------------
// 2. INTRODUCE A RAKE. The property under test: the gate goes RED, and the
//    message names the actual amount rather than NaN.
// ---------------------------------------------------------------------------
{
    // One e8. The smallest rake there is; a gate that only catches large ones is
    // not a gate on "no rake".
    const raked = full({ rake: 1n, winners: [{ seat: 0, amount: 2_399_999_999n, pot_type: 'main' }] });
    const fold = foldArchivedHand(summary(), raked);
    const problems = verdict(fold);
    check(
        'a rake of 1 e8 is caught',
        problems.some((p) => p.includes('RAKE TAKEN') && p.includes('rake=1 e8s')),
        JSON.stringify(problems),
    );
    check(
        'and the amount is a number, never NaN',
        !problems.some((p) => p.includes('NaN')),
        JSON.stringify(problems),
    );
}
{
    // A rake big enough to matter, taken the way a real one would be: out of the
    // pot, so the winners are paid less and the arithmetic still balances. This is
    // the case that a pot-vs-payout check alone would MISS -- it only shows up in
    // the `rake` field itself.
    const raked = full({
        rake: 120_000_000n,
        winners: [{ seat: 0, amount: 2_280_000_000n, pot_type: 'main' }],
    });
    const problems = verdict(foldArchivedHand(summary(), raked));
    check(
        'a balanced 5% rake is still caught',
        problems.some((p) => p.includes('RAKE TAKEN') && p.includes('120000000')),
        JSON.stringify(problems),
    );
    check(
        'and it is caught by the rake check, not by an arithmetic mismatch',
        !problems.some((p) => p.includes('!=')),
        JSON.stringify(problems),
    );
}

// ---------------------------------------------------------------------------
// 3. THE REGRESSION (docs/DEFECTS.md E-61). Feed the fold exactly what the
//    defect fed it: a HandSummary in the place the full record belongs.
// ---------------------------------------------------------------------------
{
    const fold = foldArchivedHand(summary(), summary());
    const problems = verdict(fold);
    check(
        'a record with no `rake` field is a STRUCTURAL failure',
        problems.some((p) => p.startsWith('STRUCTURAL') && p.includes("'rake'")),
        JSON.stringify(problems),
    );
    check(
        'it is never reported as a rake of NaN',
        !problems.some((p) => p.includes('RAKE TAKEN')) && !problems.some((p) => p.includes('NaN')),
        JSON.stringify(problems),
    );
    check('and `rake` folds to null, not NaN', fold.rake === null, String(fold.rake));
}

// ---------------------------------------------------------------------------
// 4. The archive listing a hand it cannot then produce.
// ---------------------------------------------------------------------------
{
    const problems = verdict(foldArchivedHand(summary(), null));
    check(
        'get_hand returning nothing is a structural failure',
        problems.some((p) => p.includes('get_hand(7) returned nothing')),
        JSON.stringify(problems),
    );
}

// ---------------------------------------------------------------------------
// 5. The two archive read paths disagreeing with each other.
// ---------------------------------------------------------------------------
{
    const problems = verdict(foldArchivedHand(summary({ total_pot: 2_400_000_001n }), full()));
    check(
        'the summary and the full record must agree on the pot',
        problems.some((p) => p.includes('the archive disagrees with itself')),
        JSON.stringify(problems),
    );
}
{
    const problems = verdict(foldArchivedHand(
        summary({ winners: [{ seat: 0, amount: 1n, pot_type: 'main' }] }),
        full(),
    ));
    check(
        'the summary and the full record must agree on what was paid',
        problems.some((p) => p.includes('winners sum to')),
        JSON.stringify(problems),
    );
}

// ---------------------------------------------------------------------------
// 6. Pot and payout must still balance when the rake is honest.
// ---------------------------------------------------------------------------
{
    const problems = verdict(foldArchivedHand(
        summary({ total_pot: 2_400_000_000n, winners: [{ seat: 0, amount: 2_300_000_000n, pot_type: 'main' }] }),
        full({ total_pot: 2_400_000_000n, rake: 0n, winners: [{ seat: 0, amount: 2_300_000_000n, pot_type: 'main' }] }),
    ));
    check(
        '100,000,000 e8s that went nowhere is caught',
        problems.some((p) => p.includes('!=')),
        JSON.stringify(problems),
    );
}

// ---------------------------------------------------------------------------
const failed = cases.filter((c) => !c.ok);
for (const c of cases) {
    console.log(`${c.ok ? 'ok  ' : 'FAIL'} ${c.name}${c.ok ? '' : `\n     ${c.detail}`}`);
}
if (failed.length) {
    console.error(`\n${failed.length} of ${cases.length} NO-RAKE GATE CASES FAILED`);
    process.exit(1);
}
console.log(`\nALL ${cases.length} NO-RAKE GATE CASES PASS`);
