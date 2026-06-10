pub struct Cpu{
    pub(crate) a: u8,
    pub(crate) b: u8,
    pub(crate) c: u8,
    pub(crate) d: u8,
    pub(crate) e: u8,
    pub(crate) f: u8,
    pub(crate) h: u8,
    pub(crate) l: u8,
    pub(crate) sp: u16,
    pub(crate) pc: u16,
    pub(crate) halted: bool,
}

 impl Cpu{
     pub fn new() -> Cpu{
         Cpu {
             a: 0,
             b: 0,
             c: 0,
             d: 0,
             e: 0,
             f: 0,
             h: 0,
             l: 0,
             sp: 0,
             pc: 0,
             halted: false,
         }

     }

     pub(crate) fn hl(&self) -> u16{
         ((self.h as u16) << 8) | (self.l as u16)
     }pub(crate) fn set_hl(&mut self, value: u16){
         self.h = (value >> 8) as u8;
         self.l = (value & 0x00FF) as u8;
     }

     pub(crate) fn bc(&self) -> u16{
         ((self.b as u16) << 8) | (self.c as u16)
     }pub(crate) fn set_bc(&mut self, value: u16){
         self.b = (value >> 8) as u8;
         self.c = (value & 0x00FF) as u8;
     }

     pub(crate) fn af(&self) -> u16{
         ((self.a as u16) << 8) | (self.f as u16)
     }pub(crate) fn set_af(&mut self, value: u16){
         self.a = (value >> 8) as u8;
         self.f = (value & 0x00F0) as u8;
     }

     pub(crate) fn de(&self) -> u16{
         ((self.d as u16) << 8) | (self.e as u16)
     }pub(crate) fn set_de(&mut self, value: u16){
         self.d = (value >> 8) as u8;
         self.e = (value & 0x00FF) as u8;
     }

     pub(crate) fn flag_z(&self) -> bool {
         (self.f & 0b1000_0000) != 0
     }
     pub(crate) fn set_flag_z(&mut self, flag: bool){
         if flag {
             self.f |= 0b1000_0000;
         } else {
             self.f &= 0b0111_1111;
         }
     }

     pub(crate) fn flag_n(&self) -> bool {
         (self.f & 0b0100_0000) != 0
     }
     pub(crate) fn set_flag_n(&mut self, flag: bool){
         if flag {
             self.f |= 0b0100_0000;
         } else {
             self.f &= 0b1011_1111;
         }
     }

     pub(crate) fn flag_h(&self) -> bool {
         (self.f & 0b0010_0000) != 0
     }
     pub(crate) fn set_flag_h(&mut self, flag: bool){
         if flag {
             self.f |= 0b0010_0000;
         } else {
             self.f &= 0b1101_1111;
         }
     }

     pub(crate) fn flag_c(&self) -> bool {
         (self.f & 0b0001_0000) != 0
     }
     pub(crate) fn set_flag_c(&mut self, flag: bool){
         if flag {
             self.f |= 0b0001_0000;
         } else {
             self.f &= 0b1110_1111;
         }
     }
 }