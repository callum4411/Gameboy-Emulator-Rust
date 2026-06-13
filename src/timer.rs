pub(crate) struct Timer {
    pub(crate) div: u8,
    pub(crate) tima: u8,
    pub(crate) tma: u8,
    pub(crate) tac: u8,
    pub(crate) tima_counter: u32,
    pub(crate) div_counter: u32,
}

impl Timer {
    pub(crate) fn new() -> Timer {
        Timer {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            tima_counter: 0,
            div_counter: 0,
        }
    }

    pub(crate) fn process_cycles(&mut self, cycles: u8)-> bool {

        self.div_counter += (cycles as u32);
        let mut overflowed = false;
        while self.div_counter >= 256{
            self.div_counter -= 256;
            self.div =self.div.wrapping_add(1);
        }
        let mut n:u32 = 0;
        n = match self.tac & 0b0000_0011{
            0b01 => 16,
            0b10 => 64,
            0b11 => 256,
            0b00 => 1024,
            _ => unreachable!("tac register to N doesnt match")
        };
        if self.tac & 0b100 !=0 {
            self.tima_counter += (cycles as u32);
            while (self.tima_counter) >= n {
                self.tima_counter -= n;
                if self.tima == 0xFF {
                    self.tima = self.tma;
                    overflowed = true;
                } else {
                    self.tima += 1
                }
            }
        }
        overflowed

    }
}