pub struct Cartridge {
    pub(crate) rom: Vec<u8>,
}

impl Cartridge {
    pub fn new(path: &str) -> Cartridge {
        let rom = std::fs::read(path).expect("Failed to read file");
        Cartridge{
            rom,
        }
    }

    pub(crate) fn read(&self, addr: u16) -> u8 {
        self.rom[addr as usize]
    }
}