mod gameboy;
mod cpu;
mod ppu;
mod timer;
mod mmu;
mod cartridge;
mod platform;

use crate::gameboy::GameBoy;
use crate::cpu::Cpu;

fn main() {
    let mut gb = GameBoy::new();

    gb.cpu.a = 0x42;
    gb.cpu.pc = 0xC000;
    gb.mmu.write(0xC000, 0x47);

    gb.step();
    println!("A  = {} ({:#04X})", gb.cpu.a, gb.cpu.a);
    println!("B  = {} ({:#04X})", gb.cpu.b, gb.cpu.b);
    println!("PC = {} ({:#06X})", gb.cpu.pc, gb.cpu.pc);
}
