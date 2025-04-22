use super::*;

pub struct Mapper000 {
    nametables: BuiltinNametables,
    prg_chunk_0: PrgChunk,
    prg_chunk_1: Option<PrgChunk>,
    chr_chunk_0: ChrChunk,
    chr_write_enable: bool,
}

impl Mapper000 {
    pub fn new(header: &NESFileHeader, prg_chunks: Vec<PrgChunk>, mut chr_chunks: Vec<ChrChunk>) -> Result<Self, String> {
        if prg_chunks.len() != 1 && prg_chunks.len() != 2 {
            return Err(format!("Unexpected number of PRG-ROM chunks: {}.", prg_chunks.len()));
        }
        else if chr_chunks.len() > 1 {
            return Err(format!("Unexpected number of CHR-ROM chunks: {}.", chr_chunks.len()));
        }

        let mut chr_write_enable = false;
        if chr_chunks.is_empty() {
            // Add CHR-RAM, I guess?
            chr_chunks.push(Box::new([0; CHR_CHUNK_SIZE]));
            chr_write_enable = true;
        }

        let mut prg_chunks = prg_chunks.into_iter();
        let mut chr_chunks = chr_chunks.into_iter();
        Ok(Self {
            nametables: BuiltinNametables::new(header.nametable_arrangement),
            prg_chunk_0: prg_chunks.next().unwrap(),
            prg_chunk_1: prg_chunks.next(),
            chr_chunk_0: chr_chunks.next().unwrap(),
            chr_write_enable,
        })
    }
}

impl Mapper for Mapper000 {
    fn name(&self) -> &'static str {
        "Mapper 000 (NROM)"
    }

    fn read_cpu_byte(&self, address: u16) -> u8 {
        match address {
            0x8000 ..= 0xBFFF => {
                self.prg_chunk_0[address as usize & PRG_CHUNK_OFFSET_MASK]
            }
            0xC000 ..= 0xFFFF => match &self.prg_chunk_1 {
                Some(prg_rom_bank_1) => prg_rom_bank_1[address as usize & PRG_CHUNK_OFFSET_MASK],
                None => self.prg_chunk_0[address as usize & PRG_CHUNK_OFFSET_MASK],
            }
            _ => crate::hardware::OPEN_BUS
        }
    }

    fn read_ppu_byte(&mut self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x1FFF => {
                self.chr_chunk_0[address as usize]
            }
            _ => {
                self.nametables.read_byte(address)
            }
        }
    }

    fn write_ppu_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000 ..= 0x1FFF => if self.chr_write_enable {
                self.chr_chunk_0[address as usize] = value;
            }
            _ => {
                self.nametables.write_byte(address, value)
            }
        }
    }

    fn debug_print_state(&self) {
        println!("{}:", self.name());
        println!("    PRG-ROM chunks: {}", if self.prg_chunk_1.is_some() { 2 } else { 1 });
        println!("    Nametable arrangement: {}", self.nametables.arrangement);
    }
}
