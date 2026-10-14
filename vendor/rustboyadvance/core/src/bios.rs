use super::SysBus;
use arm7tdmi::{
    Arm7tdmiCore,
    memory::{Addr, BusIO, DebugRead},
};

use rustboyadvance_utils::WeakPointer;

/// Struct representing the sytem ROM
#[derive(Clone)]
pub struct Bios {
    /// Underlying memory
    rom: Box<[u8]>,
    /// Last read value
    last_opcode: u32,
    /// Arm pointer - used only to read the PC register
    arm_core: WeakPointer<Arm7tdmiCore<SysBus>>,
}

impl Bios {
    pub fn new(bios_rom: Box<[u8]>) -> Bios {
        Bios {
            rom: bios_rom,
            last_opcode: 0xe129f000, // the opcode at [00DCh+8]
            arm_core: WeakPointer::default(),
        }
    }

    pub(super) fn connect_arm_core(&mut self, arm_ptr: WeakPointer<Arm7tdmiCore<SysBus>>) {
        self.arm_core = arm_ptr;
    }

    #[inline]
    fn read_allowed(&self) -> bool {
        self.arm_core.pc < 0x4000
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.rom.len()
    }
}

/// Impl of Bus trait for Bios
impl BusIO for Bios {
    #[inline]
    fn read_32(&mut self, addr: Addr) -> u32 {
        if self.read_allowed() {
            let value = self.rom.read_32(addr);
            // 32-bit read from bios is most probably an opcode fetch
            self.last_opcode = value;
            value
        } else {
            self.last_opcode
        }
    }
    #[inline]
    fn read_16(&mut self, addr: Addr) -> u16 {
        if self.read_allowed() {
            self.rom.read_16(addr)
        } else {
            (self.last_opcode >> ((addr & 2) << 3)) as u16
        }
    }

    #[inline]
    fn read_8(&mut self, addr: Addr) -> u8 {
        if self.read_allowed() {
            self.rom.read_8(addr)
        } else {
            (self.last_opcode >> ((addr & 3) << 3)) as u8
        }
    }

    #[inline]
    fn write_8(&mut self, _addr: Addr, _value: u8) {
        // The bios is RO
    }
}

impl DebugRead for Bios {
    fn debug_read_8(&mut self, addr: Addr) -> u8 {
        self.rom[addr as usize]
    }
}

/// A minimal 16KiB BIOS image for use with `GameBoyAdvance::enable_hle_bios`.
///
/// Only the IRQ vector carries code: it saves the scratch registers and calls the game's handler
/// at [0x03FFFFFC], like the real BIOS. All `swi` calls are handled in Rust (arm7tdmi::hle).
pub fn hle_bios_image() -> Box<[u8]> {
    const IRQ_HANDLER: usize = 0x128;
    let mut rom = vec![0u8; 0x4000];
    let mut put = |addr: usize, word: u32| rom[addr..addr + 4].copy_from_slice(&word.to_le_bytes());
    // Every vector spins forever except IRQ, which branches to the handler
    for vector in (0x00..0x20).step_by(4) {
        put(vector, 0xeaff_fffe); // b .
    }
    put(0x18, 0xea00_0000 | ((IRQ_HANDLER - 0x18 - 8) >> 2) as u32); // b IRQ_HANDLER
    let handler = [
        0xe92d_500f, // push {r0-r3, r12, lr}
        0xe3a0_0301, // mov r0, #0x04000000
        0xe28f_e000, // add lr, pc, #0
        0xe510_f004, // ldr pc, [r0, #-4]
        0xe8bd_500f, // pop {r0-r3, r12, lr}
        0xe25e_f004, // subs pc, lr, #4
    ];
    for (i, word) in handler.iter().enumerate() {
        put(IRQ_HANDLER + i * 4, *word);
    }
    rom.into_boxed_slice()
}
