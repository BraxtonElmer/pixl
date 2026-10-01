//! Platform-free heart of Pixl: given what the user is doing, decide when each
//! screen fades out, turns off and wakes up again. No Windows code, so every
//! rule is unit tested.

mod engine;
mod rule;

pub use engine::{Action, Engine, Inputs, Pace, Phase, ScreenInputs};
pub use rule::{Rule, Trigger, Wake};
