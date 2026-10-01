//! The tray app's hidden window: watches the user a few times a second, runs
//! the idle engine, and turns screens off and on. Also the tray icon and menu,
//! shortcuts, messages from the settings window, and display, power and
//! session changes. Everything but DDC/CI runs on this one thread.

use std::cell::RefCell;
use std::collections::HashSet;
use std::ptr::{null, null_mut};

use pixl_core::{Action, Engine, Inputs, Phase, ScreenInputs};
use pixl_platform::config::{Config, FADE_MS, Hotkey, Method, ScreenRule, TriggerSetting};
use pixl_platform::ddc::{POWER_OFF, POWER_ON, Physical, VCP_POWER};
use pixl_platform::display::{self, Display};
use pixl_platform::status::{
    OffBy, PowerResult, PowerStore, PowerSupport, ScreenPhase, ScreenStatus, Status, TestStatus, TestStep, unix_ms,
};
use pixl_platform::tray::{
    MSG_ALL, MSG_OPEN_SETTINGS, MSG_PAUSE, MSG_RECHECK, MSG_RELOAD, MSG_TEST_ANSWER, MSG_TEST_CANCEL, MSG_TEST_START,
    MSG_WATCH, PAUSE_UNTIL_RESTART, WINDOW_CLASS, taskbar_created_message,
};
use pixl_platform::wide::{fill_wide, to_wide};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::RemoteDesktop::{NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_RESPECT_QUIET_TIME, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, Shell_NotifyIconW,
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
use crate::power::{Done, Job, MSG_POWER_DONE, Worker};
use crate::sampler::Sampler;
use crate::{icon, overlay, watch};

const WM_TRAY: u32 = WM_APP + 1;

const TIMER_TICK: usize = 1;
const TIMER_FADE: usize = 2;
const TIMER_REDETECT: usize = 3;
const TICK_MS: u32 = 250;
const FADE_FRAME_MS: u32 = 30;

const HOTKEY_OFF: i32 = 1;
const HOTKEY_WAKE: i32 = 2;
const HOTKEY_PAUSE: i32 = 3;

/// Look at an idle screen's picture this often.
const SAMPLE_EVERY_MS: u64 = 2000;
/// ...but only once it has been unused this long.
const SAMPLE_AFTER_IDLE_MS: u64 = 5000;
const APPS_EVERY_MS: u64 = 5000;
/// Keep status.json fresh this long after the settings window last asked.
const WATCH_MS: u64 = 5000;
/// "Test power off": how long the screen stays off, then how long it gets to come back.
const TEST_OFF_MS: u64 = 5000;
const TEST_WAKE_MS: u64 = 3500;
/// After waking an untested screen, check it's really on after this long.
const VERIFY_AFTER_MS: u64 = 3500;

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

#[derive(Clone, Copy)]
enum Verify {
    /// Woke an untested screen; check it's on at this time.
    AfterOn { at: u64, retried: bool },
    /// Asked it to read back its power state; waiting for the answer.
    Reading { retried: bool },
}

struct Screen {
    d: Display,
    rule: ScreenRule,
    support: PowerSupport,
    note: String,
    off_by: Option<OffBy>,
    verify: Option<Verify>,
    last_change: u64,
    next_sample: u64,
    /// The power-mode value that means "off" for this monitor (04 or 05).
    off_value: u32,
    /// Why it's being kept on right now (fullscreen game, paused...).
    held_by: Option<String>,
}

struct Test {
    id: String,
    step: TestStep,
    at: u64,
    stayed_connected: bool,
    reports_on: bool,
}

struct App {
    hwnd: HWND,
    config: Config,
    store: PowerStore,
    screens: Vec<Screen>,
    engine: Engine,
    input: Input,
    sampler: Sampler,
    worker: Worker,
    pause: Option<Pause>,
    locked: bool,
    watch_until: u64,
    running: HashSet<String>,
    next_apps_scan: u64,
    hotkeys_taken: Vec<String>,
    test: Option<Test>,
    ticks: u64,
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
    unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) };

    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            hwnd,
            config: Config::load(),
            store: PowerStore::load(),
            screens: Vec::new(),
            engine: Engine::new(),
            input: Input::new(),
            sampler: Sampler::default(),
            worker: Worker::start(hwnd as usize),
            pause: None,
            locked: false,
            watch_until: 0,
            running: HashSet::new(),
            next_apps_scan: 0,
            hotkeys_taken: Vec::new(),
            test: None,
            ticks: 0,
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
    unsafe { SetTimer(hwnd, TIMER_TICK, TICK_MS, None) };
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

        // A screen we powered off that Windows no longer sees: it dropped off
        // the cable when it went to sleep, so DDC/CI can't wake it.
        for s in old.iter().filter(|s| s.off_by == Some(OffBy::Power) && !found.iter().any(|d| d.id == s.d.id)) {
            let note = "Windows disconnects this screen while it's powered off, which moves your open windows. \
                        Pixl covers it with black instead.";
            remember(&mut self.store, &s.d.id, false, note);
            self.notify(
                "Press the screen's power button",
                &format!(
                    "Screen {} disconnected when it turned off, so Pixl can't turn it back on. From now on Pixl will \
                     cover it with black instead.",
                    s.d.number
                ),
            );
        }

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
                None => {
                    let mut s = Screen {
                        d,
                        rule,
                        support: PowerSupport::Checking,
                        note: String::new(),
                        off_by: None,
                        verify: None,
                        last_change: 0,
                        next_sample: 0,
                        off_value: POWER_OFF,
                        held_by: None,
                    };
                    learn_support(&self.store, &self.worker, &mut s);
                    s
                }
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
        let _ = self.store.save();
        self.sync_engine();
        self.tray(NIM_MODIFY);
    }

    fn sync_engine(&mut self) {
        let now = self.now();
        let list: Vec<_> = self.screens.iter().map(|s| (s.d.id.clone(), s.rule.to_rule())).collect();
        let actions = self.engine.set_screens(&list, now);
        self.apply(actions);
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.screens.iter().position(|s| s.d.id == id)
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
        self.step_test(now);
        self.step_verify(now);

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

        if self.ticks.is_multiple_of(8) {
            overlay::raise();
        }
        if now < self.watch_until && self.ticks.is_multiple_of(2) {
            self.write_status();
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
        let testing = self.test.as_ref().map(|t| t.id.clone());

        let mut out = Vec::with_capacity(self.screens.len());
        for (i, s) in self.screens.iter_mut().enumerate() {
            let held_by = if !c.enabled {
                Some("Pixl is turned off".to_string())
            } else if self.pause.is_some() {
                Some("Paused".into())
            } else if testing.as_deref() == Some(s.d.id.as_str()) {
                Some("Testing power off".into())
            } else if fullscreen == Some(s.d.hmonitor) {
                Some("A fullscreen app is open".into())
            } else if let Some(app) = &app {
                Some(format!("{app} is running"))
            } else if awake && s.rule.trigger == TriggerSetting::Pc {
                Some("An app is keeping the display on".into())
            } else {
                None
            };

            // Only look at the picture of a screen that's on and idle.
            let idle_for = self.engine.remaining(i, now).map(|left| s.rule.to_rule().timeout_ms.saturating_sub(left));
            if s.rule.enabled
                && s.rule.stay_on_while_playing
                && held_by.is_none()
                && idle_for.is_some_and(|t| t >= SAMPLE_AFTER_IDLE_MS)
                && now >= s.next_sample
            {
                s.next_sample = now + SAMPLE_EVERY_MS;
                if self.sampler.changed(&s.d.id, s.d.px) {
                    s.last_change = now;
                }
            } else if idle_for.is_none_or(|t| t < SAMPLE_AFTER_IDLE_MS) {
                self.sampler.forget(&s.d.id);
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
                Action::TurnOff(i) => self.turn_off(i),
                Action::Wake(i) => self.wake(i),
            }
        }
        if fading {
            unsafe { SetTimer(self.hwnd, TIMER_FADE, FADE_FRAME_MS, None) };
        }
        if self.now() < self.watch_until {
            self.write_status();
        }
    }

    fn turn_off(&mut self, i: usize) {
        let now = self.now();
        let Some(s) = self.screens.get_mut(i) else { return };
        self.sampler.forget(&s.d.id);
        // Most monitors drop off the cable when powered down and then only their
        // button wakes them, so Automatic powers off only screens that passed the
        // test. "Power off only" is the user opting in, so it tries untested ones.
        let can_power = match s.rule.method {
            Method::Power => matches!(s.support, PowerSupport::Works | PowerSupport::Untested),
            _ => s.support == PowerSupport::Works,
        };
        match (s.rule.method, can_power) {
            // A screen that can't be powered off is always covered, so it's never left lit.
            (Method::Black, _) | (_, false) => {
                overlay::show(&s.d.id, s.d.px, now, 0);
                s.off_by = Some(OffBy::Black);
            }
            (method, true) => {
                // The black cover stays up under a powered-off screen, so it
                // stays dark even if the monitor wakes itself up.
                if method == Method::Power {
                    overlay::hide(&s.d.id);
                } else {
                    overlay::show(&s.d.id, s.d.px, now, 0);
                }
                s.off_by = Some(OffBy::Power);
                s.verify = None;
                self.worker.send(Job::Set { id: s.d.id.clone(), hmonitor: s.d.hmonitor, value: s.off_value });
            }
        }
    }

    fn wake(&mut self, i: usize) {
        let now = self.now();
        let Some(s) = self.screens.get_mut(i) else { return };
        overlay::hide(&s.d.id);
        if s.off_by == Some(OffBy::Power) {
            self.worker.send(Job::Set { id: s.d.id.clone(), hmonitor: s.d.hmonitor, value: POWER_ON });
            if s.support == PowerSupport::Untested {
                s.verify = Some(Verify::AfterOn { at: now + VERIFY_AFTER_MS, retried: false });
            }
        }
        s.off_by = None;
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

    // ---- learning whether power off works ----

    /// Untested screens that were woken: make sure they really came back.
    fn step_verify(&mut self, now: u64) {
        for s in &mut self.screens {
            if let Some(Verify::AfterOn { at, retried }) = s.verify
                && now >= at
            {
                s.verify = Some(Verify::Reading { retried });
                self.worker.send(Job::Read { id: s.d.id.clone(), hmonitor: s.d.hmonitor });
            }
        }
    }

    fn power_done(&mut self) {
        let now = self.now();
        for done in self.worker.finished() {
            match done {
                Done::Probed { id, supported, note, off_value } => {
                    let Some(i) = self.index_of(&id) else { continue };
                    let s = &mut self.screens[i];
                    s.off_value = off_value;
                    // A remembered test result wins over what the monitor claims.
                    if s.support == PowerSupport::Checking {
                        s.support = if supported { PowerSupport::Untested } else { PowerSupport::Unsupported };
                        s.note = note.to_string();
                    }
                }
                Done::Set { id, on: false, ok: false } => {
                    if let Some(t) = &mut self.test
                        && t.id == id
                    {
                        t.step = TestStep::Refused;
                    }
                    let note = "This screen didn't accept the power-off command, so Pixl covers it with black.";
                    remember(&mut self.store, &id, false, note);
                    let _ = self.store.save();
                    if let Some(i) = self.index_of(&id) {
                        let s = &mut self.screens[i];
                        s.support = PowerSupport::Failed;
                        s.note = note.into();
                        if s.off_by == Some(OffBy::Power) && s.rule.method != Method::Power {
                            overlay::show(&s.d.id, s.d.px, now, 0);
                            s.off_by = Some(OffBy::Black);
                        }
                    }
                }
                Done::Set { .. } => {}
                Done::Read { id, on } => {
                    if let Some(t) = &mut self.test
                        && t.id == id
                        && t.step == TestStep::Waking
                    {
                        t.reports_on = on == Some(true);
                        t.step = TestStep::Ask;
                    }
                    let Some(i) = self.index_of(&id) else { continue };
                    let Some(Verify::Reading { retried }) = self.screens[i].verify else { continue };
                    let s = &mut self.screens[i];
                    if on == Some(true) {
                        s.verify = None;
                        s.support = PowerSupport::Works;
                        s.note = "Turns off and comes back on by itself.".into();
                        remember(&mut self.store, &id, true, &s.note);
                        let _ = self.store.save();
                    } else if !retried {
                        self.worker.send(Job::Set { id: id.clone(), hmonitor: s.d.hmonitor, value: POWER_ON });
                        s.verify = Some(Verify::AfterOn { at: now + VERIFY_AFTER_MS, retried: true });
                    } else {
                        s.verify = None;
                        s.support = PowerSupport::Failed;
                        s.note =
                            "This screen didn't come back on by itself, so Pixl covers it with black instead.".into();
                        remember(&mut self.store, &id, false, &s.note);
                        let _ = self.store.save();
                        let n = s.d.number;
                        self.notify(
                            "Press the screen's power button",
                            &format!(
                                "Screen {n} didn't turn back on by itself. From now on Pixl will cover it with black \
                                 instead."
                            ),
                        );
                    }
                }
            }
        }
        if self.now() < self.watch_until {
            self.write_status();
        }
    }

    fn start_test(&mut self, i: usize) {
        if self.test.is_some() {
            return;
        }
        let now = self.now();
        let Some(s) = self.screens.get(i) else { return };
        if s.d.internal {
            return;
        }
        let id = s.d.id.clone();
        if let Some(a) = self.engine.wake(i, now) {
            self.apply(vec![a]);
        }
        let s = &self.screens[i];
        overlay::hide(&id);
        self.worker.send(Job::Set { id: id.clone(), hmonitor: s.d.hmonitor, value: s.off_value });
        self.test =
            Some(Test { id, step: TestStep::Off, at: now + TEST_OFF_MS, stayed_connected: true, reports_on: false });
        self.write_status();
    }

    fn step_test(&mut self, now: u64) {
        let Some(t) = &mut self.test else { return };
        if now < t.at {
            return;
        }
        match t.step {
            TestStep::Off => {
                // Does Windows still see it while it's off?
                let found = display::detect().into_iter().find(|d| d.id == t.id);
                t.stayed_connected = found.is_some();
                if let Some(d) = found {
                    self.worker.send(Job::Set { id: t.id.clone(), hmonitor: d.hmonitor, value: POWER_ON });
                    t.step = TestStep::Waking;
                    t.at = now + TEST_WAKE_MS;
                } else {
                    t.step = TestStep::Ask;
                }
            }
            TestStep::Waking => {
                if let Some(d) = self.screens.iter().find(|s| s.d.id == t.id) {
                    self.worker.send(Job::Read { id: t.id.clone(), hmonitor: d.d.hmonitor });
                }
                // The read answers through power_done; don't send it twice.
                t.at = u64::MAX;
            }
            TestStep::Ask | TestStep::Refused => {}
        }
        self.write_status();
    }

    fn answer_test(&mut self, came_back: bool) {
        let Some(t) = self.test.take() else { return };
        let (works, note) = if t.step == TestStep::Refused {
            (false, "This screen didn't accept the power-off command, so Pixl covers it with black.")
        } else if !came_back {
            (false, "This screen didn't come back on by itself, so Pixl covers it with black instead.")
        } else if !t.stayed_connected {
            (
                false,
                "Windows disconnects this screen while it's powered off, which moves your open windows. Pixl covers \
                 it with black instead.",
            )
        } else {
            (true, "Tested: it turns off and comes back on by itself.")
        };
        remember(&mut self.store, &t.id, works, note);
        let _ = self.store.save();
        if let Some(i) = self.index_of(&t.id) {
            let s = &mut self.screens[i];
            s.support = if works { PowerSupport::Works } else { PowerSupport::Failed };
            s.note = note.into();
            s.verify = None;
            let now = self.now();
            if let Some(a) = self.engine.wake(i, now) {
                self.apply(vec![a]);
            }
        }
        self.write_status();
    }

    fn cancel_test(&mut self) {
        let Some(t) = self.test.take() else { return };
        if let Some(s) = self.screens.iter().find(|s| s.d.id == t.id) {
            self.worker.send(Job::Set { id: t.id.clone(), hmonitor: s.d.hmonitor, value: POWER_ON });
        }
        self.write_status();
    }

    /// Forget what we learned and ask the monitor(s) again.
    fn recheck(&mut self, which: usize) {
        let ids: Vec<String> = match self.screens.get(which) {
            Some(s) => vec![s.d.id.clone()],
            None => self.screens.iter().map(|s| s.d.id.clone()).collect(),
        };
        for id in ids {
            self.store.monitors.remove(&id);
            if let Some(i) = self.index_of(&id) {
                learn_support(&self.store, &self.worker, &mut self.screens[i]);
            }
        }
        let _ = self.store.save();
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
                        internal: s.d.internal,
                        power: s.support,
                        power_note: s.note.clone(),
                        phase,
                        off_by: s.off_by,
                        remaining_secs: if held_by.is_some() {
                            None
                        } else {
                            self.engine.remaining(i, now).map(|ms| ms.div_ceil(1000) as u32)
                        },
                        held_by,
                    }
                })
                .collect(),
            test: self.test.as_ref().map(|t| TestStatus {
                id: t.id.clone(),
                step: t.step,
                stayed_connected: t.stayed_connected,
                reports_on: t.reports_on,
            }),
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

    fn notify(&self, title: &str, text: &str) {
        let mut nid = self.nid();
        nid.uFlags = NIF_INFO;
        nid.dwInfoFlags = NIIF_WARNING | NIIF_RESPECT_QUIET_TIME;
        fill_wide(&mut nid.szInfoTitle, title);
        fill_wide(&mut nid.szInfo, text);
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
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

    /// Put every powered-off screen back on before quitting.
    fn shutdown(&mut self) {
        overlay::hide_all();
        for s in &self.screens {
            if s.off_by == Some(OffBy::Power)
                && let Some(m) = Physical::open(s.d.hmonitor)
            {
                m.set(VCP_POWER, POWER_ON);
            }
        }
        unsafe { Shell_NotifyIconW(NIM_DELETE, &self.nid()) };
        for id in [HOTKEY_OFF, HOTKEY_WAKE, HOTKEY_PAUSE] {
            unsafe { UnregisterHotKey(self.hwnd, id) };
        }
    }
}

/// Set a screen's power support from what we remember, or ask the monitor.
fn learn_support(store: &PowerStore, worker: &Worker, s: &mut Screen) {
    if s.d.internal {
        s.support = PowerSupport::Unsupported;
        s.note = "Built-in screens can't be powered off by Pixl, so they're covered with black.".into();
        return;
    }
    if let Some(r) = store.monitors.get(&s.d.id) {
        s.support = if r.works { PowerSupport::Works } else { PowerSupport::Failed };
        s.note = r.note.clone();
    } else {
        s.support = PowerSupport::Checking;
        s.note.clear();
    }
    // Always asked: the answer also says which value means "off" for this monitor.
    worker.send(Job::Probe { id: s.d.id.clone(), hmonitor: s.d.hmonitor });
}

fn remember(store: &mut PowerStore, id: &str, works: bool, note: &str) {
    let at = unix_ms() / 1000;
    store.monitors.insert(id.to_string(), PowerResult { works, note: note.to_string(), at });
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
        MSG_POWER_DONE => {
            with_app(App::power_done);
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
        MSG_TEST_START => {
            with_app(|a| a.start_test(wparam));
            0
        }
        MSG_TEST_ANSWER => {
            with_app(|a| a.answer_test(lparam == 1));
            0
        }
        MSG_TEST_CANCEL => {
            with_app(App::cancel_test);
            0
        }
        MSG_RECHECK => {
            with_app(|a| a.recheck(wparam));
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
