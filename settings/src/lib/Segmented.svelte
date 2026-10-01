<script lang="ts" generics="T extends string">
  let {
    value,
    options,
    label,
    onchange,
  }: { value: T | null; options: { value: T; label: string }[]; label: string; onchange: (v: T) => void } =
    $props();
</script>

<div class="seg" role="radiogroup" aria-label={label}>
  {#each options as o (o.value)}
    <button role="radio" aria-checked={value === o.value} class:active={value === o.value} onclick={() => onchange(o.value)}>
      {o.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    border-radius: var(--radius-sm);
    background: var(--well);
    border: 1px solid var(--stroke);
  }
  button {
    height: 28px;
    padding: 0 12px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    cursor: pointer;
    transition: background var(--fast);
    white-space: nowrap;
  }
  button:hover {
    background: var(--control-hover);
  }
  .active,
  .active:hover {
    background: var(--control);
    box-shadow: 0 0 0 1px var(--stroke-strong);
  }
</style>
