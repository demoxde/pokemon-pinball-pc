# Local changes to rustboyadvance-ng

Vendored from https://github.com/michelhe/rustboyadvance-ng at commit
1d7ff23a6adb42d803ebd393c3d27d94e4fea15e (MIT licence, see LICENSE). Only the `core`, `arm7tdmi`
and `utils` crates are included.

- **HLE BIOS** (`arm7tdmi/src/hle.rs`, `core/src/bios.rs`): with `GameBoyAdvance::enable_hle_bios`,
  `swi` calls are serviced in Rust, and `hle_bios_image()` provides a minimal BIOS image holding
  only the IRQ dispatcher. No BIOS dump is needed.
- **HALTCNT byte writes** (`core/src/iodev.rs`): `strb` to 0x04000301 now halts the CPU (it
  previously landed on POSTFLG), and Stop mode is treated as Halt instead of panicking.
- **Halt fast-forward** (`core/src/sched.rs`): a halted CPU now always moves the clock forward by at
  least one cycle; before, landing exactly on the next event spun the run loop forever.
- Removed the criterion benchmark from `core/Cargo.toml`.
