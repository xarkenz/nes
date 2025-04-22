use super::*;

pub struct Mapper001 {
    nametables: BuiltinNametables,
    prg_chunks: Vec<PrgChunk>,
    chr_chunks: Vec<ChrChunk>,
    shift_register: u8,
    separate_prg_banks: bool,
    fix_last_prg_bank: bool,
    separate_chr_banks: bool,
    prg_bank_register: u8,
    chr_bank_0_register: u8,
    chr_bank_1_register: u8,
    chr_write_enable: bool,
    ignore_serial_port_writes: bool,
    prg_bank_mask: u8,
    chr_bank_mask: u8,
    prg_bank_0_chunk: usize,
    prg_bank_1_chunk: usize,
    chr_bank_0_chunk: usize,
    chr_bank_0_base: usize,
    chr_bank_1_chunk: usize,
    chr_bank_1_base: usize,
}

impl Mapper001 {
    const SHIFT_REGISTER_RESET: u8 = 0b10000;
    
    pub fn new(header: &NESFileHeader, prg_chunks: Vec<PrgChunk>, mut chr_chunks: Vec<ChrChunk>) -> Result<Self, String> {
        let mut chr_write_enable = false;
        if chr_chunks.is_empty() {
            // Add CHR-RAM, I guess?
            chr_chunks.push(Box::new([0; CHR_CHUNK_SIZE]));
            chr_write_enable = true;
        }

        let mut prg_bank_mask = 0b1111;
        while prg_bank_mask as usize >= prg_chunks.len() {
            prg_bank_mask >>= 1;
        }
        let mut chr_bank_mask = 0b11111;
        while chr_bank_mask as usize >= (chr_chunks.len() << 1) {
            chr_bank_mask >>= 1;
        }

        let mut mapper = Self {
            nametables: BuiltinNametables::new(header.nametable_arrangement),
            prg_chunks,
            chr_chunks,
            shift_register: Self::SHIFT_REGISTER_RESET,
            separate_prg_banks: true,
            fix_last_prg_bank: true,
            separate_chr_banks: false,
            prg_bank_register: 0,
            chr_bank_0_register: 0,
            chr_bank_1_register: 0,
            chr_write_enable,
            ignore_serial_port_writes: false,
            prg_bank_mask,
            chr_bank_mask,
            prg_bank_0_chunk: 0,
            prg_bank_1_chunk: 0,
            chr_bank_0_chunk: 0,
            chr_bank_0_base: 0,
            chr_bank_1_chunk: 0,
            chr_bank_1_base: 0,
        };

        mapper.update_banks();
        Ok(mapper)
    }

    fn update_banks(&mut self) {
        if self.separate_prg_banks {
            if self.fix_last_prg_bank {
                self.prg_bank_0_chunk = (self.prg_bank_register & self.prg_bank_mask) as usize;
                self.prg_bank_1_chunk = self.prg_bank_mask as usize;
            }
            else {
                self.prg_bank_0_chunk = 0;
                self.prg_bank_1_chunk = (self.prg_bank_register & self.prg_bank_mask) as usize;
            }
        }
        else {
            self.prg_bank_0_chunk = (self.prg_bank_register & self.prg_bank_mask & !1) as usize;
            self.prg_bank_1_chunk = self.prg_bank_0_chunk + 1;
        }

        if self.separate_chr_banks {
            let bank_0 = (self.chr_bank_0_register & self.chr_bank_mask) as usize;
            self.chr_bank_0_chunk = bank_0 >> 1;
            self.chr_bank_0_base = (bank_0 & 1) << 12;
            let bank_1 = (self.chr_bank_1_register & self.chr_bank_mask) as usize;
            self.chr_bank_1_chunk = bank_1 >> 1;
            self.chr_bank_1_base = (bank_1 & 1) << 12;
        }
        else {
            self.chr_bank_0_chunk = (self.chr_bank_0_register & self.chr_bank_mask) as usize >> 1;
            self.chr_bank_1_chunk = self.chr_bank_0_chunk;
            self.chr_bank_0_base = 0x0000;
            self.chr_bank_1_base = 0x1000;
        }
    }
}

impl Mapper for Mapper001 {
    fn name(&self) -> &'static str {
        "Mapper 001 (MMC1)"
    }

    fn tick(&mut self) {
        self.ignore_serial_port_writes = false;
    }

    fn read_cpu_byte(&self, address: u16) -> u8 {
        match address {
            0x8000 ..= 0xBFFF => {
                self.prg_chunks[self.prg_bank_0_chunk][address as usize & PRG_CHUNK_OFFSET_MASK]
            }
            0xC000 ..= 0xFFFF => {
                self.prg_chunks[self.prg_bank_1_chunk][address as usize & PRG_CHUNK_OFFSET_MASK]
            }
            _ => crate::hardware::OPEN_BUS
        }
    }

    fn write_cpu_byte(&mut self, address: u16, value: u8) {
        if self.ignore_serial_port_writes || address < 0x8000 {
            return;
        }
        else if value & 0b10000000 != 0 {
            // Reset mapper
            self.shift_register = Self::SHIFT_REGISTER_RESET;
            self.separate_prg_banks = true;
            self.fix_last_prg_bank = true;
            return;
        }

        let shift_register_full = self.shift_register & 1 != 0;
        self.shift_register >>= 1;
        self.shift_register |= (value & 1) << 4;

        if shift_register_full {
            match address {
                0x8000 ..= 0x9FFF => {
                    // Control
                    self.nametables.arrangement = match self.shift_register & 0b00011 {
                        0b00 => NametableArrangement::OneScreenLower,
                        0b01 => NametableArrangement::OneScreenUpper,
                        0b10 => NametableArrangement::Horizontal,
                        0b11 => NametableArrangement::Vertical,
                        _ => unreachable!(),
                    };
                    self.fix_last_prg_bank = self.shift_register & 0b00100 != 0;
                    self.separate_prg_banks = self.shift_register & 0b01000 != 0;
                    self.separate_chr_banks = self.shift_register & 0b10000 != 0;
                }
                0xA000 ..= 0xBFFF => {
                    // CHR bank 0
                    self.chr_bank_0_register = self.shift_register;
                }
                0xC000 ..= 0xDFFF => {
                    // CHR bank 1
                    self.chr_bank_1_register = self.shift_register;
                }
                0xE000 ..= 0xFFFF => {
                    // PRG bank
                    self.prg_bank_register = self.shift_register;
                }
                _ => unreachable!()
            }
            self.update_banks();
            self.shift_register = Self::SHIFT_REGISTER_RESET;
        }

        // If a write happens twice in one instruction, only the first write counts
        self.ignore_serial_port_writes = true;
    }

    fn read_ppu_byte(&mut self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x0FFF => {
                let chunk_address = self.chr_bank_0_base + address as usize;
                self.chr_chunks[self.chr_bank_0_chunk][chunk_address]
            }
            0x1000 ..= 0x1FFF => {
                let chunk_address = self.chr_bank_1_base + (address & 0x0FFF) as usize;
                self.chr_chunks[self.chr_bank_1_chunk][chunk_address]
            }
            _ => {
                self.nametables.read_byte(address)
            }
        }
    }

    fn write_ppu_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000 ..= 0x0FFF => if self.chr_write_enable {
                let chunk_address = self.chr_bank_0_base + address as usize;
                self.chr_chunks[self.chr_bank_0_chunk][chunk_address] = value;
            }
            0x1000 ..= 0x1FFF => if self.chr_write_enable {
                let chunk_address = self.chr_bank_1_base + (address & 0x0FFF) as usize;
                self.chr_chunks[self.chr_bank_1_chunk][chunk_address] = value;
            }
            _ => {
                self.nametables.write_byte(address, value)
            }
        }
    }

    fn debug_print_state(&self) {
        println!("{}:", self.name());
        println!("    Nametable arrangement: {}", self.nametables.arrangement);
    }
}
