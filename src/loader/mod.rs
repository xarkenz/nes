use std::io::Read;
use crate::hardware::Machine;

pub const PRG_BANK_SIZE: usize = 0x4000;
pub const CHR_BANK_SIZE: usize = 0x2000;

pub struct Cartridge {
    pub mapper_number: u8,
    pub nametable_arrangement: bool,
    pub uses_alt_nametable_layout: bool,
    pub has_battery: bool,
    pub prg_rom: Vec<[u8; PRG_BANK_SIZE]>,
    pub chr_rom: Vec<[u8; CHR_BANK_SIZE]>,
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

        let mut cartridge = Cartridge {
            mapper_number: header[6] >> 4,
            nametable_arrangement: header[6] & 0x01 != 0,
            uses_alt_nametable_layout: header[6] & 0x08 != 0,
            has_battery: header[6] & 0x02 != 0,
            prg_rom: Vec::with_capacity(prg_rom_banks),
            chr_rom: Vec::with_capacity(chr_rom_banks),
        };

        for bank in 0..prg_rom_banks {
            cartridge.prg_rom.push([0; PRG_BANK_SIZE]);
            reader.read_exact(&mut cartridge.prg_rom[bank]).map_err(|err| err.to_string())?;
        }
        for bank in 0..chr_rom_banks {
            cartridge.chr_rom.push([0; CHR_BANK_SIZE]);
            reader.read_exact(&mut cartridge.chr_rom[bank]).map_err(|err| err.to_string())?;
        }

        Ok(cartridge)
    }

    pub fn load_into(&self, machine: &mut Machine) {
        // TODO: extremely temporary hardcode
        machine.load_memory_bank(0x8000, &self.prg_rom[0]);
        machine.load_memory_bank(0xC000, &self.prg_rom[1]);
    }
}
