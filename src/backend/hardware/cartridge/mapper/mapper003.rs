use super::*;

const PRG_RAM_SIZE: usize = 0x0800; // 2 KiB
const PRG_RAM_MASK: usize = PRG_RAM_SIZE - 1;

pub struct Mapper003 {
    nametables: BuiltinNametables,
    prg_chunks: Vec<PrgChunk>,
    chr_chunks: Vec<ChrChunk>,
    chr_writeable: bool,
    prg_ram: Box<[u8; PRG_RAM_SIZE]>,
    prg_chunk_mask: usize,
    chr_chunk_mask: usize,
    chr_bank_chunk: usize,
}

impl Mapper003 {
    pub fn new(header: &NESFileHeader, prg_chunks: Vec<PrgChunk>, mut chr_chunks: Vec<ChrChunk>) -> Result<Self, String> {
        if prg_chunks.is_empty() {
            return Err("Expected at least one PRG-ROM chunk.".to_string());
        }

        let chr_writeable = chr_chunks.is_empty();
        if chr_writeable {
            chr_chunks.push(Box::new([0; CHR_CHUNK_SIZE]));
        }

        let mut prg_chunk_mask = 0b1;
        while prg_chunk_mask >= prg_chunks.len() {
            prg_chunk_mask >>= 1;
        }
        let mut chr_chunk_mask = 0b1111;
        while chr_chunk_mask >= chr_chunks.len() {
            chr_chunk_mask >>= 1;
        }

        let mapper = Self {
            nametables: BuiltinNametables::new(header.nametable_arrangement),
            prg_chunks,
            chr_chunks,
            chr_writeable,
            prg_ram: Box::new([0; PRG_RAM_SIZE]),
            prg_chunk_mask,
            chr_chunk_mask,
            chr_bank_chunk: 0,
        };

        Ok(mapper)
    }
}

impl Mapper for Mapper003 {
    fn name(&self) -> &'static str {
        "Mapper 003 (CNROM)"
    }

    fn read_cpu_byte(&self, address: u16) -> u8 {
        match address {
            0x8000 ..= 0xBFFF => {
                self.prg_chunks[0][address as usize & PRG_CHUNK_OFFSET_MASK]
            }
            0xC000 ..= 0xFFFF => {
                self.prg_chunks[self.prg_chunk_mask][address as usize & PRG_CHUNK_OFFSET_MASK]
            }
            _ => crate::hardware::OPEN_BUS
        }
    }

    fn write_cpu_byte(&mut self, address: u16, value: u8) {
        if address >= 0x8000 {
            self.chr_bank_chunk = value as usize & self.chr_chunk_mask;
        }
        else if address >= 0x6000 {
            self.prg_ram[address as usize & PRG_RAM_MASK] = value;
        }
    }

    fn read_ppu_byte(&mut self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x1FFF => {
                self.chr_chunks[self.chr_bank_chunk][address as usize]
            }
            _ => {
                self.nametables.read_byte(address)
            }
        }
    }

    fn write_ppu_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000 ..= 0x1FFF => if self.chr_writeable {
                self.chr_chunks[self.chr_bank_chunk][address as usize] = value;
            }
            _ => {
                self.nametables.write_byte(address, value)
            }
        }
    }

    fn debug_print_state(&self) {
        println!("{}:", self.name());
        println!("    Nametable arrangement: {}", self.nametables.arrangement);
        println!("    CHR bank: {}", self.chr_bank_chunk);
    }
}
