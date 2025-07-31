use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct Mapper007 {
    nametables: BuiltinNametables,
    prg_chunks: Vec<PrgChunk>,
    chr_chunks: Vec<ChrChunk>,
    chr_writeable: bool,
    prg_bank_mask: usize,
    prg_bank: usize,
}

impl Mapper007 {
    pub fn new(_header: &NESFileHeader, prg_chunks: Vec<PrgChunk>, mut chr_chunks: Vec<ChrChunk>) -> Result<Self, String> {
        if prg_chunks.len() < 2 {
            return Err("Expected at least two PRG-ROM chunks.".to_string());
        }

        let chr_writeable = chr_chunks.is_empty();
        if chr_writeable {
            chr_chunks.push(Box::new([0; CHR_CHUNK_SIZE].into()));
        }

        let mut prg_bank_mask = 0b1111;
        while prg_bank_mask >= prg_chunks.len() >> 1 {
            prg_bank_mask >>= 1;
        }

        let mapper = Self {
            nametables: BuiltinNametables::new(NametableArrangement::OneScreenLower),
            prg_chunks,
            chr_chunks,
            chr_writeable,
            prg_bank_mask,
            prg_bank: 0,
        };

        Ok(mapper)
    }
}

#[typetag::serde]
impl Mapper for Mapper007 {
    fn name(&self) -> &'static str {
        "Mapper 007 (AxROM)"
    }

    fn read_cpu_byte(&self, address: u16) -> u8 {
        let lower_chunk = self.prg_bank << 1;
        match address {
            0x8000 ..= 0xBFFF => {
                self.prg_chunks[lower_chunk][address as usize & PRG_CHUNK_OFFSET_MASK]
            }
            0xC000 ..= 0xFFFF => {
                self.prg_chunks[lower_chunk + 1][address as usize & PRG_CHUNK_OFFSET_MASK]
            }
            _ => crate::hardware::OPEN_BUS
        }
    }

    fn write_cpu_byte(&mut self, address: u16, value: u8) {
        if address >= 0x8000 {
            self.prg_bank = value as usize & self.prg_bank_mask;
            self.nametables.arrangement = if value & 0b10000 != 0 {
                NametableArrangement::OneScreenUpper
            } else {
                NametableArrangement::OneScreenLower
            };
        }
    }

    fn read_ppu_byte(&mut self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x1FFF => {
                self.chr_chunks[0][address as usize]
            }
            _ => {
                self.nametables.read_byte(address)
            }
        }
    }

    fn write_ppu_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000 ..= 0x1FFF => if self.chr_writeable {
                self.chr_chunks[0][address as usize] = value;
            }
            _ => {
                self.nametables.write_byte(address, value)
            }
        }
    }

    fn debug_print_state(&self) {
        println!("{}:", self.name());
        println!("    Nametable arrangement: {}", self.nametables.arrangement);
        println!("    PRG bank: {}", self.prg_bank);
    }
}
