// Self-check for the INVERTED money gate: node tools/shots/test-census.mjs
//
// The census is the thing that decides whether a scene's green verdict means
// anything, so it needs its own failing cases. Its first draft passed a rule
// table into the page WITHOUT the `kind` field, every site was therefore treated
// as an allowlist rule, and `new RegExp(undefined)` is `/(?:)/` — which matches
// every string. The run reported "74 tokens on screen, 74 allowlisted, 0
// unaccounted for" and gated nothing at all. That is the class of bug this file
// exists to catch: a verifier that has quietly stopped verifying.
//
// The page is stubbed (the token list is supplied directly), so this runs with no
// replica, no browser and no canister — the logic under test is the
// classification, not the DOM walk.

import { assertEveryTokenAccountedFor, CHAIN_SITES } from './lib/token-census.mjs';
import { ALLOWLIST } from './token-allowlist.mjs';

/** A page stub whose evaluate() returns a fixed token list. */
const pageOf = (tokens) => ({
    evaluate: async () => tokens.map((t) => ({
        token: t.token,
        source: t.source ?? 'text',
        visible: t.visible ?? true,
        chainRuleId: t.chainRuleId ?? null,
        allowRuleIds: t.allowRuleIds ?? [],
        path: t.path ?? 'div > span',
        elementText: t.elementText ?? t.token,
        context: t.elementText ?? t.token,
        fontSizePx: 14,
        rect: { x: 0, y: 0, w: 40, h: 14 },
    })),
});

const figure = (label, screen, ok = true) => ({ label, chain: 1, screen, ok, detail: 'stub' });

const cases = [];
const check = (name, condition, detail) => cases.push({ name, ok: Boolean(condition), detail });

// ---------------------------------------------------------------------------
// 1. A token with a passing check behind it is accounted for.
// ---------------------------------------------------------------------------
{
    const r = await assertEveryTokenAccountedFor(
        pageOf([{ token: '0.30', chainRuleId: 'pot-headline' }]),
        [figure('pot (headline) vs get_pot()', '0.30')],
    );
    check('a matched pot token passes', r.ok && r.checks.chainMatched === 1, JSON.stringify(r.checks.unassertedTokens));
}

// ---------------------------------------------------------------------------
// 2. THE WHOLE POINT: a money token nothing looked at FAILS the scene.
// ---------------------------------------------------------------------------
{
    const r = await assertEveryTokenAccountedFor(
        pageOf([{ token: '9.99', chainRuleId: null, allowRuleIds: [] }]), [],
    );
    check('an unasserted money token fails the scene', !r.ok && r.checks.unasserted === 1, r.notes);
}

// ---------------------------------------------------------------------------
// 3. A site declared chain-asserted but with NO check this scene still fails.
//    (The old harness's failure mode: the site is "covered" in principle.)
// ---------------------------------------------------------------------------
{
    const r = await assertEveryTokenAccountedFor(
        pageOf([{ token: '0.30', chainRuleId: 'pot-headline' }]), [],
    );
    check('a declared site with no check that ran fails', !r.ok && /NO check/.test(r.checks.unassertedTokens[0].why), r.notes);
}

// ---------------------------------------------------------------------------
// 4. Matching is ONE-TO-ONE: three identical stacks need three checks.
// ---------------------------------------------------------------------------
{
    const three = ['12.00', '12.00', '12.00'].map((token) => ({ token, chainRuleId: 'seat-stack' }));
    const two = [figure('seat 0 stack', '12.00'), figure('seat 1 stack', '12.00')];
    const r = await assertEveryTokenAccountedFor(pageOf(three), two);
    check('a third identical token with only two checks fails', !r.ok && r.checks.unasserted === 1, r.notes);

    const r2 = await assertEveryTokenAccountedFor(
        pageOf(three), [...two, figure('seat 2 stack', '12.00')],
    );
    check('...and passes once the third check exists', r2.ok && r2.checks.chainMatched === 3, r2.notes);
}

// ---------------------------------------------------------------------------
// 5. A composite check covers its own literals, and no more.
// ---------------------------------------------------------------------------
{
    const ratio = [{ token: '3.5', chainRuleId: 'pot-odds-ratio' }, { token: '1', chainRuleId: 'pot-odds-ratio' }];
    const r = await assertEveryTokenAccountedFor(
        pageOf(ratio), [figure('pot-odds ratio vs get_pot()/call_amount', '3.5:1')],
    );
    check('a "3.5:1" ratio check covers both of its numbers', r.ok, r.notes);

    const extra = [...ratio, { token: '3.5', chainRuleId: 'pot-odds-ratio' }];
    const r2 = await assertEveryTokenAccountedFor(
        pageOf(extra), [figure('pot-odds ratio vs get_pot()/call_amount', '3.5:1')],
    );
    check('...but not a third number it never quoted', !r2.ok && r2.checks.unasserted === 1, r2.notes);
}

// ---------------------------------------------------------------------------
// 6. A FAILING check does not silence the census, and is not counted as matched.
// ---------------------------------------------------------------------------
{
    const r = await assertEveryTokenAccountedFor(
        pageOf([{ token: '0.60', chainRuleId: 'pot-headline' }]),
        [figure('pot (headline) vs get_pot()', '0.60', false)],
    );
    check('a disagreeing figure is reported as chain-DISAGREES, not matched',
        r.checks.chainDisagrees === 1 && r.checks.chainMatched === 0, JSON.stringify(r.checks));
}

// ---------------------------------------------------------------------------
// 7. The allowlist excuses what it was written for, and refuses the rest.
// ---------------------------------------------------------------------------
{
    const ok = await assertEveryTokenAccountedFor(
        pageOf([{ token: '30', allowRuleIds: ['action-countdown'], elementText: '30' }]), [],
    );
    check('an allowlisted countdown passes', ok.ok && ok.checks.allowlisted === 1, ok.notes);

    // THE SHAPE INVARIANT: money-shaped tokens are never excused.
    const money = await assertEveryTokenAccountedFor(
        pageOf([{ token: '0.30', allowRuleIds: ['action-countdown'], elementText: '0.30' }]), [],
    );
    check('a money-shaped token in an allowlisted element is REFUSED',
        !money.ok && money.checks.unasserted === 1, money.notes);

    // A context-scoped rule refuses a token whose element text is different.
    const wrongContext = await assertEveryTokenAccountedFor(
        pageOf([{ token: '30', allowRuleIds: ['time-bank-button'], elementText: 'Call 30' }]), [],
    );
    check('a context-scoped rule refuses a token in the wrong context',
        !wrongContext.ok, wrongContext.notes);
}

// ---------------------------------------------------------------------------
// 8. Hidden tokens are counted but not gated.
// ---------------------------------------------------------------------------
{
    const r = await assertEveryTokenAccountedFor(
        pageOf([{ token: '9.99', visible: false }]), [],
    );
    check('an invisible token cannot fail a scene', r.ok && r.checks.hiddenTokensNotGated === 1, r.notes);
}

// ---------------------------------------------------------------------------
// 9. Every allowlist rule is well-formed, and the list is SMALL.
//    Both are properties the artifact claims, so both are asserted.
// ---------------------------------------------------------------------------
{
    const malformed = ALLOWLIST.filter((r) => !r.id || !r.selector || typeof r.tokens !== 'string' || !r.why);
    check('every allowlist rule has id, selector, tokens and a reason', malformed.length === 0,
        malformed.map((r) => r.id).join(', '));

    const moneyProbes = ['0.30', '12.00', '155.00', '0.0001', '1.5', '1,000.00'];
    const sneaky = ALLOWLIST.filter(
        (r) => !r.moneyShaped && moneyProbes.some((p) => new RegExp(r.tokens).test(p)),
    );
    check('no rule accepts a money-shaped token without declaring it', sneaky.length === 0,
        sneaky.map((r) => r.id).join(', '));

    check(`the allowlist is small (${ALLOWLIST.length} rules)`, ALLOWLIST.length <= 25,
        `${ALLOWLIST.length} rules — if this trips, the inversion is being defeated by growth`);

    const ids = ALLOWLIST.map((r) => r.id);
    check('allowlist rule ids are unique', new Set(ids).size === ids.length, ids.join(', '));
}

// ---------------------------------------------------------------------------
// 10. THE COMMITTED-STAKE READOUT (docs/DEFECTS.md E-64).
//
//     Five scenes went red with `"0.20" in span.committed-value ... asserted by
//     nothing`, and the census was right: the custody-visibility work rendered a
//     real ICP amount and no gate tied it to a canister figure. The fix is a
//     chain check, not an allowlist entry -- allowlisting money is the one thing
//     this instrument exists to refuse.
//
//     A CHAIN_SITES entry is only a CLAIM that something asserts the site; the
//     claim is verified per scene by matching `label`. So the regex has to match
//     the label chain-agreement.mjs actually emits, and a typo there would leave
//     the token silently unasserted -- the same shape of defect one layer over.
//     Both halves are checked.
// ---------------------------------------------------------------------------
{
    const site = (id) => CHAIN_SITES.find((s) => s.id === id);
    const valueLabel = 'committed stake vs get_table_view().my_committed_in_pot';
    const noteLabel = 'the hand named in the committed note vs get_table_view().hand_number';

    check('the committed-stake site is declared', Boolean(site('committed-stake')), 'CHAIN_SITES');
    check(
        "...and its label matches the check chain-agreement.mjs emits",
        site('committed-stake')?.label.test(valueLabel),
        `${site('committed-stake')?.label} vs ${valueLabel}`,
    );
    check(
        'the committed-note hand number is declared and matches too',
        site('committed-note-hand')?.label.test(noteLabel),
        `${site('committed-note-hand')?.label} vs ${noteLabel}`,
    );

    const r = await assertEveryTokenAccountedFor(
        pageOf([
            { token: '0.20', chainRuleId: 'committed-stake' },
            { token: '1', chainRuleId: 'committed-note-hand' },
        ]),
        [figure(valueLabel, '0.20'), figure(noteLabel, '1')],
    );
    check(
        'both committed-stake tokens are accounted for once the checks run',
        r.ok && r.checks.chainMatched === 2,
        JSON.stringify(r.checks),
    );

    // And the failure that was actually recorded: the site is listed, but no
    // check ran on this scene. Listing a site must never be enough.
    const unchecked = await assertEveryTokenAccountedFor(
        pageOf([{ token: '0.20', chainRuleId: 'committed-stake' }]), [],
    );
    check(
        'a listed site with no check behind it is still UNASSERTED',
        !unchecked.ok && unchecked.checks.unasserted === 1,
        unchecked.notes,
    );
}

let failed = 0;
for (const c of cases) {
    if (!c.ok) failed += 1;
    console.log(`${c.ok ? 'ok  ' : 'FAIL'} ${c.name}${c.ok ? '' : `\n       ${c.detail}`}`);
}
console.log(
    failed === 0
        ? `\nALL ${cases.length} CENSUS CASES PASS (allowlist: ${ALLOWLIST.length} rules)`
        : `\n${failed} of ${cases.length} CENSUS CASES FAILED`,
);
process.exit(failed === 0 ? 0 : 1);
