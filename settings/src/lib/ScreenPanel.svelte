<script lang="ts">
  // The selected screens: when they turn off and how they wake. With several
  // selected, a setting they share shows normally, one they differ on shows
  // as mixed, and whatever you change applies to all of them.
  import type { ScreenRule, ScreenStatus } from './api';
  import Help from './Help.svelte';
  import Switch from './Switch.svelte';
  import TimeInput from './TimeInput.svelte';

  let {
    screens,
    rules,
    onchange,
  }: {
    screens: ScreenStatus[];
    rules: ScreenRule[];
    /** The settings to change on every selected screen. */
    onchange: (patch: Partial<ScreenRule>) => void;
  } = $props();

  /** The value every selected screen has, or null when they differ. */
  function same<K extends keyof ScreenRule>(k: K): ScreenRule[K] | null {
    const first = rules[0]?.[k];
    return rules.every((r) => r[k] === first) ? first : null;
  }

  const one = $derived(screens.length === 1);
  const enabled = $derived(same('enabled'));
  const trigger = $derived(same('trigger'));
  const timeout = $derived(same('timeoutSecs'));
  const wake = $derived(same('wake'));
  const anyEnabled = $derived(rules.some((r) => r.enabled));

  const details = $derived(
    one
      ? [
          screens[0].inches ? `${screens[0].inches.toFixed(1)}″` : '',
          screens[0].connection,
          `${screens[0].px.w} × ${screens[0].px.h}`,
          screens[0].primary ? 'Main display' : '',
        ]
          .filter(Boolean)
          .join(' · ')
      : screens.map((s) => s.name).join(', '),
  );
  const it = $derived(one ? 'it' : 'they');
</script>

<section class="card pad panel">
  <div class="head">
    <span class="badge" class:many={!one}>{one ? screens[0].number : screens.length}</span>
    <div class="who">
      <h2>{one ? screens[0].name : `Editing ${screens.length} screens`}</h2>
      <span class="muted small">{details}</span>
    </div>
    <span class="muted small">
      {enabled === null ? 'Mixed: ' : ''}Let Pixl turn {one ? 'this screen' : 'these screens'} off
    </span>
    <Switch
      checked={enabled === true}
      label="Let Pixl turn {one ? 'this screen' : 'these screens'} off"
      onchange={(v) => onchange({ enabled: enabled === null ? true : v })}
    />
  </div>

  {#if !anyEnabled}
    <p class="note off-note">
      Pixl leaves {one ? 'this screen' : 'these screens'} alone. {one ? 'It stays' : 'They stay'} on until Windows' own power
      settings turn {one ? 'it' : 'them'} off.
    </p>
  {:else}
    <div class="block">
      <div class="block-title">
        When should {it} turn off?
        {#if trigger === null}<span class="mixed">Mixed</span>{/if}
        <Help text="Pick the one that matches how you use the screen. Every screen can have its own rule." />
      </div>
      <div class="choices" role="radiogroup" aria-label="When should {it} turn off">
        <button class="choice" role="radio" aria-checked={trigger === 'pc'} onclick={() => onchange({ trigger: 'pc' })}>
          <span class="dot"></span>
          <b>When I stop using the PC</b>
          <span class="desc">No mouse or keyboard input anywhere. It stays on while you work on another screen.</span>
          <span class="eg">Good for a second screen showing chat, music or a dashboard</span>
        </button>
        <button class="choice" role="radio" aria-checked={trigger === 'away'} onclick={() => onchange({ trigger: 'away' })}>
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
        {#if timeout === null}<span class="mixed">Mixed</span>{/if}
        <Help text="The timer starts again every time you use the screen. Drag the slider or type any time from 10 seconds to 24 hours." />
      </div>
      <TimeInput secs={timeout} onchange={(v) => onchange({ timeoutSecs: v })} />
    </div>

    <div class="block">
      <div class="block-title">
        What brings {one ? 'it' : 'them'} back?
        {#if wake === null}<span class="mixed">Mixed</span>{/if}
        <Help text="Any input brings a screen back the moment you touch the mouse or keyboard. 'Moving the cursor onto it' keeps it dark while you work on other screens and wakes it only when you move the mouse over to it." />
      </div>
      <div class="choices" role="radiogroup" aria-label="What brings {one ? 'it' : 'them'} back">
        <button class="choice" role="radio" aria-checked={wake === 'any'} onclick={() => onchange({ wake: 'any' })}>
          <span class="dot"></span>
          <b>Any mouse or key</b>
          <span class="desc">Touch anything and it's back straight away.</span>
        </button>
        <button class="choice" role="radio" aria-checked={wake === 'cursor'} onclick={() => onchange({ wake: 'cursor' })}>
          <span class="dot"></span>
          <b>Moving the cursor onto it</b>
          <span class="desc">Stays dark while you work on other screens.</span>
        </button>
      </div>
      {#if wake === 'any' && trigger === 'away'}
        <p class="note">
          With this rule, a screen comes back whenever you touch the mouse or keyboard, then turns off again after the
          time above. "Moving the cursor onto it" usually fits better.
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
  .mixed {
    padding: 1px 7px;
    border-radius: 9px;
    background: var(--well);
    border: 1px solid var(--stroke-strong);
    color: var(--text-2);
    font-size: 11px;
    font-weight: 600;
  }
  .badge.many {
    background: var(--text);
    color: var(--bg-solid);
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
