# Pokémon Pinball: Ruby & Sapphire for PC

**Made by [demoxde](https://github.com/demoxde)**

Play Pokémon Pinball: Ruby & Sapphire on Windows. Double-click and play: no emulator to set up and
no BIOS file to find.

> **No game files are included.** You need your own Pokémon Pinball: Ruby & Sapphire ROM (`.gba`).
> This is an unofficial fan project, not affiliated with or endorsed by Nintendo, Creatures Inc.,
> GAME FREAK inc. or The Pokémon Company.

## Download and play

1. Download **[PokemonPinball.exe](download/PokemonPinball.exe)** (open it, then click the download button).
2. Double-click it. If Windows says "Windows protected your PC", click **More info**, then **Run anyway**.
3. The first time, pick your Pokémon Pinball: Ruby & Sapphire `.gba` file. It's remembered after that.

## Controls

| Action | Keys |
|---|---|
| Left flipper | Left Shift, Z or ← |
| Right flipper | Right Shift, X or / |
| Launch ball | Space |
| Nudge table | A and D |
| Pause | Enter |
| Fast forward | hold Tab |

Gamepads work too (bumpers are the flippers). Press **F1** in the game to see the controls.
High scores and the Pokédex save automatically. More details are in [HOW-TO-PLAY.txt](download/HOW-TO-PLAY.txt).

## Credits

The Game Boy Advance core is based on [rustboyadvance-ng](https://github.com/michelhe/rustboyadvance-ng)
by Michel Heily (MIT licence, see [vendor/rustboyadvance/LICENSE](vendor/rustboyadvance/LICENSE)), with a
built-in replacement for the GBA BIOS so no BIOS dump is needed.

## Building from source

The game app is in `app/` and the GBA core in `vendor/rustboyadvance/` (changes listed in
[`vendor/rustboyadvance/CHANGES.md`](vendor/rustboyadvance/CHANGES.md)).

```sh
# Windows .exe from Linux (needs the mingw-w64 package)
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu -p pokepinball

# Natively on Windows, macOS or Linux
cargo build --release -p pokepinball
```
