<script>
  /**
   * The bet sizer: presets, a slider and a typed amount, IN THE DOCK.
   *
   * It never paints over the felt or the hero's cards (the old popover did
   * both). On desktop it is a row above the action buttons whenever the hero
   * can raise; on the phone it stays in the DOM but hidden until the caret
   * beside Raise opens it, and it opens IN FLOW (the dock grows for a moment)
   * rather than as a sheet over the hero's pod.
   *
   * The arithmetic is $lib/bet-sizing.js; this file only renders and reports.
   * The parent owns `value` (the raise-to figure in the smallest unit) and is
   * told about every change through `onChange`, so the primary button, the
   * keyboard shortcuts and this control all read one number.
   *
   * Harness contract (tools/shots/lib/dom-scrape.mjs, chain-agreement.mjs):
   * `.raise-slider-panel`, `.preset-buttons button` with the labels "½ Pot"
   * and "Pot", `.raise-slider` whose value is the figure that would be SENT,
   * and `.raise-input` (the typed readout). The presets are clicked and read
   * by the harness and compared with the canister's get_pot().
   */
  import {
    PRESETS, formatAmountInput, parseAmountInput, presetAt, presetTarget, raiseCap, raiseFloor,
    sliderStep, stepByBlind,
  } from '$lib/bet-sizing.js';

  const {
    value = 0,
    ctx,                     // SizingContext (see bet-sizing.js)
    bigBlind = 0,
    isBTC = false,
    decimals = 2,
    unit = 'ICP',
    open = true,             // phone: the caret state; desktop: always true
    compact = false,         // phone layout
    problem = null,          // an illegal size, in words, or null
    onChange = () => {},
    onCommit = () => {},
    onClose = () => {},
  } = $props();

  const floor = $derived(raiseFloor(ctx));
  const cap = $derived(raiseCap(ctx));
  const step = $derived(sliderStep(ctx.minRaise));
  const onPreset = $derived(presetAt(value, ctx));
  const targets = $derived(Object.fromEntries(PRESETS.map((p) => [p.id, presetTarget(p.id, ctx)])));

  // The typed field: mirrors `value` unless the player is typing in it.
  let text = $state('');
  let typing = $state(false);
  $effect(() => {
    if (!typing) text = formatAmountInput(value, { isBTC, decimals });
  });

  function pick(id) {
    onChange(targets[id]);
  }

  function onSlide(event) {
    onChange(Number(event.currentTarget.value));
  }

  function nudge(direction) {
    onChange(stepByBlind(value, bigBlind, direction, floor, cap));
  }

  function onType(event) {
    text = event.currentTarget.value;
    const parsed = parseAmountInput(text, { isBTC, decimals });
    if (parsed !== null) onChange(parsed);
  }

  function onFieldKey(event) {
    if (event.key === 'Enter') {
      event.preventDefault();
      typing = false;
      if (!problem) onCommit();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      typing = false;
      event.currentTarget.blur();
      if (compact) onClose();
    }
    // Every other key stays in the field: digits typed here must never reach
    // the table's preset shortcuts.
    event.stopPropagation();
  }
</script>

<div class="raise-slider-panel" class:compact hidden={compact && !open} role="group" aria-label="Bet size">
  <div class="preset-buttons">
    {#each PRESETS as p (p.id)}
      <button
        type="button"
        class:on={onPreset === p.id}
        aria-pressed={onPreset === p.id}
        title="{p.label} (key {p.key})"
        onclick={() => pick(p.id)}
      >{p.label}</button>
    {/each}
  </div>
  <div class="sizer-row">
    <button type="button" class="step" onclick={() => nudge(-1)} aria-label="One big blind less">&minus;</button>
    <input
      type="range"
      class="raise-slider"
      min={floor}
      max={Math.max(cap, floor)}
      {step}
      value={value}
      oninput={onSlide}
      aria-label="Raise amount"
    />
    <button type="button" class="step" onclick={() => nudge(1)} aria-label="One big blind more">+</button>
    <label class="amount-field" class:bad={!!problem}>
      <input
        class="raise-input cd-money"
        type="text"
        inputmode="decimal"
        autocomplete="off"
        spellcheck="false"
        value={text}
        aria-label="Raise to"
        oninput={onType}
        onfocus={() => { typing = true; }}
        onblur={() => { typing = false; }}
        onkeydown={onFieldKey}
      />
      <span class="unit">{unit}</span>
    </label>
    {#if compact}
      <button type="button" class="step close-slider" onclick={onClose} aria-label="Close bet size">&times;</button>
    {/if}
  </div>
  {#if problem}
    <div class="sizer-problem" role="status">{problem}</div>
  {/if}
</div>

<style>
  .raise-slider-panel {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: var(--cd-space-1);
    min-width: 0;
    width: 100%;
  }

  /* THE PHONE'S CLOSED STATE. An author `display: flex` outranks the UA's
     `[hidden]` rule, so the closed sizer must say so itself; measured: without
     this the phone dock grew by 130 px and the felt fell to 33.9%. */
  .raise-slider-panel[hidden] { display: none; }

  .preset-buttons {
    display: flex;
    gap: var(--cd-space-1);
    justify-content: center;
  }

  .preset-buttons button {
    flex: 1 1 0;
    min-width: 0;
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-2);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-2);
    color: var(--cd-ink-1);
    font-family: inherit;
    font-size: var(--cd-text-sm);
    font-weight: var(--cd-weight-figure);
    white-space: nowrap;
    cursor: pointer;
    transition: background-color var(--cd-fast) var(--cd-ease), color var(--cd-fast) var(--cd-ease),
                border-color var(--cd-fast) var(--cd-ease);
  }

  .preset-buttons button:hover { background: var(--cd-money-dim); color: var(--cd-ink); }
  .preset-buttons button.on {
    background: var(--cd-money-dim);
    border-color: var(--cd-money-line);
    color: var(--cd-money);
  }
  .preset-buttons button:focus-visible,
  .step:focus-visible,
  .raise-slider:focus-visible { outline: 2px solid var(--cd-accent-hi); outline-offset: 2px; }

  .sizer-row {
    display: flex;
    align-items: center;
    gap: var(--cd-space-1);
    min-width: 0;
  }

  .step {
    flex: 0 0 auto;
    width: var(--cd-control-sm);
    min-height: var(--cd-control-sm);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-line-strong);
    background: var(--cd-surface-2);
    color: var(--cd-ink-1);
    font-family: inherit;
    font-size: var(--cd-text-md);
    font-weight: var(--cd-weight-figure);
    line-height: 1;
    cursor: pointer;
  }

  .step:hover { background: var(--cd-surface-3); color: var(--cd-ink); }

  .raise-slider {
    flex: 1 1 0;
    min-width: 0;
    accent-color: var(--cd-money);
    cursor: pointer;
  }

  .amount-field {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    gap: var(--cd-space-1);
    min-height: var(--cd-control-sm);
    padding: 0 var(--cd-space-2);
    border-radius: var(--cd-radius-chip);
    border: 1px solid var(--cd-money-line);
    background: var(--cd-surface-1);
  }

  .amount-field.bad { border-color: var(--cd-danger-line); }

  .raise-input {
    width: 6.5em;
    border: 0;
    background: transparent;
    color: var(--cd-money);
    font-family: inherit;
    font-size: var(--cd-text-md);
    text-align: right;
    outline: none;
  }

  .amount-field:focus-within { border-color: var(--cd-money); box-shadow: 0 0 0 2px var(--cd-money-dim); }

  .unit {
    font-size: var(--cd-text-xs);
    letter-spacing: var(--cd-tracking-label);
    color: var(--cd-ink-2);
  }

  .sizer-problem {
    font-size: var(--cd-text-xs);
    color: var(--cd-danger-hi);
    text-align: center;
  }

  /* THE PHONE: the sizer opens in flow above the action row. Two rows, the
     presets at the touch floor, the slider the width of the dock. */
  .raise-slider-panel.compact { gap: var(--cd-space-2); }
  .raise-slider-panel.compact .preset-buttons button { min-height: var(--cd-touch-min); font-size: var(--cd-text-md); }
  .raise-slider-panel.compact .step { width: var(--cd-touch-min); min-height: var(--cd-touch-min); }
  .raise-slider-panel.compact .amount-field { min-height: var(--cd-touch-min); }
  .raise-slider-panel.compact .raise-input { width: 5.5em; }
</style>
