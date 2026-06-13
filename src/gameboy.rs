use crate::cpu::Cpu;
use crate::mmu::Mmu;
use crate::ppu::Ppu;
use crate::ppu::Mode;

pub struct GameBoy{
    pub(crate) cpu: Cpu,
    pub(crate) mmu: Mmu,
    pub(crate) ppu: Ppu,
}
impl GameBoy{
    pub fn new(path: &str) -> GameBoy{
        GameBoy{
            cpu: Cpu::new(),
            mmu: Mmu::new(path),
            ppu: Ppu::new()
        }
    }

    pub(crate) fn tick_ppu(&mut self, cycles: u8) -> bool{
        // LCD disabled (LCDC bit 7 = 0): PPU is off, LY resets, screen is blank.
        if self.mmu.lcdc & 0b1000_0000 == 0 {
            self.mmu.ly = 0;
            self.ppu.dots = 0;
            self.ppu.mode = Mode::HBlank;
            self.mmu.stat &= 0b1111_1100;
            self.ppu.line_rendered = false;
            self.ppu.stat_line = false;
            return false;
        }

        self.ppu.dots = self.ppu.dots.wrapping_add(cycles as u32);
        let mut frame_done = false;

        if self.ppu.dots >= 456 {
            self.ppu.dots -= 456;
            self.mmu.ly += 1;
            if self.mmu.ly > 153 {
                self.mmu.ly = 0;
            }
            self.ppu.line_rendered = false;
            if self.mmu.ly == 144 {
                self.ppu.frame_count = self.ppu.frame_count.wrapping_add(1);
                frame_done = true;
                self.mmu.interrupt_flag |= 0b0000_0001; // VBlank
            }
        }

        // Determine the current mode from LY / dots.
        let mode = if self.mmu.ly >= 144 {
            Mode::VBlank
        } else if self.ppu.dots < 80 {
            Mode::OamScan
        } else if self.ppu.dots < 252 {
            Mode::Drawing
        } else {
            Mode::HBlank
        };
        self.ppu.mode = mode;
        self.mmu.stat = (self.mmu.stat & 0b1111_1100) | (mode as u8);

        // Render each visible line once, at the start of its Drawing phase, so that
        // SCX/SCY writes the game makes during the previous line's HBlank land on the
        // correct scanline (this is what makes per-line parallax scrolling work).
        if matches!(mode, Mode::Drawing) && !self.ppu.line_rendered && self.mmu.ly <= 143 {
            self.render_scanline();
            self.ppu.line_rendered = true;
        }

        // LYC == LY coincidence flag (STAT bit 2).
        let coincidence = self.mmu.ly == self.mmu.lyc;
        if coincidence {
            self.mmu.stat |= 0b0000_0100;
        } else {
            self.mmu.stat &= !0b0000_0100;
        }

        // STAT interrupt: fires on the rising edge of the OR of all enabled sources.
        let stat = self.mmu.stat;
        let stat_line =
            (stat & 0b0100_0000 != 0 && coincidence)                  // LYC == LY
            || (stat & 0b0010_0000 != 0 && matches!(mode, Mode::OamScan)) // mode 2
            || (stat & 0b0001_0000 != 0 && matches!(mode, Mode::VBlank))  // mode 1
            || (stat & 0b0000_1000 != 0 && matches!(mode, Mode::HBlank)); // mode 0
        if stat_line && !self.ppu.stat_line {
            self.mmu.interrupt_flag |= 0b0000_0010; // STAT
        }
        self.ppu.stat_line = stat_line;

        frame_done
    }

    pub(crate) fn set_buttons(&mut self, buttons: u8){
        self.mmu.buttons = buttons;
    }

    fn render_scanline(&mut self){
        let ly = self.mmu.ly;
        let lcdc = self.mmu.lcdc;
        // Raw (pre-palette) BG/window colour index per pixel, kept for sprite priority.
        let mut bg_colour = [0u8; 160];

        // ---------- Background + Window ----------
        // On DMG, LCDC bit 0 = 0 blanks the BG/window to colour 0 (and disables window).
        if lcdc & 0b0000_0001 != 0 {
            let bg_map: u16    = if lcdc & 0b0000_1000 != 0 { 0x9C00 } else { 0x9800 };
            let win_map: u16   = if lcdc & 0b0100_0000 != 0 { 0x9C00 } else { 0x9800 };
            let signed         = lcdc & 0b0001_0000 == 0; // bit4: 1 => $8000 unsigned, 0 => $9000 signed
            let window_on      = lcdc & 0b0010_0000 != 0;
            let wy = self.mmu.wy;
            let wx = self.mmu.wx;

            for screen_x in 0u8..160 {
                // Pick window or background for this pixel.
                let use_window = window_on
                    && ly >= wy
                    && (screen_x as i16) >= (wx as i16 - 7);
                let (map_base, tx, ty) = if use_window {
                    let win_x = (screen_x as i16 - (wx as i16 - 7)) as u16;
                    let win_y = (ly - wy) as u16;
                    (win_map, win_x, win_y)
                } else {
                    let map_x = (screen_x as u16 + self.mmu.scx as u16) & 0xFF;
                    let map_y = (ly as u16 + self.mmu.scy as u16) & 0xFF;
                    (bg_map, map_x, map_y)
                };

                let tile_col = tx / 8;
                let tile_row = ty / 8;
                let map_index = tile_row * 32 + tile_col;
                let tile_number = self.mmu.read(map_base + map_index);
                let tile_address: u16 = if signed {
                    (0x9000i32 + (tile_number as i8 as i32) * 16) as u16
                } else {
                    0x8000 + (tile_number as u16) * 16
                };
                let px = tx % 8;
                let py = ty % 8;
                let low_byte = self.mmu.read(tile_address + py * 2);
                let high_byte = self.mmu.read(tile_address + py * 2 + 1);
                let bit = 7 - px;
                let colour_index = (((high_byte >> bit) & 1) << 1) | ((low_byte >> bit) & 1);

                bg_colour[screen_x as usize] = colour_index;
                let shade = (self.mmu.bgp >> (colour_index * 2)) & 0b11;
                self.ppu.framebuffer[ly as usize * 160 + screen_x as usize] = shade;
            }
        } else {
            for screen_x in 0..160 {
                self.ppu.framebuffer[ly as usize * 160 + screen_x] = 0;
            }
        }

        // ---------- Sprites (OBJ) ----------
        if lcdc & 0b0000_0010 != 0 {
            let height: u8 = if lcdc & 0b0000_0100 != 0 { 16 } else { 8 };
            let ly_i = ly as i16;

            // Scan OAM in order, keep the first 10 sprites that cover this line.
            // Tuple: (x, oam_index, y, tile, attr).
            let mut line: Vec<(u8, usize, u8, u8, u8)> = Vec::new();
            for i in 0..40 {
                let base = 0xFE00 + (i as u16) * 4;
                let y = self.mmu.read(base);
                let top = y as i16 - 16;
                if ly_i >= top && ly_i < top + height as i16 {
                    let x = self.mmu.read(base + 1);
                    let tile = self.mmu.read(base + 2);
                    let attr = self.mmu.read(base + 3);
                    line.push((x, i, y, tile, attr));
                    if line.len() == 10 { break; }
                }
            }
            // DMG priority: smaller X wins; ties broken by lower OAM index.
            // Sort highest-priority first, then let `painted` block lower sprites.
            line.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

            let mut painted = [false; 160];
            for (x, _idx, y, tile, attr) in line {
                let flip_y      = attr & 0b0100_0000 != 0;
                let flip_x      = attr & 0b0010_0000 != 0;
                let palette     = if attr & 0b0001_0000 != 0 { self.mmu.obp1 } else { self.mmu.obp0 };
                let behind_bg   = attr & 0b1000_0000 != 0; // priority bit: sprite behind BG colours 1-3

                let top = y as i16 - 16;
                let mut row = (ly_i - top) as u8;
                if flip_y { row = height - 1 - row; }

                // 8x16 ignores the tile's low bit; the two stacked tiles are contiguous
                // in VRAM so a single offset formula spans rows 0..15.
                let tile_index = if height == 16 { tile & 0xFE } else { tile };
                let tile_addr = 0x8000u16 + (tile_index as u16) * 16 + (row as u16) * 2;
                let low_byte = self.mmu.read(tile_addr);
                let high_byte = self.mmu.read(tile_addr + 1);

                let sprite_x = x as i16 - 8;
                for col in 0u8..8 {
                    let screen_x = sprite_x + col as i16;
                    if screen_x < 0 || screen_x >= 160 { continue; }
                    let sx = screen_x as usize;
                    if painted[sx] { continue; } // a higher-priority sprite already owns this pixel

                    let bit = if flip_x { col } else { 7 - col };
                    let colour_index = (((high_byte >> bit) & 1) << 1) | ((low_byte >> bit) & 1);
                    if colour_index == 0 { continue; } // colour 0 is transparent

                    // This sprite claims the pixel (sprite-sprite priority resolved).
                    painted[sx] = true;
                    if !(behind_bg && bg_colour[sx] != 0) {
                        let shade = (palette >> (colour_index * 2)) & 0b11;
                        self.ppu.framebuffer[ly as usize * 160 + sx] = shade;
                    }
                }
            }
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
                hl = hl.wrapping_add(1);
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
            0x88..=0x8F =>{
                let a = self.cpu.a;
                let r = self.get_register_by_bits(opcode & 0b0000_0111);
                let c = if self.cpu.flag_c() {1} else {0};
                let result = a.wrapping_add(r).wrapping_add(c);

                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((a & 0x0F) + (r & 0x0F) + c > 0x0F);
                self.cpu.set_flag_c((a as u16) + (r as u16) + (c as u16) > 0xFF);

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

                self.cpu.set_flag_z(a==r);
                self.cpu.set_flag_n(true);
                self.cpu.set_flag_h((a & 0x0F) < (r & 0x0F));
                self.cpu.set_flag_c(a<r);
                if opcode == 0xBE {8} else {4}
            },
            0x09 | 0x19 | 0x29 | 0x39 => {
                let rr = self.get_pair_by_bits((opcode >> 4) & 0b0000_0011);
                let hl = self.cpu.hl();
                let result = hl.wrapping_add(rr);

                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((hl & 0x0FFF) + (rr & 0x0FFF) > 0x0FFF);
                self.cpu.set_flag_c((hl as u32) + (rr as u32) > 0xFFFF);

                self.cpu.set_hl(result);
                8
            },
            0x03 | 0x13 | 0x23 | 0x33 => {
                let rr = self.get_pair_by_bits((opcode>>4) & 0b0000_0011);
                self.set_pair_by_bits((opcode >>4)& 0b0000_0011, rr.wrapping_add(1));
                8
            },
            0x0B | 0x1B | 0x2B | 0x3B => {
                let rr = self.get_pair_by_bits((opcode>>4) & 0b0000_0011);
                self.set_pair_by_bits((opcode >>4) & 0b0000_0011, rr.wrapping_sub(1));
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
                if self.check_condition(opcode >> 3 & 0b0000_0011) {
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
                if self.check_condition(opcode >>3 & 0b0000_0011) {
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
                if self.check_condition(opcode >> 3 & 0b0000_0011) {
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
                if self.check_condition(opcode >> 3 & 0b0000_0011) {
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
            0xC6 =>{
                let a = self.cpu.a;
                let literal = self.fetch_byte();
                let result = a.wrapping_add(literal);
                self.cpu.a = result;
                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((a & 0x0F) + (literal & 0x0F) > 0x0F);
                self.cpu.set_flag_c((a as u16) + (literal as u16) > 0xFF);
                8
            },
            0xCE => {
                let a = self.cpu.a;
                let literal = self.fetch_byte();
                let c = if self.cpu.flag_c() {1} else {0};
                let result = a.wrapping_add(literal).wrapping_add(c);
                self.cpu.a = result;
                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h((a & 0x0F) + (literal & 0x0F) + c > 0x0F);
                self.cpu.set_flag_c((a as u16) + (literal as u16) + (c as u16) > 0xFF);
                8

            },
            0xD6 => {
                let a = self.cpu.a;
                let literal = self.fetch_byte();
                let result = a.wrapping_sub(literal);
                self.cpu.a = result;
                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(true);
                self.cpu.set_flag_h((a & 0x0F) < (literal & 0x0F));
                self.cpu.set_flag_c((a as u16) < (literal as u16));
                8
            },
            0xDE => {
                let a = self.cpu.a;
                let literal = self.fetch_byte();
                let c = if self.cpu.flag_c() {1} else {0};
                let result = a.wrapping_sub(literal).wrapping_sub(c);
                self.cpu.a = result;
                self.cpu.set_flag_z(result == 0);
                self.cpu.set_flag_n(true);
                self.cpu.set_flag_h((a & 0x0F) < (literal & 0x0F) + c);
                self.cpu.set_flag_c((a as u16) < (literal as u16) + (c as u16));
                8
            },
            0xE6 => {
                let a = self.cpu.a;
                let literal = self.fetch_byte();
                let result = (a & literal);
                self.cpu.a = result;
                self.cpu.set_flag_z(self.cpu.a==0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h(true);
                self.cpu.set_flag_c(false);
                8
            },
            0xEE => {
                let a = self.cpu.a;
                let literal = self.fetch_byte();
                let result = (a ^ literal);
                self.cpu.a = result;
                self.cpu.set_flag_z(self.cpu.a ==0);
                self.cpu.set_flag_h(false);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_c(false);
                8
            },
            0xF6 => {
                let a = self.cpu.a;
                let literal = self.fetch_byte();
                let result = (a | literal);
                self.cpu.a = result;
                self.cpu.set_flag_z(self.cpu.a ==0);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h(false);
                self.cpu.set_flag_c(false);
                8
            },
            0xFE => {
                let a = self.cpu.a;
                let literal = self.fetch_byte();

                self.cpu.set_flag_z(a==literal);
                self.cpu.set_flag_n(true);
                self.cpu.set_flag_h((a & 0x0F) < (literal & 0x0F));
                self.cpu.set_flag_c(a<literal);
                8
            },
            0xF9 => {
                self.cpu.sp = self.cpu.hl();
                8
            },
            0x27 => {
                let a = self.cpu.a;
                if !self.cpu.flag_n() {
                    if self.cpu.flag_h() || ((a & 0b0000_1111) > 9) {
                        self.cpu.a = self.cpu.a.wrapping_add(0x06);
                    }
                    if self.cpu.flag_c() || (a > 0x99){
                        self.cpu.a = self.cpu.a.wrapping_add(0x60);
                        self.cpu.set_flag_c(true);
                    }
                }
                if self.cpu.flag_n(){
                    if self.cpu.flag_h(){
                        self.cpu.a = self.cpu.a.wrapping_sub(0x06);
                    }
                    if self.cpu.flag_c(){
                        self.cpu.a = self.cpu.a.wrapping_sub(0x60);
                    }
                }
                self.cpu.set_flag_z(self.cpu.a == 0);
                self.cpu.set_flag_h(false);

                4
            },
            0x07 => {
                let mut a = self.cpu.a;
                let b7 = (a & 0b1000_0000) >>7;
                a = a<<1;
                a = a + b7;
                self.cpu.set_flag_c(b7==1);

                self.cpu.a = a;
                self.cpu.set_flag_z(false);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h(false);
                4
            },
            0x0F => {
                let mut a = self.cpu.a;
                let b7 = (a & 0b0000_0001);
                a = a>>1;
                a = a + (b7<<7);
                self.cpu.set_flag_c(b7==1);
                self.cpu.a = a;
                self.cpu.set_flag_z(false);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h(false);
                4
            },
            0x17 => {
                let mut a = self.cpu.a;
                let b7 = (a & 0b1000_0000) >>7;
                a = a<<1;
                let mut c = 0;
                if self.cpu.flag_c() == true{
                    c = 1;
                } else {
                    c = 0;
                }
                a = a + c;

                self.cpu.a = a;
                self.cpu.set_flag_c(b7==1);
                self.cpu.set_flag_z(false);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h(false);
                4
            },
            0x1F => {
                let mut a = self.cpu.a;
                let b7 = (a & 0b0000_0001) ;
                a = a>>1;
                let mut c = 0;
                if self.cpu.flag_c() == true{
                    c = 1;
                } else {
                    c = 0;
                }
                a = a + (c<<7);

                self.cpu.a = a;
                self.cpu.set_flag_c(b7==1);
                self.cpu.set_flag_z(false);
                self.cpu.set_flag_n(false);
                self.cpu.set_flag_h(false);
                4
            },
            _=> panic!("not implemented: {:#04X}", opcode),
        }
    }

    pub(crate) fn step (&mut self) -> u8 {
        if self.cpu.ime && ((self.mmu.interrupt_enable & self.mmu.interrupt_flag & 0x1F) !=0){
            let index = (self.mmu.interrupt_enable & self.mmu.interrupt_flag & 0x1F).trailing_zeros();
            self.cpu.halted = false;
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