pub mod frame;
pub mod palette;
pub mod rect;

use crate::cartridge::Mirroring;
use crate::ppu::PPU;
use frame::Frame;
use rect::Rect;

fn bg_palette(ppu: &PPU, attribute_table: &[u8], tile_column: usize, tile_row: usize) -> [u8; 4] {
    let attr_table_idx = tile_row / 4 * 8 + tile_column / 4;
    let attr_byte = attribute_table[attr_table_idx]; 

    let palette_idx = match (tile_column % 4 / 2, tile_row % 4 / 2) {
        (0, 0) => attr_byte & 0b11,
        (1, 0) => (attr_byte >> 2) & 0b11,
        (0, 1) => (attr_byte >> 4) & 0b11,
        (1, 1) => (attr_byte >> 6) & 0b11,
        (_, _) => panic!("[PPU] shouldn't happen"),
    };

    let palette_start: usize = 1 + (palette_idx as usize) * 4;
    [
        ppu.palette_table[0],
        ppu.palette_table[palette_start],
        ppu.palette_table[palette_start + 1],
        ppu.palette_table[palette_start + 2],
    ]
}


fn sprite_palette(ppu: &PPU, palette_idx: u8) -> [u8; 4] {
    let start = 0x11 + (palette_idx * 4) as usize;
    [
        0,
        ppu.palette_table[start],
        ppu.palette_table[start + 1],
        ppu.palette_table[start + 2],
    ]
}

fn render_nametable(ppu: &PPU, frame: &mut Frame, nametable: &[u8], 
    view_port: Rect, shift_x: isize, shift_y: isize) {
    let bank = ppu.control.background_pattern_address();

    let attribute_table = &nametable[0x3c0.. 0x400];

    for i in 0..0x3c0 {
        let tile_column = i % 32;
        let tile_row = i / 32;
        let tile_idx = nametable[i] as u16;
        let tile = &ppu.chr_rom[(bank + tile_idx * 16) as usize..=(bank + tile_idx * 16 + 15) as usize];
        let palette = bg_palette(ppu, attribute_table, tile_column, tile_row);

        for y in 0..=7 {
            let mut upper = tile[y];
            let mut lower = tile[y + 8];

            for x in (0..=7).rev() {
                let value = (1 & lower) << 1 | (1 & upper);
                upper = upper >> 1;
                lower = lower >> 1;
                let mut rgb = match value {
                    0 => palette::SYSTEM_PALETTE[(ppu.palette_table[0] % 64) as usize],
                    1 => palette::SYSTEM_PALETTE[(palette[1] % 64) as usize],
                    2 => palette::SYSTEM_PALETTE[(palette[2] % 64) as usize],
                    3 => palette::SYSTEM_PALETTE[(palette[3] % 64) as usize],
                    _ => panic!("[PPU] shouldn't happen"),
                };

                frame.apply_emphasis(&mut rgb, ppu.mask.emphasize());
                frame.apply_greyscale(&mut rgb, ppu.mask.is_greyscale());

                let pixel_x = tile_column * 8 + x;
                let pixel_y = tile_row * 8 + y;

                if pixel_x >= view_port.x1 && pixel_x < view_port.x2 && pixel_y >= view_port.y1 && pixel_y < view_port.y2 {
                    frame.set_pixel((shift_x + pixel_x as isize) as usize, (shift_y + pixel_y as isize) as usize, rgb);
                }
            }
        }
    }
}

pub fn render(ppu: &PPU, frame: &mut Frame) {
    let scroll_x = (ppu.scroll.scroll_x) as usize;
    let scroll_y = (ppu.scroll.scroll_y) as usize;

    let (main_nametable, second_nametable) = match (&ppu.mirroring, ppu.control.nametable_address()) {
        (Mirroring::Vertical, 0x2000) | (Mirroring::Vertical, 0x2800) | (Mirroring::Horizontal, 0x2000) | (Mirroring::Horizontal, 0x2400) => {
            (&ppu.vram[0..0x400], &ppu.vram[0x400..0x800])
        }
        (Mirroring::Vertical, 0x2400) | (Mirroring::Vertical, 0x2C00) | (Mirroring::Horizontal, 0x2800) | (Mirroring::Horizontal, 0x2C00) => {
            ( &ppu.vram[0x400..0x800], &ppu.vram[0..0x400])
        }
        (_,_) => {
            panic!("[PPU] unsupported mirroring type {:?}", ppu.mirroring);
        }
    };

    render_nametable(ppu, frame, 
        main_nametable, 
        Rect::new(scroll_x, scroll_y, frame.width, frame.height ),
        -(scroll_x as isize), -(scroll_y as isize)
    );
    if scroll_x > 0 {
        render_nametable(ppu, frame, 
            second_nametable, 
            Rect::new(0, 0, scroll_x, frame.height),
            (frame.width - scroll_x) as isize, 0
        );
    } else if scroll_y > 0 {
        render_nametable(ppu, frame, 
            second_nametable, 
            Rect::new(0, 0, frame.width, scroll_y),
            0, (frame.height - scroll_y) as isize
        );
    }

    for i in (0..ppu.oam_data.len()).step_by(4).rev() {
        let tile_idx = ppu.oam_data[i + 1] as u16;
        let tile_x = ppu.oam_data[i + 3] as usize;
        let tile_y = ppu.oam_data[i] as usize;

        let flip_vertical = if ppu.oam_data[i + 2] >> 7 & 1 == 1 {
            true
        } else {
            false
        };

        let flip_horizontal = if ppu.oam_data[i + 2] >> 6 & 1 == 1 {
            true
        } else {
            false
        };

        let palette_idx = ppu.oam_data[i + 2] & 0b11;
        let sprite_palette = sprite_palette(ppu, palette_idx);
        let bank: u16 = ppu.control.sprite_pattern_address();

        let tile =
            &ppu.chr_rom[(bank + tile_idx * 16) as usize..=(bank + tile_idx * 16 + 15) as usize];

        for y in 0..=7 {
            let mut upper = tile[y];
            let mut lower = tile[y + 8];
            'pixel: for x in (0..=7).rev() {
                let value = (1 & lower) << 1 | (1 & upper);
                upper = upper >> 1;
                lower = lower >> 1;
                let mut rgb = match value {
                    0 => continue 'pixel, // skip drawing pixel
                    1 => palette::SYSTEM_PALETTE[(sprite_palette[1] % 64) as usize],
                    2 => palette::SYSTEM_PALETTE[(sprite_palette[2] % 64) as usize],
                    3 => palette::SYSTEM_PALETTE[(sprite_palette[3] % 64) as usize],
                    _ => panic!("[RENDER] impossible pixel"),
                };

                frame.apply_emphasis(&mut rgb, ppu.mask.emphasize());
                frame.apply_greyscale(&mut rgb, ppu.mask.is_greyscale());

                match (flip_horizontal, flip_vertical) {
                    (false, false) => {
                        frame.set_pixel(tile_x + x , tile_y + y, rgb);
                    },
                    (true, false) => {
                        frame.set_pixel(tile_x + 7 - x , tile_y + y , rgb);
                    }
                    (false, true) => {
                        frame.set_pixel(tile_x + x  , tile_y + 7 - y, rgb);
                    }
                    (true, true) => {
                        frame.set_pixel(tile_x + 7 - x , tile_y + 7 - y , rgb);
                    }
                }
            }
        }
    }
}