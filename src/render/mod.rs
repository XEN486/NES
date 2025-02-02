pub mod frame;
pub mod palette;
pub mod rect;

use crate::cartridge::Mirroring;
use crate::ppu::PPU;
use frame::Frame;
use rect::Rect;

fn bg_palette(ppu: &PPU, attribute_table: &[u8], tile_column: usize, tile_row: usize) -> [u8; 4] {
    let attr_table_idx = (tile_row / 4) * 8 + (tile_column / 4);
    let attr_byte = attribute_table[attr_table_idx]; 

    let palette_idx = match (tile_column % 4 / 2, tile_row % 4 / 2) {
        (0, 0) => attr_byte & 0b11,
        (1, 0) => (attr_byte >> 2) & 0b11,
        (0, 1) => (attr_byte >> 4) & 0b11,
        (1, 1) => (attr_byte >> 6) & 0b11,
        (_, _) => unreachable!("[PPU] impossible background palette"),
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

fn render_nametable(ppu: &PPU, frame: &mut Frame, nametable: &[u8], viewport: Rect, shift_x: isize, shift_y: isize) {
    let bank = ppu.control.background_pattern_address();

    let attribute_table = &nametable[0x3c0 .. 0x400];

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
                let rgb = match value {
                    0 => palette::get_colour(ppu.palette_table[0] % 64, ppu.mask.emphasis(), ppu.mask.is_greyscale()),
                    1 => palette::get_colour(palette[1] % 64, ppu.mask.emphasis(), ppu.mask.is_greyscale()),
                    2 => palette::get_colour(palette[2] % 64, ppu.mask.emphasis(), ppu.mask.is_greyscale()),
                    3 => palette::get_colour(palette[3] % 64, ppu.mask.emphasis(), ppu.mask.is_greyscale()),
                    _ => unreachable!("[PPU] impossible palette colour"),
                };

                let pixel_x = tile_column * 8 + x;
                let pixel_y = tile_row * 8 + y;

                // leftmost 8 pixel background
                let screen_x = (shift_x + pixel_x as isize) as usize;
                if screen_x < 8 && !ppu.mask.leftmost_8pixel_background() {
                    continue;
                }

                if pixel_x >= viewport.x1 && pixel_x < viewport.x2 && pixel_y >= viewport.y1 && pixel_y < viewport.y2 {
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
            unimplemented!("[PPU] unsupported mirroring type {:?}", ppu.mirroring);
        }
    };

    // only show the background if it's enabled in the mask register
    if ppu.mask.show_background() {
        // main nametable
        render_nametable(
            ppu,
            frame, 
            main_nametable, 
            Rect::new(scroll_x, scroll_y, 256, 240 ),
            -(scroll_x as isize), -(scroll_y as isize)
        );

        // screen 2
        if scroll_x > 0 {
            render_nametable(
                ppu,
                frame, 
                second_nametable, 
                Rect::new(0, 0, scroll_x, 240),
                (256 - scroll_x) as isize, 0
            );
        } else if scroll_y > 0 {
            render_nametable(
                ppu,
                frame, 
                second_nametable, 
                Rect::new(0, 0, 256, scroll_y),
                0, (240 - scroll_y) as isize
            );
        }
    }

    // don't show sprites if they are disabled in the mask register
    if !ppu.mask.show_sprites() {
        return;
    }

    for i in (0..ppu.oam_data.len()).step_by(4).rev() {
        let tile_idx = ppu.oam_data[i + 1] as u16;
        let tile_x = ppu.oam_data[i + 3] as usize;
        let tile_y = ppu.oam_data[i] as usize + 1;
    
        // skip leftmost 8-pixel sprite if disabled
        if tile_x < 8 && !ppu.mask.leftmost_8pixel_sprite() {
            continue;
        }
    
        let flip_vertical = ppu.oam_data[i + 2] >> 7 & 1 == 1;
        let flip_horizontal = ppu.oam_data[i + 2] >> 6 & 1 == 1;
        let palette_idx = ppu.oam_data[i + 2] & 0b11;
        let sprite_palette = sprite_palette(ppu, palette_idx);
        let bank: u16 = ppu.control.sprite_pattern_address();
    
        let sprite_height = ppu.control.sprite_size();
        let tile = if sprite_height == 16 {
            // 8x16 sprite
            let bank = tile_idx & 1;
            &ppu.chr_rom[((bank as u16 * 0x1000) + (tile_idx & 0xFE) as u16 * 16) as usize
                ..=((bank as u16 * 0x1000) + (tile_idx & 0xFE) as u16 * 16 + 31) as usize]
        } else {
            // 8x8 sprite
            &ppu.chr_rom[(bank + tile_idx * 16) as usize..=(bank + tile_idx * 16 + 15) as usize]
        };
    
        for y in 0..sprite_height as usize {
            // for 8x16 sprite, split into upper (0..7) and lower (8..15)
            let mut upper = if y < 8 { tile[y] } else { tile[y - 8] };
            let mut lower = if y < 8 { tile[y + 8] } else { tile[y] };
        
            'pixel: for x in (0..=7).rev() {
                let value = (1 & lower) << 1 | (1 & upper);
                upper = upper >> 1;
                lower = lower >> 1;
        
                // skip transparent pixels
                if value == 0 {
                    continue 'pixel;
                }
        
                let rgb = match value {
                    1 => palette::get_colour(sprite_palette[1] % 64, ppu.mask.emphasis(), ppu.mask.is_greyscale()),
                    2 => palette::get_colour(sprite_palette[2] % 64, ppu.mask.emphasis(), ppu.mask.is_greyscale()),
                    3 => palette::get_colour(sprite_palette[3] % 64, ppu.mask.emphasis(), ppu.mask.is_greyscale()),
                    _ => unreachable!("[PPU] impossible palette colour"),
                };
        
                let (sprite_pixel_x, sprite_pixel_y) = match (flip_horizontal, flip_vertical) {
                    (false, false) => (tile_x + x, tile_y + y),
                    (true, false) => (tile_x + 7 - x, tile_y + y),
                    (false, true) => (tile_x + x, tile_y + 7 - y),
                    (true, true) => (tile_x + 7 - x, tile_y + 7 - y),
                };
        
                // check if the sprite pixel is within the frame bounds
                if sprite_pixel_x >= 256 || sprite_pixel_y >= 240 {
                    continue 'pixel;
                }
        
                // check sprite priority
                let priority = ppu.oam_data[i + 2] >> 5 & 1 == 0; // priority bit (0 = front, 1 = behind)
                let bg_pixel = frame.get_pixel(sprite_pixel_x, sprite_pixel_y);
                if priority || bg_pixel == palette::get_colour(ppu.palette_table[0] % 64, ppu.mask.emphasis(), ppu.mask.is_greyscale()) {
                    frame.set_pixel(sprite_pixel_x, sprite_pixel_y, rgb);
                }
            }
        }
    }        
}