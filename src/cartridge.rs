#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Mbc {
    None,
    Mbc1,
    Mbc2,
    Mbc3,
    Mbc5,
}

pub struct Cartridge {
    pub(crate) rom: Vec<u8>,
    pub(crate) ram: Vec<u8>,
    pub(crate) mbc: Mbc,
    pub(crate) bank_low: u8,   // primary ROM bank register
    pub(crate) bank_high: u8,  // MBC1: 2-bit secondary reg; MBC5: ROM bank bit 8
    pub(crate) ram_bank: u8,   // RAM bank select (MBC3/MBC5); MBC3 >=0x08 selects an RTC reg
    pub(crate) ram_enabled: bool,
    pub(crate) mode: bool,     // MBC1 banking mode (false = ROM, true = RAM/advanced)
    pub(crate) rtc: [u8; 5],   // MBC3 real-time-clock registers (not ticking)
    pub(crate) rom_banks: usize,
    pub(crate) ram_banks: usize,
    save_path: Option<String>, // Some(...) only for battery-backed carts with RAM
}

impl Cartridge {
    pub fn new(path: &str) -> Cartridge {
        let rom = std::fs::read(path).expect("Failed to read file");

        let mbc = match rom[0x0147] {
            0x00 => Mbc::None,
            0x01..=0x03 => Mbc::Mbc1,
            0x05 | 0x06 => Mbc::Mbc2,
            0x0F..=0x13 => Mbc::Mbc3,
            0x19..=0x1E => Mbc::Mbc5,
            other => panic!("unsupported cartridge type {:#04X}", other),
        };

        // Real ROM sizes are powers of two * 16KB, so this stays a power of two
        // and `rom_banks - 1` is a valid mask.
        let rom_banks = (rom.len() / 0x4000).max(2);

        let ram_banks = match rom[0x0149] {
            0x02 => 1,
            0x03 => 4,
            0x04 => 16,
            0x05 => 8,
            _ => 0,
        };

        // MBC2 has a fixed 512 x 4-bit built-in RAM regardless of the header.
        let mut ram = if mbc == Mbc::Mbc2 {
            vec![0; 512]
        } else {
            vec![0; ram_banks * 0x2000]
        };

        // Cartridge types with a battery keep their RAM across power cycles.
        let has_battery = matches!(
            rom[0x0147],
            0x03 | 0x06 | 0x09 | 0x0D | 0x0F | 0x10 | 0x13 | 0x1B | 0x1E | 0x22 | 0xFF
        );
        let save_path = if has_battery && !ram.is_empty() {
            let sp = std::path::Path::new(path)
                .with_extension("sav")
                .to_string_lossy()
                .into_owned();
            // Load an existing save if its size matches our RAM.
            if let Ok(data) = std::fs::read(&sp) {
                if data.len() == ram.len() {
                    ram.copy_from_slice(&data);
                }
            }
            Some(sp)
        } else {
            None
        };

        Cartridge {
            rom,
            ram,
            mbc,
            bank_low: 1,
            bank_high: 0,
            ram_bank: 0,
            ram_enabled: false,
            mode: false,
            rtc: [0; 5],
            rom_banks,
            ram_banks,
            save_path,
        }
    }

    // Write battery-backed RAM to disk. Call on exit (and periodically if desired).
    pub(crate) fn save(&self) {
        if let Some(sp) = &self.save_path {
            if let Err(e) = std::fs::write(sp, &self.ram) {
                eprintln!("failed to write save file {}: {}", sp, e);
            }
        }
    }

    // ROM region: 0x0000 ..= 0x7FFF
    pub(crate) fn read(&self, addr: u16) -> u8 {
        let mask = self.rom_banks - 1;
        match self.mbc {
            Mbc::None => self.rom[addr as usize],
            Mbc::Mbc1 => {
                if addr < 0x4000 {
                    // In advanced mode the high bits also remap the low region on large carts.
                    let bank = if self.mode { (self.bank_high as usize) << 5 } else { 0 };
                    self.rom[(bank & mask) * 0x4000 + addr as usize]
                } else {
                    let low = if self.bank_low == 0 { 1 } else { self.bank_low } as usize;
                    let bank = ((self.bank_high as usize) << 5) | low;
                    self.rom[(bank & mask) * 0x4000 + (addr as usize - 0x4000)]
                }
            }
            Mbc::Mbc2 => {
                if addr < 0x4000 {
                    self.rom[addr as usize]
                } else {
                    let bank = (self.bank_low as usize & 0x0F).max(1);
                    self.rom[(bank & mask) * 0x4000 + (addr as usize - 0x4000)]
                }
            }
            Mbc::Mbc3 => {
                if addr < 0x4000 {
                    self.rom[addr as usize]
                } else {
                    let bank = (self.bank_low as usize).max(1);
                    self.rom[(bank & mask) * 0x4000 + (addr as usize - 0x4000)]
                }
            }
            Mbc::Mbc5 => {
                if addr < 0x4000 {
                    self.rom[addr as usize]
                } else {
                    let bank = ((self.bank_high as usize) << 8) | self.bank_low as usize;
                    self.rom[(bank & mask) * 0x4000 + (addr as usize - 0x4000)]
                }
            }
        }
    }

    // ROM region writes are MBC control registers: 0x0000 ..= 0x7FFF
    pub(crate) fn write(&mut self, addr: u16, data: u8) {
        match self.mbc {
            Mbc::None => {}
            Mbc::Mbc1 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = (data & 0x0F) == 0x0A,
                0x2000..=0x3FFF => self.bank_low = data & 0x1F,
                0x4000..=0x5FFF => self.bank_high = data & 0x03,
                0x6000..=0x7FFF => self.mode = (data & 0x01) != 0,
                _ => {}
            },
            Mbc::Mbc2 => {
                // Below 0x4000, bit 8 of the address selects RAM-enable vs ROM-bank.
                if addr < 0x4000 {
                    if addr & 0x0100 == 0 {
                        self.ram_enabled = (data & 0x0F) == 0x0A;
                    } else {
                        self.bank_low = (data & 0x0F).max(1);
                    }
                }
            }
            Mbc::Mbc3 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = (data & 0x0F) == 0x0A,
                0x2000..=0x3FFF => self.bank_low = data & 0x7F,
                0x4000..=0x5FFF => self.ram_bank = data,
                0x6000..=0x7FFF => { /* RTC latch — clock isn't ticking, so no-op */ }
                _ => {}
            },
            Mbc::Mbc5 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = (data & 0x0F) == 0x0A,
                0x2000..=0x2FFF => self.bank_low = data,
                0x3000..=0x3FFF => self.bank_high = data & 0x01,
                0x4000..=0x5FFF => self.ram_bank = data & 0x0F,
                _ => {}
            },
        }
    }

    // External RAM region: 0xA000 ..= 0xBFFF
    pub(crate) fn read_ram(&self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        match self.mbc {
            Mbc::Mbc2 => self.ram[(addr as usize) & 0x01FF] | 0xF0,
            Mbc::Mbc3 if self.ram_bank >= 0x08 => {
                let idx = (self.ram_bank - 0x08) as usize;
                if idx < 5 { self.rtc[idx] } else { 0xFF }
            }
            _ => {
                if self.ram.is_empty() {
                    return 0xFF;
                }
                let offset = (self.ram_offset(addr)) % self.ram.len();
                self.ram[offset]
            }
        }
    }

    pub(crate) fn write_ram(&mut self, addr: u16, data: u8) {
        if !self.ram_enabled {
            return;
        }
        match self.mbc {
            Mbc::Mbc2 => self.ram[(addr as usize) & 0x01FF] = data & 0x0F,
            Mbc::Mbc3 if self.ram_bank >= 0x08 => {
                let idx = (self.ram_bank - 0x08) as usize;
                if idx < 5 {
                    self.rtc[idx] = data;
                }
            }
            _ => {
                if self.ram.is_empty() {
                    return;
                }
                let len = self.ram.len();
                let offset = self.ram_offset(addr) % len;
                self.ram[offset] = data;
            }
        }
    }

    // Flat offset into `ram` for the currently selected bank (non-MBC2, non-RTC).
    fn ram_offset(&self, addr: u16) -> usize {
        let bank = match self.mbc {
            Mbc::Mbc1 => if self.mode { self.bank_high as usize } else { 0 },
            Mbc::Mbc3 | Mbc::Mbc5 => self.ram_bank as usize,
            _ => 0,
        };
        bank * 0x2000 + (addr as usize - 0xA000)
    }
}
