use std::io::Read;
use crate::hardware::map::*;
use crate::hardware::ppu::PPU_COLOR_COUNT;

const NES_HEADER_SIZE: usize = 16;
const TRAINER_SIZE: usize = 0x0200;

#[derive(Copy, Clone, Debug)]
pub enum NESFileVersion {
    Version1,
    Version2,
}

#[derive(Clone, Debug)]
pub struct NESFileHeader {
    pub version: NESFileVersion,
    pub mapper_number: usize,
    pub submapper_number: usize,
    pub nametable_mirroring: NametableMirroring,
    pub uses_custom_nametables: bool,
    pub has_backup_battery: bool,
    pub has_trainer: bool,
    pub prg_rom_chunks: usize,
    pub chr_rom_chunks: usize,
    pub prg_ram_size: usize,
    pub chr_ram_size: usize,
}

pub struct Cartridge {
    header: NESFileHeader,
    mapper: Box<dyn Mapper>,
    trainer: Option<Box<[u8; TRAINER_SIZE]>>,
}

impl Cartridge {
    pub fn parse_nes(reader: &mut impl Read) -> Result<Self, String> {
        let mut header_data = [0_u8; NES_HEADER_SIZE];
        reader.read_exact(&mut header_data).map_err(|err| err.to_string())?;

        if &header_data[0 .. 4] != b"NES\x1A" {
            return Err("NES file format error.".to_string());
        }

        let header = NESFileHeader {
            version: match header_data[7] & 0b00001100 {
                0b00001000 => NESFileVersion::Version2,
                _ => NESFileVersion::Version1
            },
            mapper_number: (header_data[6] >> 4) as usize,
            submapper_number: 0,
            nametable_mirroring: if header_data[6] & 0b00000001 != 0 {
                NametableMirroring::Horizontal
            } else {
                NametableMirroring::Vertical
            },
            uses_custom_nametables: header_data[6] & 0b00001000 != 0,
            has_backup_battery: header_data[6] & 0b00000010 != 0,
            has_trainer: header_data[6] & 0b00000100 != 0,
            prg_rom_chunks: header_data[4] as usize,
            chr_rom_chunks: header_data[5] as usize,
            prg_ram_size: 0,
            chr_ram_size: 0,
        };

        let trainer = header.has_trainer
            .then(|| {
                let mut trainer = Box::new([0; TRAINER_SIZE]);
                reader.read_exact(trainer.as_mut_slice()).map_err(|err| err.to_string())?;
                Ok(trainer)
            })
            // Icky code to turn Option<Result> into Result<Option>
            .map_or(Ok(None), |result: Result<_, String>| result.map(Some))?;

        let mut prg_rom = Vec::with_capacity(header.prg_rom_chunks);
        for chunk in 0 .. header.prg_rom_chunks {
            prg_rom.push(Box::new([0; PRG_CHUNK_SIZE]));
            reader.read_exact(prg_rom[chunk].as_mut_slice()).map_err(|err| err.to_string())?;
        }

        let mut chr_rom = Vec::with_capacity(header.chr_rom_chunks);
        for chunk in 0 .. header.chr_rom_chunks {
            chr_rom.push(Box::new([0; CHR_CHUNK_SIZE]));
            reader.read_exact(chr_rom[chunk].as_mut_slice()).map_err(|err| err.to_string())?;
        }

        let mapper = initialize_mapper(&header, prg_rom, chr_rom)?;

        Ok(Self {
            header,
            mapper,
            trainer,
        })
    }
    
    pub fn header(&self) -> &NESFileHeader {
        &self.header
    }

    pub fn read_cpu_byte(&self, address: u16) -> u8 {
        if let Some(trainer) = &self.trainer {
            if address >= 0x7000 && address < 0x7200 {
                return trainer[(address & 0x1FF) as usize];
            }
        }
        
        self.mapper.read_cpu_byte(address)
    }

    pub fn write_cpu_byte(&mut self, address: u16, value: u8) {
        self.mapper.write_cpu_byte(address, value)
    }

    pub fn read_ppu_byte(&self, address: u16) -> u8 {
        self.mapper.read_ppu_byte(address & 0x3FFF)
    }

    pub fn write_ppu_byte(&mut self, address: u16, value: u8) {
        self.mapper.write_ppu_byte(address & 0x3FFF, value)
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
