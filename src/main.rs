mod cartridge;
mod bus;
mod cpu;
mod ppu;
mod render;
mod interrupt;
mod joypad;

use bus::Bus;
use cartridge::Rom;
use cpu::CPU;
use render::frame::Frame;
use joypad::JoypadButton;
use joypad::Joypad;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

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
    // init sdl2
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("NES", (256.0 * 3.0) as u32, (240.0 * 3.0) as u32)
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
    let bytes: Vec<u8> = std::fs::read("pacman.nes").unwrap();
    let rom = Rom::new(&bytes).unwrap();

    let mut frame = Frame::new();
    let bus = Bus::new(rom, move |ppu, joypad: &mut Joypad| {
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
    cpu.run(false);
}