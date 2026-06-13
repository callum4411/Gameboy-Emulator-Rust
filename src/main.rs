mod gameboy;
mod cpu;
mod ppu;
mod timer;
mod mmu;
mod cartridge;
mod platform;
mod hide;

use crate::gameboy::GameBoy;
use crate::cpu::Cpu;
use crate::hide::Hide;
use crate::platform::Platform;

fn main() {
    let hide = Hide::new();
    let path = &hide.path;

    let mut gb = GameBoy::new(path);
    let mut platform = Platform::new();

    while platform.is_open() {
        let cycles =gb.step();
        let check = gb.mmu.timer.process_cycles(cycles);
        let frame_done =gb.tick_ppu(cycles);
        if check {
            gb.mmu.interrupt_flag |= 0b0000_0100
        }
        if frame_done {
            platform.render(&gb.ppu.framebuffer);
            let inp = platform.get_input();
            if inp != 0 {println!("input {:08b}", inp)}
            gb.set_buttons(inp)
        }


    }

}
