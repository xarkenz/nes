use std::io::Read;
use mapper::*;

pub mod mapper;

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
    pub nametable_arrangement: NametableArrangement,
    pub uses_custom_nametables: bool,
    pub has_backup_battery: bool,
    pub has_trainer: bool,
    pub prg_rom_chunks: usize,
    pub chr_rom_chunks: usize,
    pub prg_ram_size: usize,
    pub chr_ram_size: usize,
}

pub struct Cartridge {
    name: String,
    header: NESFileHeader,
    mapper: Box<dyn Mapper>,
    trainer: Option<Box<[u8; TRAINER_SIZE]>>,
}

impl Cartridge {
    pub fn parse_nes(name: String, reader: &mut impl Read) -> Result<Self, String> {
        let mut header_data = [0_u8; NES_HEADER_SIZE];
        reader.read_exact(&mut header_data).map_err(|err| err.to_string())?;

        if &header_data[0 .. 4] != b"NES\x1A" {
            return Err("NES file format error.".to_string());
        }

        let mut header = NESFileHeader {
            version: match header_data[7] & 0b0000_11_00 {
                0b0000_10_00 => NESFileVersion::Version2,
                _ => NESFileVersion::Version1
            },
            mapper_number: (header_data[6] as usize) >> 4,
            submapper_number: 0,
            nametable_arrangement: if header_data[6] & 0b00000001 == 0 {
                NametableArrangement::Vertical
            } else {
                NametableArrangement::Horizontal
            },
            uses_custom_nametables: header_data[6] & 0b00001000 != 0,
            has_backup_battery: header_data[6] & 0b00000010 != 0,
            has_trainer: header_data[6] & 0b00000100 != 0,
            prg_rom_chunks: header_data[4] as usize,
            chr_rom_chunks: header_data[5] as usize,
            prg_ram_size: 0,
            chr_ram_size: 0,
        };

        match header.version {
            NESFileVersion::Version1 => {}
            NESFileVersion::Version2 => {
                header.mapper_number |= (header_data[7] as usize) >> 4 << 4;
                header.mapper_number |= (header_data[8] as usize & 0b1111) << 8;
                header.submapper_number = (header_data[8] as usize) >> 4;
            }
        }

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
            name,
            header,
            mapper,
            trainer,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn header(&self) -> &NESFileHeader {
        &self.header
    }

    pub fn tick(&mut self) {
        self.mapper.tick();
    }

    pub fn check_irq(&mut self) -> bool {
        self.mapper.check_irq()
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

    pub fn read_ppu_byte(&mut self, address: u16) -> u8 {
        self.mapper.read_ppu_byte(address & 0x3FFF)
    }

    pub fn write_ppu_byte(&mut self, address: u16, value: u8) {
        self.mapper.write_ppu_byte(address & 0x3FFF, value)
    }

    pub fn debug_print_mapper_state(&self) {
        self.mapper.debug_print_state();
    }
}
