//! Standalone Pokémon Pinball: Ruby & Sapphire.
//!
//! Runs the player's own ROM in a built-in GBA core with a Rust stand-in for the BIOS, so nothing
//! else needs to be installed. No game data ships with this program.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod input;
mod rom;

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use minifb::{Key, KeyRepeat, Scale, ScaleMode, Window, WindowOptions};
use rfd::{MessageButtons, MessageDialog, MessageLevel};
use rustboyadvance_core::prelude::*;

const TITLE: &str = "Pokémon Pinball: Ruby & Sapphire";
/// The GBA refreshes at 16.78 MHz / 280896 cycles per frame ≈ 59.73 Hz
const FRAME_TIME: Duration = Duration::from_nanos(16_742_706);

fn main() {
    if let Err(message) = run() {
        MessageDialog::new()
            .set_level(MessageLevel::Error)
            .set_title(TITLE)
            .set_description(message)
            .set_buttons(MessageButtons::Ok)
            .show();
    }
}

fn data_dir() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("PokePinball");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn run() -> Result<(), String> {
    let data_dir = data_dir();
    let Some(rom) = rom::locate(&data_dir)? else {
        return Ok(()); // the player cancelled the file picker
    };

    let save_path = data_dir.join("pokepinball.sav");
    let cartridge = GamepakBuilder::new()
        .take_buffer(rom.into_boxed_slice())
        .save_path(&save_path)
        .build()
        .map_err(|e| format!("Could not load the ROM: {e:?}"))?;

    // The game still runs (silently) if there is no usable sound device
    let (audio_interface, _stream): (DynAudioInterface, _) = match audio::start() {
        Ok((interface, stream)) => (interface, Some(stream)),
        Err(e) => {
            log::warn!("audio disabled: {e}");
            (NullAudio::new(), None)
        }
    };

    let mut gba = GameBoyAdvance::new(hle_bios_image(), cartridge, audio_interface);
    gba.enable_hle_bios();
    gba.skip_bios();

    let mut window = Window::new(
        TITLE,
        DISPLAY_WIDTH,
        DISPLAY_HEIGHT,
        WindowOptions {
            resize: true,
            scale: Scale::X4,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .map_err(|e| format!("Could not open the game window: {e}"))?;

    let mut gamepads = input::Gamepads::new();
    let mut next_frame = Instant::now();

    while window.is_open() {
        if window.is_key_pressed(Key::F1, KeyRepeat::No) {
            show_controls();
            next_frame = Instant::now();
        }
        let fast_forward = window.is_key_down(Key::Tab);

        *gba.get_key_state_mut() = input::key_state(&window, &mut gamepads);
        gba.frame();
        window
            .update_with_buffer(gba.get_frame_buffer(), DISPLAY_WIDTH, DISPLAY_HEIGHT)
            .map_err(|e| format!("Display error: {e}"))?;

        if fast_forward {
            next_frame = Instant::now();
            continue;
        }
        next_frame += FRAME_TIME;
        let now = Instant::now();
        if next_frame > now {
            thread::sleep(next_frame - now);
        } else if now - next_frame > FRAME_TIME * 4 {
            // Far behind (window dragged, PC busy): resync instead of racing to catch up
            next_frame = now;
        }
    }
    Ok(())
}

fn show_controls() {
    MessageDialog::new()
        .set_title("Controls")
        .set_description(input::CONTROLS_HELP)
        .set_buttons(MessageButtons::Ok)
        .show();
}
