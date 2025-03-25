use instructions::*;
use ppu::*;
use cpu::*;
use crate::loader::Cartridge;

pub mod instructions;
pub mod ppu;
pub mod cpu;

pub const OPEN_BUS: u8 = 0xAA; // Can be any byte value
pub const STACK_PAGE: u16 = 0x0100;
pub const NMI_VECTOR: u16 = 0xFFFA;
pub const RESET_VECTOR: u16 = 0xFFFC;
pub const IRQ_VECTOR: u16 = 0xFFFE;

pub struct Machine {
    pub cartridge_slot: Option<Cartridge>,
    pub cpu: CentralProcessingUnit,
    pub ppu: PictureProcessingUnit,
    internal_ram: Box<[u8; 0x0800]>,
}

impl Machine {
    pub fn new() -> Self {
        Self {
            cartridge_slot: None,
            cpu: CentralProcessingUnit::new(),
            ppu: PictureProcessingUnit::new(),
            internal_ram: Box::new([0; 0x0800]),
        }
    }

    pub fn reset(&mut self) {
        self.cpu.program_counter = self.read_pair(RESET_VECTOR);
    }

    pub fn read_byte(&mut self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x1FFF => {
                self.internal_ram[(address & 0x07FF) as usize]
            }
            0x2000 ..= 0x3FFF => match address & 0x0007 {
                PPU_STATUS => self.ppu.read_status(),
                PPU_OAM_DATA => self.ppu.read_oam_data(),
                PPU_VRAM_DATA => self.cartridge_slot.as_ref().map_or(OPEN_BUS, |cartridge| {
                    self.ppu.read_vram_data(cartridge)
                }),
                _ => OPEN_BUS
            }
            _ => self.cartridge_slot.as_ref().map_or(OPEN_BUS, |cartridge| {
                cartridge.read_cpu_byte(address)
            })
        }
    }

    pub fn read_byte_silent(&self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x1FFF => {
                self.internal_ram[(address & 0x07FF) as usize]
            }
            _ => self.cartridge_slot.as_ref().map_or(OPEN_BUS, |cartridge| {
                cartridge.read_cpu_byte(address)
            })
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000 ..= 0x1FFF => {
                self.internal_ram[(address & 0x07FF) as usize] = value;
            }
            0x2000 ..= 0x3FFF => match address & 0x0007 {
                PPU_CONTROL => self.ppu.write_control(value),
                PPU_MASK => self.ppu.write_mask(value),
                PPU_OAM_ADDRESS => self.ppu.write_oam_address(value),
                PPU_OAM_DATA => self.ppu.write_oam_data(value),
                PPU_SCROLL => self.ppu.write_scroll(value),
                PPU_VRAM_ADDRESS => self.ppu.write_vram_address(value),
                PPU_VRAM_DATA => if let Some(cartridge) = &mut self.cartridge_slot {
                    self.ppu.write_vram_data(value, cartridge);
                },
                _ => {}
            }
            OAM_DMA_REGISTER => {
                self.cpu.start_oam_dma(value);
            }
            _ => if let Some(cartridge) = &mut self.cartridge_slot {
                cartridge.write_cpu_byte(address, value);
            }
        }
    }

    pub fn read_pair(&mut self, address: u16) -> u16 {
        let low = self.read_byte(address) as u16;
        let high = self.read_byte(address.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    pub fn read_pair_silent(&self, address: u16) -> u16 {
        let low = self.read_byte_silent(address) as u16;
        let high = self.read_byte_silent(address.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    pub fn write_pair(&mut self, address: u16, value: u16) {
        let low = (value & 0xFF) as u8;
        let high = (value >> 8) as u8;
        self.write_byte(address, low);
        self.write_byte(address.wrapping_add(1), high);
    }

    pub fn stack_push_byte(&mut self, value: u8) {
        let address = STACK_PAGE | self.cpu.stack_pointer as u16;
        self.write_byte(address, value);
        self.cpu.stack_pointer = self.cpu.stack_pointer.wrapping_sub(1);
    }

    pub fn stack_pull_byte(&mut self) -> u8 {
        self.cpu.stack_pointer = self.cpu.stack_pointer.wrapping_add(1);
        let address = STACK_PAGE | self.cpu.stack_pointer as u16;
        self.read_byte(address)
    }

    pub fn stack_push_pair(&mut self, value: u16) {
        self.stack_push_byte((value & 0x00FF) as u8);
        self.stack_push_byte((value >> 8) as u8);
    }

    pub fn stack_pull_pair(&mut self) -> u16 {
        let high = self.stack_pull_byte();
        let low = self.stack_pull_byte();
        (high as u16) << 8 | low as u16
    }

    pub fn execute_instruction(&mut self) {
        let opcode = self.fetch_byte(self.cpu.program_counter);
        let instruction = Instruction::decode(opcode);
        instruction.execute(self);
    }
}
