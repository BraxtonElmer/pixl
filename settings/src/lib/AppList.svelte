<script lang="ts">
  // The programs that keep every screen on while they run. Add one from the
  // apps open right now, or type its exe name.
  import { api } from './api';

  let { apps, onchange }: { apps: string[]; onchange: (apps: string[]) => void } = $props();

  let picking = $state(false);
  let open = $state<string[]>([]);
  let typed = $state('');
  let input = $state<HTMLInputElement>();

  const has = (name: string) => apps.some((a) => a.toLowerCase() === name.toLowerCase());
  const choices = $derived(open.filter((a) => !has(a)));

  async function startPicking() {
    picking = true;
    typed = '';
    open = await api.openApps().catch(() => []);
    input?.focus();
  }

  function add(name: string) {
    let n = name.trim();
    if (!n) return;
    if (!/\.exe$/i.test(n)) n += '.exe';
    if (!has(n)) onchange([...apps, n]);
    picking = false;
  }
</script>

<div class="apps">
  {#if apps.length}
    <div class="chips">
      {#each apps as a, i (a)}
        <span class="chip">
          {a}
          <button aria-label="Remove {a}" onclick={() => onchange(apps.filter((_, j) => j !== i))}>
            <svg width="8" height="8" viewBox="0 0 10 10"><path d="M1 1l8 8M9 1l-8 8" stroke="currentColor" stroke-width="1.5" /></svg>
          </button>
        </span>
      {/each}
    </div>
  {:else}
    <span class="muted small">No apps yet.</span>
  {/if}

  {#if picking}
    <div class="picker">
      <form
        class="type"
        onsubmit={(e) => {
          e.preventDefault();
          add(typed);
        }}
      >
        <input bind:this={input} bind:value={typed} placeholder="Type a program, e.g. obs64.exe" aria-label="Program name" />
        <button class="btn primary" type="submit" disabled={!typed.trim()}>Add</button>
        <button class="btn" type="button" onclick={() => (picking = false)}>Cancel</button>
      </form>
      {#if choices.length}
        <span class="muted small">Or pick one that's open now:</span>
        <div class="list">
          {#each choices as c (c)}
            <button class="pick" onclick={() => add(c)}>{c}</button>
          {/each}
        </div>
      {/if}
    </div>
  {:else}
    <div>
      <button class="btn" onclick={startPicking}>
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M8 3v10M3 8h10" /></svg>
        Add app
      </button>
    </div>
  {/if}
</div>

<style>
  .apps {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 6px 0 10px;
    border-radius: 13px;
    background: var(--well);
    border: 1px solid var(--stroke);
    font-size: 12.5px;
  }
  .chip button {
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: none;
    color: var(--text-2);
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  .chip button:hover {
    background: var(--control-hover);
  }
  .picker {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .type {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .type input {
    flex: 1;
    min-width: 120px;
    height: 32px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--stroke-strong);
    border-bottom-color: var(--text-2);
    background: var(--control);
  }
  .type input:focus {
    outline: none;
    border-bottom: 2px solid var(--accent);
  }
  .list {
    display: flex;
    flex-direction: column;
    max-height: 180px;
    overflow-y: auto;
    border-radius: var(--radius-sm);
    border: 1px solid var(--stroke);
    background: var(--control);
  }
  .pick {
    text-align: left;
    height: 30px;
    padding: 0 10px;
    border: 0;
    background: none;
    cursor: pointer;
    font-size: 13px;
    flex-shrink: 0;
  }
  .pick:hover {
    background: var(--control-hover);
  }
</style>
