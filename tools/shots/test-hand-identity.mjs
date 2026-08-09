// Self-check for the HISTORY CHAIN join: node tools/shots/test-hand-identity.mjs
//
// docs/DEFECTS.md E-71. `assertHandHistoryAgreement` matched the table's hand *n*
// to the archive's record by `hand_number`, and `hand_number` restarts at 1 on
// every `reset_table`. Measured on the local archive on 2026-08-08:
//
//     3,218 records          1,651 distinct (table_id, hand_number) citations
//       947 citations name MORE THAN ONE record
//     "table_2 hand 1" answers to 70 records, with 6 different pots
//
// The gate fetched the newest twenty records for `table_2` and collapsed them
// into TEN map entries. The record left under "hand 1" was `hand_id 3173`, from
// an earlier life of the table, pot 0.2 ICP; the hand actually just played was
// `hand_id 3218`, pot 24 ICP. So the sweep reported
//
//     hand 1: the TABLE canister paid 2400000000 e8s but the HISTORY canister
//             recorded 20000000 e8s paid
//
// Both figures were true. They were true of different hands. And the ten records
// the map discarded were never checked for a rake at all — half the evidence for
// the product's headline property, dropped by insertion order.
//
// THE GATE THIS FILE HOLDS. The join is now on the hand's NAME (its shuffle
// commitment) and, more importantly, the join ASSERTS ITSELF: whatever record is
// produced must carry the name of the hand it is compared against. Case 4 injects
// the exact defective lookup — by `hand_number` — and requires the join to fail
// naming both hands, which is the "would it go red again if reverted" proof.
//
// It runs with no replica, no browser and no canister: the fixtures are records
// whose answer is known by construction.

import { foldArchivedHand, handUidFrom, joinTableHandsToArchive } from './lib/chain-agreement.mjs';

const cases = [];
const check = (name, condition, detail) => cases.push({ name, ok: Boolean(condition), detail });

const TABLE = '4fbx2-kt777-77775-aaabq-cai';
const OTHER_TABLE = '4caro-hl777-77775-aaaba-cai';
const seed = (tag) => (tag.repeat(64)).slice(0, 64);

const SEED_A = seed('a1');
const SEED_B = seed('b2');
const SEED_C = seed('c3');

/** An archived record as `foldArchivedHand` produces one. */
const archivedRec = ({ handId, seedHash, handNumber, pot, rake = 0, table = TABLE }) => {
    const uid = `${table}:${seedHash}`;
    const summary = {
        hand_id: BigInt(handId),
        hand_uid: uid,
        hand_number: BigInt(handNumber),
        total_pot: BigInt(pot),
        winners: [{ amount: BigInt(pot) }],
    };
    const full = {
        hand_id: BigInt(handId),
        hand_uid: [uid],
        hand_number: BigInt(handNumber),
        table_id: { toText: () => table },
        shuffle_proof: { seed_hash: seedHash },
        total_pot: BigInt(pot),
        rake: BigInt(rake),
        winners: [{ amount: BigInt(pot - rake) }],
    };
    return foldArchivedHand(summary, full);
};

// ---------------------------------------------------------------------------
// 1. THE NAME. Derived from the table and the commitment, and refusing anything
//    that is not a commitment — a truncated paste must not become a hand.
// ---------------------------------------------------------------------------
check('a name is table_id:seed_hash', handUidFrom(TABLE, SEED_A) === `${TABLE}:${SEED_A}`);
check(
    'case and whitespace are forgiven',
    handUidFrom(TABLE, `  ${SEED_A.toUpperCase()} `) === `${TABLE}:${SEED_A}`,
);
check('a truncated commitment is not a name', handUidFrom(TABLE, SEED_A.slice(0, 63)) === null);
check('a padded commitment is not a name', handUidFrom(TABLE, `${SEED_A}0`) === null);
check('an absent commitment is not a name', handUidFrom(TABLE, null) === null);
check('a non-hex commitment is not a name', handUidFrom(TABLE, 'z'.repeat(64)) === null);
check(
    'the same commitment on two tables is two names',
    handUidFrom(TABLE, SEED_A) !== handUidFrom(OTHER_TABLE, SEED_A),
);

// ---------------------------------------------------------------------------
// 2. `foldArchivedHand` carries the name, and checks it against the record.
// ---------------------------------------------------------------------------
{
    const rec = archivedRec({ handId: 3218, seedHash: SEED_A, handNumber: 1, pot: 2_400_000_000 });
    check('a folded record carries its name', rec.handUid === `${TABLE}:${SEED_A}`, rec.handUid);
    check('a folded record has no structural problems', rec.structural.length === 0, rec.structural.join(' | '));
}
{
    // The archive publishing a name that does not follow from the record it is
    // on. Nothing else in the tree would notice; a name nobody recomputes is a
    // name nobody is checking.
    const rec = foldArchivedHand(
        { hand_id: 1n, hand_uid: `${TABLE}:${SEED_B}`, total_pot: 5n, winners: [{ amount: 5n }] },
        {
            hand_id: 1n,
            hand_uid: [`${TABLE}:${SEED_B}`],
            table_id: { toText: () => TABLE },
            shuffle_proof: { seed_hash: SEED_A },
            total_pot: 5n,
            rake: 0n,
            winners: [{ amount: 5n }],
        },
    );
    check(
        'a published name that does not follow from the hand is a structural failure',
        rec.structural.some((s) => s.includes('does not follow from the hand')),
        rec.structural.join(' | '),
    );
}
{
    // The stale-declaration case: a decoder that drops `hand_uid` must fail
    // loudly rather than join on nothing. docs/DEFECTS.md E-67 is exactly this
    // happening to four other fields.
    const rec = foldArchivedHand(
        { hand_id: 1n, total_pot: 5n, winners: [{ amount: 5n }] },
        {
            hand_id: 1n,
            table_id: { toText: () => TABLE },
            shuffle_proof: { seed_hash: SEED_A },
            total_pot: 5n,
            rake: 0n,
            winners: [{ amount: 5n }],
        },
    );
    check(
        'a HandSummary with no hand_uid is a structural failure',
        rec.structural.some((s) => s.includes('no `hand_uid`')),
        rec.structural.join(' | '),
    );
}

// ---------------------------------------------------------------------------
// 3. THE JOIN, on the archive that actually exists: eleven records all called
//    "hand 1", three different pots. The right one must come back.
// ---------------------------------------------------------------------------
const COLLIDING_ARCHIVE = [
    archivedRec({ handId: 3218, seedHash: SEED_A, handNumber: 1, pot: 2_400_000_000 }),
    archivedRec({ handId: 3205, seedHash: SEED_B, handNumber: 1, pot: 20_000_000 }),
    archivedRec({ handId: 3173, seedHash: SEED_C, handNumber: 1, pot: 20_000_000 }),
];
const TABLE_HANDS = [
    { handNumber: 1, seedHash: SEED_A, awardedByTable: 2_400_000_000 },
];

{
    const { pairs, structural } = joinTableHandsToArchive({
        tableId: TABLE, tableHands: TABLE_HANDS, archived: COLLIDING_ARCHIVE,
    });
    check('the join produces no structural problem on a good archive', structural.length === 0, structural.join(' | '));
    check(
        'the join returns THE HAND THAT WAS PLAYED, not another hand with the same number',
        pairs[0].hist && Number(pairs[0].hist.handId) === 3218,
        `got hand_id ${pairs[0].hist && pairs[0].hist.handId}`,
    );
    check(
        'and its pot is the pot of that hand',
        pairs[0].hist && pairs[0].hist.totalPot === 2_400_000_000,
        String(pairs[0].hist && pairs[0].hist.totalPot),
    );
}

// ---------------------------------------------------------------------------
// 4. THE REGRESSION. Revert the lookup to `hand_number` — the exact defect — and
//    the join must go RED, naming both hands, instead of quietly comparing the
//    money of two different deals.
// ---------------------------------------------------------------------------
{
    const { pairs, structural } = joinTableHandsToArchive({
        tableId: TABLE,
        tableHands: TABLE_HANDS,
        archived: COLLIDING_ARCHIVE,
        // The defective lookup, verbatim in spirit: first record under that
        // number wins.
        lookup: (hand, index) => index.byHandNumber.get(hand.handNumber) || null,
    });
    check(
        'joining on hand_number is caught as a BROKEN JOIN',
        structural.some((s) => s.startsWith('JOIN BROKEN')),
        structural.join(' | '),
    );
    check(
        'and the message names BOTH hands, so the reader can tell it from a money defect',
        structural.some((s) => s.includes(SEED_A) && s.includes(SEED_C)),
        structural.join(' | '),
    );
    check(
        'the wrong record is exactly the one the old gate compared against',
        pairs[0].hist && Number(pairs[0].hist.handId) === 3173,
        `got hand_id ${pairs[0].hist && pairs[0].hist.handId}`,
    );
}

// ---------------------------------------------------------------------------
// 5. A hand the archive does not hold is reported as missing, by NAME. "hand 1
//    is missing" was never a checkable claim on a table that has been reset.
// ---------------------------------------------------------------------------
{
    const { pairs, structural } = joinTableHandsToArchive({
        tableId: TABLE,
        tableHands: [{ handNumber: 1, seedHash: seed('d4'), awardedByTable: 1 }],
        archived: COLLIDING_ARCHIVE,
    });
    check('a hand the archive lacks joins to nothing', pairs[0].hist === null);
    check('and that is not itself a join failure', structural.length === 0, structural.join(' | '));
}

// ---------------------------------------------------------------------------
// 6. A table hand with no usable commitment cannot be joined at all, and must
//    say so rather than fall back to the number.
// ---------------------------------------------------------------------------
{
    const { pairs, structural } = joinTableHandsToArchive({
        tableId: TABLE,
        tableHands: [{ handNumber: 1, seedHash: '', awardedByTable: 1 }],
        archived: COLLIDING_ARCHIVE,
    });
    check(
        'a table hand with no commitment is a structural failure',
        structural.some((s) => s.includes('no usable shuffle commitment')),
        structural.join(' | '),
    );
    check('and is joined to nothing rather than to a hand number', pairs[0].hist === null);
}

// ---------------------------------------------------------------------------
// 7. TWO RECORDS UNDER ONE NAME is the archive failing its own uniqueness
//    claim, and the gate must not silently keep one of them.
// ---------------------------------------------------------------------------
{
    const twin = archivedRec({ handId: 99, seedHash: SEED_A, handNumber: 7, pot: 1 });
    const { structural } = joinTableHandsToArchive({
        tableId: TABLE,
        tableHands: TABLE_HANDS,
        archived: [...COLLIDING_ARCHIVE, twin],
    });
    check(
        'two archived records with one name is caught',
        structural.some((s) => s.includes('TWO RECORDS NAMED')),
        structural.join(' | '),
    );
}

// ---------------------------------------------------------------------------
// 8. THE RAKE HOLE. The old gate iterated a map keyed on hand_number, so records
//    that collided were never checked for a rake. This pins that the number of
//    records available to the rake loop is the number FETCHED, not the number of
//    distinct hand numbers among them.
// ---------------------------------------------------------------------------
{
    const collapsed = new Map();
    for (const rec of COLLIDING_ARCHIVE) collapsed.set(rec.handNumber, rec);
    check(
        'keying on hand_number would drop 2 of 3 archived records from the rake check',
        collapsed.size === 1 && COLLIDING_ARCHIVE.length === 3,
        `${collapsed.size} of ${COLLIDING_ARCHIVE.length} survive a hand_number map`,
    );
    const raked = COLLIDING_ARCHIVE.map((r, i) => (i === 2 ? { ...r, rake: 1 } : r));
    // The record carrying the rake is one of the two the old map discarded.
    check(
        'and the rake would be on a record that map discarded',
        raked.filter((r) => r.rake !== 0).length === 1
            && !collapsed.has(raked[2].handNumber + 1000),
    );
    check(
        'iterating the records themselves sees every rake',
        raked.filter((r) => r.rake !== null && r.rake !== 0).length === 1,
    );
}

// ---------------------------------------------------------------------------

let failed = 0;
for (const c of cases) {
    if (!c.ok) failed += 1;
    console.log(`    ${c.ok ? 'ok  ' : 'FAIL'}  ${c.name}${c.detail ? `   — ${c.detail}` : ''}`);
}
if (failed) {
    console.error(`\n  test-hand-identity: ${failed} of ${cases.length} FAILED`);
    process.exit(1);
}
console.log(`  test-hand-identity: ${cases.length} checks green`);
process.exit(0);
