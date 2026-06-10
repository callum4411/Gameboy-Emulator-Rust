use crate::cpu::Cpu;
use crate::mmu::Mmu;

pub struct GameBoy{
    pub(crate) cpu: Cpu,
    pub(crate) mmu: Mmu,
}
impl GameBoy{
    pub fn new() -> GameBoy{
        GameBoy{
            cpu: Cpu::new(),
            mmu: Mmu::new(),
        }
    }

    pub(crate) fn fetch_byte (&mut self) -> u8{
        let byte:u8 =self.mmu.read(self.cpu.pc);
        self.cpu.pc +=1;
        byte
    }

    pub(crate) fn execute (&mut self, opcode: u8){
        match opcode{
            0x00 => (),
            0x40 => self.cpu.b = self.cpu.b,
            0x47 => self.cpu.b = self.cpu.a,
            0x78 => self.cpu.a = self.cpu.b,
            _=> panic!("not implemented: {:#04X}", opcode),
        }
    }

    pub(crate) fn step (&mut self) {
        let opcode = self.fetch_byte();
        self.execute(opcode);
    }
}