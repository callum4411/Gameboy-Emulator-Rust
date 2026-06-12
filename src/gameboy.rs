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
                //let addr:u16 = ((self.cpu.h as u16)<<8) | (self.cpu.l as u16);
                //self.mmu.write(addr, data);
                data = self.mmu.read(self.cpu.hl())
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

    fn get_register_by_bits(&self, bits: u8) -> u8{
        match bits{
            0 => self.cpu.b,
            1 => self.cpu.c,
            2 => self.cpu.d,
            3 => self.cpu.e,
            4 => self.cpu.h,
            5 => self.cpu.l,
            6 => {
                let hl = self.cpu.hl();
                self.mmu.read(hl)
            },
            7 => self.cpu.a,
            _ => unreachable!()
        }
    }

    fn set_register_by_bits(&mut self, bits: u8, value: u8){
        match bits {
            0 => self.cpu.b = value,
            1 => self.cpu.c = value,
            2 => self.cpu.d = value,
            3 => self.cpu.e = value,
            4 => self.cpu.h = value,
            5 => self.cpu.l = value,
            6 => {
                let hl = self.cpu.hl();
                self.mmu.write(hl, value);
            },
            7 => self.cpu.a = value,
            _ => unreachable!(),
        }
    }

    fn get_pair_by_bits(&self, bits: u8) -> u16{
        match bits{
            0 => self.cpu.bc(),
            1 => self.cpu.de(),
            2 => self.cpu.hl(),
            3 => self.cpu.sp,
            _ => unreachable!()
        }
    }

    fn set_pair_by_bits(&mut self, bits: u8, value:u16){
        match bits{
            0 => self.cpu.set_bc(value),
            1 => self.cpu.set_de(value),
            2 => self.cpu.set_hl(value),
            3 => self.cpu.sp = value,
            _ => unreachable!()
        }
    }

    fn check_condition(&self, bits: u8) -> bool{
        match bits{
            0 => !self.cpu.flag_z(),
            1 => self.cpu.flag_z(),
            2 => !self.cpu.flag_c(),
            3 => self.cpu.flag_c(),
            _ => unreachable!()

        }
    }

    fn execute_cb(&mut self) -> u8 {
        let cb_opcode = self.fetch_byte();
        let x = (cb_opcode >> 6) & 0x03;
        let y = (cb_opcode >> 3) & 0x07;
        let z = cb_opcode & 0x07;

        let cycles = match z {
            6 => if x == 1 { 12 } else { 16 }, // BIT (HL) takes 12, others take 16
            _ => 8,                            // Register operations take 8 cycles
        };

        match x {
            0 => { // Shifts and Rotates
                let mut val = self.get_register_by_bits(z);
                let old_carry = if self.cpu.flag_c() { 1 } else { 0 };
                let mut new_carry = false;

                val = match y {
                    0 => { // RLC
                        new_carry = (val & 0x80) != 0;
                        (val << 1) | (if new_carry { 1 } else { 0 })
                    }
                    1 => { // RRC
                        new_carry = (val & 0x01) != 0;
                        (val >> 1) | (if new_carry { 0x80 } else { 0 })
                    }
                    2 => { // RL
                        new_carry = (val & 0x80) != 0;
                        (val << 1) | old_carry
                    }
                    3 => { // RR
                        new_carry = (val & 0x01) != 0;
                        (val >> 1) | (old_carry << 7)
                    }
                    4 => { // SLA
                        new_carry = (val & 0x80) != 0;
                        val << 1
                    }
                    5 => { // SRA
                        new_carry = (val & 0x01) != 0;
                        ((val as i8) >> 1) as u8
                    }
                    6 => { // SWAP
                        ((val & 0x0F) << 4) | ((val & 0xF0) >> 4)
                    }
                    7 => { // SRL
                        new_carry = (val & 0x01) != 0;
                        val >> 1
                    }
                    _ => unreachable!(),
                };

                self.cpu.set_flag_z(val == 0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h(false);
                if y == 6 {
                    self.cpu.set_flag_c(false);
                } else {
                    self.cpu.set_flag_c(new_carry);
                }

                self.set_register_by_bits(z, val);
            }
            1 => { // BIT
                let val = self.get_register_by_bits(z);
                self.cpu.set_flag_z((val & (1 << y)) == 0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h(true);
            }
            2 => { // RES
                let mut val = self.get_register_by_bits(z);
                val &= !(1 << y);
                self.set_register_by_bits(z, val);
            }
            3 => { // SET
                let mut val = self.get_register_by_bits(z);
                val |= 1 << y;
                self.set_register_by_bits(z, val);
            }
            _ => unreachable!(),
        }

        cycles
    }



    pub(crate) fn execute (&mut self, opcode: u8) -> u8{
        match opcode{
            0x00 => 4,
            0xCB => {
                self.execute_cb()
            }
            0x47 => {self.cpu.b = self.cpu.a; 4},
            0x78 => {self.cpu.a = self.cpu.b; 4}
            0xC3 => {self.cpu.pc = self.fetch_word(); 16}
            0x21 => {
                let value = self.fetch_word();
                self.cpu.set_hl(value);
                12
            },
            0x01 => {
                let value = self.fetch_word();
                self.cpu.set_bc(value);
                12
            },
            0x11 => {
                let value = self.fetch_word();
                self.cpu.set_de(value);
                12
            },
            0x31 => {
                let value = self.fetch_word();
                self.cpu.sp = value;
                12
            },
            0x08 => {
                let data = self.cpu.sp;
                let d1 = (data>>8) as u8;
                let d2 = (data & 0x00FF) as u8;
                let addr = self.fetch_word();
                self.mmu.write(addr, d2);
                self.mmu.write((addr+1), d1);
                20
            },
            0xC5 => { // push BC
                let bc = self.cpu.bc();
                self.push_u16(bc);
                16
            },
            0xD5 => { // push DE
                let de = self.cpu.de();
                self.push_u16(de);
                16
            },
            0xE5 => { // push HL
                let hl = self.cpu.hl();
                self.push_u16(hl);
                16
            },
            0xF5 => { // push AF
                let af = self.cpu.af();
                self.push_u16(af);
                16
            },
            0xC1 => { // pop BC
                let data = self.pop_u16();
                self.cpu.set_bc(data);
                12
            },
            0xD1 => {  // pop DE
                let data = self.pop_u16();
                self.cpu.set_de(data);
                12
            },
            0xE1 => { // pop hl
                let data = self.pop_u16();
                self.cpu.set_hl(data);
                12
            },
            0xF1 => { // pop af  **special case**
                let data = self.pop_u16();
                self.cpu.set_af(data);
                12
            },
            0x76 =>{ //special case, halt
                self.cpu.halted = true;
                4
            },
            0x40..=0x7F =>{
                let src = opcode & 0b0000_0111;
                let dest = (opcode >>3) & 0b0000_0111;
                let data = self.get_register_by_bits(src);
                self.set_register_by_bits(dest, data);
                if src == 6 || dest ==6 {8} else {4}
            },
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                self.load_to_register(opcode);
                if opcode == 0x36 {12} else {8}
            },
            0xE0 => {
                let data = self.cpu.a;
                let dest = 0xFF00 + (self.fetch_byte() as u16);
                self.mmu.write(dest, data);
                12
            },
            0xF0 => {
                let src = 0xFF00 + (self.fetch_byte() as u16);
                let data = self.mmu.read(src);
                self.cpu.a = data;
                12
            },
            0xE2 => {
                let dest = 0xFF00 + (self.cpu.c as u16);
                let data = self.cpu.a;
                self.mmu.write(dest, data);
                8
            },
            0xF2 => {
                let scr = 0xFF00 + (self.cpu.c as u16);
                let data = self.mmu.read(scr);
                self.cpu.a = data;
                8
            },
            0x22 => {
                let mut hl = self.cpu.hl();
                self.mmu.write(hl, self.cpu.a);
                hl +=1;
                self.cpu.set_hl(hl);
                8
            },
            0x2A => {
                let mut hl = self.cpu.hl();
                let data = self.mmu.read(hl);
                hl += 1;
                self.cpu.a = data;
                self.cpu.set_hl(hl);
                8
            },
            0x32 => {
                let mut hl = self.cpu.hl();
                self.mmu.write(hl, self.cpu.a);
                hl -=1;
                self.cpu.set_hl(hl);
                8
            },
            0x3A =>{
                let mut hl = self.cpu.hl();
                let data = self.mmu.read(hl);
                hl -= 1;
                self.cpu.a = data;
                self.cpu.set_hl(hl);
                8
            },
            0x80..=0x87 =>{
                let a = self.cpu.a;
                let r = self.get_register_by_bits(opcode & 0b0000_0111);
                let result = a.wrapping_add(r);

                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((a & 0x0F) + (r & 0x0F) > 0x0F);
                self.cpu.set_flag_c((a as u16) + (r as u16) > 0xFF);
                self.cpu.a = result;
                if opcode == 0x86 {8} else {4}
            },
            0x8E => {
                let a = self.cpu.a;
                let r = self.get_register_by_bits(opcode & 0b0000_0111);
                let c = if self.cpu.flag_c() {1} else {0};
                let result = a.wrapping_add(r).wrapping_add(c);

                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((a & 0x0F) + (r & 0x0F) > 0x0F);
                self.cpu.set_flag_c((a as u16) + (r as u16) > 0xFF);

                self.cpu.a = result;
                8
            },
            0x88..=0x8F =>{
                let a = self.cpu.a;
                let r = self.get_register_by_bits(opcode & 0b0000_0111);
                let c = if self.cpu.flag_c() {1} else {0};
                let result = a.wrapping_add(r).wrapping_add(c);

                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((a & 0x0F) + (r & 0x0F) > 0x0F);
                self.cpu.set_flag_c((a as u16) + (r as u16) > 0xFF);

                self.cpu.a = result;
                if opcode == 0x8E {8} else {4}
            },
            0x90..=0x97 => {
                let a = self.cpu.a;
                let r = self.get_register_by_bits(opcode & 0b0000_0111);
                let result = a.wrapping_sub(r);

                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(true);
                self.cpu.set_flag_h((a & 0x0F) < (r & 0x0F));
                self.cpu.set_flag_c((a as u16) < (r as u16));
                self.cpu.a = result;
                if opcode == 0x96 {8} else {4}
            },
            0x98..=0x9F => {
                let a = self.cpu.a;
                let r = self.get_register_by_bits(opcode & 0b0000_0111);
                let c = if self.cpu.flag_c() {1} else {0};
                let result = a.wrapping_sub(r).wrapping_sub(c);

                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(true);
                self.cpu.set_flag_h((a & 0x0F) < (r & 0x0F) + c);
                self.cpu.set_flag_c((a as u16) < (r as u16) + (c as u16));
                self.cpu.a = result;
                if opcode == 0x9E {8} else {4}
            },
            0xA0..=0xA7 =>{
                let r = self.get_register_by_bits(opcode & 0b0000_0111);
                self.cpu.a &=r;
                self.cpu.set_flag_z(self.cpu.a==0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h(true);
                self.cpu.set_flag_c(false);
                if opcode == 0xA6 {8} else {4}
            },
            0xA8..=0xAF => {
                let r = self.get_register_by_bits(opcode & 0b0000_0111);
                self.cpu.a ^=r;

                self.cpu.set_flag_z(self.cpu.a ==0);
                self.cpu.set_flag_h(false);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_c(false);
                if opcode == 0xAE {8} else {4}
            },
            0xB0..=0xB7 =>{
                let r = self.get_register_by_bits(opcode & 0b0000_0111);
                self.cpu.a |=r;

                self.cpu.set_flag_z(self.cpu.a ==0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h(false);
                self.cpu.set_flag_c(false);
                if opcode == 0xB6 {8} else {4}
            },
            0xB8..=0xBF => {
                let a = self.cpu.a;
                let r = self.get_register_by_bits(opcode & 0b0000_0111);

                let result = a.wrapping_sub(r);

                self.cpu.set_flag_z(a<r);
                self.cpu.set_flag_n(true);
                self.cpu.set_flag_h((a & 0x0F) < (r & 0x0F));
                self.cpu.set_flag_c(a<r);
                if opcode == 0xBE {8} else {4}
            },
            0x09 | 0x19 | 0x29 | 0x39 => {
                let rr = self.get_pair_by_bits(opcode & 0b0000_0011);
                let hl = self.cpu.hl();
                let result = hl.wrapping_add(rr);

                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((hl & 0x0FFF) + (rr & 0x0FFF) > 0x0FFF);
                self.cpu.set_flag_c((hl as u32) + (rr as u32) > 0xFFFF);

                self.cpu.set_hl(result);
                8
            },
            0x03 | 0x13 | 0x23 | 0x33 => {
                let rr = self.get_pair_by_bits(opcode & 0b0000_0011);
                self.set_pair_by_bits(opcode & 0b0000_0011, rr.wrapping_add(1));
                8
            },
            0x0B | 0x1B | 0x2B | 0x3B => {
                let rr = self.get_pair_by_bits(opcode & 0b0000_0011);
                self.set_pair_by_bits(opcode & 0b0000_0011, rr.wrapping_sub(1));
                8
            }
            0xE8 => {
                let sp = self.cpu.sp;
                let byte = self.fetch_byte();
                let offset = byte as i8 as i16 as u16;

                self.cpu.sp = sp.wrapping_add(offset);

                self.cpu.set_flag_z(false);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((sp & 0x0F) + ((byte & 0x0F) as u16) > 0x0F);
                self.cpu.set_flag_c((sp & 0xFF) + ((byte & 0xFF) as u16) > 0xFF);
                16
            },
            0xF8 =>{
                let sp = self.cpu.sp;
                let byte = self.fetch_byte();
                let offset = byte as i8 as i16 as u16;

                self.cpu.set_hl(sp.wrapping_add(offset));

                self.cpu.set_flag_z(false);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((sp & 0x0F) + ((byte & 0x0F) as u16) > 0x0F);
                self.cpu.set_flag_c((sp & 0xFF) + ((byte & 0xFF) as u16) > 0xFF);
                12
            }
            0xC2 | 0xCA | 0xD2 | 0xDA => {
                let addr = self.fetch_word();
                if self.check_condition(opcode & 0b0000_0011) {
                    self.cpu.pc = addr;
                    // **IMPORTANT** need to add extra execution cycles or else the clock will run too slowly as going down the branch takes longer than ignoring it
                    16
                } else {12}

            },
            0x18 =>{
                let offset = self.fetch_byte() as i8;
                self.cpu.pc = self.cpu.pc.wrapping_add_signed(offset as i16);
                12
            },
            0x20 | 0x28 | 0x30 | 0x38 => {
                let offset = self.fetch_byte() as i8;
                if self.check_condition(opcode & 0b0000_0011) {
                    self.cpu.pc = self.cpu.pc.wrapping_add_signed(offset as i16);
                    12
                } else {8}
            },
            0xCD => {
                let addr = self.fetch_word();
                self.push_u16(self.cpu.pc);
                self.cpu.pc=addr;
                24
            },
            0xC4 | 0xCC | 0xD4 | 0xDC => {
                let addr = self.fetch_word();
                if self.check_condition(opcode & 0b0000_0011) {
                    self.push_u16(self.cpu.pc);
                    self.cpu.pc = addr;
                    24
                } else {12}
            },
            0xC9 => {
                self.cpu.pc = self.pop_u16();
                16
            },
            0xD9 => {
                self.cpu.pc = self.pop_u16();
                self.cpu.ime = true;
                16
            },
            0xC0 | 0xC8 | 0xD0 | 0xD8 => {
                if self.check_condition(opcode & 0b0000_0011) {
                    self.cpu.pc = self.pop_u16();
                    20
                } else {8}
            },
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                let addr = (opcode & 0x38) as u16;
                self.push_u16(self.cpu.pc);
                self.cpu.pc = addr;
                16
            },
            0xE9 => {
                self.cpu.pc = self.cpu.hl();
                4
            },
            0xF3 => {
                self.cpu.ime = false;
                self.cpu.ei_delay = false;
                4
            },
            0xFB => {
                self.cpu.ei_delay = true;
                4
            },
            0x10 => {
                let _hidden_byte = self.fetch_byte();
                self.cpu.is_stopped = true;
                4
            },
            0x2F => {
                self.cpu.a = !self.cpu.a;
                self.cpu.set_flag_n(true);
                self.cpu.set_flag_h(true);
                4
            },
            0x3F => {
                self.cpu.set_flag_h(false);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_c(!self.cpu.flag_c());
                4
            },
            0x37 => {
                self.cpu.set_flag_h(false);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_c(true);
                4
            },
            0x02 => {
                let a = self.cpu.a;
                self.mmu.write(self.cpu.bc(), a);
                8
            },
            0x12 => {
                let a = self.cpu.a;
                self.mmu.write(self.cpu.de(), a);
                8
            },
            0x0A => {
                let data = self.mmu.read(self.cpu.bc());
                self.cpu.a = data;
                8
            },
            0x1A => {
                let data = self.mmu.read(self.cpu.de());
                self.cpu.a = data;
                8
            },
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                let start = self.get_register_by_bits((opcode>>3) & 0x07);
                let result = start.wrapping_add(1);
                self.set_register_by_bits(((opcode>>3) & 0x07), result);
                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((start & 0x0F) == 0x0F);
                if opcode == 0x34 {12} else {4}
            },
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                let start = self.get_register_by_bits((opcode>>3) & 0x07);
                let result = start.wrapping_sub(1);
                self.set_register_by_bits(((opcode>>3) & 0x07), result);
                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(true);
                self.cpu.set_flag_h((start & 0x0F) == 0);
                if opcode == 0x35 {12} else {4}
            },
            0xEA => {
                let a = self.cpu.a;
                let addr = self.fetch_word();
                self.mmu.write(addr, a);
                16
            },
            0xFA => {
                let addr = self.fetch_word();
                let data = self.mmu.read(addr);
                self.cpu.a = data;
                16
            }
            _=> panic!("not implemented: {:#04X}", opcode),
        }
    }

    pub(crate) fn step (&mut self) -> u8 {
        if self.cpu.ime && ((self.mmu.interrupt_enable & self.mmu.interrupt_flag & 0x1F) !=0){
            let index = (self.mmu.interrupt_enable & self.mmu.interrupt_flag & 0x1F).trailing_zeros();
            self.cpu.ime = false;
            self.mmu.interrupt_flag &=!(1<<index);
            self.push_u16(self.cpu.pc);
            self.cpu.pc = 0x40 + (index as u16)*8;
            return 20;
        }
        if self.cpu.halted{
            if (self.mmu.interrupt_enable & self.mmu.interrupt_flag & 0x1F) !=0{
                self.cpu.halted = false;
            } else {return 4;}
        }
        let was_pending = self.cpu.ei_delay;

        let opcode = self.fetch_byte();
        let cycles =self.execute(opcode);
        if was_pending {
            self.cpu.ime = true;
            self.cpu.ei_delay = false;
        }
        cycles
    }
}