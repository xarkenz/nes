use crate::loader::*;

mod mapper000;
mod mapper001;
mod mapper004;

pub trait Mapper {
    fn name(&self) -> &'static str;

    fn tick(&mut self) {
        // No-op by default
    }

    fn check_irq(&mut self) -> bool {
        // IRQ never fired by default
        false
    }

    fn read_cpu_byte(&self, address: u16) -> u8;

    fn write_cpu_byte(&mut self, address: u16, value: u8) {
        // No-op by default
        let _ = (address, value);
    }

    fn read_ppu_byte(&mut self, address: u16) -> u8;

    fn write_ppu_byte(&mut self, address: u16, value: u8) {
        // No-op by default
        let _ = (address, value);
    }
}

pub const PRG_CHUNK_SIZE: usize = 0x4000;
pub const CHR_CHUNK_SIZE: usize = 0x2000;
pub const NAMETABLE_SIZE: usize = 0x0400;

pub type PrgChunk = Box<[u8; PRG_CHUNK_SIZE]>;
pub type ChrChunk = Box<[u8; CHR_CHUNK_SIZE]>;
pub type Nametable = Box<[u8; NAMETABLE_SIZE]>;

#[derive(Copy, Clone, Debug)]
pub enum NametableMirroring {
    /// PPU A11 -> CIRAM A10
    Horizontal,
    /// PPU A10 -> CIRAM A10
    Vertical,
}

pub struct BuiltinNametables {
    pub mirroring: NametableMirroring,
    pub nametables: [Nametable; 2],
}

impl BuiltinNametables {
    pub fn new(mirroring: NametableMirroring) -> Self {
        Self {
            mirroring,
            nametables: [
                Box::new([0; NAMETABLE_SIZE]),
                Box::new([0; NAMETABLE_SIZE]),
            ],
        }
    }
    
    pub fn read_byte(&self, address: u16) -> u8 {
        let index = self.nametable_index(address);
        self.nametables[index][(address & 0x03FF) as usize]
    }
    
    pub fn write_byte(&mut self, address: u16, value: u8) {
        let index = self.nametable_index(address);
        self.nametables[index][(address & 0x03FF) as usize] = value;
    }

    fn nametable_index(&self, address: u16) -> usize {
        match self.mirroring {
            NametableMirroring::Horizontal => ((address >> 11) & 1) as usize,
            NametableMirroring::Vertical => ((address >> 10) & 1) as usize,
        }
    }
}

pub fn initialize_mapper(header: &NESFileHeader, prg_chunks: Vec<PrgChunk>, chr_chunks: Vec<ChrChunk>) -> Result<Box<dyn Mapper>, String> {
    fn boxed(result: Result<impl Mapper + 'static, String>) -> Result<Box<dyn Mapper>, String> {
        Ok(Box::new(result?))
    }

    match header.mapper_number {
        000 => boxed(mapper000::Mapper000::new(header, prg_chunks, chr_chunks)),
        001 => boxed(mapper001::Mapper001::new(header, prg_chunks, chr_chunks)),
        004 => boxed(mapper004::Mapper004::new(header, prg_chunks, chr_chunks)),
        number => Err(format!("Unsupported mapper number: {number:03}."))
    }
}
