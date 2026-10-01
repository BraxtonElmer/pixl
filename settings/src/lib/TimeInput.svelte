<script lang="ts">
  // How long before a screen turns off: a slider for the usual values and a
  // box to type any time from 10 seconds to 24 hours.
  import { STEPS, UNIT_SECS, bestUnit, clampSecs, describe, nearestStep, type Unit } from './time';

  let { secs, onchange }: { secs: number; onchange: (secs: number) => void } = $props();

  // Set from `secs` by the effect below.
  let unit = $state<Unit>('min');
  let text = $state('');
  let editing = $state(false);
  // The time before typing started, for Escape to go back to.
  let before = 0;

  // Follow outside changes (another screen selected, slider moved) unless the user is typing.
  $effect(() => {
    if (!editing) {
      unit = bestUnit(secs);
      text = String(+(secs / UNIT_SECS[unit]).toFixed(2));
    }
  });

  function commit() {
    editing = false;
    const n = parseFloat(text.replace(',', '.'));
    if (!Number.isFinite(n) || n <= 0) {
      text = String(+(secs / UNIT_SECS[unit]).toFixed(2));
      return;
    }
    onchange(clampSecs(n * UNIT_SECS[unit]));
  }

  /** Save while typing too, so closing the window doesn't lose a typed time. */
  function typed() {
    const n = parseFloat(text.replace(',', '.'));
    if (Number.isFinite(n) && n > 0) onchange(clampSecs(n * UNIT_SECS[unit]));
  }

  function setUnit(u: Unit) {
    const n = parseFloat(text.replace(',', '.'));
    unit = u;
    if (Number.isFinite(n) && n > 0) onchange(clampSecs(n * UNIT_SECS[u]));
  }
</script>

<div class="time">
  <input
    class="slider"
    type="range"
    min="0"
    max={STEPS.length - 1}
    step="1"
    value={nearestStep(secs)}
    aria-label="Time before turning off"
    aria-valuetext={describe(secs)}
    oninput={(e) => onchange(STEPS[+e.currentTarget.value])}
  />
  <div class="typed">
    <input
      class="num"
      type="text"
      inputmode="decimal"
      aria-label="Type the time"
      bind:value={text}
      onfocus={() => {
        editing = true;
        before = secs;
      }}
      oninput={typed}
      onblur={commit}
      onkeydown={(e) => {
        if (e.key === 'Enter') e.currentTarget.blur();
        if (e.key === 'Escape') {
          editing = false;
          onchange(before);
          text = String(+(before / UNIT_SECS[unit]).toFixed(2));
          e.currentTarget.blur();
        }
      }}
    />
    <select aria-label="Unit" value={unit} onchange={(e) => setUnit(e.currentTarget.value as Unit)}>
      <option value="sec">seconds</option>
      <option value="min">minutes</option>
      <option value="hr">hours</option>
    </select>
  </div>
</div>

<style>
  .time {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  .slider {
    flex: 1;
    min-width: 160px;
    accent-color: var(--accent);
  }
  .typed {
    display: flex;
    gap: 6px;
  }
  .num,
  select {
    height: 32px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--stroke-strong);
    border-bottom-color: var(--text-2);
    background: var(--control);
    padding: 0 8px;
  }
  .num {
    width: 64px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .num:focus,
  select:focus {
    outline: none;
    border-bottom: 2px solid var(--accent);
  }
  select {
    cursor: pointer;
  }
</style>
