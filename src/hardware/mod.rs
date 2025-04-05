use std::collections::BTreeMap;
use std::io::Write;
use instructions::*;
use ppu::*;
use cpu::*;
use crate::loader::Cartridge;

pub mod instructions;
pub mod ppu;
pub mod cpu;
pub mod map;

pub const OPEN_BUS: u8 = 0x00; // Can be any byte value
pub const STACK_PAGE: u16 = 0x0100;
pub const NMI_VECTOR: u16 = 0xFFFA;
pub const RESET_VECTOR: u16 = 0xFFFC;
pub const IRQ_VECTOR: u16 = 0xFFFE;
pub const CONTROLLER_1_REGISTER: u16 = 0x4016;
pub const CONTROLLER_2_REGISTER: u16 = 0x4017;
pub const BUTTON_A: usize = 0;
pub const BUTTON_B: usize = 1;
pub const BUTTON_SELECT: usize = 2;
pub const BUTTON_START: usize = 3;
pub const BUTTON_UP: usize = 4;
pub const BUTTON_DOWN: usize = 5;
pub const BUTTON_LEFT: usize = 6;
pub const BUTTON_RIGHT: usize = 7;
pub const BUTTON_COUNT: usize = 8;
const CONTROLLER_EXCESS_READ: u8 = 0x01;

pub struct Machine {
    pub cartridge_slot: Option<Cartridge>,
    pub cpu: CentralProcessingUnit,
    pub ppu: PictureProcessingUnit,
    internal_ram: Box<[u8; 0x0800]>,
    pub controller_1: [bool; BUTTON_COUNT],
    pub controller_2: [bool; BUTTON_COUNT],
    controller_strobe: bool,
    controller_1_button: usize,
    controller_2_button: usize,
    debug_disassembly: Option<BTreeMap<u16, (u16, String)>>,
}

impl Machine {
    pub fn new() -> Self {
        Self {
            cartridge_slot: None,
            cpu: CentralProcessingUnit::new(),
            ppu: PictureProcessingUnit::new(),
            internal_ram: Box::new([0; 0x0800]),
            controller_1: [false; BUTTON_COUNT],
            controller_2: [false; BUTTON_COUNT],
            controller_strobe: false,
            controller_1_button: BUTTON_COUNT,
            controller_2_button: BUTTON_COUNT,
            debug_disassembly: None,
        }
    }

    pub fn start_debug_disassembly(&mut self) {
        self.debug_disassembly = Some(BTreeMap::new());
    }
    
    pub fn cancel_debug_disassembly(&mut self) {
        self.debug_disassembly = None;
    }

    pub fn end_debug_disassembly(&mut self, writer: &mut impl Write) -> Result<(), String> {
        let Some(disassembly) = self.debug_disassembly.take() else {
            return Err("Debug disassembly is not active.".to_string());
        };

        let mut predicted_address = 0;
        for (address, (length, statement)) in disassembly {
            if predicted_address != 0 && address != predicted_address {
                // Instructions are nonconsecutive; leave a blank line
                writeln!(writer).map_err(|err| err.to_string())?;
            }
            writeln!(writer, "{statement}").map_err(|err| err.to_string())?;
            predicted_address = address.wrapping_add(length);
        }

        Ok(())
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
        self.ppu.reset();
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
            CONTROLLER_1_REGISTER => {
                // TODO: unconnected data lines
                if self.controller_1_button >= BUTTON_COUNT {
                    CONTROLLER_EXCESS_READ
                }
                else {
                    let controller_read = self.controller_1[self.controller_1_button] as u8;
                    if !self.controller_strobe {
                        self.controller_1_button += 1;
                    }
                    controller_read
                }
            }
            CONTROLLER_2_REGISTER => {
                if self.controller_2_button >= BUTTON_COUNT {
                    CONTROLLER_EXCESS_READ
                }
                else {
                    let controller_read = self.controller_2[self.controller_2_button] as u8;
                    if !self.controller_strobe {
                        self.controller_2_button += 1;
                    }
                    controller_read
                }
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
            0x2000 ..= 0x3FFF => match address & 0b111 {
                PPU_CONTROL => self.ppu.write_control(value),
                PPU_MASK => self.ppu.write_mask(value),
                PPU_OAM_ADDRESS => self.ppu.write_oam_address(value),
                PPU_OAM_DATA => self.ppu.write_oam_data(value),
                PPU_SCROLL => self.ppu.write_scroll(value),
                PPU_VRAM_ADDRESS => self.ppu.write_vram_address(value),
                PPU_VRAM_DATA => if let Some(cartridge) = &mut self.cartridge_slot {
                    // println!("${:04X}: writing ${:04X} = #${value:02X} at ({}, {})", self.cpu.program_counter, self.ppu.vram_address, self.ppu.scanline, self.ppu.dot);
                    self.ppu.write_vram_data(value, cartridge);
                }
                _ => {}
            }
            OAM_DMA_REGISTER => {
                self.cpu.start_oam_dma(value);
            }
            CONTROLLER_1_REGISTER => {
                self.controller_strobe = value & 1 != 0;
                self.controller_1_button = 0;
                self.controller_2_button = 0;
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

    pub fn read_pair_paged(&mut self, address: u16) -> u16 {
        let low = self.read_byte(address) as u16;
        // Address increment must not affect page number
        let high_address = (address & 0xFF00) | (address.wrapping_add(1) & 0x00FF);
        let high = self.read_byte(high_address) as u16;
        (high << 8) | low
    }

    pub fn read_pair_paged_silent(&self, address: u16) -> u16 {
        let low = self.read_byte_silent(address) as u16;
        // Address increment must not affect page number
        let high_address = (address & 0xFF00) | (address.wrapping_add(1) & 0x00FF);
        let high = self.read_byte_silent(high_address) as u16;
        (high << 8) | low
    }

    pub fn write_pair(&mut self, address: u16, value: u16) {
        let low = (value & 0x00FF) as u8;
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
        self.stack_push_byte((value >> 8) as u8);
        self.stack_push_byte((value & 0x00FF) as u8);
    }

    pub fn stack_pull_pair(&mut self) -> u16 {
        let low = self.stack_pull_byte();
        let high = self.stack_pull_byte();
        (high as u16) << 8 | low as u16
    }

    pub fn handle_nmi(&mut self) {
        self.cpu.pending_instruction = None;
        self.stack_push_pair(self.cpu.program_counter);
        self.stack_push_byte(self.cpu.get_status_byte(false));
        self.cpu.program_counter = self.read_pair(NMI_VECTOR);
    }

    pub fn tick(&mut self) {
        self.ppu.tick(self.cartridge_slot.as_ref());
        self.cpu.tick();

        if let Some(instruction) = self.cpu.pending_instruction {
            let ticks_needed = instruction.cycles_needed() * TICKS_PER_CPU_CYCLE;
            if ticks_needed <= self.cpu.ticks_available {
                self.cpu.ticks_available -= ticks_needed;
                self.cpu.pending_instruction = None;

                if let Some(mut disassembly) = self.debug_disassembly.take() {
                    let program_counter = self.cpu.program_counter;
                    disassembly.entry(program_counter).or_insert_with(|| (
                        instruction.size_bytes(),
                        format!("${program_counter:04X}: {}", instruction.disassemble(self, program_counter)),
                    ));
                    self.debug_disassembly = Some(disassembly);
                }

                instruction.execute(self);
            }
        }
        else if self.cpu.oam_dma_active {
            self.tick_oam_dma();
        }
        else {
            let opcode = self.read_byte(self.cpu.program_counter);
            self.cpu.pending_instruction = Some(Instruction::decode(opcode));
        }
        
        if self.ppu.check_vblank_nmi() {
            self.handle_nmi();
        }
    }

    fn tick_oam_dma(&mut self) {
        // Perform a get/put at the beginning of each CPU cycle
        if self.cpu.cycle_tick_count != 0 {
            return;
        }

        if self.cpu.is_put_cycle {
            if let Some(value) = self.cpu.oam_dma_fetch.take() {
                self.ppu.write_oam_data(value);
                if self.cpu.oam_dma_address & 0xFF == 0x00 {
                    self.cpu.oam_dma_active = false;
                }
            }
        }
        else {
            self.cpu.oam_dma_fetch = Some(self.read_byte(self.cpu.oam_dma_address));
            self.cpu.oam_dma_address = self.cpu.oam_dma_address.wrapping_add(1);
        }
    }
}
