use std::io::Read;
use crate::hardware::ppu::PPU_COLOR_COUNT;

const PRG_BANK_SIZE: usize = 0x4000;
const CHR_BANK_SIZE: usize = 0x2000;
const NAMETABLE_SIZE: usize = 0x0400;

pub struct Cartridge {
    pub mapper_number: u8,
    pub nametable_arrangement: bool, // false: horizontal mirroring, true: vertical mirroring
    pub uses_alt_nametable_layout: bool,
    pub has_battery: bool,
    pub prg_rom: Vec<Box<[u8; PRG_BANK_SIZE]>>,
    pub chr_rom: Vec<Box<[u8; CHR_BANK_SIZE]>>,
    nametables: [Box<[u8; NAMETABLE_SIZE]>; 2],
}

impl Cartridge {
    pub fn parse_ines(reader: &mut impl Read) -> Result<Self, String> {
        let mut header = [0_u8; 16];
        reader.read_exact(&mut header).map_err(|err| err.to_string())?;

        if &header[0 ..= 3] != b"NES\x1A" {
            return Err("NES file format error".to_string());
        }

        let prg_rom_banks = header[4] as usize;
        let chr_rom_banks = header[5] as usize;
        if prg_rom_banks > 16 || chr_rom_banks > 32 {
            return Err("ROM size limit exceeded".to_string());
        }

        let mapper_number = header[6] >> 4;
        if mapper_number != 0 {
            return Err("Unsupported mapper".to_string());
        }

        let mut cartridge = Cartridge {
            mapper_number,
            nametable_arrangement: header[6] & 0x01 != 0,
            uses_alt_nametable_layout: header[6] & 0x08 != 0,
            has_battery: header[6] & 0x02 != 0,
            prg_rom: Vec::with_capacity(prg_rom_banks),
            chr_rom: Vec::with_capacity(chr_rom_banks),
            nametables: [Box::new([0; NAMETABLE_SIZE]), Box::new([0; NAMETABLE_SIZE])],
        };

        for bank in 0 .. prg_rom_banks {
            cartridge.prg_rom.push(Box::new([0; PRG_BANK_SIZE]));
            reader.read_exact(cartridge.prg_rom[bank].as_mut_slice()).map_err(|err| err.to_string())?;
        }
        for bank in 0 .. chr_rom_banks {
            cartridge.chr_rom.push(Box::new([0; CHR_BANK_SIZE]));
            reader.read_exact(cartridge.chr_rom[bank].as_mut_slice()).map_err(|err| err.to_string())?;
        }

        Ok(cartridge)
    }

    pub fn read_cpu_byte(&self, address: u16) -> u8 {
        // Temporary, should be deferred to a polymorphic mapper object
        match address {
            0x8000 ..= 0xBFFF => {
                self.prg_rom[0][(address & 0x3FFF) as usize]
            }
            0xC000 ..= 0xFFFF => {
                self.prg_rom[1][(address & 0x3FFF) as usize]
            }
            _ => crate::hardware::OPEN_BUS
        }
    }

    pub fn write_cpu_byte(&mut self, _address: u16, _value: u8) {
        // Temporary, should be deferred to a polymorphic mapper object
        // For mapper 0, this is just a no-op since there's no PRG-RAM
    }

    pub fn read_ppu_byte(&self, address: u16) -> u8 {
        // Temporary, should be deferred to a polymorphic mapper object
        match address & 0x3FFF {
            0x0000 ..= 0x1FFF => {
                self.chr_rom[0][address as usize]
            }
            _ => {
                let index = self.get_nametable_index(address);
                self.nametables[index][(address & 0x03FF) as usize]
            }
        }
    }

    pub fn write_ppu_byte(&mut self, address: u16, value: u8) {
        // Temporary, should be deferred to a polymorphic mapper object
        match address & 0x3FFF {
            0x0000 ..= 0x1FFF => {
                // For mapper 0, this is just a no-op since there's no CHR-RAM
            }
            _ => {
                let index = self.get_nametable_index(address);
                self.nametables[index][(address & 0x03FF) as usize] = value;
            }
        }
    }

    fn get_nametable_index(&self, address: u16) -> usize {
        if self.nametable_arrangement {
            ((address >> 10) & 1) as usize
        }
        else {
            ((address >> 11) & 1) as usize
        }
    }
}

pub struct ColorConverter {
    table: Box<[u32; PPU_COLOR_COUNT * 8]>,
}

impl ColorConverter {
    pub fn new() -> Self {
        Self {
            table: Box::new([0_u32; PPU_COLOR_COUNT * 8]),
        }
    }
    
    pub fn parse_pal(&mut self, reader: &mut impl Read) -> Result<(), String> {
        for color in self.table.as_mut_slice() {
            let mut rgb = [0_u8; 3];
            if let Err(err) = reader.read_exact(&mut rgb) {
                if let std::io::ErrorKind::UnexpectedEof = err.kind() {
                    for section in 1 .. 8 {
                        self.table.copy_within(0 .. PPU_COLOR_COUNT, section * PPU_COLOR_COUNT);
                    }
                    break;
                }
                return Err(err.to_string());
            }
            
            *color = (rgb[0] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[2] as u32;
        }
        
        Ok(())
    }
    
    pub fn get_rgb(&self, index: u16) -> u32 {
        self.table[(index & 0b111_111111) as usize]
    }
}
