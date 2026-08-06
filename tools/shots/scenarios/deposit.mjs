// Scene: the deposit flow (modal), opened from a real table view.

import { devLogin, enterTable, openApp, settle } from '../lib/browser.mjs';
import { HERO_PLAYER, icp } from '../lib/config.mjs';
import { phaseOf, view } from '../lib/table-driver.mjs';
import {
  assertChainAgreement, assertDepositAgreement, named, withAgreement,
} from '../lib/chain-agreement.mjs';
import { prepareTable, tableDisplayName } from './_shared.mjs';

const TABLE = 'table_2';

export default {
  name: 'deposit',
  title: 'Deposit modal (ICRC-2 approve + transfer_from flow)',
  auth: true,

  async setup(ctx) {
    // Hero alone at the table: no hand in progress, so the modal is not racing a
    // 500 ms poll that changes the page underneath it.
    const { tableId } = await prepareTable(ctx, TABLE, [
      { player: HERO_PLAYER, seat: 0, chips: icp(12) },
    ]);
    const v = await view(HERO_PLAYER, tableId);
    return {
      table: TABLE,
      tableId,
      onChain: { phase: phaseOf(v), seated: 1 },
      notes: 'hero seated alone; deposit modal opened from the table wallet panel',
    };
  },

  async stage(ctx, page) {
    await openApp(page);
    await devLogin(page, HERO_PLAYER);
    await enterTable(page, tableDisplayName(ctx, TABLE));

    // The wallet panel can start collapsed, so try the toggle first.
    const deposit = page.locator('.wallet-action-btn.deposit');
    if (!(await deposit.first().isVisible().catch(() => false))) {
      const toggle = page.locator('.panel-toggle');
      if (await toggle.first().isVisible().catch(() => false)) await toggle.first().click();
    }

    // THE ENTRY POINT MAY NOT EXIST AT THIS VIEWPORT, and that is a real product
    // defect rather than something for the harness to wait out:
    //
    //   `showDepositModal` in routes/+page.svelte is set from exactly ONE place,
    //   PokerTable's `onShowDeposit`, whose button lives in
    //   `.feed-container.right`. PokerTable's `@media (max-width: 900px)` sets
    //   `.feed-container { display: none }`. So at 390px there is no way to open
    //   the deposit modal at all — the primary ICRC-2 approve + deposit funding
    //   flow has no mobile entry point.
    //
    // Rather than hang for 30s and throw, detect it, and let verify() report an
    // UNVERIFIED artifact that says exactly why.
    const reachable = await deposit.first().isVisible().catch(() => false);
    if (!reachable) {
      return { depositEntryPointReachable: false };
    }

    await deposit.first().click();
    await page.waitForSelector('.modal-content', { timeout: 30_000 });
    await page.waitForSelector('#deposit-modal-title', { timeout: 30_000 });
    await settle(page);
    return { depositEntryPointReachable: true };
  },

  async verify(ctx, page) {
    const modal = await page.locator('.modal-content').count();
    const title = modal
      ? ((await page.locator('#deposit-modal-title').textContent()) || '')
        .replace(/\s+/g, ' ').trim()
      : '';
    const sourceToggle = await page.locator('.wallet-source-toggle button').allTextContents();
    const panelPresent = (await page.locator('.wallet-panel').count()) > 0;
    const panelVisible = await page.locator('.wallet-panel').first().isVisible().catch(() => false);
    const viewport = page.viewportSize();

    // The table is still on screen behind the modal, so its money figures are
    // asserted either way.
    const tableAgreement = named('table', await assertChainAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER, requireBalance: true,
    }));

    if (modal === 0) {
      return withAgreement({
        verified: false,
        checks: {
          modals: 0,
          viewportWidth: viewport?.width ?? null,
          walletPanelInDom: panelPresent,
          walletPanelVisible: panelVisible,
          reason:
            'the deposit modal has NO entry point at this viewport: its only trigger is '
            + "PokerTable's wallet-panel Deposit button, inside .feed-container.right, which "
            + 'PokerTable hides with `@media (max-width: 900px) { .feed-container { display: none } }`',
        },
        notes:
          `deposit modal UNREACHABLE at ${viewport?.width ?? '?'}px: the only trigger is inside `
          + '.feed-container, hidden below 900px. This is an app defect, not a staging failure.',
      }, tableAgreement);
    }

    // The modal quotes the LEDGER balance and converts it to fiat. Both are money
    // figures a player acts on, so both are checked against their sources: the
    // real local ledger, and the exact quote this run served.
    const depositAgreement = named('deposit', await assertDepositAgreement(ctx, page, {
      table: TABLE, asPlayer: HERO_PLAYER,
    }));

    // ===================================================================
    // IS THE SOLVENCY VERDICT ON THIS SCREEN, AND IS IT ABOVE THE CONTROLS?
    // ===================================================================
    //
    // docs/SECURITY-FINDINGS.md FINDING 35 / docs/DEFECTS.md E-70. Mainnet
    // table_1 is 2.00 ICP short of its own books and nothing in the canister can
    // say so, so this screen showed a player nothing at all before committing
    // real money.
    //
    // Two things are asserted, and the ORDER one is the load-bearing one: a
    // warning below the amount field is a warning the player reads after
    // deciding. It is checked geometrically, not by DOM order, because a CSS
    // change can reorder a flex container without touching the markup.
    //
    // `covered` is the only state that renders nothing, so an absent block is
    // only acceptable when the canister actually said it is covered. On today's
    // deployment there is no solvency surface at all, the state is
    // `unsupported`, and the block MUST be there.
    const solvencyEl = page.locator('.modal-content .solvency');
    const solvencyPresent = (await solvencyEl.count()) > 0;
    const solvencyState = solvencyPresent
      ? await solvencyEl.first().getAttribute('data-solvency-state')
      : null;
    const solvencyVisible = solvencyPresent
      ? await solvencyEl.first().isVisible().catch(() => false)
      : false;

    let solvencyAboveControls = null;
    if (solvencyVisible) {
      const box = await solvencyEl.first().boundingBox();
      const control = page.locator('.modal-content .btn-primary, .modal-content .amount-input')
        .first();
      const cBox = await control.boundingBox().catch(() => null);
      solvencyAboveControls = box && cBox ? box.y + box.height <= cBox.y + 1 : null;
    }

    // It must not be an overlay either: HARD RULE 2 forbids anything in this app
    // covering the four protected notices, and the notices are restated inside
    // this very dialog directly above this block.
    const solvencyIsInFlow = solvencyPresent
      ? await solvencyEl.first().evaluate((el) => {
        const cs = getComputedStyle(el);
        return cs.position === 'static' || cs.position === 'relative';
      })
      : null;

    const solvencyOk = solvencyState === 'covered'
      ? !solvencyPresent || !solvencyVisible
      : solvencyVisible && solvencyAboveControls !== false && solvencyIsInFlow !== false;

    return withAgreement({
      verified: /deposit/i.test(title) && solvencyOk,
      checks: {
        modals: modal,
        title,
        viewportWidth: viewport?.width ?? null,
        walletSources: sourceToggle.map((t) => t.replace(/\s+/g, ' ').trim()).filter(Boolean),
        solvency: {
          present: solvencyPresent,
          visible: solvencyVisible,
          state: solvencyState,
          aboveMoneyControls: solvencyAboveControls,
          inDocumentFlow: solvencyIsInFlow,
          ok: solvencyOk,
          reason: solvencyOk
            ? null
            : 'the deposit screen does not warn about this table\'s solvency, or warns '
              + 'below the controls that move money (docs/SECURITY-FINDINGS.md FINDING 35)',
        },
      },
      notes: `${title}; solvency verdict on screen: ${solvencyState ?? 'ABSENT'}`,
    }, tableAgreement, depositAgreement);
  },
};
