<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  // Our own title bar so the whole window is one pane of the chosen material;
  // the native one can't take the frost's tint.
  const win = getCurrentWindow();
  let maximized = $state(false);

  onMount(() => {
    const sync = async () => (maximized = await win.isMaximized());
    sync();
    const off = win.onResized(sync);
    return () => {
      off.then((f) => f());
    };
  });
</script>

<div class="bar" data-tauri-drag-region>
  <svg class="logo" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
    <rect width="16" height="16" rx="4" fill="var(--accent)" />
    <rect x="3" y="3.6" width="10" height="7" rx="1.2" fill="none" stroke="var(--on-accent)" stroke-width="1.3" />
    <path d="M6 13h4M8 10.6v2.4" stroke="var(--on-accent)" stroke-width="1.3" stroke-linecap="round" />
    <rect x="9.6" y="5.2" width="1.8" height="1.8" rx="0.3" fill="var(--on-accent)" />
  </svg>
  <span class="title" data-tauri-drag-region>Pixl</span>
  <span class="grow" data-tauri-drag-region></span>
  <button aria-label="Minimize" onclick={() => win.minimize()}>
    <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0 5h10" stroke="currentColor" /></svg>
  </button>
  <button aria-label={maximized ? 'Restore' : 'Maximize'} onclick={() => win.toggleMaximize()}>
    {#if maximized}
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor">
        <rect x="0.5" y="2.5" width="7" height="7" rx="1" /><path d="M2.5 2.5V1.5a1 1 0 0 1 1-1h5a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-1" />
      </svg>
    {:else}
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor"><rect x="0.5" y="0.5" width="9" height="9" rx="1" /></svg>
    {/if}
  </button>
  <button class="close" aria-label="Close" onclick={() => win.close()}>
    <svg width="10" height="10" viewBox="0 0 10 10"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="currentColor" /></svg>
  </button>
</div>

<style>
  .bar {
    flex-shrink: 0;
    height: 36px;
    display: flex;
    align-items: center;
    padding-left: 14px;
    gap: 10px;
  }
  .logo {
    flex-shrink: 0;
  }
  .title {
    font-size: 12px;
    color: var(--text-2);
  }
  .grow {
    flex: 1;
    align-self: stretch;
  }
  button {
    width: 46px;
    height: 36px;
    border: 0;
    background: transparent;
    color: var(--text);
    display: grid;
    place-items: center;
    cursor: default;
    transition: background var(--fast);
  }
  button:hover {
    background: var(--control-hover);
  }
  .close:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
