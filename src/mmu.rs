use crate::cartridge;
use crate::cartridge::Cartridge;


pub struct Mmu {
    wram: [u8; 0x2000],
    vram: [u8; 0x2000],
    oam: [u8; 0xA0],
    hram: [u8; 0x7F],
    cartridge: Cartridge,
}
impl Mmu {
    pub fn new(path: &str) -> Mmu {
        Mmu{
            wram: [0; 0x2000],
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            hram: [0; 0x7F],
            cartridge: Cartridge::new(path),
        }
    }
    pub(crate) fn read(&self, addr: u16) -> u8 {
        match addr {
            0xC000 ..= 0xDFFF => self.wram[(addr - 0xC000) as usize],
            0x8000 ..= 0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xFE00 ..= 0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFF80 ..= 0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0x0000 ..=0x7FFF => self.cartridge.read(addr),
            _ =>0,
        }


    }

    pub(crate) fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0xC000 ..= 0xDFFF => self.wram[(addr - 0xC000) as usize] = data,
            0x8000 ..= 0x9FFF => self.vram[(addr - 0x8000) as usize] = data,
            0xFE00 ..= 0xFE9F => self.oam[(addr - 0xFE00) as usize] = data,
            0xFF80 ..= 0xFFFE => self.hram[(addr - 0xFF80) as usize] = data,
            _ =>(),
        };
    }
}