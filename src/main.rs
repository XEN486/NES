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
use cpu::CPU;
use render::frame::Frame;
use joypad::JoypadButton;

use sdl3::{event::Event, keyboard::Keycode, pixels::PixelFormat};
use sdl3::sys::{pixels::SDL_PIXELFORMAT_RGB24, render::SDL_SetTextureScaleMode, surface::SDL_ScaleMode};

use rfd::FileDialog;

use std::time::{Duration, Instant};
use std::env;

use std::fs::File;
use std::io::Write;

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

fn main() -> Result<(), std::io::Error> {
    // constants
    const WIDTH: u32 = 256;
    const HEIGHT: u32 = 240;
    const SCALE: u32 = 4;

    // cpu timing
    let mut cpu_clock_hz: u32 = 1_789_773;
    let mut target_fps: u32 = 60;

    // collect args
    let args: Vec<String> = env::args().collect();
    let mut rom_path: Option<String> = None;
    let mut pal_path: Option<String> = None;
    let mut trace_flag: bool = false;
    let mut pc_start: Option<String> = None;
    let mut end_brk: bool = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--palette" => {
                if i + 1 < args.len() {
                    pal_path = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    panic!("Error: --palette requires a palette file path");
                }
            }

            "--pc" => {
                if i + 1 < args.len() {
                    pc_start = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    panic!("Error: --pc requires a hex value");
                }
            }

            "--trace" => {
                println!("[MAIN] Enabled trace mode");
                trace_flag = true;
            }

            "--endonbrk" => {
                end_brk = true;
            }

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
                    "Arguments:\n  --palette <path>    Uses the custom palette at <path>\n  --pal               Use PAL timing for the CPU\n  --ntsc              Use NTSC timing for the CPU\n  --pc <start>        Start the CPU with PC set to <start>\n  --trace             Trace the instructions the CPU executes\n  --endonbrk          Ends the emulator on a BRK instruction\n  --help              Show this help message\n"
                );
                std::process::exit(0);
            }

            _ => {
                if rom_path.is_none() {
                    rom_path = Some(args[i].clone());
                }
            }
        }
        i += 1;
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

    // load palette
    let pal_path = pal_path.unwrap_or_else(|| "default.pal".to_string());

    if let Err(e) = render::palette::set_palette(&pal_path) {
        eprintln!("[MAIN] failed to set palette from '{}': {}", pal_path, e);
        return Err(e);
    }

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

    // setup fps counter
    let mut fps_counter = 0;
    let mut last_fps_update = Instant::now();

    let bus = Bus::new(rom, move |frame, _ppu, _apu, joypad, corruption| {
        fps_counter += 1;
    
        texture.update(None, &frame.data, WIDTH as usize * 3).unwrap();
        canvas.copy(&texture, None, None).unwrap();
        canvas.present();
    
        let now = Instant::now();
        if now.duration_since(last_fps_update) >= Duration::from_secs(1) {
            canvas.window_mut().set_title(&format!("pNES - FPS: {}", fps_counter)).unwrap();
            fps_counter = 0;
            last_fps_update = now;
        }

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

    // setup CPU
    let mut cpu = CPU::new(bus);
    cpu.reset();

    if pc_start.is_some() {
        cpu.pc = u16::from_str_radix(&pc_start.unwrap(), 16).expect("[MAIN] invalid hex string passed to --pc")
    }

    // setup timing
    let target_frame_duration: Duration = Duration::from_secs_f64(1.0 / target_fps as f64);
    let cycles_per_frame: u32 = cpu_clock_hz / 60;

    // create a file if we are tracing the cpu
    let mut file = File::create("cpu.log")?;

    // main loop
    loop {
        let frame_start = Instant::now();
        let mut cycles_executed = 0;

        while cycles_executed < cycles_per_frame {
            if trace_flag {
                writeln!(file, "{}", cpu.trace())?;
            }

            let (cycles, opcode) = cpu.step();
            cycles_executed += cycles as u32;

            // end on break if the endbreak flag is set
            if opcode == 0x00 && end_brk {
                return Ok(());
            }
        }

        let elapsed = frame_start.elapsed();
        if elapsed < target_frame_duration {
            std::thread::sleep(target_frame_duration - elapsed);
        }
    }
}
