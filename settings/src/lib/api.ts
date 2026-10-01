import { invoke } from '@tauri-apps/api/core';

export type Trigger = 'pc' | 'away';
export type Wake = 'any' | 'cursor';
export type Method = 'auto' | 'power' | 'black';

export type ScreenRule = {
  enabled: boolean;
  trigger: Trigger;
  timeoutSecs: number;
  method: Method;
  wake: Wake;
  typingCounts: boolean;
  stayOnWhilePlaying: boolean;
  fade: boolean;
};

export type Hotkey = { ctrl: boolean; alt: boolean; shift: boolean; win: boolean; key: number };
export type HotkeyName = 'turnOffAll' | 'wakeAll' | 'pause';
export type Hotkeys = Record<HotkeyName, Hotkey | null>;

export type Theme = 'system' | 'light' | 'dark';
export type Material = 'acrylic' | 'mica' | 'solid';
export type Appearance = { theme: Theme; accent: string; material: Material };

export type Config = {
  version: number;
  enabled: boolean;
  pauseInFullscreen: boolean;
  respectKeepAwake: boolean;
  keepOnApps: string[];
  hotkeys: Hotkeys;
  appearance: Appearance;
  screens: Record<string, ScreenRule>;
};

export type PowerSupport = 'checking' | 'unsupported' | 'untested' | 'works' | 'failed';

export type ScreenStatus = {
  id: string;
  name: string;
  number: number;
  primary: boolean;
  px: { x: number; y: number; w: number; h: number };
  inches: number | null;
  connection: string;
  internal: boolean;
  power: PowerSupport;
  powerNote: string;
  phase: 'on' | 'fading' | 'off';
  offBy: 'power' | 'black' | null;
  remainingSecs: number | null;
  heldBy: string | null;
};

export type TestStep = 'off' | 'waking' | 'ask' | 'refused';
export type TestStatus = { id: string; step: TestStep; stayedConnected: boolean; reportsOn: boolean };

export type Status = {
  written: number;
  enabled: boolean;
  pausedUntil: number | null;
  hotkeysTaken: HotkeyName[];
  screens: ScreenStatus[];
  test: TestStatus | null;
};

export type State = {
  config: Config;
  startWithWindows: boolean;
  systemAccent: string;
  mica: boolean;
  version: string;
};

export const DEFAULT_RULE: ScreenRule = {
  enabled: true,
  trigger: 'pc',
  timeoutSecs: 300,
  method: 'auto',
  wake: 'any',
  typingCounts: true,
  stayOnWhilePlaying: true,
  fade: true,
};

export const api = {
  state: () => invoke<State>('get_state'),
  saveConfig: (config: Config) => invoke<void>('save_config', { config }),
  watch: () => invoke<Status | null>('watch'),
  startTray: () => invoke<boolean>('start_tray'),
  /** Minutes; 0 resumes, -1 pauses until Pixl restarts. */
  pause: (minutes: number) => invoke<boolean>('pause', { minutes }),
  turnAll: (off: boolean) => invoke<boolean>('turn_all', { off }),
  testStart: (index: number) => invoke<boolean>('test_start', { index }),
  testAnswer: (cameBack: boolean) => invoke<boolean>('test_answer', { cameBack }),
  testCancel: () => invoke<boolean>('test_cancel'),
  recheck: (index: number) => invoke<boolean>('recheck', { index }),
  openApps: () => invoke<string[]>('open_apps'),
  setStartWithWindows: (on: boolean) => invoke<boolean>('set_start_with_windows', { on }),
  applyMaterial: (material: Material, dark: boolean) => invoke<Material>('apply_material', { material, dark }),
  open: (which: 'source' | 'issues' | 'folder') => invoke<void>('open_link', { which }),
};
