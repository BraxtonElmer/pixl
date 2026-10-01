//! The per-screen state machine: On → Fading → Off → On.
//!
//! The tray app calls [`Engine::step`] a few times a second with a snapshot of
//! the input and gets back what to do with each screen. Times are milliseconds
//! on one monotonic clock (Windows' tick count).

use crate::rule::{Rule, Trigger, Wake};

/// After a screen is turned off on purpose (shortcut, menu), input this soon
/// doesn't wake it, so releasing the shortcut's keys doesn't undo it.
const FORCED_OFF_GRACE_MS: u64 = 1500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    On,
    /// Dimming towards black; any use cancels it.
    Fading {
        since: u64,
    },
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Start fading screen `i` out over its rule's `fade_ms`.
    Fade(usize),
    /// The user came back during the fade: undo it.
    CancelFade(usize),
    TurnOff(usize),
    Wake(usize),
}

/// What happened on one screen since the last step.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScreenInputs {
    /// When its picture last changed noticeably (video, game, scrolling).
    pub last_change: u64,
    /// Something needs it on right now: a fullscreen game, an app on the
    /// keep-on list, Pixl paused.
    pub held: bool,
}

/// A snapshot of the user's input, taken right before a step.
#[derive(Clone, Copy, Debug)]
pub struct Inputs<'a> {
    pub now: u64,
    /// Last mouse input anywhere (movement, click, wheel).
    pub last_mouse: u64,
    /// Last key press anywhere.
    pub last_key: u64,
    /// Screen under the cursor.
    pub cursor_on: Option<usize>,
    /// Screen showing the window that has keyboard focus.
    pub focus_on: Option<usize>,
    /// One entry per screen, in the order given to [`Engine::set_screens`].
    pub screens: &'a [ScreenInputs],
}

#[derive(Clone, Debug)]
struct Slot {
    id: String,
    rule: Rule,
    phase: Phase,
    /// Floor for "last used": set when the screen wakes or its rule changes,
    /// so it gets a full timeout before it can turn off again.
    floor: u64,
    /// Last mouse input while the cursor was on this screen.
    mouse_here: u64,
    /// Last key press while the focused window was on this screen.
    key_here: u64,
    /// When it was last in use, by its rule (updated every step).
    last_used: u64,
    /// Input before this time doesn't wake it.
    wake_after: u64,
}

impl Slot {
    fn new(id: String, rule: Rule, now: u64) -> Self {
        Self { id, rule, phase: Phase::On, floor: now, mouse_here: 0, key_here: 0, last_used: now, wake_after: now }
    }

    fn used(&self, inp: &Inputs, screen: &ScreenInputs) -> u64 {
        let r = &self.rule;
        let (mouse, key) = match r.trigger {
            Trigger::PcIdle => (inp.last_mouse, inp.last_key),
            Trigger::AwayFromScreen => (self.mouse_here, self.key_here),
        };
        let mut t = self.floor.max(mouse);
        if r.typing_counts {
            t = t.max(key);
        }
        if r.stay_on_while_playing {
            t = t.max(screen.last_change);
        }
        if screen.held {
            t = inp.now;
        }
        t
    }

    fn should_wake(&self, inp: &Inputs, screen: &ScreenInputs) -> bool {
        if screen.held {
            return true;
        }
        let after = self.wake_after;
        match self.rule.wake {
            Wake::AnyInput => inp.last_mouse > after || inp.last_key > after,
            Wake::CursorEnters => self.mouse_here > after,
        }
    }
}

/// How often the tray app needs to look, in milliseconds (see [`Engine::next_wait`]).
#[derive(Clone, Copy, Debug)]
pub struct Pace {
    /// While something is about to happen: fading, or the last moments of a timer.
    pub fast: u64,
    /// While a screen is dark (input wakes it as an event; this only catches
    /// things like a fullscreen game starting).
    pub dark: u64,
    /// While a screen uses "not using this screen", to follow the cursor.
    pub away: u64,
    /// Never sleep longer than this (a fullscreen game ending, a keep-on app quitting).
    pub max: u64,
    /// Look this long before a timer runs out, to check for a playing video.
    pub probe_lead: u64,
}

#[derive(Debug, Default)]
pub struct Engine {
    slots: Vec<Slot>,
    seen_mouse: u64,
    seen_key: u64,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the connected screens and their rules. Screens already known (by
    /// id) keep their state; a changed rule restarts that screen's timer.
    /// Returns wakes for screens that are dimmed or off but got disabled.
    pub fn set_screens(&mut self, screens: &[(String, Rule)], now: u64) -> Vec<Action> {
        let mut old = std::mem::take(&mut self.slots);
        let mut actions = Vec::new();
        for (i, (id, rule)) in screens.iter().enumerate() {
            let slot = match old.iter().position(|s| &s.id == id) {
                Some(p) => {
                    let mut s = old.swap_remove(p);
                    if s.rule != *rule {
                        s.rule = *rule;
                        s.floor = now;
                        if !rule.enabled && s.phase != Phase::On {
                            actions.push(wake_action(s.phase, i));
                            s.phase = Phase::On;
                        }
                    }
                    s
                }
                None => Slot::new(id.clone(), *rule, now),
            };
            self.slots.push(slot);
        }
        actions
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn phase(&self, i: usize) -> Phase {
        self.slots.get(i).map_or(Phase::On, |s| s.phase)
    }

    pub fn id(&self, i: usize) -> Option<&str> {
        self.slots.get(i).map(|s| s.id.as_str())
    }

    /// Milliseconds until screen `i` starts turning off, if it's on and enabled.
    pub fn remaining(&self, i: usize, now: u64) -> Option<u64> {
        let s = self.slots.get(i)?;
        (s.rule.enabled && s.phase == Phase::On)
            .then(|| s.rule.timeout_ms.saturating_sub(now.saturating_sub(s.last_used)))
    }

    /// How long the tray app can sleep before the next step is worth doing.
    ///
    /// Nothing can turn a screen off before its timer runs out, and input
    /// only ever pushes that further away, so with every screen on Pixl can
    /// sleep until the earliest timer (less `probe_lead`, to look for a
    /// playing video first). Input that wakes a dark screen arrives as an
    /// event, so dark screens only need the occasional check.
    pub fn next_wait(&self, now: u64, pace: &Pace) -> u64 {
        let mut wait = pace.max;
        for s in self.slots.iter().filter(|s| s.rule.enabled) {
            let w = match s.phase {
                // The fade has to end on time and be easy to cancel.
                Phase::Fading { .. } => pace.fast,
                Phase::Off => pace.dark,
                Phase::On => {
                    let left = s.rule.timeout_ms.saturating_sub(now.saturating_sub(s.last_used));
                    let until_probe = left.saturating_sub(pace.probe_lead);
                    let mut w = if until_probe == 0 { pace.fast } else { until_probe };
                    // "Not using this screen" has to notice which screen the cursor is on.
                    if s.rule.trigger == Trigger::AwayFromScreen {
                        w = w.min(pace.away);
                    }
                    w
                }
            };
            wait = wait.min(w);
        }
        wait.clamp(pace.fast, pace.max)
    }

    /// Advance every screen by one tick.
    pub fn step(&mut self, inp: &Inputs) -> Vec<Action> {
        let new_mouse = inp.last_mouse > self.seen_mouse;
        let new_key = inp.last_key > self.seen_key;
        self.seen_mouse = self.seen_mouse.max(inp.last_mouse);
        self.seen_key = self.seen_key.max(inp.last_key);

        let mut actions = Vec::new();
        let none = ScreenInputs::default();
        for (i, s) in self.slots.iter_mut().enumerate() {
            if new_mouse && inp.cursor_on == Some(i) {
                s.mouse_here = inp.last_mouse;
            }
            if new_key && inp.focus_on == Some(i) {
                s.key_here = inp.last_key;
            }
            let screen = inp.screens.get(i).unwrap_or(&none);
            if !s.rule.enabled {
                s.last_used = inp.now;
                continue;
            }

            match s.phase {
                Phase::On => {
                    s.last_used = s.used(inp, screen);
                    if inp.now.saturating_sub(s.last_used) >= s.rule.timeout_ms {
                        if s.rule.fade_ms > 0 {
                            s.phase = Phase::Fading { since: inp.now };
                            actions.push(Action::Fade(i));
                        } else {
                            s.phase = Phase::Off;
                            s.wake_after = inp.now;
                            actions.push(Action::TurnOff(i));
                        }
                    }
                }
                Phase::Fading { since } => {
                    s.last_used = s.used(inp, screen);
                    if s.last_used > since {
                        s.phase = Phase::On;
                        actions.push(Action::CancelFade(i));
                    } else if inp.now.saturating_sub(since) >= s.rule.fade_ms {
                        s.phase = Phase::Off;
                        s.wake_after = inp.now;
                        actions.push(Action::TurnOff(i));
                    }
                }
                Phase::Off => {
                    if s.should_wake(inp, screen) {
                        s.phase = Phase::On;
                        s.floor = inp.now;
                        s.last_used = inp.now;
                        actions.push(Action::Wake(i));
                    }
                }
            }
        }
        actions
    }

    /// Bring every dimmed or dark screen back and restart all timers.
    pub fn wake_all(&mut self, now: u64) -> Vec<Action> {
        let mut actions = Vec::new();
        for (i, s) in self.slots.iter_mut().enumerate() {
            if s.phase != Phase::On {
                actions.push(wake_action(s.phase, i));
                s.phase = Phase::On;
            }
            s.floor = now;
            s.last_used = now;
        }
        actions
    }

    /// Turn every enabled screen off right now (the shortcut or tray menu).
    pub fn turn_off_all(&mut self, now: u64) -> Vec<Action> {
        let mut actions = Vec::new();
        for (i, s) in self.slots.iter_mut().enumerate() {
            if s.rule.enabled && s.phase != Phase::Off {
                s.phase = Phase::Off;
                s.wake_after = now + FORCED_OFF_GRACE_MS;
                actions.push(Action::TurnOff(i));
            }
        }
        actions
    }

    /// Turn one screen off right now, whatever its rule (used by the test).
    pub fn turn_off(&mut self, i: usize, now: u64) -> Option<Action> {
        let s = self.slots.get_mut(i)?;
        if s.phase == Phase::Off {
            return None;
        }
        s.phase = Phase::Off;
        s.wake_after = now + FORCED_OFF_GRACE_MS;
        Some(Action::TurnOff(i))
    }

    /// Wake one screen and restart its timer.
    pub fn wake(&mut self, i: usize, now: u64) -> Option<Action> {
        let s = self.slots.get_mut(i)?;
        s.floor = now;
        s.last_used = now;
        if s.phase == Phase::On {
            return None;
        }
        let a = wake_action(s.phase, i);
        s.phase = Phase::On;
        Some(a)
    }
}

fn wake_action(phase: Phase, i: usize) -> Action {
    match phase {
        Phase::Fading { .. } => Action::CancelFade(i),
        _ => Action::Wake(i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    fn rule(trigger: Trigger, wake: Wake) -> Rule {
        Rule { trigger, wake, timeout_ms: MIN, fade_ms: 0, ..Rule::default() }
    }

    struct Sim {
        e: Engine,
        now: u64,
        mouse: u64,
        key: u64,
        cursor: Option<usize>,
        focus: Option<usize>,
        screens: Vec<ScreenInputs>,
    }

    impl Sim {
        fn new(rules: &[Rule]) -> Self {
            let mut e = Engine::new();
            let list: Vec<_> = rules.iter().enumerate().map(|(i, r)| (format!("S{i}"), *r)).collect();
            e.set_screens(&list, 1000);
            Self {
                e,
                now: 1000,
                mouse: 0,
                key: 0,
                cursor: Some(0),
                focus: Some(0),
                screens: vec![ScreenInputs::default(); rules.len()],
            }
        }
        fn step(&mut self) -> Vec<Action> {
            let inp = Inputs {
                now: self.now,
                last_mouse: self.mouse,
                last_key: self.key,
                cursor_on: self.cursor,
                focus_on: self.focus,
                screens: &self.screens,
            };
            self.e.step(&inp)
        }
        /// Advance in 250 ms ticks, collecting every action.
        fn run(&mut self, ms: u64) -> Vec<Action> {
            let mut all = Vec::new();
            let end = self.now + ms;
            while self.now < end {
                self.now += 250;
                all.extend(self.step());
            }
            all
        }
        fn move_mouse(&mut self, on: usize) -> Vec<Action> {
            self.cursor = Some(on);
            self.now += 10;
            self.mouse = self.now;
            self.step()
        }
        fn type_key(&mut self) -> Vec<Action> {
            self.now += 10;
            self.key = self.now;
            self.step()
        }
    }

    #[test]
    fn turns_off_after_the_timeout_and_wakes_on_input() {
        let mut s = Sim::new(&[rule(Trigger::PcIdle, Wake::AnyInput)]);
        assert!(s.run(MIN - 500).is_empty());
        assert_eq!(s.run(1000), vec![Action::TurnOff(0)]);
        assert_eq!(s.e.phase(0), Phase::Off);
        assert_eq!(s.move_mouse(0), vec![Action::Wake(0)]);
        // A full timeout again before it can turn off.
        assert!(s.run(MIN - 500).is_empty());
    }

    #[test]
    fn input_restarts_the_timer() {
        let mut s = Sim::new(&[rule(Trigger::PcIdle, Wake::AnyInput)]);
        s.run(MIN - 5000);
        s.move_mouse(0);
        assert!(s.run(MIN - 5000).is_empty());
        s.type_key();
        assert!(s.run(MIN - 5000).is_empty());
        assert_eq!(s.run(10_000), vec![Action::TurnOff(0)]);
    }

    #[test]
    fn typing_can_be_ignored() {
        let r = Rule { typing_counts: false, ..rule(Trigger::PcIdle, Wake::CursorEnters) };
        let mut s = Sim::new(&[r]);
        s.run(MIN - 5000);
        s.type_key();
        assert_eq!(s.run(10_000), vec![Action::TurnOff(0)]);
        // Typing doesn't wake it either; moving onto it does.
        assert!(s.type_key().is_empty());
        assert_eq!(s.move_mouse(0), vec![Action::Wake(0)]);
    }

    #[test]
    fn pc_idle_screen_stays_on_while_working_elsewhere() {
        let mut s = Sim::new(&[rule(Trigger::PcIdle, Wake::AnyInput), rule(Trigger::PcIdle, Wake::AnyInput)]);
        for _ in 0..5 {
            s.run(MIN / 2);
            s.move_mouse(0);
        }
        assert_eq!(s.e.phase(1), Phase::On);
    }

    #[test]
    fn away_screen_turns_off_while_working_elsewhere() {
        let mut s =
            Sim::new(&[rule(Trigger::PcIdle, Wake::AnyInput), rule(Trigger::AwayFromScreen, Wake::CursorEnters)]);
        let mut actions = Vec::new();
        for _ in 0..5 {
            actions.extend(s.run(MIN / 2));
            actions.extend(s.move_mouse(0));
            actions.extend(s.type_key());
        }
        assert_eq!(actions, vec![Action::TurnOff(1)]);
        assert_eq!(s.e.phase(0), Phase::On);
        // Moving on screen 0 doesn't wake it; moving onto it does.
        assert!(s.move_mouse(0).is_empty());
        assert_eq!(s.move_mouse(1), vec![Action::Wake(1)]);
    }

    #[test]
    fn away_screen_counts_typing_into_its_windows() {
        let mut s =
            Sim::new(&[rule(Trigger::PcIdle, Wake::AnyInput), rule(Trigger::AwayFromScreen, Wake::CursorEnters)]);
        s.cursor = Some(0);
        s.focus = Some(1);
        for _ in 0..5 {
            s.run(MIN / 2);
            s.type_key();
        }
        assert_eq!(s.e.phase(1), Phase::On);
    }

    #[test]
    fn fade_runs_then_turns_off() {
        let r = Rule { fade_ms: 5000, ..rule(Trigger::PcIdle, Wake::AnyInput) };
        let mut s = Sim::new(&[r]);
        assert_eq!(s.run(MIN + 100), vec![Action::Fade(0)]);
        assert!(matches!(s.e.phase(0), Phase::Fading { .. }));
        assert_eq!(s.run(5000), vec![Action::TurnOff(0)]);
    }

    #[test]
    fn input_during_the_fade_cancels_it() {
        let r = Rule { fade_ms: 5000, ..rule(Trigger::PcIdle, Wake::CursorEnters) };
        let mut s = Sim::new(&[r]);
        s.run(MIN + 100);
        s.run(2000);
        assert_eq!(s.type_key(), vec![Action::CancelFade(0)]);
        assert_eq!(s.e.phase(0), Phase::On);
        assert!(s.run(MIN - 1000).is_empty());
    }

    #[test]
    fn a_changing_picture_keeps_it_on() {
        let mut s = Sim::new(&[rule(Trigger::PcIdle, Wake::AnyInput)]);
        for _ in 0..4 {
            s.run(MIN / 2);
            s.screens[0].last_change = s.now;
        }
        assert_eq!(s.e.phase(0), Phase::On);
        s.screens[0].last_change = 0;
        let r = Rule { stay_on_while_playing: false, ..rule(Trigger::PcIdle, Wake::AnyInput) };
        s.e.set_screens(&[("S0".into(), r)], s.now);
        s.screens[0].last_change = s.now;
        assert_eq!(s.run(MIN + 500), vec![Action::TurnOff(0)]);
    }

    #[test]
    fn held_screens_stay_on_and_wake() {
        let mut s = Sim::new(&[rule(Trigger::PcIdle, Wake::CursorEnters)]);
        s.screens[0].held = true;
        assert!(s.run(3 * MIN).is_empty());
        s.screens[0].held = false;
        assert_eq!(s.run(MIN + 500), vec![Action::TurnOff(0)]);
        s.screens[0].held = true;
        assert_eq!(s.step(), vec![Action::Wake(0)]);
    }

    #[test]
    fn disabled_screens_are_left_alone() {
        let mut s = Sim::new(&[Rule { enabled: false, ..rule(Trigger::PcIdle, Wake::AnyInput) }]);
        assert!(s.run(10 * MIN).is_empty());
        assert_eq!(s.e.remaining(0, s.now), None);
        assert!(s.e.turn_off_all(s.now).is_empty());
    }

    #[test]
    fn disabling_a_dark_screen_wakes_it() {
        let r = rule(Trigger::PcIdle, Wake::AnyInput);
        let mut s = Sim::new(&[r]);
        s.run(MIN + 500);
        let off = Rule { enabled: false, ..r };
        assert_eq!(s.e.set_screens(&[("S0".into(), off)], s.now), vec![Action::Wake(0)]);
    }

    #[test]
    fn turning_off_by_shortcut_ignores_the_shortcut_keys() {
        let mut s = Sim::new(&[rule(Trigger::PcIdle, Wake::AnyInput)]);
        assert_eq!(s.e.turn_off_all(s.now), vec![Action::TurnOff(0)]);
        assert!(s.type_key().is_empty());
        s.run(2000);
        assert_eq!(s.type_key(), vec![Action::Wake(0)]);
    }

    #[test]
    fn known_screens_keep_their_state_when_the_list_changes() {
        let r = rule(Trigger::PcIdle, Wake::CursorEnters);
        let mut s = Sim::new(&[r, r]);
        s.run(MIN + 500);
        assert_eq!(s.e.phase(1), Phase::Off);
        s.e.set_screens(&[("S1".into(), r)], s.now);
        assert_eq!(s.e.phase(0), Phase::Off);
        assert_eq!(s.e.id(0), Some("S1"));
    }

    #[test]
    fn remaining_counts_down() {
        let mut s = Sim::new(&[rule(Trigger::PcIdle, Wake::AnyInput)]);
        s.run(20_000);
        let left = s.e.remaining(0, s.now).unwrap();
        assert!((39_000..=40_000).contains(&left), "{left}");
    }

    #[test]
    fn wake_all_restores_everything() {
        let r = Rule { fade_ms: 5000, ..rule(Trigger::PcIdle, Wake::CursorEnters) };
        let mut s = Sim::new(&[r, Rule { fade_ms: 0, ..r }]);
        s.run(MIN + 300);
        assert_eq!(s.e.wake_all(s.now), vec![Action::CancelFade(0), Action::Wake(1)]);
        assert!(s.run(MIN - 1000).is_empty());
    }

    // ---- sleeping between steps ----

    const PACE: Pace = Pace { fast: 250, dark: 5000, away: 2000, max: 30_000, probe_lead: 2500 };
    /// A pace that never caps the sleep, to see the raw deadline.
    const LONG: Pace = Pace { fast: 250, dark: 5000, away: 2000, max: u64::MAX, probe_lead: 2500 };

    impl Sim {
        /// Step only when `next_wait` says to, the way the tray app does,
        /// with `input` (time, screen) arriving in between. Returns each
        /// action with the time it happened, and how many steps it took.
        fn run_sparse(&mut self, ms: u64, pace: &Pace, input: &[(u64, usize)]) -> (Vec<(u64, Action)>, usize) {
            let mut all = Vec::new();
            let mut steps = 0;
            let start = self.now;
            let end = start + ms;
            let mut pending = input.iter().copied().peekable();
            while self.now < end {
                let next = self.now + self.e.next_wait(self.now, pace);
                // Input between steps: Windows remembers it; the cursor is wherever it was moved.
                while let Some(&(t, on)) = pending.peek() {
                    if start + t > next {
                        break;
                    }
                    self.mouse = start + t;
                    self.cursor = Some(on);
                    pending.next();
                }
                self.now = next.min(end);
                steps += 1;
                all.extend(self.step().into_iter().map(|a| (self.now - start, a)));
            }
            (all, steps)
        }
    }

    const HALF_HOUR: u64 = 30 * MIN;

    #[test]
    fn sleeps_until_just_before_the_timer_runs_out() {
        let r = Rule { timeout_ms: HALF_HOUR, ..rule(Trigger::PcIdle, Wake::AnyInput) };
        let s = Sim::new(&[r]);
        assert_eq!(s.e.next_wait(s.now, &LONG), HALF_HOUR - 2500);
        // Capped, so a fullscreen game ending or a keep-on app quitting is seen.
        assert_eq!(s.e.next_wait(s.now, &PACE), 30_000);
        // Inside the last stretch: look closely.
        assert_eq!(s.e.next_wait(s.now + HALF_HOUR - 1000, &PACE), 250);
    }

    #[test]
    fn the_earliest_screen_sets_the_wait() {
        let a = Rule { timeout_ms: HALF_HOUR, ..rule(Trigger::PcIdle, Wake::AnyInput) };
        let b = Rule { timeout_ms: 2 * MIN, ..a };
        let s = Sim::new(&[a, b]);
        assert_eq!(s.e.next_wait(s.now, &LONG), 2 * MIN - 2500);
    }

    #[test]
    fn away_screens_follow_the_cursor() {
        let r = Rule { timeout_ms: HALF_HOUR, ..rule(Trigger::AwayFromScreen, Wake::CursorEnters) };
        let s = Sim::new(&[r]);
        assert_eq!(s.e.next_wait(s.now, &LONG), 2000);
    }

    #[test]
    fn disabled_screens_need_nothing() {
        let s = Sim::new(&[Rule { enabled: false, ..rule(Trigger::AwayFromScreen, Wake::AnyInput) }]);
        assert_eq!(s.e.next_wait(s.now, &PACE), 30_000);
    }

    #[test]
    fn fading_is_watched_closely_and_dark_screens_rarely() {
        let r = Rule { fade_ms: 5000, ..rule(Trigger::PcIdle, Wake::AnyInput) };
        let mut s = Sim::new(&[r]);
        s.run(MIN + 100);
        assert!(matches!(s.e.phase(0), Phase::Fading { .. }));
        assert_eq!(s.e.next_wait(s.now, &PACE), 250);
        s.run(5000);
        assert_eq!(s.e.phase(0), Phase::Off);
        assert_eq!(s.e.next_wait(s.now, &PACE), 5000);
    }

    #[test]
    fn sparse_steps_turn_off_on_time() {
        let r = Rule { timeout_ms: HALF_HOUR, ..rule(Trigger::PcIdle, Wake::AnyInput) };
        let mut s = Sim::new(&[r]);
        let (actions, steps) = s.run_sparse(HALF_HOUR + MIN, &PACE, &[]);
        assert_eq!(actions.len(), 1);
        let (at, a) = actions[0];
        assert_eq!(a, Action::TurnOff(0));
        assert!((HALF_HOUR..=HALF_HOUR + 250).contains(&at), "turned off at {at}");
        // About one step per 30 s, plus the last few seconds closely: not 7,200 steps.
        assert!(steps < 90, "{steps} steps");
    }

    #[test]
    fn input_during_a_long_sleep_pushes_the_timer_back() {
        let r = Rule { timeout_ms: HALF_HOUR, ..rule(Trigger::PcIdle, Wake::AnyInput) };
        let mut s = Sim::new(&[r]);
        // Used the mouse 20 minutes in, between two steps.
        let (actions, _) = s.run_sparse(HALF_HOUR + 25 * MIN, &PACE, &[(20 * MIN + 7, 0)]);
        assert_eq!(actions.len(), 1);
        let (at, _) = actions[0];
        let expected = 20 * MIN + 7 + HALF_HOUR;
        assert!((expected..=expected + 250).contains(&at), "turned off at {at}, expected {expected}");
    }

    #[test]
    fn sparse_away_screen_still_turns_off_while_working_elsewhere() {
        let a = Rule { timeout_ms: HALF_HOUR, ..rule(Trigger::PcIdle, Wake::AnyInput) };
        let b = Rule { timeout_ms: 5 * MIN, ..rule(Trigger::AwayFromScreen, Wake::CursorEnters) };
        let mut s = Sim::new(&[a, b]);
        // Busy on screen 0 every 10 seconds for 10 minutes.
        let input: Vec<(u64, usize)> = (1..60).map(|k| (k * 10_000, 0)).collect();
        let (actions, _) = s.run_sparse(10 * MIN, &PACE, &input);
        assert_eq!(actions.iter().map(|(_, a)| *a).collect::<Vec<_>>(), vec![Action::TurnOff(1)]);
        let (at, _) = actions[0];
        assert!((5 * MIN..=5 * MIN + 2000).contains(&at), "turned off at {at}");
    }
}
