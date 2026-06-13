use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// Host sample rate the APU produces at. platform.rs must open its audio device at
// this rate (it's the only contract between the APU and the OS-specific audio code).
pub const SAMPLE_RATE: u32 = 48000;

const CPU_CLOCK: f32 = 4_194_304.0;
const CYCLES_PER_SAMPLE: f32 = CPU_CLOCK / SAMPLE_RATE as f32;

// Square-wave duty patterns (12.5%, 25%, 50%, 75%).
const DUTY: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

// Noise channel frequency divisors.
const NOISE_DIV: [u32; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

// Read-back OR masks for NR10..NR25 (write-only bits read back as 1).
const READ_MASK: [u8; 22] = [
    0x80, 0x3F, 0x00, 0xFF, 0xBF, // NR10..NR14
    0xFF, 0x3F, 0x00, 0xFF, 0xBF, // NR15(unused)..NR24
    0x7F, 0xFF, 0x9F, 0xFF, 0xBF, // NR30..NR34
    0xFF, 0xFF, 0x00, 0x00, 0xBF, // NR1F(unused)..NR44
    0x00, 0x00,                   // NR50, NR51
];

// ---------------------------------------------------------------------------
// Square channel (CH1 has a frequency sweep, CH2 does not).
// ---------------------------------------------------------------------------
struct Square {
    has_sweep: bool,
    enabled: bool,
    dac_enabled: bool,

    duty: u8,
    duty_pos: usize,
    freq: u16,
    freq_timer: i32,

    length_counter: u16,
    length_enabled: bool,

    env_initial: u8,
    env_dir_up: bool,
    env_period: u8,
    volume: u8,
    env_timer: u8,

    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_timer: u8,
    sweep_enabled: bool,
    sweep_shadow: u16,
}

impl Square {
    fn new(has_sweep: bool) -> Square {
        Square {
            has_sweep,
            enabled: false,
            dac_enabled: false,
            duty: 0,
            duty_pos: 0,
            freq: 0,
            freq_timer: 0,
            length_counter: 0,
            length_enabled: false,
            env_initial: 0,
            env_dir_up: false,
            env_period: 0,
            volume: 0,
            env_timer: 0,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_timer: 0,
            sweep_enabled: false,
            sweep_shadow: 0,
        }
    }

    fn tick(&mut self) {
        self.freq_timer -= 1;
        if self.freq_timer <= 0 {
            self.freq_timer = (2048 - self.freq as i32) * 4;
            self.duty_pos = (self.duty_pos + 1) % 8;
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if self.env_period == 0 {
            return;
        }
        if self.env_timer > 0 {
            self.env_timer -= 1;
        }
        if self.env_timer == 0 {
            self.env_timer = self.env_period;
            if self.env_dir_up && self.volume < 15 {
                self.volume += 1;
            } else if !self.env_dir_up && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }

    fn calc_sweep(&mut self) -> u16 {
        let delta = self.sweep_shadow >> self.sweep_shift;
        let new = if self.sweep_negate {
            self.sweep_shadow.wrapping_sub(delta)
        } else {
            self.sweep_shadow + delta
        };
        if new > 2047 {
            self.enabled = false;
        }
        new
    }

    fn clock_sweep(&mut self) {
        if !self.has_sweep {
            return;
        }
        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }
        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_period > 0 { self.sweep_period } else { 8 };
            if self.sweep_enabled && self.sweep_period > 0 {
                let new = self.calc_sweep();
                if new <= 2047 && self.sweep_shift > 0 {
                    self.sweep_shadow = new;
                    self.freq = new;
                    self.calc_sweep(); // second overflow check
                }
            }
        }
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.freq_timer = (2048 - self.freq as i32) * 4;
        self.env_timer = self.env_period;
        self.volume = self.env_initial;

        if self.has_sweep {
            self.sweep_shadow = self.freq;
            self.sweep_timer = if self.sweep_period > 0 { self.sweep_period } else { 8 };
            self.sweep_enabled = self.sweep_period > 0 || self.sweep_shift > 0;
            if self.sweep_shift > 0 {
                self.calc_sweep();
            }
        }
    }

    fn sample(&self) -> f32 {
        if !self.enabled || !self.dac_enabled {
            return 0.0;
        }
        let on = DUTY[self.duty as usize][self.duty_pos];
        let digital = if on == 1 { self.volume } else { 0 };
        (digital as f32 / 7.5) - 1.0
    }
}

// ---------------------------------------------------------------------------
// Wave channel (CH3).
// ---------------------------------------------------------------------------
struct Wave {
    enabled: bool,
    dac_enabled: bool,
    length_counter: u16,
    length_enabled: bool,
    volume_code: u8,
    freq: u16,
    freq_timer: i32,
    position: usize,
    sample_buffer: u8,
    wave_ram: [u8; 16],
}

impl Wave {
    fn new() -> Wave {
        Wave {
            enabled: false,
            dac_enabled: false,
            length_counter: 0,
            length_enabled: false,
            volume_code: 0,
            freq: 0,
            freq_timer: 0,
            position: 0,
            sample_buffer: 0,
            wave_ram: [0; 16],
        }
    }

    fn tick(&mut self) {
        self.freq_timer -= 1;
        if self.freq_timer <= 0 {
            self.freq_timer = (2048 - self.freq as i32) * 2;
            self.position = (self.position + 1) % 32;
            let byte = self.wave_ram[self.position / 2];
            self.sample_buffer = if self.position % 2 == 0 {
                byte >> 4
            } else {
                byte & 0x0F
            };
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 256;
        }
        self.freq_timer = (2048 - self.freq as i32) * 2;
        self.position = 0;
    }

    fn sample(&self) -> f32 {
        if !self.enabled || !self.dac_enabled {
            return 0.0;
        }
        let digital = match self.volume_code {
            0 => 0,
            1 => self.sample_buffer,
            2 => self.sample_buffer >> 1,
            _ => self.sample_buffer >> 2,
        };
        (digital as f32 / 7.5) - 1.0
    }
}

// ---------------------------------------------------------------------------
// Noise channel (CH4).
// ---------------------------------------------------------------------------
struct Noise {
    enabled: bool,
    dac_enabled: bool,
    length_counter: u16,
    length_enabled: bool,
    env_initial: u8,
    env_dir_up: bool,
    env_period: u8,
    volume: u8,
    env_timer: u8,
    clock_shift: u8,
    width_mode: bool,
    divisor_code: u8,
    freq_timer: i32,
    lfsr: u16,
}

impl Noise {
    fn new() -> Noise {
        Noise {
            enabled: false,
            dac_enabled: false,
            length_counter: 0,
            length_enabled: false,
            env_initial: 0,
            env_dir_up: false,
            env_period: 0,
            volume: 0,
            env_timer: 0,
            clock_shift: 0,
            width_mode: false,
            divisor_code: 0,
            freq_timer: 0,
            lfsr: 0x7FFF,
        }
    }

    fn period(&self) -> i32 {
        (NOISE_DIV[self.divisor_code as usize] << self.clock_shift) as i32
    }

    fn tick(&mut self) {
        self.freq_timer -= 1;
        if self.freq_timer <= 0 {
            self.freq_timer = self.period();
            let xor = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
            self.lfsr >>= 1;
            self.lfsr |= xor << 14;
            if self.width_mode {
                self.lfsr &= !(1 << 6);
                self.lfsr |= xor << 6;
            }
        }
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }

    fn clock_envelope(&mut self) {
        if self.env_period == 0 {
            return;
        }
        if self.env_timer > 0 {
            self.env_timer -= 1;
        }
        if self.env_timer == 0 {
            self.env_timer = self.env_period;
            if self.env_dir_up && self.volume < 15 {
                self.volume += 1;
            } else if !self.env_dir_up && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }

    fn trigger(&mut self) {
        self.enabled = self.dac_enabled;
        if self.length_counter == 0 {
            self.length_counter = 64;
        }
        self.freq_timer = self.period();
        self.lfsr = 0x7FFF;
        self.env_timer = self.env_period;
        self.volume = self.env_initial;
    }

    fn sample(&self) -> f32 {
        if !self.enabled || !self.dac_enabled {
            return 0.0;
        }
        let digital = if self.lfsr & 1 == 0 { self.volume } else { 0 };
        (digital as f32 / 7.5) - 1.0
    }
}

// ---------------------------------------------------------------------------
// APU: frame sequencer, mixing, register file, sample generation.
// ---------------------------------------------------------------------------
pub struct Apu {
    enabled: bool,
    nr50: u8,
    nr51: u8,
    regs: [u8; 22], // raw NR10..NR25 bytes for read-back

    ch1: Square,
    ch2: Square,
    ch3: Wave,
    ch4: Noise,

    fs_counter: u32,
    fs_step: u8,
    sample_timer: f32,

    buffer: Arc<Mutex<VecDeque<f32>>>,
}

impl Apu {
    pub fn new() -> Apu {
        Apu {
            enabled: false,
            nr50: 0,
            nr51: 0,
            regs: [0; 22],
            ch1: Square::new(true),
            ch2: Square::new(false),
            ch3: Wave::new(),
            ch4: Noise::new(),
            fs_counter: 0,
            fs_step: 0,
            sample_timer: CYCLES_PER_SAMPLE,
            buffer: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    // Shared sample sink; platform.rs takes a clone to feed the OS audio device.
    pub fn buffer(&self) -> Arc<Mutex<VecDeque<f32>>> {
        self.buffer.clone()
    }

    pub fn step(&mut self, cycles: u32) {
        for _ in 0..cycles {
            self.ch1.tick();
            self.ch2.tick();
            self.ch3.tick();
            self.ch4.tick();

            self.fs_counter += 1;
            if self.fs_counter >= 8192 {
                self.fs_counter -= 8192;
                self.frame_sequencer();
            }

            self.sample_timer -= 1.0;
            if self.sample_timer <= 0.0 {
                self.sample_timer += CYCLES_PER_SAMPLE;
                self.generate_sample();
            }
        }
    }

    fn frame_sequencer(&mut self) {
        match self.fs_step {
            0 => self.clock_length(),
            2 => {
                self.clock_length();
                self.ch1.clock_sweep();
            }
            4 => self.clock_length(),
            6 => {
                self.clock_length();
                self.ch1.clock_sweep();
            }
            7 => {
                self.ch1.clock_envelope();
                self.ch2.clock_envelope();
                self.ch4.clock_envelope();
            }
            _ => {}
        }
        self.fs_step = (self.fs_step + 1) % 8;
    }

    fn clock_length(&mut self) {
        self.ch1.clock_length();
        self.ch2.clock_length();
        self.ch3.clock_length();
        self.ch4.clock_length();
    }

    fn generate_sample(&mut self) {
        let (mut left, mut right) = (0.0f32, 0.0f32);

        if self.enabled {
            let s = [
                self.ch1.sample(),
                self.ch2.sample(),
                self.ch3.sample(),
                self.ch4.sample(),
            ];
            for (i, v) in s.iter().enumerate() {
                if self.nr51 & (1 << (i + 4)) != 0 {
                    left += v;
                }
                if self.nr51 & (1 << i) != 0 {
                    right += v;
                }
            }
            left /= 4.0;
            right /= 4.0;

            let lvol = ((self.nr50 >> 4) & 7) as f32 / 7.0;
            let rvol = (self.nr50 & 7) as f32 / 7.0;
            left *= lvol;
            right *= rvol;
        }

        let mut buf = self.buffer.lock().unwrap();
        // Cap latency: drop samples if the consumer is behind (~0.25s of stereo).
        let max = (SAMPLE_RATE as usize / 4) * 2;
        if buf.len() < max {
            buf.push_back(left);
            buf.push_back(right);
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF10..=0xFF25 => {
                let i = (addr - 0xFF10) as usize;
                self.regs[i] | READ_MASK[i]
            }
            0xFF26 => {
                let mut v = 0x70;
                if self.enabled {
                    v |= 0x80;
                }
                if self.ch1.enabled {
                    v |= 0x01;
                }
                if self.ch2.enabled {
                    v |= 0x02;
                }
                if self.ch3.enabled {
                    v |= 0x04;
                }
                if self.ch4.enabled {
                    v |= 0x08;
                }
                v
            }
            0xFF30..=0xFF3F => self.ch3.wave_ram[(addr - 0xFF30) as usize],
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        // When powered off, only NR52 and wave RAM are writable.
        if !self.enabled && addr != 0xFF26 && !(0xFF30..=0xFF3F).contains(&addr) {
            return;
        }

        match addr {
            0xFF10..=0xFF25 => {
                self.regs[(addr - 0xFF10) as usize] = data;
                self.decode(addr, data);
            }
            0xFF26 => {
                let en = data & 0x80 != 0;
                if !en && self.enabled {
                    self.power_off();
                }
                self.enabled = en;
            }
            0xFF30..=0xFF3F => self.ch3.wave_ram[(addr - 0xFF30) as usize] = data,
            _ => {}
        }
    }

    fn decode(&mut self, addr: u16, data: u8) {
        match addr {
            // ---- CH1 ----
            0xFF10 => {
                self.ch1.sweep_period = (data >> 4) & 7;
                self.ch1.sweep_negate = data & 0x08 != 0;
                self.ch1.sweep_shift = data & 7;
            }
            0xFF11 => {
                self.ch1.duty = data >> 6;
                self.ch1.length_counter = 64 - (data & 0x3F) as u16;
            }
            0xFF12 => {
                self.ch1.env_initial = data >> 4;
                self.ch1.env_dir_up = data & 0x08 != 0;
                self.ch1.env_period = data & 7;
                self.ch1.dac_enabled = data & 0xF8 != 0;
                if !self.ch1.dac_enabled {
                    self.ch1.enabled = false;
                }
            }
            0xFF13 => self.ch1.freq = (self.ch1.freq & 0x700) | data as u16,
            0xFF14 => {
                self.ch1.freq = (self.ch1.freq & 0xFF) | (((data & 7) as u16) << 8);
                self.ch1.length_enabled = data & 0x40 != 0;
                if data & 0x80 != 0 {
                    self.ch1.trigger();
                }
            }
            // ---- CH2 ----
            0xFF16 => {
                self.ch2.duty = data >> 6;
                self.ch2.length_counter = 64 - (data & 0x3F) as u16;
            }
            0xFF17 => {
                self.ch2.env_initial = data >> 4;
                self.ch2.env_dir_up = data & 0x08 != 0;
                self.ch2.env_period = data & 7;
                self.ch2.dac_enabled = data & 0xF8 != 0;
                if !self.ch2.dac_enabled {
                    self.ch2.enabled = false;
                }
            }
            0xFF18 => self.ch2.freq = (self.ch2.freq & 0x700) | data as u16,
            0xFF19 => {
                self.ch2.freq = (self.ch2.freq & 0xFF) | (((data & 7) as u16) << 8);
                self.ch2.length_enabled = data & 0x40 != 0;
                if data & 0x80 != 0 {
                    self.ch2.trigger();
                }
            }
            // ---- CH3 ----
            0xFF1A => {
                self.ch3.dac_enabled = data & 0x80 != 0;
                if !self.ch3.dac_enabled {
                    self.ch3.enabled = false;
                }
            }
            0xFF1B => self.ch3.length_counter = 256 - data as u16,
            0xFF1C => self.ch3.volume_code = (data >> 5) & 3,
            0xFF1D => self.ch3.freq = (self.ch3.freq & 0x700) | data as u16,
            0xFF1E => {
                self.ch3.freq = (self.ch3.freq & 0xFF) | (((data & 7) as u16) << 8);
                self.ch3.length_enabled = data & 0x40 != 0;
                if data & 0x80 != 0 {
                    self.ch3.trigger();
                }
            }
            // ---- CH4 ----
            0xFF20 => self.ch4.length_counter = 64 - (data & 0x3F) as u16,
            0xFF21 => {
                self.ch4.env_initial = data >> 4;
                self.ch4.env_dir_up = data & 0x08 != 0;
                self.ch4.env_period = data & 7;
                self.ch4.dac_enabled = data & 0xF8 != 0;
                if !self.ch4.dac_enabled {
                    self.ch4.enabled = false;
                }
            }
            0xFF22 => {
                self.ch4.clock_shift = data >> 4;
                self.ch4.width_mode = data & 0x08 != 0;
                self.ch4.divisor_code = data & 7;
            }
            0xFF23 => {
                self.ch4.length_enabled = data & 0x40 != 0;
                if data & 0x80 != 0 {
                    self.ch4.trigger();
                }
            }
            // ---- Control ----
            0xFF24 => self.nr50 = data,
            0xFF25 => self.nr51 = data,
            _ => {}
        }
    }

    fn power_off(&mut self) {
        let wave = self.ch3.wave_ram; // wave RAM is preserved across power-off
        self.ch1 = Square::new(true);
        self.ch2 = Square::new(false);
        self.ch3 = Wave::new();
        self.ch3.wave_ram = wave;
        self.ch4 = Noise::new();
        self.nr50 = 0;
        self.nr51 = 0;
        self.regs = [0; 22];
        self.fs_step = 0;
    }
}
