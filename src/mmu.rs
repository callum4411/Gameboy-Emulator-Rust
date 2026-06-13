use crate::cartridge;
use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
use crate::timer::Timer;
use crate::apu::Apu;


pub struct Mmu {
    wram: [u8; 0x2000],
    vram: [u8; 0x2000],
    oam: [u8; 0xA0],
    hram: [u8; 0x7F],
    cartridge: Cartridge,
    serial_data: u8,
    pub(crate) interrupt_flag: u8,
    pub(crate) interrupt_enable: u8,
    pub(crate) timer: Timer,
    pub(crate) apu: Apu,
    pub(crate) lcdc: u8,
    pub(crate) stat: u8,
    pub(crate) scy: u8,
    pub(crate) scx: u8,
    pub(crate) lyc: u8,
    pub(crate) bgp: u8,
    pub(crate) obp0: u8,
    pub(crate) obp1: u8,
    pub(crate) wy: u8,
    pub(crate) wx: u8,
    pub(crate) ly: u8,
    pub(crate) buttons: u8,
    pub(crate) joypad_select: u8,
}
impl Mmu {
    pub fn new(path: &str) -> Mmu {
        Mmu{
            wram: [0; 0x2000],
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            hram: [0; 0x7F],
            cartridge: Cartridge::new(path),
            serial_data: 0,
            interrupt_flag: 0,
            interrupt_enable: 0,
            timer: Timer::new(),
            apu: Apu::new(),
            lcdc: 0x91, // post-boot-ROM value: LCD on + BG on (we skip the boot ROM)
            stat: 0x85,
            scy: 0,
            scx: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            ly: 0,
            buttons: 0,
            joypad_select: 0,
        }
    }
    pub(crate) fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF00 => {
                let mut output = 0xCF;
                if self.joypad_select & 0b0010_0000 == 0{ //buttons selected
                    let a = ((!self.buttons) & 0b0001_0000)>>4;
                    let b = ((!self.buttons) & 0b0010_0000)>>5;
                    let start = ((!self.buttons) & 0b1000_0000)>>7;
                    let select = ((!self.buttons) & 0b0100_0000)>>6;
                    output = 0b1101_0000;
                    output = output + (a) + (b<<1)+ (start<<3) + (select<<2);
                } else if self.joypad_select & 0b0001_0000 == 0 {
                    let right = (!self.buttons) & 0b000_0001;
                    let up = ((!self.buttons) & 0b0000_0100)>>2;
                    let left = ((!self.buttons) & 0b0000_0010)>>1;
                    let down = ((!self.buttons) & 0b0000_1000)>>3;
                    output = 0b1110_0000;
                    output = output + (right) + (up<<2)+ (left<<1) + (down<<3);
                }
                output
            },
            0xFF40 => self.lcdc,
            0xFF41 => self.stat,
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            0xFF04 => self.timer.div,
            0xFF05 => self.timer.tima,
            0xFF06 => self.timer.tma,
            0xFF07 => self.timer.tac,
            0xFF0F => 0xE0 | self.interrupt_flag,
            0xFFFF => self.interrupt_enable,
            0xFF10 ..= 0xFF3F => self.apu.read(addr),
            0xC000 ..= 0xDFFF => self.wram[(addr - 0xC000) as usize],
            0x8000 ..= 0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xFE00 ..= 0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFF80 ..= 0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0x0000 ..=0x7FFF => self.cartridge.read(addr),
            0xA000 ..= 0xBFFF => self.cartridge.read_ram(addr),
            _ =>0,
        }


    }

    pub(crate) fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0xFF00 => self.joypad_select = data,
            0xFF40 => self.lcdc = data,
            0xFF41 => self.stat = data,
            0xFF42 => self.scy = data,
            0xFF43 => self.scx = data,
            0xFF45 => self.lyc = data,
            0xFF47 => self.bgp = data,
            0xFF48 => self.obp0 = data,
            0xFF49 => self.obp1 = data,
            0xFF4A => self.wy = data,
            0xFF4B => self.wx = data,
            0xFF46 => {
                // OAM DMA: copy 0xA0 bytes from data*0x100 into OAM.
                let src = (data as u16) << 8;
                for i in 0..0xA0u16 {
                    let b = self.read(src + i);
                    self.oam[i as usize] = b;
                }
            },
            0xFF04 => self.timer.div =0,
            0xFF05 => self.timer.tima = data,
            0xFF06 => self.timer.tma =data,
            0xFF07 => self.timer.tac = data,
            0xFF0F => self.interrupt_flag = data,
            0xFFFF => self.interrupt_enable = data,
            0xFF10 ..= 0xFF3F => self.apu.write(addr, data),
            0x0000 ..= 0x7FFF => self.cartridge.write(addr, data),
            0xA000 ..= 0xBFFF => self.cartridge.write_ram(addr, data),
            0xC000 ..= 0xDFFF => self.wram[(addr - 0xC000) as usize] = data,
            0x8000 ..= 0x9FFF => self.vram[(addr - 0x8000) as usize] = data,
            0xFE00 ..= 0xFE9F => self.oam[(addr - 0xFE00) as usize] = data,
            0xFF80 ..= 0xFFFE => self.hram[(addr - 0xFF80) as usize] = data,
            0xFF01 => {
                self.serial_data = data;
            },
            0xFF02 => {
                if data == 0x81{
                    print!("{}", self.serial_data as char);
                }
            },
            _ =>(),
        };
    }

    // Persist battery-backed cartridge RAM to disk (no-op unless RAM changed).
    pub(crate) fn save(&mut self) {
        self.cartridge.save();
    }
}