pub struct Cpu{
    pub(crate) a: u8,
    pub(crate) b: u8,
    c: u8,
    d: u8,
    e: u8,
    f: u8,
    h: u8,
    l: u8,
    sp: u16,
    pub(crate) pc: u16,
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
         }

     }
 }