<script lang="ts">
  // Shows a shortcut as keycaps; click it and press new keys to change it.
  import type { Hotkey } from './api';
  import { hotkeyParts, vkFromCode } from './keys';

  let {
    value,
    label,
    taken = false,
    onchange,
  }: { value: Hotkey | null; label: string; taken?: boolean; onchange: (h: Hotkey | null) => void } = $props();

  let recording = $state(false);
  let hint = $state('');

  function keydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      stop();
      return;
    }
    if ((e.key === 'Backspace' || e.key === 'Delete') && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
      onchange(null);
      stop();
      return;
    }
    const key = vkFromCode(e.code);
    if (key === null) return; // a modifier on its own: keep waiting
    if (!e.ctrlKey && !e.altKey && !e.metaKey) {
      hint = 'Hold Ctrl, Alt or Win with it';
      return;
    }
    onchange({ ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, win: e.metaKey, key });
    stop();
  }

  function stop() {
    recording = false;
    hint = '';
  }
</script>

<button
  class="hotkey"
  class:recording
  class:taken
  type="button"
  aria-label="{label}: {value ? hotkeyParts(value).join(' + ') : 'none'}. Click to change."
  onclick={() => {
    recording = !recording;
    hint = '';
  }}
  onkeydown={keydown}
  onblur={stop}
>
  {#if recording}
    <span class="prompt">{hint || 'Press keys… (Esc cancels, Backspace clears)'}</span>
  {:else if value}
    {#each hotkeyParts(value) as k}<kbd>{k}</kbd>{/each}
  {:else}
    <span class="prompt">None</span>
  {/if}
</button>

<style>
  .hotkey {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    min-height: 30px;
    padding: 3px 6px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    cursor: pointer;
    flex-shrink: 0;
    transition: background var(--fast), border-color var(--fast);
  }
  .hotkey:hover {
    background: var(--control-hover);
    border-color: var(--stroke);
  }
  .recording {
    border-color: var(--accent);
    background: var(--control);
  }
  .taken kbd {
    border-color: var(--bad);
    color: var(--bad);
  }
  .prompt {
    font-size: 12px;
    color: var(--text-2);
    padding: 0 4px;
  }
  kbd {
    display: inline-block;
    min-width: 18px;
    padding: 0 5px;
    border-radius: 4px;
    border: 1px solid var(--stroke-strong);
    border-bottom-width: 2px;
    background: var(--control);
    font: 11.5px/18px var(--font);
    text-align: center;
    color: var(--text);
  }
</style>
