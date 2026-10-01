//! The tray app's hidden window: watches the user a few times a second, runs
//! the idle engine, and turns screens off and on. Also the tray icon and menu,
//! shortcuts, messages from the settings window, and display, power and
//! session changes. Everything runs on this one thread.

use std::cell::RefCell;
use std::collections::HashSet;
use std::ptr::{null, null_mut};

use pixl_core::{Action, Engine, Inputs, Phase, ScreenInputs};
use pixl_platform::config::{Config, FADE_MS, Hotkey, ScreenRule, TriggerSetting};
use pixl_platform::display::{self, Display};
use pixl_platform::status::{ScreenPhase, ScreenStatus, Status, unix_ms};
use pixl_platform::tray::{
    MSG_ALL, MSG_OPEN_SETTINGS, MSG_PAUSE, MSG_RELOAD, MSG_WATCH, PAUSE_UNTIL_RESTART, WINDOW_CLASS,
    taskbar_created_message,
};
use pixl_platform::wide::{fill_wide, to_wide};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::RemoteDesktop::{NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu, DispatchMessageW,
    GetCursorPos, GetMessageW, HICON, HMENU, KillTimer, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG,
    PBT_APMRESUMEAUTOMATIC, PBT_APMSUSPEND, PostMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow,
    SetMenuDefaultItem, SetTimer, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage,
    WM_APP, WM_CLOSE, WM_DISPLAYCHANGE, WM_ENDSESSION, WM_HOTKEY, WM_INPUT, WM_LBUTTONUP, WM_NULL, WM_POWERBROADCAST,
    WM_RBUTTONUP, WM_SETTINGCHANGE, WM_TIMER, WM_WTSSESSION_CHANGE, WNDCLASSW, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
};

use crate::input::{self, Input};
use crate::sampler::Sampler;
use crate::{audio, icon, overlay, watch};

const WM_TRAY: u32 = WM_APP + 1;

const TIMER_TICK: usize = 1;
const TIMER_FADE: usize = 2;
const TIMER_REDETECT: usize = 3;
/// Check once a second normally; four times a second while a screen is about
/// to turn off, fading or black, so waking and cancelling feel instant.
const TICK_SLOW_MS: u32 = 1000;
const TICK_FAST_MS: u32 = 250;
/// Speed up this long before a screen would turn off.
const FAST_BEFORE_MS: u64 = 3000;
const FADE_FRAME_MS: u32 = 30;

const HOTKEY_OFF: i32 = 1;
const HOTKEY_WAKE: i32 = 2;
const HOTKEY_PAUSE: i32 = 3;

/// "Stay on while something is playing" only looks at a screen right before it
/// would turn off: one thumbnail this long before, and another a second later.
const PROBE_BEFORE_MS: u64 = 2500;
const PROBE_GAP_MS: u64 = 1000;
const APPS_EVERY_MS: u64 = 5000;
/// Keep status.json fresh this long after the settings window last asked.
const WATCH_MS: u64 = 5000;

const CMD_OPEN: usize = 10;
const CMD_TOGGLE: usize = 11;
const CMD_OFF_NOW: usize = 12;
const CMD_WAKE_ALL: usize = 13;
const CMD_RESUME: usize = 14;
const CMD_PAUSE_30: usize = 15;
const CMD_PAUSE_60: usize = 16;
const CMD_PAUSE_180: usize = 17;
const CMD_PAUSE_FOREVER: usize = 18;
const CMD_QUIT: usize = 19;
/// CMD_SCREEN + i toggles screen i.
const CMD_SCREEN: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pause {
    Until(u64),
    UntilRestart,
}

struct Screen {
    d: Display,
    rule: ScreenRule,
    last_change: u64,
    /// When the first thumbnail of the before-turning-off check was taken.
    probe: Option<u64>,
    /// Why it's being kept on right now (fullscreen game, paused...).
    held_by: Option<String>,
}

struct App {
    hwnd: HWND,
    config: Config,
    screens: Vec<Screen>,
    engine: Engine,
    input: Input,
    sampler: Sampler,
    pause: Option<Pause>,
    locked: bool,
    watch_until: u64,
    running: HashSet<String>,
    next_apps_scan: u64,
    hotkeys_taken: Vec<String>,
    ticks: u64,
    tick_ms: u32,
    icon: HICON,
    taskbar_created: u32,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f)))
}

pub fn run(open_settings_now: bool) {
    let hinst = unsafe { GetModuleHandleW(null()) };
    let class = to_wide(WINDOW_CLASS);
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: null_mut(),
        hCursor: null_mut(),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class.as_ptr(),
    };
    unsafe { RegisterClassW(&wc) };
    // A real (hidden) top-level window: message-only windows miss WM_DISPLAYCHANGE.
    let title = to_wide("Pixl Tray");
    let hwnd = unsafe {
        CreateWindowExW(0, class.as_ptr(), title.as_ptr(), 0, 0, 0, 0, 0, null_mut(), null_mut(), hinst, null())
    };
    if hwnd.is_null() {
        return;
    }
    allow_dark_menus();
    input::listen_for_keys(hwnd);
    audio::init();
    unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) };

    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            hwnd,
            config: Config::load(),
            screens: Vec::new(),
            engine: Engine::new(),
            input: Input::new(),
            sampler: Sampler::default(),
            pause: None,
            locked: false,
            watch_until: 0,
            running: HashSet::new(),
            next_apps_scan: 0,
            hotkeys_taken: Vec::new(),
            ticks: 0,
            tick_ms: TICK_SLOW_MS,
            icon: null_mut(),
            taskbar_created: taskbar_created_message(),
        });
    });
    with_app(|app| {
        app.register_hotkeys();
        app.redetect();
        app.tray(NIM_ADD);
        app.write_status();
    });
    unsafe { SetTimer(hwnd, TIMER_TICK, TICK_SLOW_MS, None) };
    if open_settings_now {
        open_settings();
    }

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Start the settings window (it focuses an already open one by itself).
pub fn open_settings() {
    let Ok(exe) = std::env::current_exe() else { return };
    let _ = std::process::Command::new(exe.with_file_name("pixl-settings.exe")).spawn();
}

impl App {
    fn now(&self) -> u64 {
        input::now()
    }

    // ---- monitors ----

    /// Read the monitors again, keeping what we know about the ones still there.
    fn redetect(&mut self) {
        let now = self.now();
        let found = display::detect();
        let mut old = std::mem::take(&mut self.screens);
        for d in found {
            let rule = self.config.rule_for(&d.id);
            let screen = match old.iter().position(|s| s.d.id == d.id) {
                Some(p) => {
                    let mut s = old.swap_remove(p);
                    if s.d.px != d.px && overlay::is_shown(&s.d.id) {
                        overlay::show(&s.d.id, d.px, now, 0);
                    }
                    s.d = d;
                    s.rule = rule;
                    s
                }
                None => Screen { d, rule, last_change: 0, probe: None, held_by: None },
            };
            self.screens.push(screen);
        }
        for s in &old {
            overlay::hide(&s.d.id);
            self.sampler.forget(&s.d.id);
        }
        // Anything still covered that isn't one of our screens any more.
        for id in overlay::shown() {
            if !self.screens.iter().any(|s| s.d.id == id) {
                overlay::hide(&id);
            }
        }
        self.sync_engine();
        self.tray(NIM_MODIFY);
    }

    fn sync_engine(&mut self) {
        let now = self.now();
        let list: Vec<_> = self.screens.iter().map(|s| (s.d.id.clone(), s.rule.to_rule())).collect();
        let actions = self.engine.set_screens(&list, now);
        self.apply(actions);
    }

    // ---- the tick ----

    fn tick(&mut self) {
        let now = self.now();
        self.ticks += 1;
        if let Some(Pause::Until(t)) = self.pause
            && now >= t
        {
            self.pause = None;
            self.tray(NIM_MODIFY);
        }

        if !self.locked {
            self.input.sample();
            let inputs = self.screen_inputs(now);
            let (cx, cy) = input::cursor();
            let cursor_on = self.screens.iter().position(|s| s.d.px.contains(cx, cy));
            let focus = watch::focus_monitor();
            let focus_on = focus.and_then(|m| self.screens.iter().position(|s| s.d.hmonitor == m));
            let inp = Inputs {
                now,
                last_mouse: self.input.last_mouse,
                last_key: self.input.last_key,
                cursor_on,
                focus_on,
                screens: &inputs,
            };
            let actions = self.engine.step(&inp);
            self.apply(actions);
        }

        if self.ticks.is_multiple_of(4) {
            overlay::raise();
        }
        if now < self.watch_until {
            self.write_status();
        }
        self.pace(now);
    }

    /// Pick the tick speed: fast only while something is about to happen.
    fn pace(&mut self, now: u64) {
        let busy = (0..self.engine.len()).any(|i| {
            self.engine.phase(i) != Phase::On || self.engine.remaining(i, now).is_some_and(|ms| ms <= FAST_BEFORE_MS)
        });
        let ms = if busy { TICK_FAST_MS } else { TICK_SLOW_MS };
        if ms != self.tick_ms {
            self.tick_ms = ms;
            unsafe { SetTimer(self.hwnd, TIMER_TICK, ms, None) };
        }
    }

    /// Per screen: what the engine needs. Also notes on each screen why it is
    /// being kept on, for the settings window.
    fn screen_inputs(&mut self, now: u64) -> Vec<ScreenInputs> {
        let c = &self.config;
        let fullscreen = if c.pause_in_fullscreen { watch::fullscreen_monitor() } else { None };
        let awake = c.respect_keep_awake && watch::display_kept_awake();
        if !c.keep_on_apps.is_empty() && now >= self.next_apps_scan {
            self.running = pixl_platform::apps::running();
            self.next_apps_scan = now + APPS_EVERY_MS;
        } else if c.keep_on_apps.is_empty() {
            self.running.clear();
        }
        let app = c.keep_on_apps.iter().find(|a| self.running.contains(&a.to_lowercase())).cloned();

        // Screens where an app with a window on them is making sound. Only
        // worked out while some screen is about to turn off.
        let about_to_turn_off = (0..self.screens.len()).any(|i| {
            self.screens[i].rule.stay_on_while_playing
                && self.engine.remaining(i, now).is_some_and(|ms| ms <= PROBE_BEFORE_MS)
        });
        let mut with_sound: HashSet<usize> = HashSet::new();
        if about_to_turn_off {
            let ignored: HashSet<String> = c.ignore_sound_from.iter().map(|a| a.to_lowercase()).collect();
            let sounding: HashSet<String> = audio::sounding().into_iter().filter(|n| !ignored.contains(n)).collect();
            if !sounding.is_empty() {
                for (mon, names) in pixl_platform::apps::on_screens() {
                    if names.iter().any(|n| sounding.contains(n)) {
                        with_sound.insert(mon);
                    }
                }
            }
        }

        let mut out = Vec::with_capacity(self.screens.len());
        for (i, s) in self.screens.iter_mut().enumerate() {
            let held_by = if !c.enabled {
                Some("Pixl is turned off".to_string())
            } else if self.pause.is_some() {
                Some("Paused".into())
            } else if fullscreen == Some(s.d.hmonitor) {
                Some("A fullscreen app is open".into())
            } else if let Some(app) = &app {
                Some(format!("{app} is running"))
            } else if awake && s.rule.trigger == TriggerSetting::Pc {
                Some("An app is keeping the display on".into())
            } else {
                None
            };

            // Is something playing? Only asked right before the screen would
            // turn off: an app on it making sound, or a picture that moves
            // between two thumbnails a second apart. Either starts the timer
            // over. (Protected streaming video captures as black, so the
            // sound is what gives it away.)
            let left = self.engine.remaining(i, now);
            let probing =
                s.rule.stay_on_while_playing && held_by.is_none() && left.is_some_and(|ms| ms <= PROBE_BEFORE_MS);
            if probing && with_sound.contains(&s.d.hmonitor) {
                s.last_change = now;
                s.probe = None;
                self.sampler.forget(&s.d.id);
            }
            match (probing, s.probe) {
                (true, None) => {
                    self.sampler.forget(&s.d.id);
                    self.sampler.changed(&s.d.id, s.d.px);
                    s.probe = Some(now);
                }
                (true, Some(t)) if now.saturating_sub(t) >= PROBE_GAP_MS => {
                    if self.sampler.changed(&s.d.id, s.d.px) {
                        s.last_change = now;
                    }
                    s.probe = None;
                    self.sampler.forget(&s.d.id);
                }
                (false, Some(_)) => {
                    s.probe = None;
                    self.sampler.forget(&s.d.id);
                }
                _ => {}
            }
            out.push(ScreenInputs { last_change: s.last_change, held: held_by.is_some() });
            s.held_by = held_by;
        }
        out
    }

    fn apply(&mut self, actions: Vec<Action>) {
        let now = self.now();
        let mut fading = false;
        for a in actions {
            match a {
                Action::Fade(i) => {
                    if let Some(s) = self.screens.get(i) {
                        overlay::show(&s.d.id, s.d.px, now, FADE_MS);
                        fading = true;
                    }
                }
                Action::CancelFade(i) => {
                    if let Some(s) = self.screens.get(i) {
                        overlay::hide(&s.d.id);
                    }
                }
                Action::TurnOff(i) => {
                    if let Some(s) = self.screens.get(i) {
                        overlay::show(&s.d.id, s.d.px, now, 0);
                        self.sampler.forget(&s.d.id);
                    }
                }
                Action::Wake(i) => {
                    if let Some(s) = self.screens.get(i) {
                        overlay::hide(&s.d.id);
                    }
                }
            }
        }
        if fading {
            unsafe { SetTimer(self.hwnd, TIMER_FADE, FADE_FRAME_MS, None) };
        }
        if self.now() < self.watch_until {
            self.write_status();
        }
    }

    fn wake_all(&mut self) {
        let actions = self.engine.wake_all(self.now());
        self.apply(actions);
    }

    fn turn_off_all(&mut self) {
        if self.pause.is_some() {
            self.pause = None;
            self.tray(NIM_MODIFY);
        }
        let actions = self.engine.turn_off_all(self.now());
        self.apply(actions);
    }

    fn set_pause(&mut self, pause: Option<Pause>) {
        self.pause = pause;
        if pause.is_some() {
            self.wake_all();
        }
        self.tray(NIM_MODIFY);
        self.write_status();
    }

    // ---- config, shortcuts ----

    fn reload(&mut self) {
        self.config = Config::load();
        for s in &mut self.screens {
            s.rule = self.config.rule_for(&s.d.id);
        }
        self.register_hotkeys();
        self.sync_engine();
        self.tray(NIM_MODIFY);
        self.write_status();
    }

    fn register_hotkeys(&mut self) {
        self.hotkeys_taken.clear();
        let h = self.config.hotkeys;
        for (id, key, name) in [
            (HOTKEY_OFF, h.turn_off_all, "turnOffAll"),
            (HOTKEY_WAKE, h.wake_all, "wakeAll"),
            (HOTKEY_PAUSE, h.pause, "pause"),
        ] {
            unsafe { UnregisterHotKey(self.hwnd, id) };
            if let Some(k) = key
                && !register_hotkey(self.hwnd, id, &k)
            {
                self.hotkeys_taken.push(name.into());
            }
        }
    }

    fn set_enabled(&mut self, on: bool) {
        self.config = Config::load();
        self.config.enabled = on;
        let _ = self.config.save();
        self.reload();
    }

    fn toggle_screen(&mut self, i: usize) {
        let Some(s) = self.screens.get(i) else { return };
        let id = s.d.id.clone();
        self.config = Config::load();
        let mut rule = self.config.rule_for(&id);
        rule.enabled = !rule.enabled;
        self.config.screens.insert(id, rule);
        let _ = self.config.save();
        self.reload();
    }

    // ---- status for the settings window ----

    fn write_status(&self) {
        let now = self.now();
        let status = Status {
            written: unix_ms(),
            enabled: self.config.enabled,
            paused_until: self.pause.map(|p| match p {
                Pause::Until(t) => unix_ms() + t.saturating_sub(now),
                Pause::UntilRestart => 0,
            }),
            hotkeys_taken: self.hotkeys_taken.clone(),
            screens: self
                .screens
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let phase = match self.engine.phase(i) {
                        Phase::On => ScreenPhase::On,
                        Phase::Fading { .. } => ScreenPhase::Fading,
                        Phase::Off => ScreenPhase::Off,
                    };
                    let held_by = s.held_by.clone();
                    ScreenStatus {
                        id: s.d.id.clone(),
                        name: s.d.name.clone(),
                        number: s.d.number,
                        primary: s.d.primary,
                        px: s.d.px,
                        inches: s.d.inches,
                        connection: s.d.connection.clone(),
                        phase,
                        remaining_secs: if held_by.is_some() {
                            None
                        } else {
                            self.engine.remaining(i, now).map(|ms| ms.div_ceil(1000) as u32)
                        },
                        held_by,
                    }
                })
                .collect(),
        };
        let _ = status.save();
    }

    // ---- tray icon ----

    fn tray(&mut self, action: u32) {
        let old = self.icon;
        self.icon = icon::tray_icon(self.config.enabled && self.pause.is_none());
        let mut nid = self.nid();
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = self.icon;
        fill_wide(&mut nid.szTip, &self.status_line());
        unsafe { Shell_NotifyIconW(action, &nid) };
        if !old.is_null() {
            unsafe { DestroyIcon(old) };
        }
    }

    fn nid(&self) -> NOTIFYICONDATAW {
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = 1;
        nid
    }

    fn status_line(&self) -> String {
        let managed = self.screens.iter().filter(|s| s.rule.enabled).count();
        match (self.config.enabled, self.pause) {
            (false, _) => "Pixl is off".into(),
            (true, Some(Pause::UntilRestart)) => "Pixl is paused".into(),
            (true, Some(Pause::Until(t))) => {
                let mins = t.saturating_sub(self.now()).div_ceil(60_000);
                format!("Pixl is paused for {mins} more min")
            }
            (true, None) if managed == 1 => "Pixl is looking after 1 screen".into(),
            (true, None) => format!("Pixl is looking after {managed} screens"),
        }
    }

    fn any_off(&self) -> bool {
        (0..self.engine.len()).any(|i| self.engine.phase(i) != Phase::On)
    }

    /// Uncover every screen before quitting.
    fn shutdown(&mut self) {
        overlay::hide_all();
        unsafe { Shell_NotifyIconW(NIM_DELETE, &self.nid()) };
        for id in [HOTKEY_OFF, HOTKEY_WAKE, HOTKEY_PAUSE] {
            unsafe { UnregisterHotKey(self.hwnd, id) };
        }
    }
}

fn register_hotkey(hwnd: HWND, id: i32, k: &Hotkey) -> bool {
    let mut mods = MOD_NOREPEAT;
    if k.ctrl {
        mods |= MOD_CONTROL;
    }
    if k.alt {
        mods |= MOD_ALT;
    }
    if k.shift {
        mods |= MOD_SHIFT;
    }
    if k.win {
        mods |= MOD_WIN;
    }
    unsafe { RegisterHotKey(hwnd, id, mods, k.key) != 0 }
}

/// Let the tray menu follow Windows' dark mode (uxtheme's unnamed exports).
fn allow_dark_menus() {
    unsafe {
        let lib = LoadLibraryW(to_wide("uxtheme.dll").as_ptr());
        if lib.is_null() {
            return;
        }
        // 135: SetPreferredAppMode(AllowDark), 136: FlushMenuThemes.
        if let Some(f) = GetProcAddress(lib, 135 as _) {
            let set_mode: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(f);
            set_mode(1);
        }
        if let Some(f) = GetProcAddress(lib, 136 as _) {
            let flush: unsafe extern "system" fn() = std::mem::transmute(f);
            flush();
        }
    }
}

fn add(menu: HMENU, flags: u32, id: usize, text: &str) {
    unsafe { AppendMenuW(menu, flags, id, to_wide(text).as_ptr()) };
}

fn show_menu(hwnd: HWND) {
    struct Item {
        label: String,
        enabled: bool,
    }
    let Some((status, enabled, paused, any_off, off_key, screens)) = with_app(|a| {
        let screens: Vec<Item> = a
            .screens
            .iter()
            .map(|s| Item {
                label: format!("{}. {}{}", s.d.number, s.d.name, if s.d.primary { " (main)" } else { "" }),
                enabled: s.rule.enabled,
            })
            .collect();
        let off_key = a.config.hotkeys.turn_off_all.map(|k| format!("\t{}", k.label())).unwrap_or_default();
        (a.status_line(), a.config.enabled, a.pause.is_some(), a.any_off(), off_key, screens)
    }) else {
        return;
    };
    unsafe {
        let menu = CreatePopupMenu();
        add(menu, MF_STRING | MF_GRAYED, 0, &status);
        add(menu, MF_SEPARATOR, 0, "");
        if enabled {
            add(menu, MF_STRING, CMD_OFF_NOW, &format!("Turn screens off now{off_key}"));
            if any_off {
                add(menu, MF_STRING, CMD_WAKE_ALL, "Wake all screens");
            }
            if paused {
                add(menu, MF_STRING, CMD_RESUME, "Resume");
            } else {
                let sub = CreatePopupMenu();
                add(sub, MF_STRING, CMD_PAUSE_30, "For 30 minutes");
                add(sub, MF_STRING, CMD_PAUSE_60, "For 1 hour");
                add(sub, MF_STRING, CMD_PAUSE_180, "For 3 hours");
                add(sub, MF_STRING, CMD_PAUSE_FOREVER, "Until I resume");
                add(menu, MF_POPUP, sub as usize, "Pause");
            }
            if !screens.is_empty() {
                let sub = CreatePopupMenu();
                for (i, s) in screens.iter().enumerate() {
                    let check = if s.enabled { MF_CHECKED } else { 0 };
                    add(sub, MF_STRING | check, CMD_SCREEN + i, &s.label);
                }
                add(menu, MF_POPUP, sub as usize, "Screens Pixl looks after");
            }
            add(menu, MF_SEPARATOR, 0, "");
        }
        add(menu, MF_STRING, CMD_OPEN, "Open Pixl");
        add(menu, MF_STRING, CMD_TOGGLE, if enabled { "Turn Pixl off" } else { "Turn Pixl on" });
        add(menu, MF_STRING, CMD_QUIT, "Quit");
        SetMenuDefaultItem(menu, CMD_OPEN as u32, 0);

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
        run_command(hwnd, cmd as usize);
    }
}

fn run_command(hwnd: HWND, cmd: usize) {
    let now = input::now();
    let pause_for = |mins: u64| Some(Pause::Until(now + mins * 60_000));
    match cmd {
        CMD_OPEN => open_settings(),
        CMD_TOGGLE => {
            with_app(|a| a.set_enabled(!a.config.enabled));
        }
        CMD_OFF_NOW => {
            with_app(App::turn_off_all);
        }
        CMD_WAKE_ALL => {
            with_app(App::wake_all);
        }
        CMD_RESUME => {
            with_app(|a| a.set_pause(None));
        }
        CMD_PAUSE_30 => {
            with_app(|a| a.set_pause(pause_for(30)));
        }
        CMD_PAUSE_60 => {
            with_app(|a| a.set_pause(pause_for(60)));
        }
        CMD_PAUSE_180 => {
            with_app(|a| a.set_pause(pause_for(180)));
        }
        CMD_PAUSE_FOREVER => {
            with_app(|a| a.set_pause(Some(Pause::UntilRestart)));
        }
        CMD_QUIT => unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        },
        c if c >= CMD_SCREEN => {
            with_app(|a| a.toggle_screen(c - CMD_SCREEN));
        }
        _ => {}
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_INPUT => {
            with_app(|a| a.input.key_pressed());
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_TIMER if wparam == TIMER_TICK => {
            with_app(App::tick);
            0
        }
        WM_TIMER if wparam == TIMER_FADE => {
            if !overlay::animate(input::now()) {
                unsafe { KillTimer(hwnd, TIMER_FADE) };
            }
            0
        }
        WM_TIMER if wparam == TIMER_REDETECT => {
            unsafe { KillTimer(hwnd, TIMER_REDETECT) };
            with_app(App::redetect);
            0
        }
        WM_TRAY => {
            match (lparam & 0xFFFF) as u32 {
                WM_LBUTTONUP => open_settings(),
                WM_RBUTTONUP => show_menu(hwnd),
                _ => {}
            }
            0
        }
        MSG_RELOAD => {
            with_app(App::reload);
            0
        }
        MSG_OPEN_SETTINGS => {
            open_settings();
            0
        }
        MSG_WATCH => {
            with_app(|a| {
                a.watch_until = a.now() + WATCH_MS;
                a.write_status();
            });
            0
        }
        MSG_PAUSE => {
            let now = input::now();
            let pause = match wparam {
                0 => None,
                PAUSE_UNTIL_RESTART => Some(Pause::UntilRestart),
                mins => Some(Pause::Until(now + mins as u64 * 60_000)),
            };
            with_app(|a| a.set_pause(pause));
            0
        }
        MSG_ALL => {
            if wparam == 1 {
                with_app(App::turn_off_all);
            } else {
                with_app(App::wake_all);
            }
            0
        }
        WM_HOTKEY => {
            match wparam as i32 {
                HOTKEY_OFF => {
                    with_app(App::turn_off_all);
                }
                HOTKEY_WAKE => {
                    with_app(App::wake_all);
                }
                HOTKEY_PAUSE => {
                    with_app(|a| {
                        let next = if a.pause.is_some() { None } else { Some(Pause::UntilRestart) };
                        a.set_pause(next);
                    });
                }
                _ => {}
            }
            0
        }
        WM_DISPLAYCHANGE => {
            // Right away, so no black cover lands on the wrong screen, then
            // again once the displays have settled.
            with_app(App::redetect);
            unsafe { SetTimer(hwnd, TIMER_REDETECT, 1000, None) };
            0
        }
        WM_POWERBROADCAST => {
            if wparam == PBT_APMSUSPEND as WPARAM {
                with_app(|a| a.locked = true);
            } else if wparam == PBT_APMRESUMEAUTOMATIC as WPARAM {
                with_app(|a| {
                    a.locked = false;
                    a.wake_all();
                });
                unsafe { SetTimer(hwnd, TIMER_REDETECT, 1500, None) };
            }
            1
        }
        WM_WTSSESSION_CHANGE => {
            match wparam as u32 {
                WTS_SESSION_LOCK => {
                    with_app(|a| a.locked = true);
                }
                WTS_SESSION_UNLOCK => {
                    with_app(|a| {
                        a.locked = false;
                        a.wake_all();
                    });
                }
                _ => {}
            }
            0
        }
        WM_SETTINGCHANGE => {
            // Taskbar switched between light and dark: redraw the icon to match.
            with_app(|a| a.tray(NIM_MODIFY));
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_ENDSESSION => {
            if wparam != 0 {
                with_app(App::shutdown);
            }
            0
        }
        WM_CLOSE => {
            with_app(App::shutdown);
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => {
            if with_app(|a| a.taskbar_created == msg).unwrap_or(false) {
                // Explorer restarted: our icon is gone, add it back.
                with_app(|a| a.tray(NIM_ADD));
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}
