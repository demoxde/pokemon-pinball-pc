//! Keyboard and gamepad mapping onto the GBA's buttons.

use gilrs::{Axis, Button, Gilrs};
use minifb::{Key, Window};
use rustboyadvance_core::keypad::{KEYINPUT_ALL_RELEASED, Keys};

pub const CONTROLS_HELP: &str = "\
Left flipper:   Left Shift, Z or Left arrow
Right flipper:  Right Shift, X or /
Launch ball:    Space (or the right flipper key)
Nudge table:    A (left) and D (right)
Menus:          arrow keys, X or Space to choose, Esc to go back
Pause:          Enter
Select button:  Backspace

F1: this help    Tab (hold): fast forward

Gamepad: bumpers are the flippers, triggers nudge the table,
A/Cross launches and chooses, B/Circle goes back.";

/// Each GBA button with every keyboard key that presses it.
/// In the game, Left is the left flipper, A the right flipper/launcher and L/R nudge the table.
const KEY_MAP: &[(Keys, &[Key])] = &[
    (
        Keys::ButtonA,
        &[Key::X, Key::RightShift, Key::Slash, Key::Space],
    ),
    (Keys::ButtonB, &[Key::Escape, Key::C]),
    (Keys::Select, &[Key::Backspace]),
    (Keys::Start, &[Key::Enter]),
    (Keys::Right, &[Key::Right]),
    (Keys::Left, &[Key::Left, Key::LeftShift, Key::Z]),
    (Keys::Up, &[Key::Up]),
    (Keys::Down, &[Key::Down]),
    (Keys::ButtonR, &[Key::D]),
    (Keys::ButtonL, &[Key::A]),
];

const PAD_MAP: &[(Keys, &[Button])] = &[
    (Keys::ButtonA, &[Button::South, Button::RightTrigger]),
    (Keys::ButtonB, &[Button::East]),
    (Keys::Select, &[Button::Select]),
    (Keys::Start, &[Button::Start]),
    (Keys::Right, &[Button::DPadRight]),
    (Keys::Left, &[Button::DPadLeft, Button::LeftTrigger]),
    (Keys::Up, &[Button::DPadUp]),
    (Keys::Down, &[Button::DPadDown]),
    (Keys::ButtonR, &[Button::RightTrigger2]),
    (Keys::ButtonL, &[Button::LeftTrigger2]),
];

pub struct Gamepads(Option<Gilrs>);

impl Gamepads {
    pub fn new() -> Self {
        Gamepads(Gilrs::new().ok())
    }
}

/// Builds KEYINPUT (a cleared bit means pressed) from the current keyboard and gamepad state.
pub fn key_state(window: &Window, gamepads: &mut Gamepads) -> u16 {
    let mut pressed = [false; 10];
    if window.is_active() {
        for (key, keys) in KEY_MAP {
            pressed[*key as usize] |= keys.iter().any(|k| window.is_key_down(*k));
        }
    }
    if let Some(gilrs) = &mut gamepads.0 {
        while gilrs.next_event().is_some() {}
        for (_, pad) in gilrs.gamepads() {
            for (key, buttons) in PAD_MAP {
                pressed[*key as usize] |= buttons.iter().any(|b| pad.is_pressed(*b));
            }
            let (x, y) = (pad.value(Axis::LeftStickX), pad.value(Axis::LeftStickY));
            pressed[Keys::Left as usize] |= x < -0.5;
            pressed[Keys::Right as usize] |= x > 0.5;
            pressed[Keys::Up as usize] |= y > 0.5;
            pressed[Keys::Down as usize] |= y < -0.5;
        }
    }
    let mut state = KEYINPUT_ALL_RELEASED;
    for (bit, down) in pressed.iter().enumerate() {
        if *down {
            state &= !(1 << bit);
        }
    }
    state
}
