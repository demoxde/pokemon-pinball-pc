//! Finding the player's ROM: command line, the last one used, one next to the program, or a picker.

use std::fs;
use std::path::{Path, PathBuf};

use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

const EXPECTED_TITLE: &[u8] = b"POKEPIN R/S";
const REMEMBERED: &str = "rom_path.txt";

/// Returns the ROM bytes, or `None` if the player cancelled.
pub fn locate(data_dir: &Path) -> Result<Option<Vec<u8>>, String> {
    let remembered_file = data_dir.join(REMEMBERED);
    let candidates = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .into_iter()
        .chain(
            fs::read_to_string(&remembered_file)
                .ok()
                .map(|s| PathBuf::from(s.trim())),
        )
        .chain(roms_next_to_exe());

    for path in candidates {
        if let Ok(bytes) = fs::read(&path) {
            if is_pokemon_pinball(&bytes) {
                let _ = fs::write(&remembered_file, path.to_string_lossy().as_bytes());
                return Ok(Some(bytes));
            }
        }
    }

    MessageDialog::new()
        .set_title("Pokémon Pinball: Ruby & Sapphire")
        .set_description(
            "Welcome! To play, choose your Pokémon Pinball: Ruby & Sapphire ROM file (.gba).\n\n\
             You only need to do this once.",
        )
        .set_buttons(MessageButtons::Ok)
        .show();

    loop {
        let Some(path) = FileDialog::new()
            .set_title("Choose your Pokémon Pinball: Ruby & Sapphire ROM")
            .add_filter("Game Boy Advance ROM", &["gba", "GBA", "bin"])
            .pick_file()
        else {
            return Ok(None);
        };
        let bytes =
            fs::read(&path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
        let accepted = is_pokemon_pinball(&bytes)
            || MessageDialog::new()
                .set_level(MessageLevel::Warning)
                .set_title("Unexpected ROM")
                .set_description(
                    "This file doesn't look like Pokémon Pinball: Ruby & Sapphire.\n\nUse it anyway?",
                )
                .set_buttons(MessageButtons::YesNo)
                .show()
                == MessageDialogResult::Yes;
        if accepted {
            let _ = fs::write(&remembered_file, path.to_string_lossy().as_bytes());
            return Ok(Some(bytes));
        }
    }
}

fn is_pokemon_pinball(rom: &[u8]) -> bool {
    rom.get(0xa0..0xa0 + EXPECTED_TITLE.len()) == Some(EXPECTED_TITLE)
}

fn roms_next_to_exe() -> Vec<PathBuf> {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    else {
        return vec![];
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut roms: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gba"))
        })
        .collect();
    roms.sort();
    roms
}
