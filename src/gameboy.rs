use crate::cpu::Cpu;
use crate::mmu::Mmu;

pub struct GameBoy{
    pub(crate) cpu: Cpu,
    pub(crate) mmu: Mmu,
}
impl GameBoy{
    pub fn new(path: &str) -> GameBoy{
        GameBoy{
            cpu: Cpu::new(),
            mmu: Mmu::new(path),
        }
    }

    pub(crate) fn fetch_byte (&mut self) -> u8{
        let byte:u8 =self.mmu.read(self.cpu.pc);
        self.cpu.pc +=1;
        byte
    }
    fn fetch_word (&mut self) -> u16{
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }

    fn register_to_register(&mut self, src:u8, dest:u8){
        let mut data:u8 = 0x00; //0x00 by default
        match src {
            0b000 => {data = self.cpu.b}, //corresponds do B
            0b001 => {data = self.cpu.c},
            0b010 => {data = self.cpu.d},
            0b011 => {data = self.cpu.e},
            0b100 => {data = self.cpu.h},
            0b101 => {data = self.cpu.l},
            0b110 => {
                let addr:u16 = ((self.cpu.h as u16)<<8) | (self.cpu.l as u16);
                self.mmu.write(addr, data);
            },
            0b111 => {data = self.cpu.a},
            _ =>()
        }
        match dest {
            0b000 =>{
                self.cpu.b = data;
            },
            0b001 => {
                self.cpu.c = data;
            },
            0b010 =>{
                self.cpu.d = data;
            },
            0b011 =>{
                self.cpu.e = data;
            },
            0b100 =>{
                self.cpu.h = data;
            },
            0b101 => {
                self.cpu.l = data;
            },
            _ => ()
        }
    }

    fn load_to_register(&mut self, opcode:u8){
        let data = self.fetch_byte();
        match opcode {
            0x06 => {self.cpu.b = data},
            0x0E => {self.cpu.c = data},
            0x16 => {self.cpu.d = data},
            0x1E => {self.cpu.e = data},
            0x26 => {self.cpu.h = data},
            0x2E => {self.cpu.l = data},
            0x36 =>{
                let hl = ((self.cpu.h as u16)<<8) | (self.cpu.l as u16);
                self.mmu.write(hl, data);
            },
            0x3E => {self.cpu.a = data},
            _ =>()
        }
    }
    fn push_u16(&mut self, data:u16){
        let high = (data >> 8) as u8;
        let low = (data & 0x00FF) as u8;
        self.cpu.sp -= 1;
        self.mmu.write(self.cpu.sp, high);
        self.cpu.sp -= 1;
        self.mmu.write(self.cpu.sp, low);
    }
    fn pop_u16(&mut self) -> u16{
        let low = self.mmu.read(self.cpu.sp) as u16;
        self.cpu.sp +=1;
        let high = self.mmu.read(self.cpu.sp) as u16;
        self.cpu.sp +=1;
        (high << 8) | low
    }

    pub(crate) fn execute (&mut self, opcode: u8){
        match opcode{
            0x00 => (),
            0x47 => self.cpu.b = self.cpu.a,
            0x78 => self.cpu.a = self.cpu.b,
            0xC3 => self.cpu.pc = self.fetch_word(),
            0x21 => {
                let value = self.fetch_word();
                self.cpu.set_hl(value);
            },
            0x01 => {
                let value = self.fetch_word();
                self.cpu.set_bc(value);
            },
            0x11 => {
                let value = self.fetch_word();
                self.cpu.set_de(value);
            },
            0x31 => {
                let value = self.fetch_word();
                self.cpu.sp = value;
            },
            0x08 => {
                let data = self.cpu.sp;
                let d1 = (data>>8) as u8;
                let d2 = (data & 0x00FF) as u8;
                let addr = self.fetch_word();
                self.mmu.write(addr, d2);
                self.mmu.write((addr+1), d1)
            },
            0xC5 => { // push BC
                let bc = self.cpu.bc();
                self.push_u16(bc);
            },
            0xD5 => { // push DE
                let de = self.cpu.de();
                self.push_u16(de);
            },
            0xE5 => { // push HL
                let hl = self.cpu.hl();
                self.push_u16(hl);
            },
            0xF5 => { // push AF
                let af = self.cpu.af();
                self.push_u16(af);
            },
            0xC1 => { // pop BC
                let data = self.pop_u16();
                self.cpu.set_bc(data);
            },
            0xD1 => {  // pop DE
                let data = self.pop_u16();
                self.cpu.set_de(data);
            },
            0xE1 => { // pop hl
                let data = self.pop_u16();
                self.cpu.set_hl(data);
            },
            0xF1 => { // pop af  **special case**
                let data = self.pop_u16();
                self.cpu.set_af(data);
                
            },
            0x76 =>{ //special case, halt
                self.cpu.halted = true;
            },
            0x40..=0x7F =>{
                let sss = opcode & 0b0000_0111;
                let ddd = (opcode & 0b0011_1000)>>3;
                self.register_to_register(sss, ddd);
            },
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {self.load_to_register(opcode)},
            0xE0 => {
                let data = self.cpu.a;
                let dest = 0xFF00 + (self.fetch_byte() as u16);
                self.mmu.write(dest, data);
            },
            0xF0 => {
                let src = 0xFF00 + (self.fetch_byte() as u16);
                let data = self.mmu.read(src);
                self.cpu.a = data;
            },
            0xE2 => {
                let dest = 0xFF00 + (self.cpu.c as u16);
                let data = self.cpu.a;
                self.mmu.write(dest, data);
            },
            0xF2 => {
                let scr = 0xFF00 + (self.cpu.c as u16);
                let data = self.mmu.read(scr);
                self.cpu.a = data;
            },
            0x22 => {
                let mut hl = self.cpu.hl();
                self.mmu.write(hl, self.cpu.a);
                hl +=1;
                self.cpu.set_hl(hl);
            },
            0x2A => {
                let mut hl = self.cpu.hl();
                let data = self.mmu.read(hl);
                hl += 1;
                self.cpu.a = data;
                self.cpu.set_hl(hl);
            },
            0x32 => {
                let mut hl = self.cpu.hl();
                self.mmu.write(hl, self.cpu.a);
                hl -=1;
                self.cpu.set_hl(hl);
            },
            0x3A =>{
                let mut hl = self.cpu.hl();
                let data = self.mmu.read(hl);
                hl -= 1;
                self.cpu.a = data;
                self.cpu.set_hl(hl);
            },

            _=> panic!("not implemented: {:#04X}", opcode),
        }
    }

    pub(crate) fn step (&mut self) {
        let opcode = self.fetch_byte();
        self.execute(opcode);
    }
}