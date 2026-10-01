/// What counts as "not in use" for a screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// No mouse (or keyboard) input anywhere on the PC.
    PcIdle,
    /// No mouse input while the cursor was on this screen, and no typing into
    /// a window on it, even if the user is busy on another screen.
    AwayFromScreen,
}

/// What brings a screen back once it's off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    /// Any mouse movement or key press anywhere.
    AnyInput,
    /// Only the mouse moving while the cursor is on this screen.
    CursorEnters,
}

/// One screen's settings, as far as timing goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub enabled: bool,
    pub trigger: Trigger,
    /// How long the screen must be unused before it turns off.
    pub timeout_ms: u64,
    pub wake: Wake,
    /// Key presses count as using the screen, not only the mouse.
    pub typing_counts: bool,
    /// A changing picture (video, game) counts as using the screen.
    pub stay_on_while_playing: bool,
    /// Fade to black over this long first; any use during the fade cancels it.
    /// Zero turns the screen off straight away.
    pub fade_ms: u64,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger: Trigger::PcIdle,
            timeout_ms: 5 * 60 * 1000,
            wake: Wake::AnyInput,
            typing_counts: true,
            stay_on_while_playing: true,
            fade_ms: 5000,
        }
    }
}
