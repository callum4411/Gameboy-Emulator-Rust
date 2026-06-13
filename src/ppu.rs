use crate::ppu::Mode::OamScan;

#[derive(Copy, Clone)]
pub(crate) enum Mode {
    OamScan = 2,
    Drawing = 3,
    HBlank = 0,
    VBlank = 1,
}

pub(crate) struct Ppu {
    pub(crate) mode: Mode,
    pub(crate) dots: u32,
    pub(crate) framebuffer: [u8;160*144],
    pub(crate) frame_count: u32,
}

impl Ppu {
    pub fn new() -> Ppu{
        Ppu {
            mode: OamScan,
            dots: 0,
            framebuffer: [0;160*144],
            frame_count: 0,
        }
    }
}