use minifb::{Window, WindowOptions, Key, Scale};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::apu::SAMPLE_RATE;

pub(crate) struct Platform {
    window: Window,
    buffer: [u32; 160*144],
    // Kept alive for the program's lifetime; dropping it stops audio playback.
    _audio_stream: Option<cpal::Stream>,
}

impl Platform {
    pub(crate) fn new() -> Platform {
        let mut window = Window::new(
            "Game Boy",
            160,
            144,
            WindowOptions { scale: Scale::X2, ..WindowOptions::default() },
        ).unwrap();
        window.set_target_fps(60);
        Platform { window, buffer: [0; 160 * 144], _audio_stream: None }
    }

    // Open the OS audio device and stream samples out of the shared APU buffer.
    // All cpal/platform-audio specifics live here; the APU never sees them.
    pub(crate) fn init_audio(&mut self, samples: Arc<Mutex<VecDeque<f32>>>) {
        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => { eprintln!("no audio output device found; running without sound"); return; }
        };

        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device.build_output_stream(
            &config,
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buf = samples.lock().unwrap();
                for s in out.iter_mut() {
                    // Underrun -> output silence rather than glitching.
                    *s = buf.pop_front().unwrap_or(0.0);
                }
            },
            |err| eprintln!("audio stream error: {}", err),
            None,
        );

        match stream {
            Ok(s) => {
                if let Err(e) = s.play() {
                    eprintln!("failed to start audio stream: {}", e);
                } else {
                    self._audio_stream = Some(s);
                }
            }
            Err(e) => eprintln!("failed to build audio stream ({}); running without sound", e),
        }
    }

    pub(crate) fn render(&mut self, framebuffer: &[u8;160*144]) {
        for i in 0..framebuffer.len() {
            self.buffer[i] = match framebuffer[i] {
                0 => 0xFFFFFF,
                1 => 0xAAAAAA,
                2 => 0x555555,
                3 => 0x000000,
                _ => 0xFF0000,
            };
        }
        self.window.update_with_buffer(&self.buffer, 160, 144).unwrap();
    }

    pub(crate) fn get_input(&self) -> u8 {
        let mut byte: u8 = 0;
        if self.window.is_key_down(Key::Right) { byte |= 0b0000_0001; }
        if self.window.is_key_down(Key::Left)  { byte |= 0b0000_0010; }
        if self.window.is_key_down(Key::Up)    { byte |= 0b0000_0100; }
        if self.window.is_key_down(Key::Down)  { byte |= 0b0000_1000; }
        if self.window.is_key_down(Key::Z)     { byte |= 0b0001_0000; }  // A
        if self.window.is_key_down(Key::X)     { byte |= 0b0010_0000; }  // B
        if self.window.is_key_down(Key::RightShift) { byte |= 0b0100_0000; }  // Select
        if self.window.is_key_down(Key::Enter) { byte |= 0b1000_0000; }  // Start
        byte
    }

    pub(crate) fn is_open(&self) -> bool {
        self.window.is_open() && !self.window.is_key_down(Key::Escape)
    }
}