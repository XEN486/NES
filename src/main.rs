mod cartridge;
mod bus;
mod cpu;

use bus::Bus;
use cartridge::Rom;
use bus::Mem;
use cpu::CPU;
use rand::Rng;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::pixels::PixelFormatEnum;
use sdl2::EventPump;

fn color(byte: u8) -> Color {
    match byte {
        0 => sdl2::pixels::Color::BLACK,
        1 => sdl2::pixels::Color::WHITE,
        2 | 9 => sdl2::pixels::Color::GREY,
        3 | 10 => sdl2::pixels::Color::RED,
        4 | 11 => sdl2::pixels::Color::GREEN,
        5 | 12 => sdl2::pixels::Color::BLUE,
        6 | 13 => sdl2::pixels::Color::MAGENTA,
        7 | 14 => sdl2::pixels::Color::YELLOW,
        _ => sdl2::pixels::Color::CYAN,
    }
}

fn set_screen_state(cpu: &CPU, frame: &mut [u8; 32 * 3 * 32]) -> bool {
    let mut frame_index = 0;
    let mut update = false;

    for i in 0x0200..0x600 {
        let color_index = cpu.mem_read(i as u16);
        let (b1, b2, b3) = color(color_index).rgb();

        if frame[frame_index] != b1 || frame[frame_index + 1] != b2 || frame[frame_index + 2] != b3 {
            frame[frame_index] = b1;
            frame[frame_index + 1] = b2;
            frame[frame_index + 2] = b3;
            update = true;
        }

        frame_index += 3;
    }
    update
}

fn handle_inputs(cpu: &mut CPU, event_pump: &mut EventPump) {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => std::process::exit(0),

            Event::KeyDown {
                keycode: Some(Keycode::W),
                ..
            } => {
                cpu.mem_write(0xff, 0x77);
            }
            
            Event::KeyDown {
                keycode: Some(Keycode::S),
                ..
            } => {
                cpu.mem_write(0xff, 0x73);
            }

            Event::KeyDown {
                keycode: Some(Keycode::A),
                ..
            } => {
                cpu.mem_write(0xff, 0x61);
            }

            Event::KeyDown {
                keycode: Some(Keycode::D),
                ..
            } => {
                cpu.mem_write(0xff, 0x64);
            }
            _ => {}
        }
    }
}

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("NES", (32.0 * 30.0) as u32, (32.0 * 30.0) as u32)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().present_vsync().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();
    canvas.set_scale(30.0, 30.0).unwrap();

    let creator = canvas.texture_creator();
    let mut texture = creator
        .create_texture_target(PixelFormatEnum::RGB24, 32, 32)
        .unwrap();

    let game: Vec<u8> = std::fs::read("snake.nes").unwrap();
    let rom = Rom::new(&game).unwrap();

    let bus = Bus::new(rom);
    let mut cpu = CPU::new(bus);
    cpu.reset();

    let mut screen_state = [0 as u8; 32 * 3 * 32];

    cpu.run_with_callback(move |cpu| {
        handle_inputs(cpu, &mut event_pump);

        cpu.mem_write(0xfe, rand::thread_rng().gen_range(1, 16));

        if set_screen_state(cpu, &mut screen_state) {
            texture.update(None, &screen_state, 32 * 3).unwrap();

            canvas.copy(&texture, None, None).unwrap();

            canvas.present();
        }

        //std::thread::sleep(std::time::Duration::new(0, 70_000));
    });
}