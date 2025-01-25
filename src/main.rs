mod cartridge;
mod bus;
mod cpu;
mod ppu;
mod render;
mod interrupt;
mod joypad;
mod apu;

#[rustfmt::skip]
mod cpu_test;

use bus::Bus;
use cartridge::Rom;
use apu::APU;
use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use ppu::PPU;
use cpu::CPU;
use render::frame::Frame;
use joypad::JoypadButton;
use joypad::Joypad;

use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::pixels::PixelFormat;
use sdl3::sys::pixels::SDL_PIXELFORMAT_RGB24;
use sdl3::sys::render::SDL_SetTextureScaleMode;
use sdl3::sys::surface::SDL_ScaleMode;

use std::time::{Duration, Instant};

fn get_button(keycode: &Keycode) -> Option<JoypadButton> {
    match keycode {
        &Keycode::Down => Some(JoypadButton::Down),
        &Keycode::Up => Some(JoypadButton::Up),
        &Keycode::Right => Some(JoypadButton::Right),
        &Keycode::Left => Some(JoypadButton::Left),
        &Keycode::Space => Some(JoypadButton::Select),
        &Keycode::Return => Some(JoypadButton::Start),
        &Keycode::A => Some(JoypadButton::A),
        &Keycode::S => Some(JoypadButton::B),
        _ => None,
    }
}

fn main() {
    let width: u32 = 256;
    let height: u32 = 240;
    let scale: u32 = 4;

    // init sdl3
    let sdl_context = sdl3::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("fNES", width * scale, height * scale)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas();
    let mut event_pump = sdl_context.event_pump().unwrap();
    canvas.set_scale(scale as f32, scale as f32).unwrap();

    let creator = canvas.texture_creator();
    let mut texture = unsafe {
        let t = creator
            .create_texture_target(PixelFormat::from_ll(SDL_PIXELFORMAT_RGB24), width, height)
            .unwrap();

        SDL_SetTextureScaleMode(t.raw(), SDL_ScaleMode::NEAREST);
        t
    };

    // load the game
    let bytes = std::fs::read("mb.nes").expect("failed to read ROM file");
    let rom = Rom::new(&bytes).expect("failed to initialize ROM");

    let mut frame = Frame::new(width as usize, height as usize);
    let bus = Bus::new(rom, move |ppu: &PPU, apu: &mut APU, joypad: &mut Joypad, corruption: &mut u8, ram_corruption: &mut u8| {
        render::render(ppu, &mut frame);
        texture.update(None, &frame.data, width as usize * 3).unwrap();

        canvas.copy(&texture, None, None).unwrap();
        canvas.present();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => std::process::exit(0),

                Event::KeyDown { keycode, .. } => {
                    if keycode == Some(Keycode::KpPlus) {
                        *corruption = corruption.wrapping_add(1);
                        println!("[BUS] ppu corruption at {}", corruption);
                    }
                    if keycode == Some(Keycode::KpMinus) {
                        *corruption = corruption.wrapping_sub(1);
                        println!("[BUS] ppu corruption at {}", corruption);
                    }
                    if keycode == Some(Keycode::KpMultiply) {
                        *ram_corruption = 255;
                        println!("[BUS] corrupted ram!")
                    }

                    if let Some(key) = get_button(&keycode.unwrap()) {
                        joypad.set_button_status(key, true);
                    }
                }

                Event::KeyUp { keycode, .. } => {
                    if let Some(key) = get_button(&keycode.unwrap()) {
                        joypad.set_button_status(key, false);
                    }
                }

                _ => {}
            }
        }
    });

    let buffer = bus.get_apu_buffer();
    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device available");
    let config = device.default_output_config().expect("failed to get default config");

    assert_eq!(
        config.sample_format(),
        cpal::SampleFormat::F32,
        "the audio device does not support f32 format"
    );

    let stream_config = config.into();

    // Create the audio stream
    let stream = device
        .build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buffer_lock = buffer.lock().expect("failed to lock buffer");

                for frame in data.iter_mut() {
                    // Fetch the next sample or play silence if the buffer is empty
                    *frame = buffer_lock.pop().unwrap_or(0.0);
                }
            },
            move |err| {
                eprintln!("an error occurred on the audio stream: {}", err);
            },
            None
        )
        .expect("failed to build audio stream");

    // Play the audio stream
    //stream.play().expect("failed to play stream");

    let mut cpu = CPU::new(bus);
    cpu.reset();
    //cpu.pc = 0xC000;

    // Timing constants
    let cpu_clock_hz: u32 = 1_789_773;
    let cycles_per_frame = cpu_clock_hz / 60;
    let target_frame_duration = Duration::from_secs_f64(1.0 / 60.0);

    loop {
        let frame_start = Instant::now();

        let mut cycles_executed = 0;
        while cycles_executed < cycles_per_frame {
            let cycles = cpu.step();
            cycles_executed += cycles as u32;
        }

        let elapsed = frame_start.elapsed();
        if elapsed < target_frame_duration {
            std::thread::sleep(target_frame_duration - elapsed);
        }
    }
}