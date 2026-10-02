<script lang="ts">
  import { installUpdate, type Update } from './update';

  let {
    update,
    current,
    variant,
    onlater,
  }: { update: Update; current: string; variant: 'window' | 'banner'; onlater: () => void } = $props();

  let progress = $state<number | null>(null);
  let error = $state('');
  // The popup always shows what's new; the banner only when asked.
  let notesOpen = $state(false);
  const showNotes = $derived(variant === 'window' || notesOpen);

  async function install() {
    error = '';
    progress = 0;
    try {
      await installUpdate(update, (p) => (progress = p));
    } catch (e) {
      progress = null;
      error = `The update couldn't be installed. ${String(e)}`;
    }
  }
</script>

<section class="update {variant}" class:card={variant === 'window'} aria-live="polite">
  <div class="head">
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M10 3v10M6 9l4 4 4-4" /><path d="M4 16h12" />
    </svg>
    <div class="text">
      <strong>Pixl {update.version} is available</strong>
      <span class="muted">You have {current}. Your settings are kept.</span>
    </div>
    {#if variant === 'banner' && update.body}
      <button class="link" onclick={() => (notesOpen = !notesOpen)}>{showNotes ? 'Hide' : "What's new"}</button>
    {/if}
  </div>

  {#if showNotes && update.body}
    <pre class="notes">{update.body}</pre>
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {/if}

  {#if progress !== null}
    <div class="progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(progress * 100)}>
      <span style:width="{Math.round(progress * 100)}%"></span>
    </div>
    <span class="muted small">{progress < 1 ? 'Downloading…' : 'Installing, Pixl will restart in a moment…'}</span>
  {:else}
    <div class="actions">
      <button class="btn" onclick={onlater}>Later</button>
      <button class="btn primary" onclick={install}>Update now</button>
    </div>
  {/if}
</section>

<style>
  .update {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .update.window {
    flex: 1;
    margin: 4px 20px 20px;
    padding: 20px;
    min-height: 0;
  }
  .update.banner {
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .head svg {
    flex-shrink: 0;
    color: var(--accent);
  }
  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .window strong {
    font: 600 18px var(--font-display);
  }
  .notes {
    flex: 1;
    min-height: 0;
    max-height: 180px;
    overflow: auto;
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--well);
    font: 13px/1.5 var(--font);
    white-space: pre-wrap;
    user-select: text;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .banner .actions {
    margin-top: -4px;
  }
  .progress {
    height: 4px;
    border-radius: 2px;
    background: var(--well);
    overflow: hidden;
  }
  .progress span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width var(--fast);
  }
  .error {
    margin: 0;
    color: var(--warn-text);
    font-size: 13px;
  }
  .small {
    font-size: 12px;
  }
</style>
