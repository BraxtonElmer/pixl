<script lang="ts">
  // "Test power off": turn the screen off for a few seconds, turn it back on,
  // and ask whether it really came back.
  import type { ScreenStatus, TestStatus } from './api';

  let {
    screen,
    test,
    onstart,
    oncancel,
    onanswer,
    onclose,
  }: {
    screen: ScreenStatus;
    test: TestStatus | null;
    onstart: () => void;
    oncancel: () => void;
    onanswer: (cameBack: boolean) => void;
    onclose: () => void;
  } = $props();

  let started = $state(false);
  let primary = $state<HTMLButtonElement>();

  const mine = $derived(test && test.id === screen.id ? test : null);
  // Index of the step in progress: 0 off, 1 check connection, 2 back on, 3 ask.
  const stage = $derived(!started || !mine ? -1 : mine.step === 'off' ? 0 : mine.step === 'waking' ? 2 : 3);

  $effect(() => {
    // Focus the main button of each stage.
    void stage;
    primary?.focus();
  });

  function start() {
    started = true;
    onstart();
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (started && mine && mine.step !== 'ask') oncancel();
      onclose();
    }
  }

  const steps = ['Turn the screen off', 'Check that Windows still sees it', 'Turn it back on', 'Ask you whether it came back'];
</script>

<svelte:window onkeydown={keydown} />

<div class="scrim">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="test-title">
    <div class="body">
      {#if mine?.step === 'refused'}
        <h3 id="test-title">Screen {screen.number} didn't turn off</h3>
        <p class="muted">
          {screen.name} didn't accept the power-off command. Pixl will cover it with a black screen instead, which on OLED
          keeps the pixels off too.
        </p>
      {:else if mine?.step === 'ask'}
        <h3 id="test-title">Did screen {screen.number} come back on by itself?</h3>
        {#if !mine.stayedConnected}
          <p class="banner warn">
            Windows lost this screen while it was off, so Pixl couldn't send the command to turn it back on, and your open
            windows may have moved. If it's still dark, press its power button. Pixl will use a black screen for it.
          </p>
        {:else}
          <p class="muted">
            Pixl sent the command to turn it back on{mine.reportsOn ? ' and the screen says it is on' : ''}. If it's still
            dark, press its power button, then answer No.
          </p>
        {/if}
      {:else}
        <h3 id="test-title">Test power off on screen {screen.number}</h3>
        <p class="muted">
          {screen.name} will turn off for about 5 seconds, then Pixl will turn it back on. Your PC and apps keep running.
        </p>
        <ol class="steps">
          {#each steps as text, i}
            <li class:done={stage > i || (stage === 2 && i === 1)} class:now={stage === i || (stage === 0 && i === 1)}>
              <span class="ix">{stage > i ? '✓' : i + 1}</span><span>{text}</span>
            </li>
          {/each}
        </ol>
        {#if !started}
          <p class="note">
            If the screen stays dark at the end, press its power button. Pixl will then use a black screen for it.
          </p>
        {/if}
      {/if}
    </div>
    <div class="foot">
      {#if mine?.step === 'refused'}
        <button bind:this={primary} class="btn primary" onclick={() => onanswer(false)}>OK</button>
      {:else if mine?.step === 'ask'}
        <button class="btn" onclick={() => onanswer(false)}>No, it stayed dark</button>
        <button bind:this={primary} class="btn primary" onclick={() => onanswer(true)}>Yes, it came back</button>
      {:else if started}
        <button bind:this={primary} class="btn" onclick={() => (oncancel(), onclose())}>Stop and turn it back on</button>
      {:else}
        <button class="btn" onclick={onclose}>Cancel</button>
        <button bind:this={primary} class="btn primary" onclick={start}>Start test</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    padding: 16px;
    background: rgba(0, 0, 0, 0.4);
  }
  .dialog {
    width: 520px;
    max-width: 100%;
    border-radius: 10px;
    border: 1px solid var(--stroke-strong);
    background: var(--bg-solid);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 22px 24px;
  }
  h3 {
    margin: 0;
    font: 600 20px/1.25 var(--font-display);
  }
  p {
    margin: 0;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 8px;
    padding: 16px 24px;
    background: var(--surface-2);
    border-top: 1px solid var(--stroke);
  }
  .steps {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .steps li {
    display: grid;
    grid-template-columns: 22px 1fr;
    gap: 10px;
    align-items: center;
    font-size: 13px;
    color: var(--text-2);
  }
  .ix {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 11.5px;
    font-weight: 600;
    background: var(--well);
    border: 1px solid var(--stroke-strong);
  }
  .steps li.done {
    color: var(--text);
  }
  .steps li.done .ix {
    background: var(--good);
    color: #fff;
    border-color: transparent;
  }
  .steps li.now {
    color: var(--text);
    font-weight: 600;
  }
  .steps li.now .ix {
    background: var(--accent);
    color: var(--on-accent);
    border-color: transparent;
  }
  .note {
    font-size: 12.5px;
    color: var(--text-2);
  }
</style>
