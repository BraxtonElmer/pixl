<script lang="ts">
  // Everything about one screen: when it turns off, how, and how it wakes.
  import type { Method, ScreenRule, ScreenStatus } from './api';
  import Help from './Help.svelte';
  import Segmented from './Segmented.svelte';
  import Switch from './Switch.svelte';
  import TimeInput from './TimeInput.svelte';

  let {
    screen,
    rule,
    onchange,
    ontest,
    onrecheck,
  }: {
    screen: ScreenStatus;
    rule: ScreenRule;
    onchange: (r: ScreenRule) => void;
    ontest: () => void;
    onrecheck: () => void;
  } = $props();

  const set = <K extends keyof ScreenRule>(k: K, v: ScreenRule[K]) => onchange({ ...rule, [k]: v });

  /** The screen offers power control, so it can be tested or forced. */
  const canPower = $derived(screen.power === 'works' || screen.power === 'untested');
  /** Automatic only powers off screens that passed the test. */
  const autoPowers = $derived(screen.power === 'works');
  const usesPower = $derived(rule.method === 'power' ? canPower : rule.method === 'auto' && autoPowers);

  const pill = $derived.by((): [string, string] => {
    switch (screen.power) {
      case 'checking':
        return ['neutral', 'Checking the screen…'];
      case 'works':
        return ['good', 'Power off works'];
      case 'untested':
        return ['warn', 'Power off not tested'];
      case 'failed':
        return ['bad', "Power off didn't work"];
      default:
        return ['bad', "Can't power off"];
    }
  });

  const note = $derived.by(() => {
    if (screen.powerNote) return screen.powerNote;
    if (screen.power === 'untested')
      return "This screen says the PC can power it off. Many screens then disconnect and only their power button brings them back, so Pixl uses a black screen unless a test shows this one comes back by itself.";
    if (screen.power === 'checking') return 'Asking the screen whether the PC can power it off.';
    return '';
  });

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
        How should it turn off?
        <Help text="Power off uses the screen's own controls (DDC/CI) to put it to sleep, like pressing its power button. A black screen covers it with a black window instead; on OLED that switches the pixels off too. Automatic uses black, and powers the screen off only once 'Test power off' has shown it comes back by itself." />
      </div>
      <div class="method">
        <div class="top">
          <span class="pill {pill[0]}">{pill[1]}</span>
          <span class="grow"></span>
          {#if !screen.internal}
            {#if canPower}
              <button class="btn" onclick={ontest}>
                <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M8 1.5v6" /><path d="M4.2 4a5.5 5.5 0 1 0 7.6 0" /></svg>
                {screen.power === 'works' ? 'Test again' : 'Test power off'}
              </button>
            {:else if screen.power !== 'checking'}
              <button class="btn" onclick={onrecheck}>Check again</button>
            {/if}
          {/if}
        </div>
        <Segmented
          label="How to turn it off"
          value={rule.method}
          options={[
            { value: 'auto' as Method, label: 'Automatic' },
            ...(canPower || rule.method === 'power' ? [{ value: 'power' as Method, label: 'Power off only' }] : []),
            { value: 'black' as Method, label: 'Black screen only' },
          ]}
          onchange={(m) => set('method', m)}
        />
        <div class="flow" aria-label="What Pixl will do">
          {#if rule.method === 'black'}
            <span class="step cur">Black screen</span>
          {:else if rule.method === 'power' && canPower}
            <span class="step cur">Power off</span><span>no black cover underneath</span>
          {:else if autoPowers}
            <span class="step cur">Power off</span><span>→ if that fails →</span><span class="step">Black screen</span>
          {:else}
            <span class="step cur">Black screen</span>
            {#if canPower}<span>power off after a passing test</span>{/if}
          {/if}
        </div>
        {#if note}<p class="note">{note}</p>{/if}
      </div>
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
      {#if rule.wake === 'cursor' && usesPower}
        <p class="note">
          If Windows stops seeing this screen while it's powered off, the cursor can't move onto it. Testing power off
          checks this; if it happens, Pixl uses a black screen for it instead.
        </p>
      {/if}
      {#if rule.wake === 'any' && rule.trigger === 'away'}
        <p class="note">
          With this rule, the screen comes back whenever you touch the mouse or keyboard, then turns off again after the
          time above. "Only when the cursor moves onto it" usually fits better.
        </p>
      {/if}
    </div>

    <div class="block rows">
      <div class="row">
        <div class="text">
          <span class="t">
            Count typing as activity
            <Help text="When off, only the mouse keeps this screen on and wakes it. Handy if you type on one screen and want another to sleep." />
          </span>
        </div>
        <Switch checked={rule.typingCounts} label="Count typing as activity" onchange={(v) => set('typingCounts', v)} />
      </div>
      <div class="row">
        <div class="text">
          <span class="t">
            Stay on while something is playing
            <Help text="Once the screen is idle, Pixl glances at its picture every 2 seconds. If most of it is changing, like a video or a game, it counts as in use. A clock or a blinking cursor doesn't." />
          </span>
          <span class="muted small">Videos, games, live charts</span>
        </div>
        <Switch
          checked={rule.stayOnWhilePlaying}
          label="Stay on while something is playing"
          onchange={(v) => set('stayOnWhilePlaying', v)}
        />
      </div>
      <div class="row">
        <div class="text">
          <span class="t">
            Fade out first
            <Help text="The screen dims over 5 seconds before it turns off. Move the mouse or press a key during the fade and it cancels." />
          </span>
          <span class="muted small">5 seconds to change your mind</span>
        </div>
        <Switch checked={rule.fade} label="Fade out first" onchange={(v) => set('fade', v)} />
      </div>
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
  .block.rows {
    gap: 0;
    padding-top: 4px;
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
  .method {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--well);
    border: 1px solid var(--stroke);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
  }
  .flow {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .step {
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid var(--stroke-strong);
    background: var(--control);
    color: var(--text);
  }
  .step.cur {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
  .step.skip {
    text-decoration: line-through;
    opacity: 0.6;
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
