use std::fs::File;
use std::io::Read;

static mut PALETTE: Option<Vec<u8>> = None;

pub fn set_palette(filename: &str) -> Result<(), std::io::Error> {
    let mut file = File::open(filename)?;
    let mut palette = Vec::new();
    file.read_to_end(&mut palette)?;

    // set the global palette
    unsafe {
        PALETTE = Some(palette);
    }
    
    Ok(())
}

#[allow(static_mut_refs)]
pub fn get_colour(in_byte: u8, emphasis: (bool, bool, bool), greyscale: bool) -> (u8, u8, u8) {
    let mut byte: usize = in_byte as usize;

    let palette = unsafe {
        PALETTE.as_ref().expect("[PALETTE] palette not loaded!")
    };

    // apply emphasis
    if emphasis.0 { byte |= 0x040; }
    if emphasis.1 { byte |= 0x080; }
    if emphasis.2 { byte |= 0x100; }

    // fetch colour
    let rgb: (u8, u8, u8) = (palette[byte * 3], palette[byte * 3 + 1], palette[byte * 3 + 2]);

    // apply greyscale
    if greyscale {
        let luminance: u8 = ((rgb.0 as u16 * 30 + rgb.1 as u16 * 59 + rgb.2 as u16 * 11) / 100) as u8;
        return (luminance, luminance, luminance);
    }

    rgb
}