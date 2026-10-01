<script lang="ts">
  // A small "?" that explains the setting next to it, on hover, focus or click.
  let { text }: { text: string } = $props();

  let button: HTMLButtonElement;
  let open = $state(false);
  let x = $state(0);
  let y = $state(0);
  let tip = $state<HTMLDivElement>();

  async function show() {
    open = true;
    await Promise.resolve();
    if (!tip) return;
    const r = button.getBoundingClientRect();
    const tw = tip.offsetWidth;
    const th = tip.offsetHeight;
    x = Math.min(Math.max(8, r.left + r.width / 2 - tw / 2), innerWidth - tw - 8);
    y = r.bottom + 8 + th > innerHeight - 8 ? r.top - th - 8 : r.bottom + 8;
  }

  function hide() {
    open = false;
  }
</script>

<button
  bind:this={button}
  class="help"
  type="button"
  aria-label="What does this mean?"
  aria-expanded={open}
  onmouseenter={show}
  onmouseleave={hide}
  onfocus={show}
  onblur={hide}
  onclick={() => (open ? hide() : show())}>?</button
>
{#if open}
  <div bind:this={tip} class="tip" role="tooltip" style:left="{x}px" style:top="{y}px">{text}</div>
{/if}

<style>
  .help {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    border-radius: 50%;
    border: 1px solid var(--stroke-strong);
    background: transparent;
    color: var(--text-2);
    font: 600 11px/1 var(--font);
    cursor: help;
    display: inline-grid;
    place-items: center;
    padding: 0;
    vertical-align: middle;
    transition: background var(--fast), color var(--fast);
  }
  .help:hover,
  .help[aria-expanded='true'] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .tip {
    position: fixed;
    z-index: 50;
    max-width: 300px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--tip-bg);
    color: var(--tip-text);
    font-size: 12.5px;
    font-weight: 400;
    line-height: 1.5;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
    pointer-events: none;
    white-space: normal;
  }
</style>
