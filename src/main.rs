mod gameboy;
mod cpu;
mod ppu;
mod timer;
mod mmu;
mod cartridge;
mod platform;
mod apu;
mod hide;

use crate::gameboy::GameBoy;
use crate::cpu::Cpu;
use crate::hide::Hide;
use crate::platform::Platform;

fn main() {
    // ROM path from the first CLI argument, falling back to the hardcoded dev path.
    let args: Vec<String> = std::env::args().collect();
    let hide = Hide::new();
    let path: String = if args.len() > 1 { args[1].clone() } else { hide.path.clone() };

    let mut gb = GameBoy::new(&path);
    let mut platform = Platform::new();
    platform.init_audio(gb.mmu.apu.buffer());

    let mut frames_since_save: u32 = 0;
    while platform.is_open() {
        let cycles =gb.step();
        let check = gb.mmu.timer.process_cycles(cycles);
        gb.mmu.apu.step(cycles as u32);
        let frame_done =gb.tick_ppu(cycles);
        if check {
            gb.mmu.interrupt_flag |= 0b0000_0100
        }
        if frame_done {
            platform.render(&gb.ppu.framebuffer);
            let inp = platform.get_input();
            if inp != 0 {println!("input {:08b}", inp)}
            gb.set_buttons(inp);

            // Flush save RAM ~every 5s (no-op unless it changed) so a crash or
            // force-kill doesn't lose progress.
            frames_since_save += 1;
            if frames_since_save >= 300 {
                gb.mmu.save();
                frames_since_save = 0;
            }
        }


    }

    // Window closed: persist battery-backed save RAM.
    gb.mmu.save();
}
