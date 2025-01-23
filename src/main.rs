mod cartridge;
mod bus;
mod cpu;
mod ppu;
mod render;
mod interrupt;
mod joypad;
mod apu;
mod cpu_test;

use bus::Bus;
use cartridge::Rom;
use cpu::CPU;
use render::frame::Frame;
use joypad::JoypadButton;
use joypad::Joypad;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
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
    // init sdl2
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("XeNES", (256.0 * 3.0) as u32, (240.0 * 3.0) as u32)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().present_vsync().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();
    canvas.set_scale(3.0, 3.0).unwrap();

    let creator = canvas.texture_creator();
    let mut texture = creator
        .create_texture_target(PixelFormatEnum::RGB24, 256, 240)
        .unwrap();

    // load the game
    let bytes: Vec<u8> = std::fs::read("dk.nes").unwrap();
    let rom = Rom::new(&bytes).unwrap();
    let mut frame = Frame::new();

    let bus = Bus::new(rom, move |ppu, apu, joypad: &mut Joypad, corruption: &mut u8, ram_corruption: &mut u8| {
        render::render(ppu, &mut frame);
        texture.update(None, &frame.data, 256 * 3).unwrap();

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