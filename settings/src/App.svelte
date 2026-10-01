<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import {
    api,
    DEFAULT_RULE,
    type Config,
    type Hotkey,
    type HotkeyName,
    type Material,
    type ScreenRule,
    type State,
    type Status,
    type Theme,
  } from './lib/api';
  import { textOn } from './lib/color';
  import { sameHotkey } from './lib/keys';
  import AppList from './lib/AppList.svelte';
  import Desk from './lib/Desk.svelte';
  import Help from './lib/Help.svelte';
  import HotkeyInput from './lib/HotkeyInput.svelte';
  import ScreenPanel from './lib/ScreenPanel.svelte';
  import Segmented from './lib/Segmented.svelte';
  import Switch from './lib/Switch.svelte';
  import TitleBar from './lib/TitleBar.svelte';

  let st = $state<State | null>(null);
  let status = $state<Status | null>(null);
  let fetchedAt = $state(Date.now());
  let now = $state(Date.now());
  let selected = $state(0);
  let pauseMenu = $state(false);
  let error = $state('');
  /** The tray app hasn't answered for a while. */
  let trayMissing = $state(false);
  let missingSince = 0;

  const media = matchMedia('(prefers-color-scheme: dark)');
  let systemDark = $state(media.matches);
  media.addEventListener('change', (e) => (systemDark = e.matches));

  const ACCENTS = ['#0067c0', '#0f7b6c', '#8764b8', '#c239b3', '#ca5010', '#498205'];

  const config = $derived(st?.config);
  const screens = $derived(status?.screens ?? []);
  const rules = $derived(screens.map((s) => config?.screens[s.id] ?? DEFAULT_RULE));
  const screen = $derived(screens[selected]);
  const paused = $derived(status?.pausedUntil != null);
  const elapsed = $derived(Math.max(0, (now - fetchedAt) / 1000));
  const managed = $derived(rules.filter((r) => r.enabled).length);
  const accent = $derived(st ? st.config.appearance.accent || st.systemAccent : '#0067c0');

  const headline = $derived(
    !config
      ? ''
      : !config.enabled
        ? 'Pixl is off'
        : paused
          ? 'Pixl is paused'
          : managed === 1
            ? 'Looking after 1 screen'
            : `Looking after ${managed} screens`,
  );
  const subline = $derived(
    !config
      ? ''
      : !config.enabled
        ? 'No screen will turn off. Turn Pixl back on to protect your screens.'
        : paused
          ? pausedText()
          : 'Screens turn off when you step away and come back the moment you return. Your PC keeps running.',
  );

  function pausedText() {
    const until = status?.pausedUntil;
    if (!until) return 'No screen will turn off until you resume.';
    const t = new Date(until).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
    return `No screen will turn off until ${t}.`;
  }

  // Theme, accent and window material: applied to the page and to the window itself.
  $effect(() => {
    if (!st) return;
    const theme = st.config.appearance.theme;
    const dark = theme === 'system' ? systemDark : theme === 'dark';
    const root = document.documentElement;
    root.dataset.theme = dark ? 'dark' : 'light';
    root.style.setProperty('--accent', accent);
    root.style.setProperty('--on-accent', textOn(accent));
    getCurrentWindow()
      .setTheme(theme === 'system' ? null : theme)
      .catch(() => {});
    api
      .applyMaterial(st.config.appearance.material, dark)
      .then((m) => (root.dataset.material = m))
      .catch(() => {});
  });

  async function poll() {
    try {
      const s = await api.watch();
      fetchedAt = Date.now();
      if (s) {
        const first = !status;
        status = s;
        trayMissing = false;
        missingSince = 0;
        if (first) selected = Math.max(0, s.screens.findIndex((x) => x.primary));
        if (selected >= s.screens.length) selected = 0;
      } else {
        missingSince ||= Date.now();
        // Give a just-started tray app a moment before saying it isn't running.
        trayMissing = Date.now() - missingSince > 3000;
      }
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    (async () => {
      try {
        st = await api.state();
      } catch (e) {
        error = String(e);
      }
      await poll();
      await tick();
      await getCurrentWindow().show();
      await getCurrentWindow().setFocus();
    })();
    const p = setInterval(poll, 1000);
    const t = setInterval(() => (now = Date.now()), 250);
    // Pick up changes made from the tray menu when coming back to the window,
    // but never over a change of ours that hasn't been written yet.
    const focus = async () => {
      if (dirty) return;
      const s = await api.state();
      if (st && !dirty) {
        st.config = s.config;
        st.startWithWindows = s.startWithWindows;
      }
    };
    window.addEventListener('focus', focus);
    // Write any waiting change before the window closes.
    const closing = getCurrentWindow().onCloseRequested(async () => {
      await flush();
    });
    return () => {
      clearInterval(p);
      clearInterval(t);
      window.removeEventListener('focus', focus);
      closing.then((off) => off());
    };
  });

  // ---- saving ----

  // Changes are written a moment after the last one (dragging the slider makes
  // many), and straight away when the window closes.
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let dirty = false;

  function save() {
    dirty = true;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(flush, 120);
  }

  async function flush() {
    clearTimeout(saveTimer);
    if (!st || !dirty) return;
    dirty = false;
    try {
      await api.saveConfig($state.snapshot(st.config) as Config);
      error = '';
    } catch (e) {
      dirty = true;
      error = String(e);
    }
  }

  function setRule(id: string, rule: ScreenRule) {
    if (!st) return;
    st.config.screens[id] = rule;
    save();
  }

  function setConfig<K extends keyof Config>(k: K, v: Config[K]) {
    if (!st) return;
    st.config[k] = v;
    save();
  }

  function setHotkey(name: HotkeyName, h: Hotkey | null) {
    if (!st) return;
    // One key combination can only do one thing.
    for (const other of Object.keys(st.config.hotkeys) as HotkeyName[]) {
      if (other !== name && sameHotkey(st.config.hotkeys[other], h)) st.config.hotkeys[other] = null;
    }
    st.config.hotkeys[name] = h;
    save();
  }

  function setAppearance<K extends keyof State['config']['appearance']>(k: K, v: State['config']['appearance'][K]) {
    if (!st) return;
    st.config.appearance[k] = v;
    save();
  }

  async function setStartup(on: boolean) {
    if (!st) return;
    try {
      st.startWithWindows = await api.setStartWithWindows(on);
    } catch (e) {
      error = String(e);
    }
  }

  async function pauseFor(minutes: number) {
    pauseMenu = false;
    await api.pause(minutes);
    await poll();
  }

  async function startTray() {
    if (!(await api.startTray())) error = "Pixl.exe wasn't found next to the settings app. Reinstalling Pixl fixes this.";
    missingSince = Date.now();
  }

  const HOTKEYS: { name: HotkeyName; label: string; help?: string }[] = [
    { name: 'turnOffAll', label: 'Turn off all screens now' },
    {
      name: 'wakeAll',
      label: 'Wake every screen',
      help: 'Works even when a screen seems stuck off. Keep this one set so you can always get your screens back.',
    },
    { name: 'pause', label: 'Pause or resume Pixl' },
  ];
</script>

<svelte:window onclick={() => (pauseMenu = false)} />

<TitleBar />
{#if st && config}
  <main>
    <header>
      <div class="titles">
        <h1>{headline}</h1>
        <p class="muted">{subline}</p>
      </div>
      {#if config.enabled}
        {#if paused}
          <button class="btn" onclick={() => pauseFor(0)}>
            <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M4.5 2.8v10.4L13 8z" /></svg>
            Resume
          </button>
        {:else}
          <div class="menu-wrap">
            <button
              class="btn"
              aria-haspopup="menu"
              aria-expanded={pauseMenu}
              onclick={(e) => {
                e.stopPropagation();
                pauseMenu = !pauseMenu;
              }}
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><rect x="4" y="3" width="3" height="10" rx="1" /><rect x="9" y="3" width="3" height="10" rx="1" /></svg>
              Pause
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4"><path d="M2 3.5l3 3 3-3" /></svg>
            </button>
            {#if pauseMenu}
              <div class="menu card" role="menu">
                <button role="menuitem" onclick={() => pauseFor(30)}>For 30 minutes</button>
                <button role="menuitem" onclick={() => pauseFor(60)}>For 1 hour</button>
                <button role="menuitem" onclick={() => pauseFor(180)}>For 3 hours</button>
                <button role="menuitem" onclick={() => pauseFor(-1)}>Until I resume</button>
              </div>
            {/if}
          </div>
          <button class="btn" onclick={() => api.turnAll(true)} title="Turn every screen Pixl looks after off now">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4"><rect x="2" y="3" width="12" height="8" rx="1.2" fill="currentColor" /><path d="M6 14h4" /></svg>
            Turn off now
          </button>
        {/if}
      {/if}
      <span class="state">{config.enabled ? 'On' : 'Off'}</span>
      <Switch big checked={config.enabled} label="Turn Pixl on or off" onchange={(v) => setConfig('enabled', v)} />
    </header>

    {#if error}
      <div class="banner error" role="alert">
        <span class="grow">{error}</span>
        <button class="link" onclick={() => (error = '')}>Dismiss</button>
      </div>
    {/if}
    {#if trayMissing}
      <div class="banner warn" role="alert">
        <span class="grow">Pixl isn't running in the tray, so no screen will turn off.</span>
        <button class="btn" onclick={startTray}>Start Pixl</button>
      </div>
    {/if}

    <div class="grid">
      <div class="col">
        <section class="card pad">
          <div class="section-head">
            <h2>Your screens</h2>
            <span class="muted small">Pick a screen to choose when it turns off.</span>
          </div>
          {#if screens.length}
            <Desk
              {screens}
              {rules}
              {selected}
              {elapsed}
              {paused}
              enabled={config.enabled}
              onselect={(i) => (selected = i)}
            />
          {:else}
            <div class="loading muted">{trayMissing ? 'Start Pixl to see your screens.' : 'Looking for your screens…'}</div>
          {/if}
        </section>

        {#if screen}
          <ScreenPanel
            {screen}
            rule={rules[selected]}
            onchange={(r) => setRule(screen.id, r)}
          />
        {/if}
      </div>

      <aside class="col">
        <section class="card options">
          <div class="row">
            <div class="text">
              <span>Start with Windows</span>
              <span class="muted small">Waits quietly in the tray</span>
            </div>
            <Switch checked={st.startWithWindows} label="Start with Windows" onchange={setStartup} />
          </div>
          <div class="row">
            <div class="text">
              <span class="t">
                Keep on during fullscreen
                <Help text="While a game, video or presentation fills a screen, that screen never turns off, even if you're only using a controller." />
              </span>
              <span class="muted small">Games, videos, presentations</span>
            </div>
            <Switch
              checked={config.pauseInFullscreen}
              label="Keep screens on during fullscreen"
              onchange={(v) => setConfig('pauseInFullscreen', v)}
            />
          </div>
          <div class="row">
            <div class="text">
              <span class="t">
                Respect "keep awake" apps
                <Help text="Video players, calls and presentations can ask Windows to keep the display on. When this is on, screens set to 'When I stop using the PC' listen to them. Some apps ask all the time, which would keep those screens on forever, so it's off by default." />
              </span>
              <span class="muted small">Video players, calls</span>
            </div>
            <Switch
              checked={config.respectKeepAwake}
              label="Respect keep awake requests"
              onchange={(v) => setConfig('respectKeepAwake', v)}
            />
          </div>
        </section>

        <section class="card options">
          <div class="row stack">
            <span class="t">
              Keep every screen on while these apps run
              <Help text="Useful for apps you watch without touching, like OBS while streaming or a long render." />
            </span>
            <AppList apps={config.keepOnApps} onchange={(a) => setConfig('keepOnApps', a)} />
          </div>
        </section>

        <section class="card options">
          <div class="row">
            <div class="text">
              <span class="t">
                Ignore music players
                <Help text="A screen stays on while an app on it plays sound, if 'Stay on while something is playing' is on for that screen. With this on, music players don't count, so music alone won't keep a screen on while you're away. Covers Spotify, Apple Music, iTunes, TIDAL, Deezer, Amazon Music, foobar2000, MusicBee, AIMP and Winamp." />
              </span>
              <span class="muted small">Music alone won't keep a screen on</span>
            </div>
            <Switch
              checked={config.ignoreMusicPlayers}
              label="Ignore music players"
              onchange={(v) => setConfig('ignoreMusicPlayers', v)}
            />
          </div>
          <div class="row stack">
            <span class="t">
              Also ignore sound from
              <Help text="Other apps whose sound shouldn't keep a screen on, like a podcast app or a game launcher that plays music." />
            </span>
            <AppList apps={config.ignoreSoundFrom} onchange={(a) => setConfig('ignoreSoundFrom', a)} />
          </div>
        </section>

        <section class="card options">
          <div class="row stack">
            <span class="t">
              Shortcuts
              <Help text="These work anywhere in Windows. Click one and press the keys you want. Backspace removes it." />
            </span>
          </div>
          {#each HOTKEYS as h (h.name)}
            {@const taken = status?.hotkeysTaken.includes(h.name) ?? false}
            <div class="row">
              <div class="text">
                <span class="t">{h.label}{#if h.help}<Help text={h.help} />{/if}</span>
                {#if taken}<span class="small taken">Another app already uses this. Pick other keys.</span>{/if}
              </div>
              <HotkeyInput
                value={config.hotkeys[h.name]}
                label={h.label}
                {taken}
                onchange={(v) => setHotkey(h.name, v)}
              />
            </div>
          {/each}
        </section>

        <section class="card options">
          <div class="row stack">
            <span>Appearance</span>
            <Segmented
              label="Theme"
              value={config.appearance.theme}
              options={[
                { value: 'system' as Theme, label: 'Windows' },
                { value: 'light' as Theme, label: 'Light' },
                { value: 'dark' as Theme, label: 'Dark' },
              ]}
              onchange={(v) => setAppearance('theme', v)}
            />
            <Segmented
              label="Window background"
              value={config.appearance.material === 'solid' ? 'solid' : 'acrylic'}
              options={[
                { value: 'acrylic' as Material, label: 'Frosted glass' },
                { value: 'solid' as Material, label: 'Solid' },
              ]}
              onchange={(v) => setAppearance('material', v)}
            />
          </div>
          <div class="row stack">
            <span>Accent colour</span>
            <div class="swatches" role="radiogroup" aria-label="Accent colour">
              <button
                class="swatch system"
                role="radio"
                aria-checked={config.appearance.accent === ''}
                aria-label="Windows accent colour"
                title="Same as Windows"
                style:--c={st.systemAccent}
                onclick={() => setAppearance('accent', '')}
              ></button>
              {#each ACCENTS as c (c)}
                <button
                  class="swatch"
                  role="radio"
                  aria-checked={config.appearance.accent === c}
                  aria-label="Accent {c}"
                  style:--c={c}
                  onclick={() => setAppearance('accent', c)}
                ></button>
              {/each}
            </div>
          </div>
        </section>
      </aside>
    </div>

    <footer class="muted">
      <span>Pixl {st.version} · Free and open source</span>
      <span class="spacer"></span>
      <button class="link" onclick={() => api.open('source')}>Source code</button>
      <button class="link" onclick={() => api.open('issues')}>Report a problem</button>
      <button class="link" onclick={() => api.open('folder')}>Settings folder</button>
    </footer>
  </main>

{/if}

<style>
  main {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 6px 24px 14px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .titles {
    flex: 1;
    min-width: 260px;
  }
  h1 {
    font: 600 26px/1.2 var(--font-display);
  }
  h1 + p {
    margin-top: 4px;
  }
  .state {
    font-weight: 600;
    min-width: 26px;
    text-align: right;
    margin-left: 4px;
  }
  .menu-wrap {
    position: relative;
  }
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    padding: 4px;
    min-width: 170px;
    background: var(--bg-solid);
  }
  .menu button {
    height: 32px;
    padding: 0 10px;
    text-align: left;
    border: 0;
    border-radius: 4px;
    background: none;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--control-hover);
  }
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 16px;
    align-items: start;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .section-head {
    display: flex;
    align-items: baseline;
    gap: 12px;
    flex-wrap: wrap;
    margin-bottom: 12px;
  }
  .loading {
    padding: 40px 0;
    text-align: center;
    background: var(--well);
    border-radius: var(--radius);
  }
  .options {
    padding: 4px 16px;
  }
  .taken {
    color: var(--bad);
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding: 2px;
  }
  .swatch {
    width: 24px;
    height: 24px;
    padding: 0;
    border-radius: 50%;
    border: 2px solid transparent;
    background: var(--c);
    box-shadow: inset 0 0 0 2px var(--surface);
    cursor: pointer;
  }
  .swatch.system {
    background: conic-gradient(var(--c), color-mix(in srgb, var(--c) 40%, white), var(--c));
  }
  .swatch[aria-checked='true'] {
    border-color: var(--text);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
    font-size: 12.5px;
  }
  .spacer {
    flex: 1;
  }
  @media (max-width: 900px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
