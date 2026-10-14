//! High-level emulation of the GBA BIOS software interrupts.
//!
//! With `Arm7tdmiCore::hle_bios` set, `swi` instructions are serviced here directly instead of
//! jumping into a BIOS image, so no copyrighted BIOS dump is needed. The behaviour follows GBATEK
//! and mGBA's HLE BIOS.

use std::f32::consts::PI;

use log::warn;

use super::cpu::CpuAction;
use super::memory::{MemoryAccess, MemoryInterface};
use super::psr::RegPSR;
use super::{Arm7tdmiCore, CpuMode, CpuState};

const REG_IME: u32 = 0x0400_0208;
const REG_HALTCNT: u32 = 0x0400_0301;
const REG_SOUNDBIAS: u32 = 0x0400_0088;
const REG_DISPCNT: u32 = 0x0400_0000;
/// Interrupt flags the game's IRQ handler acknowledges for IntrWait
const BIOS_IRQ_FLAGS: u32 = 0x0300_7ff8;

impl<I: MemoryInterface> Arm7tdmiCore<I> {
    fn r8(&mut self, addr: u32) -> u8 {
        self.load_8(addr, MemoryAccess::NonSeq)
    }
    fn r16(&mut self, addr: u32) -> u16 {
        self.load_16(addr, MemoryAccess::NonSeq)
    }
    fn r32(&mut self, addr: u32) -> u32 {
        self.load_32(addr, MemoryAccess::NonSeq)
    }
    fn w8(&mut self, addr: u32, v: u8) {
        self.store_8(addr, v, MemoryAccess::NonSeq)
    }
    fn w16(&mut self, addr: u32, v: u16) {
        self.store_16(addr, v, MemoryAccess::NonSeq)
    }
    fn w32(&mut self, addr: u32, v: u32) {
        self.store_32(addr, v, MemoryAccess::NonSeq)
    }

    /// Writes decompressed output with halfword stores, which are valid for both WRAM and VRAM.
    fn write_out(&mut self, dst: u32, data: &[u8]) {
        let mut chunks = data.chunks_exact(2);
        let mut addr = dst;
        for c in &mut chunks {
            self.w16(addr, u16::from_le_bytes([c[0], c[1]]));
            addr = addr.wrapping_add(2);
        }
        if let [b] = chunks.remainder() {
            self.w8(addr, *b);
        }
    }

    /// Service `swi num` executed at `swi_addr`.
    pub(crate) fn hle_swi(&mut self, num: u8, swi_addr: u32) -> CpuAction {
        if log::log_enabled!(log::Level::Debug) {
            let (flags, ie, irf, ime) = (
                self.r16(BIOS_IRQ_FLAGS),
                self.r16(0x0400_0200),
                self.r16(0x0400_0202),
                self.r16(REG_IME),
            );
            log::debug!(
                "swi {:#04x} at {:#010x} r0={:#x} r1={:#x} r2={:#x} cpsr={:#x} flags={:#x} ie={:#x} if={:#x} ime={:#x}",
                num,
                swi_addr,
                self.gpr[0],
                self.gpr[1],
                self.gpr[2],
                self.cpsr.get(),
                flags,
                ie,
                irf,
                ime
            );
        }
        let r0 = self.gpr[0];
        let r1 = self.gpr[1];
        let r2 = self.gpr[2];
        let r3 = self.gpr[3];
        match num {
            0x00 => return self.hle_soft_reset(),
            0x01 => self.hle_register_ram_reset(r0),
            0x02 => self.w8(REG_HALTCNT, 0),
            0x03 => self.w8(REG_HALTCNT, 0x80),
            0x04 => return self.hle_intr_wait(r0, r1, swi_addr),
            0x05 => return self.hle_intr_wait(1, 1, swi_addr),
            0x06 => self.hle_div(r0 as i32, r1 as i32),
            0x07 => self.hle_div(r1 as i32, r0 as i32),
            0x08 => self.gpr[0] = (r0 as f64).sqrt() as u32,
            0x09 => {
                let (res, a, b) = arc_tan(r0 as i16 as i32);
                self.gpr[0] = res as u32 & 0xffff;
                self.gpr[1] = a as u32;
                self.gpr[3] = b as u32;
            }
            0x0a => self.gpr[0] = arc_tan2(r0 as i16 as i32, r1 as i16 as i32) as u32 & 0xffff,
            0x0b => self.hle_cpu_set(r0, r1, r2),
            0x0c => self.hle_cpu_fast_set(r0, r1, r2),
            0x0d => self.gpr[0] = 0xbaae_187f,
            0x0e => self.hle_bg_affine_set(r0, r1, r2),
            0x0f => self.hle_obj_affine_set(r0, r1, r2, r3),
            0x10 => self.hle_bit_unpack(r0, r1, r2),
            0x11 | 0x12 => {
                let out = self.lz77(r0);
                self.write_out(r1, &out);
            }
            0x13 => self.hle_huffman(r0, r1),
            0x14 | 0x15 => {
                let out = self.run_length(r0);
                self.write_out(r1, &out);
            }
            0x16 | 0x17 => {
                let out = self.diff8(r0);
                self.write_out(r1, &out);
            }
            0x18 => self.hle_diff16(r0, r1),
            0x19 => {
                let bias = self.r16(REG_SOUNDBIAS);
                let level = if r0 != 0 { 0x200 } else { 0 };
                self.w16(REG_SOUNDBIAS, (bias & !0x3ff) | level);
            }
            0x1f => {
                let base = self.r32(r0.wrapping_add(4)) as f32;
                let exp = (180.0 - r1 as f32 - r2 as f32 / 256.0) / 12.0;
                self.gpr[0] = (base / exp.exp2()) as u32;
            }
            _ => warn!("unhandled BIOS call swi {:#04x} at {:#010x}", num, swi_addr),
        }
        CpuAction::AdvancePC(MemoryAccess::Seq)
    }

    fn hle_soft_reset(&mut self) -> CpuAction {
        let boot_from_ewram = self.r8(0x0300_7ffa) != 0;
        for addr in (0x0300_7e00..0x0300_8000).step_by(4) {
            self.w32(addr, 0);
        }
        let mode = self.cpsr.mode();
        self.change_mode(mode, CpuMode::System);
        self.banks.gpr_banked_r13[CpuMode::Irq.bank_index()] = 0x0300_7fa0;
        self.banks.gpr_banked_r13[CpuMode::Supervisor.bank_index()] = 0x0300_7fe0;
        self.banks.gpr_banked_r14[CpuMode::Irq.bank_index()] = 0;
        self.banks.gpr_banked_r14[CpuMode::Supervisor.bank_index()] = 0;
        self.gpr = [0; 15];
        self.gpr[13] = 0x0300_7f00;
        self.cpsr = RegPSR::new(0x1f);
        self.cpsr.set_state(CpuState::ARM);
        self.pc = if boot_from_ewram {
            0x0200_0000
        } else {
            0x0800_0000
        };
        self.reload_pipeline32();
        CpuAction::PipelineFlushed
    }

    fn hle_register_ram_reset(&mut self, flags: u32) {
        self.w16(REG_DISPCNT, 0x80);
        let clear = |cpu: &mut Self, start: u32, len: u32| {
            for addr in (start..start + len).step_by(4) {
                cpu.w32(addr, 0);
            }
        };
        if flags & 0x01 != 0 {
            clear(self, 0x0200_0000, 0x4_0000);
        }
        if flags & 0x02 != 0 {
            clear(self, 0x0300_0000, 0x7e00);
        }
        if flags & 0x04 != 0 {
            clear(self, 0x0500_0000, 0x400);
        }
        if flags & 0x08 != 0 {
            clear(self, 0x0600_0000, 0x1_8000);
        }
        if flags & 0x10 != 0 {
            clear(self, 0x0700_0000, 0x400);
        }
        if flags & 0x40 != 0 {
            // Sound registers (SOUNDBIAS is kept)
            for addr in (0x0400_0060..0x0400_0088).step_by(2) {
                self.w16(addr, 0);
            }
            for addr in (0x0400_0090..0x0400_00a8).step_by(2) {
                self.w16(addr, 0);
            }
        }
        if flags & 0x80 != 0 {
            // Everything else: LCD, DMA, timers, keypad and interrupt control
            for addr in (0x0400_0002..0x0400_0060).step_by(2) {
                self.w16(addr, 0);
            }
            for addr in [0x0400_0020, 0x0400_0026, 0x0400_0030, 0x0400_0036] {
                self.w16(addr, 0x100); // identity BG2/BG3 affine matrices
            }
            for addr in (0x0400_00b0..0x0400_00e0).step_by(2) {
                self.w16(addr, 0);
            }
            for addr in (0x0400_0100..0x0400_0110).step_by(2) {
                self.w16(addr, 0);
            }
            self.w16(0x0400_0132, 0);
            self.w16(REG_IME, 0);
            self.w16(0x0400_0200, 0);
            self.w16(0x0400_0202, 0xffff);
            self.w16(0x0400_0204, 0);
        }
    }

    /// IntrWait: halt until one of the interrupts in `mask` is flagged at 0x03007FF8.
    /// While nothing is flagged the CPU halts and the `swi` is re-executed after the next IRQ.
    fn hle_intr_wait(&mut self, discard_old: u32, mask: u32, swi_addr: u32) -> CpuAction {
        let mask = mask as u16;
        self.w16(REG_IME, 1);
        let mut flags = self.r16(BIOS_IRQ_FLAGS);
        // Re-entries after wake-up must not discard the flag that woke us
        if discard_old != 0 && !self.hle_intr_waiting {
            flags &= !mask;
            self.w16(BIOS_IRQ_FLAGS, flags);
        }
        if flags & mask != 0 {
            self.w16(BIOS_IRQ_FLAGS, flags & !mask);
            self.hle_intr_waiting = false;
            return CpuAction::AdvancePC(MemoryAccess::Seq);
        }
        self.hle_intr_waiting = true;
        self.w8(REG_HALTCNT, 0);
        self.pc = swi_addr;
        match self.cpsr.state() {
            CpuState::ARM => self.reload_pipeline32(),
            CpuState::THUMB => self.reload_pipeline16(),
        }
        CpuAction::PipelineFlushed
    }

    fn hle_div(&mut self, num: i32, den: i32) {
        if den == 0 {
            // The real BIOS hangs; return something sane instead
            self.gpr[0] = if num < 0 { -1i32 as u32 } else { 1 };
            self.gpr[1] = num as u32;
            self.gpr[3] = 1;
            return;
        }
        let q = num.wrapping_div(den);
        self.gpr[0] = q as u32;
        self.gpr[1] = num.wrapping_rem(den) as u32;
        self.gpr[3] = q.unsigned_abs();
    }

    fn hle_cpu_set(&mut self, src: u32, dst: u32, ctrl: u32) {
        let count = ctrl & 0x1f_ffff;
        let fixed = ctrl & (1 << 24) != 0;
        if ctrl & (1 << 26) != 0 {
            let (src, dst) = (src & !3, dst & !3);
            let fill = self.r32(src);
            for i in 0..count {
                let v = if fixed { fill } else { self.r32(src + i * 4) };
                self.w32(dst + i * 4, v);
            }
        } else {
            let (src, dst) = (src & !1, dst & !1);
            let fill = self.r16(src);
            for i in 0..count {
                let v = if fixed { fill } else { self.r16(src + i * 2) };
                self.w16(dst + i * 2, v);
            }
        }
    }

    fn hle_cpu_fast_set(&mut self, src: u32, dst: u32, ctrl: u32) {
        let count = ((ctrl & 0x1f_ffff) + 7) & !7;
        let fixed = ctrl & (1 << 24) != 0;
        let (src, dst) = (src & !3, dst & !3);
        let fill = self.r32(src);
        for i in 0..count {
            let v = if fixed { fill } else { self.r32(src + i * 4) };
            self.w32(dst + i * 4, v);
        }
    }

    fn hle_bg_affine_set(&mut self, mut src: u32, mut dst: u32, count: u32) {
        for _ in 0..count {
            let ox = self.r32(src) as i32 as f32 / 256.0;
            let oy = self.r32(src + 4) as i32 as f32 / 256.0;
            let cx = self.r16(src + 8) as i16 as f32;
            let cy = self.r16(src + 10) as i16 as f32;
            let sx = self.r16(src + 12) as i16 as f32 / 256.0;
            let sy = self.r16(src + 14) as i16 as f32 / 256.0;
            let theta = (self.r16(src + 16) >> 8) as f32 / 128.0 * PI;
            src += 20;
            let (sin, cos) = theta.sin_cos();
            let (a, b, c, d) = (cos * sx, -sin * sx, sin * sy, cos * sy);
            let rx = ox - (a * cx + b * cy);
            let ry = oy - (c * cx + d * cy);
            self.w16(dst, (a * 256.0) as i32 as u16);
            self.w16(dst + 2, (b * 256.0) as i32 as u16);
            self.w16(dst + 4, (c * 256.0) as i32 as u16);
            self.w16(dst + 6, (d * 256.0) as i32 as u16);
            self.w32(dst + 8, (rx * 256.0) as i32 as u32);
            self.w32(dst + 12, (ry * 256.0) as i32 as u32);
            dst += 16;
        }
    }

    fn hle_obj_affine_set(&mut self, mut src: u32, mut dst: u32, count: u32, diff: u32) {
        for _ in 0..count {
            let sx = self.r16(src) as i16 as f32 / 256.0;
            let sy = self.r16(src + 2) as i16 as f32 / 256.0;
            let theta = (self.r16(src + 4) >> 8) as f32 / 128.0 * PI;
            src += 8;
            let (sin, cos) = theta.sin_cos();
            let (a, b, c, d) = (cos * sx, -sin * sx, sin * sy, cos * sy);
            self.w16(dst, (a * 256.0) as i32 as u16);
            self.w16(dst + diff, (b * 256.0) as i32 as u16);
            self.w16(dst + diff * 2, (c * 256.0) as i32 as u16);
            self.w16(dst + diff * 3, (d * 256.0) as i32 as u16);
            dst += diff * 4;
        }
    }

    fn hle_bit_unpack(&mut self, src: u32, dst: u32, info: u32) {
        let len = self.r16(info) as u32;
        let src_width = self.r8(info + 2) as u32;
        let dst_width = self.r8(info + 3) as u32;
        let offset_word = self.r32(info + 4);
        let offset = offset_word & 0x7fff_ffff;
        let zero_flag = offset_word & 0x8000_0000 != 0;
        if !matches!(src_width, 1 | 2 | 4 | 8) || !matches!(dst_width, 1 | 2 | 4 | 8 | 16 | 32) {
            warn!("BitUnPack: invalid widths {} -> {}", src_width, dst_width);
            return;
        }
        let src_mask = (1u32 << src_width) - 1;
        let mut out: u32 = 0;
        let mut out_bits = 0;
        let mut dst = dst;
        for i in 0..len {
            let byte = self.r8(src + i) as u32;
            for shift in (0..8).step_by(src_width as usize) {
                let mut v = (byte >> shift) & src_mask;
                if v != 0 || zero_flag {
                    v = v.wrapping_add(offset);
                }
                if dst_width < 32 {
                    v &= (1u32 << dst_width) - 1;
                }
                out |= v << out_bits;
                out_bits += dst_width;
                if out_bits == 32 {
                    self.w32(dst, out);
                    dst += 4;
                    out = 0;
                    out_bits = 0;
                }
            }
        }
    }

    fn lz77(&mut self, mut src: u32) -> Vec<u8> {
        let size = (self.r32(src) >> 8) as usize;
        src += 4;
        let mut out = Vec::with_capacity(size);
        while out.len() < size {
            let flags = self.r8(src);
            src += 1;
            for bit in (0..8).rev() {
                if out.len() >= size {
                    break;
                }
                if flags & (1 << bit) == 0 {
                    out.push(self.r8(src));
                    src += 1;
                } else {
                    let b0 = self.r8(src) as usize;
                    let b1 = self.r8(src + 1) as usize;
                    src += 2;
                    let len = (b0 >> 4) + 3;
                    let disp = (((b0 & 0xf) << 8) | b1) + 1;
                    for _ in 0..len {
                        let v = if disp <= out.len() {
                            out[out.len() - disp]
                        } else {
                            0
                        };
                        out.push(v);
                    }
                }
            }
        }
        out.truncate(size);
        out
    }

    fn run_length(&mut self, mut src: u32) -> Vec<u8> {
        let size = (self.r32(src) >> 8) as usize;
        src += 4;
        let mut out = Vec::with_capacity(size);
        while out.len() < size {
            let flag = self.r8(src);
            src += 1;
            if flag & 0x80 != 0 {
                let v = self.r8(src);
                src += 1;
                for _ in 0..(flag & 0x7f) as usize + 3 {
                    out.push(v);
                }
            } else {
                for _ in 0..(flag & 0x7f) as usize + 1 {
                    out.push(self.r8(src));
                    src += 1;
                }
            }
        }
        out.truncate(size);
        out
    }

    fn diff8(&mut self, src: u32) -> Vec<u8> {
        let size = self.r32(src) >> 8;
        let mut out = Vec::with_capacity(size as usize);
        let mut acc = 0u8;
        for i in 0..size {
            acc = acc.wrapping_add(self.r8(src + 4 + i));
            out.push(acc);
        }
        out
    }

    fn hle_diff16(&mut self, src: u32, dst: u32) {
        let size = self.r32(src) >> 8;
        let mut acc = 0u16;
        for i in (0..size).step_by(2) {
            acc = acc.wrapping_add(self.r16(src + 4 + i));
            self.w16(dst + i, acc);
        }
    }

    fn hle_huffman(&mut self, src: u32, mut dst: u32) {
        let header = self.r32(src);
        let bits = header & 0xf;
        let size = header >> 8;
        if bits != 4 && bits != 8 {
            warn!("HuffUnComp: unsupported data size {}", bits);
            return;
        }
        let tree_base = src + 5;
        let tree_size = (self.r8(src + 4) as u32 + 1) * 2;
        let mut stream = src + 4 + tree_size;
        let mut node_addr = tree_base;
        let mut node = self.r8(node_addr);
        let mut out = 0u32;
        let mut out_bits = 0;
        let mut written = 0;
        while written < size {
            let word = self.r32(stream);
            stream += 4;
            for bit in (0..32).rev() {
                let child_addr = (node_addr & !1) + (node as u32 & 0x3f) * 2 + 2;
                let right = (word >> bit) & 1 != 0;
                let (addr, is_data) = if right {
                    (child_addr + 1, node & 0x40 != 0)
                } else {
                    (child_addr, node & 0x80 != 0)
                };
                if is_data {
                    out |= (self.r8(addr) as u32) << out_bits;
                    out_bits += bits;
                    node_addr = tree_base;
                    node = self.r8(node_addr);
                    if out_bits == 32 {
                        self.w32(dst, out);
                        dst += 4;
                        written += 4;
                        out = 0;
                        out_bits = 0;
                        if written >= size {
                            return;
                        }
                    }
                } else {
                    node_addr = addr;
                    node = self.r8(node_addr);
                }
            }
        }
    }
}

/// Polynomial arctangent used by the BIOS, returning (result, r1, r3).
fn arc_tan(i: i32) -> (i32, i32, i32) {
    let a = -(i.wrapping_mul(i) >> 14);
    let mut b = ((0xa9 * a) >> 14) + 0x390;
    for k in [0x91c, 0xfb6, 0x16aa, 0x2081, 0x3651, 0xa2f9] {
        b = (b.wrapping_mul(a) >> 14) + k;
    }
    (i.wrapping_mul(b) >> 16, a, b)
}

fn arc_tan2(x: i32, y: i32) -> i32 {
    let atan = |v: i32| arc_tan(v).0;
    if y == 0 {
        return if x >= 0 { 0 } else { 0x8000 };
    }
    if x == 0 {
        return if y >= 0 { 0x4000 } else { 0xc000 };
    }
    if y >= 0 {
        if x >= 0 {
            if x >= y {
                return atan((y << 14) / x);
            }
        } else if -x >= y {
            return atan((y << 14) / x) + 0x8000;
        }
        0x4000 - atan((x << 14) / y)
    } else {
        if x <= 0 {
            if -x > -y {
                return atan((y << 14) / x) + 0x8000;
            }
        } else if x >= -y {
            return atan((y << 14) / x) + 0x10000;
        }
        0xc000 - atan((x << 14) / y)
    }
}
