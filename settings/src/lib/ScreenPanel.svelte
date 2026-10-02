<script lang="ts">
  // Everything about one screen: when it turns off, how, and how it wakes.
  import type { ScreenRule, ScreenStatus } from './api';
  import Help from './Help.svelte';
  import Segmented from './Segmented.svelte';
  import Switch from './Switch.svelte';
  import TimeInput from './TimeInput.svelte';

  let {
    screen,
    rule,
    onchange,
  }: {
    screen: ScreenStatus;
    rule: ScreenRule;
    onchange: (r: ScreenRule) => void;
  } = $props();

  const set = <K extends keyof ScreenRule>(k: K, v: ScreenRule[K]) => onchange({ ...rule, [k]: v });

  const details = $derived(
    [screen.inches ? `${screen.inches.toFixed(1)}″` : '', screen.connection, `${screen.px.w} × ${screen.px.h}`]
      .filter(Boolean)
      .join(' · '),
  );
</script>

<section class="card pad panel">
  <div class="head">
    <span class="badge">{screen.number}</span>
    <div class="who">
      <h2>{screen.name}</h2>
      <span class="muted small">{details}{screen.primary ? ' · Main display' : ''}</span>
    </div>
    <span class="muted small">Let Pixl turn this screen off</span>
    <Switch checked={rule.enabled} label="Let Pixl turn this screen off" onchange={(v) => set('enabled', v)} />
  </div>

  {#if !rule.enabled}
    <p class="note off-note">Pixl leaves this screen alone. It stays on until Windows' own power settings turn it off.</p>
  {:else}
    <div class="block">
      <div class="block-title">
        When should it turn off?
        <Help text="Pick the one that matches how you use this screen. Every screen can have its own rule." />
      </div>
      <div class="choices" role="radiogroup" aria-label="When should it turn off">
        <button class="choice" role="radio" aria-checked={rule.trigger === 'pc'} onclick={() => set('trigger', 'pc')}>
          <span class="dot"></span>
          <b>When I stop using the PC</b>
          <span class="desc">No mouse or keyboard input anywhere. It stays on while you work on another screen.</span>
          <span class="eg">Good for a second screen showing chat, music or a dashboard</span>
        </button>
        <button class="choice" role="radio" aria-checked={rule.trigger === 'away'} onclick={() => set('trigger', 'away')}>
          <span class="dot"></span>
          <b>When I'm not using this screen</b>
          <span class="desc">The cursor hasn't been on it and you haven't typed into its windows, even if you're busy on another screen.</span>
          <span class="eg">Good for protecting an OLED you're not looking at</span>
        </button>
      </div>
    </div>

    <div class="block">
      <div class="block-title">
        After how long?
        <Help text="The timer starts again every time you use the screen. Drag the slider or type any time from 10 seconds to 24 hours. For OLED screens, 1 to 5 minutes is a good range." />
      </div>
      <TimeInput secs={rule.timeoutSecs} onchange={(v) => set('timeoutSecs', v)} />
    </div>

    <div class="block">
      <div class="block-title">
        How should it wake up?
        <Help text="Any input brings it back the moment you touch the mouse or keyboard. 'Cursor moves onto it' keeps it dark while you work on other screens and wakes it only when you move the mouse over to it." />
      </div>
      <Segmented
        label="How to wake it"
        value={rule.wake}
        options={[
          { value: 'any', label: 'Any mouse or key' },
          { value: 'cursor', label: 'Only when the cursor moves onto it' },
        ]}
        onchange={(w) => set('wake', w)}
      />
      {#if rule.wake === 'any' && rule.trigger === 'away'}
        <p class="note">
          With this rule, the screen comes back whenever you touch the mouse or keyboard, then turns off again after the
          time above. "Only when the cursor moves onto it" usually fits better.
        </p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .badge {
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: var(--accent);
    color: var(--on-accent);
    font: 600 17px var(--font-display);
  }
  .who {
    flex: 1;
    min-width: 160px;
    display: flex;
    flex-direction: column;
  }
  .block {
    border-top: 1px solid var(--stroke);
    margin-top: 16px;
    padding-top: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .block-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
  }
  .choices {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 10px;
  }
  .choice {
    text-align: left;
    display: grid;
    grid-template-columns: 18px 1fr;
    gap: 4px 10px;
    padding: 12px 14px;
    border-radius: var(--radius);
    border: 1px solid var(--stroke-strong);
    background: var(--control);
    cursor: pointer;
    transition: background var(--fast), border-color var(--fast);
  }
  .choice:hover {
    background: var(--control-hover);
  }
  .choice[aria-checked='true'] {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .dot {
    width: 18px;
    height: 18px;
    margin-top: 1px;
    border-radius: 50%;
    border: 1px solid var(--text-2);
  }
  .choice[aria-checked='true'] .dot {
    border: 5px solid var(--accent);
  }
  .choice b {
    font-weight: 600;
  }
  .desc,
  .eg {
    grid-column: 2;
    font-size: 12.5px;
  }
  .desc {
    color: var(--text-2);
  }
  .eg {
    color: var(--accent);
  }
  .note {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .off-note {
    margin-top: 14px;
  }
</style>
