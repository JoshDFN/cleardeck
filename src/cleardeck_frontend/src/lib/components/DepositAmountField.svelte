<script>
  // THE AMOUNT: its label, the unit toggle on a BTC table, the field with its
  // unit suffix, MAX, the quick chips and the preview line (the amount with
  // its unit, its fiat hint). The parent owns the arithmetic (what the text
  // means in the smallest unit, what it costs, what MAX is); this component
  // paints the field and hands back what was typed.
  //
  // Harness contract: `#deposit-amount` (the census reads its value and
  // placeholder), `.amount-field`, `.max-btn`, `.conversion-preview .usd-amount`
  // (chain-agreement's fiat check), `.quick-amounts .quick-amount`.

  import QuickAmounts from './QuickAmounts.svelte';

  let {
    /** The field's text, the player's from the moment the sheet opens. */
    value = $bindable(''),
    /** 'sats' | 'btc': the unit a BTC table's field is typed in. */
    unit = $bindable('sats'),
    btc = false,
    /** The input's own floor in the typed unit, also its placeholder. */
    minAttr = '',
    disabled = false,
    /** lib/deposit-amounts.js quickChips */
    chips = [],
    /** The typed amount in the smallest unit; 0n renders no preview. */
    typedSmallest = 0n,
    /** The typed amount with its unit (or its other unit on a BTC table). */
    equivalentText = '',
    /** The typed amount's dollars, or null with no quote. */
    typedUsd = null,
    onMax = () => {},
  } = $props();

  function pickUnit(next) {
    unit = next;
    value = '';
  }
</script>

<div class="form-section">
  <div class="section-label">
    <label for="deposit-amount">Amount</label>
    {#if btc}
      <div class="unit-toggle">
        <button type="button" class:active={unit === 'sats'} onclick={() => pickUnit('sats')}>sats</button>
        <button type="button" class:active={unit === 'btc'} onclick={() => pickUnit('btc')}>BTC</button>
      </div>
    {/if}
  </div>
  <div class="input-row">
    <div class="amount-field" class:btc>
      <input
        id="deposit-amount"
        type="number"
        inputmode="decimal"
        step={btc && unit === 'sats' ? "1" : "0.00000001"}
        min={minAttr}
        placeholder={minAttr}
        bind:value
        {disabled}
      />
      <span class="input-suffix" class:btc>{btc ? unit : 'ICP'}</span>
    </div>
    <button type="button" class="max-btn" class:btc onclick={onMax} {disabled}>MAX</button>
  </div>
  <QuickAmounts
    {chips}
    selected={value}
    {btc}
    {disabled}
    onPick={(text) => { value = text; }}
  />
  {#if typedSmallest > 0n}
    <p class="conversion-preview">
      <span class="crypto-equiv">{equivalentText}</span>
      {#if typedUsd !== null}
        <span class="usd-preview"><span class="usd-amount">{typedUsd}</span></span>
      {/if}
    </p>
  {/if}
</div>

<style lang="scss">
  @use './cashier' as cashier;

  @include cashier.amount-field;

  @media #{cashier.$phone} {
    @include cashier.amount-field-phone;
  }
</style>
