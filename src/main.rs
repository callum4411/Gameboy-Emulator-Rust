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

fn main() {
    let hide = Hide::new();
    let path = &hide.path;

    let mut gb = GameBoy::new(path);

    gb.cpu.pc = 0x0100;

    for _ in 0..100{
        gb.step();
    }

}
