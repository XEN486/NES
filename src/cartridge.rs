#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mirroring {
    Vertical,
    Horizontal,
    FourScreen,
}

pub struct Rom {
    pub prg_rom: Vec<u8>,
    pub chr_rom: Vec<u8>,
    pub mapper: u8,
    pub mirroring: Mirroring,
}

impl Rom {
    pub fn new(raw: &Vec<u8>) -> Result<Rom, String> {
        if &raw[0..4] != [0x4E, 0x45, 0x53, 0x1A] {
            return Err("[CART] only iNES roms are supported!".to_string());
        }

        let mapper = (raw[7] & 0b1111_0000) | (raw[6] >> 4);
        let version = (raw[7] >> 2) & 0b11;
        
        let four_screen = raw[6] & 0b1000 != 0;
        let vertical_mirroring = raw[6] & 0b1 != 0;
        let mirroring = match (four_screen, vertical_mirroring) {
            (true, _) => Mirroring::FourScreen,
            (false, true) => Mirroring::Vertical,
            (false, false) => Mirroring::Horizontal,
        };

        if version != 0 {
            println!("[CARTRIDGE] iNES 2.0 detected. Unsupported, but some games should work. Using: Mapper {}, {:?} Mirroring", mapper, mirroring);
            //return Err("[CART] only iNES 1.0 roms are supported!".to_string());
        } else {
            println!("[CARTRIDGE] iNES 1.0. Using: Mapper {}, {:?} mirroring", mapper, mirroring);
        }


        let prg_rom_size = raw[4] as usize * 16384;
        let chr_rom_size = raw[5] as usize * 8192;
        
        let skip_trainer = raw[6] & 0b100 != 0;

        let prg_rom_start = 16 + if skip_trainer {512} else {0};
        let chr_rom_start = prg_rom_start + prg_rom_size;

        Ok(Rom {
            prg_rom: raw[prg_rom_start..(prg_rom_start + prg_rom_size)].to_vec(),
            chr_rom: raw[chr_rom_start..(chr_rom_start + chr_rom_size)].to_vec(),
            mapper: mapper,
            mirroring: mirroring,
        })
    }
}