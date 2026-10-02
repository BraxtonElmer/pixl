<script lang="ts">
  // The screens, drawn where Windows has them, each showing what it's doing now.
  import type { ScreenRule, ScreenStatus } from './api';
  import { clock } from './time';

  let {
    screens,
    rules,
    selected,
    elapsed,
    paused,
    enabled,
    onselect,
  }: {
    screens: ScreenStatus[];
    rules: ScreenRule[];
    /** Indexes of the screens being edited. */
    selected: number[];
    /** Seconds since `screens` was fetched, so countdowns keep ticking between fetches. */
    elapsed: number;
    paused: boolean;
    enabled: boolean;
    /** `add` is true for Ctrl-click: add or remove the screen instead of picking only it. */
    onselect: (i: number, add: boolean) => void;
  } = $props();

  let width = $state(600);
  const MAX_H = 230;
  const GAP = 4;

  const box = $derived.by(() => {
    const xs = screens.flatMap((s) => [s.px.x, s.px.x + s.px.w]);
    const ys = screens.flatMap((s) => [s.px.y, s.px.y + s.px.h]);
    const minX = Math.min(...xs);
    const minY = Math.min(...ys);
    const w = Math.max(...xs) - minX;
    const h = Math.max(...ys) - minY;
    const scale = Math.min((width - 8) / w, MAX_H / h);
    return { minX, minY, scale, w: w * scale, h: h * scale };
  });

  function line(s: ScreenStatus, rule: ScreenRule): [string, string] {
    if (!enabled) return ['Pixl is off', ''];
    if (!rule.enabled) return ['Always on', 'Pixl leaves it alone'];
    if (paused) return ['Paused', ''];
    if (s.phase === 'off') {
      return [
        'Off',
        rule.wake === 'cursor' ? 'Move the cursor here to wake' : 'Move the mouse to wake',
      ];
    }
    if (s.phase === 'fading') return ['Fading out…', 'Move the mouse to cancel'];
    if (s.heldBy) return ['Kept on', s.heldBy];
    if (s.remainingSecs !== null) {
      return [
        `Off in ${clock(s.remainingSecs - elapsed)}`,
        rule.trigger === 'pc' ? 'when the PC is idle' : "when you're away from it",
      ];
    }
    return ['On', ''];
  }
</script>

<div class="desk" bind:clientWidth={width}>
  <div class="stage" style:width="{box.w}px" style:height="{box.h}px">
    {#each screens as s, i (s.id)}
      {@const [title, sub] = line(s, rules[i])}
      <button
        class="screen"
        class:off={s.phase === 'off' && enabled && !paused}
        class:fading={s.phase === 'fading'}
        class:ignored={!rules[i].enabled}
        aria-pressed={selected.includes(i)}
        aria-label="Screen {s.number}, {s.name}. {title}. {sub}"
        style:left="{(s.px.x - box.minX) * box.scale + GAP / 2}px"
        style:top="{(s.px.y - box.minY) * box.scale + GAP / 2}px"
        style:width="{s.px.w * box.scale - GAP}px"
        style:height="{s.px.h * box.scale - GAP}px"
        onclick={(e) => onselect(i, e.ctrlKey || e.metaKey || e.shiftKey)}
      >
        <span class="num">{s.number}</span>
        <span class="state">
          <b>{title}</b>
          {#if sub}<span>{sub}</span>{/if}
        </span>
        <span class="name">{s.name}{s.primary ? ' · Main' : ''}</span>
      </button>
    {/each}
  </div>
</div>

<style>
  .desk {
    display: flex;
    justify-content: center;
    padding: 16px 4px;
    border-radius: var(--radius);
    /* The same dotted canvas as ScreenStitch's desk. */
    background-color: var(--well);
    background-image: radial-gradient(var(--stroke-strong) 1px, transparent 1px);
    background-size: 18px 18px;
    overflow: hidden;
  }
  .stage {
    position: relative;
    flex-shrink: 0;
  }
  .screen {
    position: absolute;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 4px;
    padding: 8px;
    text-align: left;
    border-radius: 6px;
    border: 2px solid var(--stroke-strong);
    background: var(--screen-on);
    color: var(--text);
    cursor: pointer;
    overflow: hidden;
    transition:
      background 500ms ease,
      color 500ms ease,
      border-color var(--fast);
  }
  .screen:hover {
    border-color: var(--text-2);
  }
  .screen[aria-pressed='true'] {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 30%, transparent);
    z-index: 1;
  }
  .fading {
    background: color-mix(in srgb, var(--screen-on) 45%, var(--screen-off));
  }
  .off {
    background: var(--screen-off);
    color: #a7adb6;
  }
  .ignored {
    background: repeating-linear-gradient(-45deg, var(--screen-on) 0 8px, var(--well) 8px 16px);
  }
  .num {
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    border-radius: 5px;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--on-accent);
    font: 600 12px var(--font-display);
  }
  .state {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .state b {
    font-weight: 600;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .state span,
  .name {
    font-size: 11px;
    opacity: 0.8;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
