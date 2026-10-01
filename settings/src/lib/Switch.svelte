<script lang="ts">
  let {
    checked,
    label,
    onchange,
    big = false,
  }: { checked: boolean; label: string; onchange: (on: boolean) => void; big?: boolean } = $props();
</script>

<button
  class="switch"
  class:on={checked}
  class:big
  role="switch"
  aria-checked={checked}
  aria-label={label}
  onclick={() => onchange(!checked)}
>
  <span class="knob"></span>
</button>

<style>
  .switch {
    position: relative;
    flex-shrink: 0;
    width: 40px;
    height: 20px;
    padding: 0;
    border-radius: 10px;
    border: 1px solid var(--text-2);
    background: transparent;
    cursor: pointer;
    transition: background var(--fast), border-color var(--fast);
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--text-2);
    transition: transform var(--fast), background var(--fast), width var(--fast);
  }
  .switch:active .knob {
    width: 15px;
  }
  .on {
    background: var(--accent);
    border-color: var(--accent);
  }
  .on .knob {
    background: var(--on-accent);
    transform: translateX(20px);
  }
  .on:active .knob {
    transform: translateX(17px);
  }
  .big {
    width: 52px;
    height: 26px;
    border-radius: 13px;
  }
  .big .knob {
    top: 4px;
    left: 4px;
    width: 16px;
    height: 16px;
  }
  .big.on .knob {
    transform: translateX(26px);
  }
</style>
