//! Runs the game without a window and dumps screenshots, for checking that it boots.
//! Usage: cargo run --release --example headless -- <rom.gba> <out-dir> [frames] [every] [keys-script]
use std::{env, fs, io::BufWriter, path::Path};

use rustboyadvance_core::prelude::*;

fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    let rom = fs::read(&args[1]).unwrap();
    let out = Path::new(&args[2]);
    let frames: u32 = args.get(3).map_or(600, |s| s.parse().unwrap());
    let every: u32 = args.get(4).map_or(60, |s| s.parse().unwrap());
    // "frame:keys,frame:keys" where keys is a hex KEYINPUT mask of pressed buttons
    let script: Vec<(u32, u16)> = args.get(5).map_or(vec![], |s| {
        s.split(',')
            .map(|e| {
                let (f, k) = e.split_once(':').unwrap();
                (f.parse().unwrap(), u16::from_str_radix(k, 16).unwrap())
            })
            .collect()
    });
    fs::create_dir_all(out).unwrap();

    let cart = GamepakBuilder::new()
        .take_buffer(rom.into_boxed_slice())
        .without_backup_to_file()
        .build()
        .unwrap();
    let (audio, mut samples) = SimpleAudioInterface::create_channel(48000, Some(1 << 16));
    let mut gba = GameBoyAdvance::new(hle_bios_image(), cart, audio);
    let mut peak = 0i16;
    gba.enable_hle_bios();
    gba.skip_bios();

    let mut pressed = 0u16;
    for f in 0..frames {
        if let Some(&(_, k)) = script.iter().find(|(sf, _)| *sf == f) {
            pressed = k;
        }
        *gba.get_key_state_mut() = !pressed & 0x3ff;
        gba.frame();
        {
            use ringbuf::traits::Consumer;
            while let Some(s) = samples.try_pop() {
                peak = peak.max(s.saturating_abs());
            }
        }
        if f % every == 0 || f == frames - 1 {
            let path = out.join(format!("frame_{f:05}.png"));
            let file = BufWriter::new(fs::File::create(&path).unwrap());
            let mut enc = png::Encoder::new(file, 240, 160);
            enc.set_color(png::ColorType::Rgb);
            let mut w = enc.write_header().unwrap();
            let rgb: Vec<u8> = gba
                .get_frame_buffer()
                .iter()
                .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
                .collect();
            w.write_image_data(&rgb).unwrap();
        }
    }
    println!(
        "pc = {:#x}, loudest audio sample = {peak}",
        gba.cpu.get_next_pc()
    );
    // DUMP=<addr>:<len>:<file> writes a memory range for inspection
    if let Ok(spec) = env::var("DUMP") {
        use rustboyadvance_core::prelude::arm7tdmi::memory::DebugRead;
        let mut it = spec.splitn(3, ':');
        let addr = u32::from_str_radix(it.next().unwrap().trim_start_matches("0x"), 16).unwrap();
        let len = u32::from_str_radix(it.next().unwrap().trim_start_matches("0x"), 16).unwrap();
        let bytes: Vec<u8> = (addr..addr + len)
            .map(|a| gba.cpu.bus.debug_read_8(a))
            .collect();
        fs::write(it.next().unwrap(), bytes).unwrap();
    }
}
