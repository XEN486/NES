mod cartridge;
mod bus;
mod cpu;
mod ppu;
mod render;
mod interrupt;
mod joypad;
mod apu;
mod mapper;

use bus::Bus;
use cartridge::Rom;
use apu::APU;
use cpal::traits::{HostTrait, DeviceTrait};
use ppu::PPU;
use cpu::CPU;
use render::frame::Frame;
use joypad::{Joypad, JoypadButton};

use sdl3::{event::Event, keyboard::Keycode, pixels::PixelFormat};
use sdl3::sys::{pixels::SDL_PIXELFORMAT_RGB24, render::SDL_SetTextureScaleMode, surface::SDL_ScaleMode};

use rfd::FileDialog;

use std::time::{Duration, Instant};
use std::env;

fn get_button(keycode: &Keycode) -> Option<JoypadButton> {
    match keycode {
        Keycode::Down => Some(JoypadButton::Down),
        Keycode::Up => Some(JoypadButton::Up),
        Keycode::Right => Some(JoypadButton::Right),
        Keycode::Left => Some(JoypadButton::Left),
        Keycode::Space => Some(JoypadButton::Select),
        Keycode::Return => Some(JoypadButton::Start),
        Keycode::A => Some(JoypadButton::A),
        Keycode::S => Some(JoypadButton::B),
        _ => None,
    }
}

fn main() {
    // constants
    const WIDTH: u32 = 256;
    const HEIGHT: u32 = 240;
    const SCALE: u32 = 4;

    // cpu timing
    let mut cpu_clock_hz: u32 = 1_789_773;
    let mut target_fps: u32 = 60;

    // collect args and filter out known flags
    let args: Vec<String> = env::args().collect();
    let mut rom_path: Option<String> = None;

    for arg in &args[1..] {
        match arg.as_str() {
            "--pal" => {
                cpu_clock_hz = 1_662_607;
                target_fps = 50;
                println!(
                    "[MAIN] Using PAL timing ({:.6} MHz, {} FPS)", 
                    cpu_clock_hz as f64 / 1_000_000.0, 
                    target_fps
                );
            }
            "--ntsc" => {
                println!(
                    "[MAIN] Using NTSC timing ({:.6} MHz, {} FPS)", 
                    cpu_clock_hz as f64 / 1_000_000.0, 
                    target_fps
                );
            }
            "--help" => {
                println!(
                    "Arguments:\n  --pal        Use PAL timing for the CPU\n  --ntsc       Use NTSC timing for the CPU\n"
                );
                std::process::exit(0);
            }
            _ => {
                if rom_path.is_none() {
                    rom_path = Some(arg.clone());
                }
            }
        }
    }

    // if no ROM path was provided, open file dialog
    let rom_path = rom_path.unwrap_or_else(|| {
        FileDialog::new()
            .add_filter("iNES ROMs", &["nes"])
            .pick_file()
            .expect("[MAIN] failed to open file dialog")
            .to_str()
            .unwrap()
            .to_string()
    });

    // initialize SDL3
    let sdl_context = sdl3::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("pNES", WIDTH * SCALE, HEIGHT * SCALE)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas();
    let mut event_pump = sdl_context.event_pump().unwrap();
    canvas.set_scale(SCALE as f32, SCALE as f32).unwrap();

    let creator = canvas.texture_creator();
    let mut texture = unsafe {
        let t = creator
            .create_texture_target(PixelFormat::from_ll(SDL_PIXELFORMAT_RGB24), WIDTH, HEIGHT)
            .unwrap();
        SDL_SetTextureScaleMode(t.raw(), SDL_ScaleMode::NEAREST);
        t
    };

    // load the game
    let rom_bytes = std::fs::read(rom_path).expect("[MAIN] failed to read ROM file");
    let rom = Rom::new(&rom_bytes).expect("[MAIN] failed to initialize ROM");
    let mut frame = Frame::new(WIDTH as usize, HEIGHT as usize);

    let bus = Bus::new(rom, move |ppu: &PPU, _apu: &mut APU, joypad: &mut Joypad, corruption: &mut u8| {
        render::render(ppu, &mut frame);
        texture.update(None, &frame.data, WIDTH as usize * 3).unwrap();
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
                    if let Some(key) = keycode {
                        match key {
                            Keycode::KpPlus => {
                                *corruption = corruption.wrapping_add(1);
                                println!("[MAIN] PPU corruption at {}", corruption);
                            }
                            Keycode::KpMinus => {
                                *corruption = corruption.wrapping_sub(1);
                                println!("[MAIN] PPU corruption at {}", corruption);
                            }
                            _ => {
                                if let Some(button) = get_button(&key) {
                                    joypad.set_button_status(button, true);
                                }
                            }
                        }
                    }
                }

                Event::KeyUp { keycode, .. } => {
                    if let Some(key) = keycode {
                        if let Some(button) = get_button(&key) {
                            joypad.set_button_status(button, false);
                        }
                    }
                }

                _ => {}
            }
        }
    });

    // setup audio
    let buffer = bus.get_apu_buffer();
    let host = cpal::default_host();
    let device = host.default_output_device().expect("[MAIN] no output device available");
    let config = device.default_output_config().expect("[MAIN] failed to get default config");

    assert_eq!(
        config.sample_format(),
        cpal::SampleFormat::F32,
        "[MAIN] the audio device does not support f32 format"
    );

    let stream_config = config.into();

    let _stream = device
        .build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buffer_lock = buffer.lock().expect("[MAIN] failed to lock buffer");
                for frame in data.iter_mut() {
                    *frame = buffer_lock.pop().unwrap_or(0.0);
                }
            },
            move |err| {
                eprintln!("[MAIN] audio stream error: {}", err);
            },
            None,
        )
        .expect("[MAIN] failed to build audio stream");

    // setup CPU
    let mut cpu = CPU::new(bus);
    cpu.reset();

    // setup timing
    let target_frame_duration: Duration = Duration::from_secs_f64(1.0 / target_fps as f64);
    let cycles_per_frame: u32 = cpu_clock_hz / 60;

    // main loop
    loop {
        let frame_start = Instant::now();
        let mut cycles_executed = 0;

        while cycles_executed < cycles_per_frame {
            cycles_executed += cpu.step() as u32;
        }

        let elapsed = frame_start.elapsed();
        if elapsed < target_frame_duration {
            std::thread::sleep(target_frame_duration - elapsed);
        }
    }
}
