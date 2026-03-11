use std::collections::BTreeMap;
use std::io::Write;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteArray;
use cpu::*;
use cpu::instruction::*;
use ppu::*;
use apu::*;
use cartridge::*;
use joypad::*;
use game_genie::GameGenie;
use crate::state::PullState;

pub mod cpu;
pub mod ppu;
pub mod apu;
pub mod cartridge;
pub mod joypad;
pub mod timing;
pub mod game_genie;

pub const NTSC_FRAMES_PER_SECOND: usize = 60;
pub const OPEN_BUS: u8 = 0x00;
pub const STACK_PAGE: u16 = 0x0100;
pub const NMI_VECTOR: u16 = 0xFFFA;
pub const RESET_VECTOR: u16 = 0xFFFC;
pub const IRQ_VECTOR: u16 = 0xFFFE;
const INTERNAL_RAM_SIZE: usize = 0x0800;

#[derive(Serialize, Deserialize)]
pub struct Machine {
    pub cpu: CentralProcessingUnit,
    pub ppu: PictureProcessingUnit,
    pub apu: AudioProcessingUnit,
    pub cartridge: Option<Cartridge>,
    pub joypads: Joypads,
    pub internal_ram: Box<ByteArray<INTERNAL_RAM_SIZE>>,
    pub game_genie: Option<GameGenie>,
    #[serde(skip)]
    pub debug_printing: bool,
    #[serde(skip)]
    debug_disassembly: Option<BTreeMap<u16, (u16, String)>>,
}

impl Machine {
    pub fn new() -> Self {
        Self {
            cpu: CentralProcessingUnit::new(),
            ppu: PictureProcessingUnit::new(),
            apu: AudioProcessingUnit::new(),
            cartridge: None,
            joypads: Joypads::new(),
            internal_ram: Box::new([0; INTERNAL_RAM_SIZE].into()),
            game_genie: None,
            debug_printing: false,
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
        self.apu.reset();
        self.cpu.program_counter = self.read_word(RESET_VECTOR);
    }

    pub fn read_byte(&mut self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x1FFF => {
                self.internal_ram[(address & 0x07FF) as usize]
            }
            0x2000 ..= 0x3FFF => match address & 0x2007 {
                PPU_STATUS => {
                    self.ppu.read_status()
                }
                PPU_OAM_DATA => {
                    self.ppu.read_oam_data()
                }
                PPU_VRAM_DATA => {
                    self.cartridge.as_mut().map_or(OPEN_BUS, |cartridge| {
                        self.ppu.read_vram_data(cartridge)
                    })
                }
                _ => OPEN_BUS
            }
            APU_STATUS => {
                self.apu.read_status()
            }
            JOYPAD_1_REGISTER => {
                self.joypads.read_player_1()
            }
            JOYPAD_2_REGISTER => {
                self.joypads.read_player_2()
            }
            _ => {
                self.read_rom_byte(address)
            }
        }
    }

    pub fn read_byte_silent(&self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x1FFF => {
                self.internal_ram[(address & 0x07FF) as usize]
            }
            _ => {
                self.read_rom_byte(address)
            }
        }
    }

    fn read_rom_byte(&self, address: u16) -> u8 {
        let read_from_cartridge = || {
            self.cartridge.as_ref().map_or(OPEN_BUS, |cartridge| {
                cartridge.read_cpu_byte(address)
            })
        };

        if let Some(game_genie) = &self.game_genie {
            game_genie.read_byte(address, read_from_cartridge)
        }
        else {
            read_from_cartridge()
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000 ..= 0x1FFF => {
                self.internal_ram[(address & 0x07FF) as usize] = value;
            }
            0x2000 ..= 0x3FFF => match address & 0x2007 {
                PPU_CONTROL => {
                    self.ppu.write_control(value);
                }
                PPU_MASK => {
                    self.ppu.write_mask(value);
                }
                PPU_OAM_ADDRESS => {
                    self.ppu.write_oam_address(value);
                }
                PPU_OAM_DATA => {
                    self.ppu.write_oam_data(value);
                }
                PPU_SCROLL => {
                    self.ppu.write_scroll(value);
                }
                PPU_VRAM_ADDRESS => {
                    if let Some(cartridge) = &mut self.cartridge {
                        self.ppu.write_vram_address(value, cartridge);
                    }
                }
                PPU_VRAM_DATA => {
                    if let Some(cartridge) = &mut self.cartridge {
                        self.ppu.write_vram_data(value, cartridge);
                    }
                }
                _ => {}
            }
            APU_CHANNEL_START ..= APU_CHANNEL_END => {
                self.apu.write_channel_register(address, value);
            }
            OAM_DMA_REGISTER => {
                self.cpu.start_oam_dma(value);
                if self.debug_printing {
                    println!("[OAM DMA started]");
                }
            }
            APU_STATUS => {
                self.apu.write_status(value);
            }
            JOYPAD_1_REGISTER => {
                self.joypads.write_strobe(value);
            }
            APU_FRAME_COUNTER => {
                self.apu.write_frame_counter(value);
            }
            _ => {
                if let Some(cartridge) = &mut self.cartridge {
                    cartridge.write_cpu_byte(address, value);
                }
            }
        }
    }

    pub fn read_word(&mut self, address: u16) -> u16 {
        let low = self.read_byte(address) as u16;
        let high = self.read_byte(address.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    pub fn read_word_silent(&self, address: u16) -> u16 {
        let low = self.read_byte_silent(address) as u16;
        let high = self.read_byte_silent(address.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    pub fn read_word_paged(&mut self, address: u16) -> u16 {
        let low = self.read_byte(address) as u16;
        // Address increment must not affect page number
        let high_address = (address & 0xFF00) | (address.wrapping_add(1) & 0x00FF);
        let high = self.read_byte(high_address) as u16;
        (high << 8) | low
    }

    pub fn read_word_paged_silent(&self, address: u16) -> u16 {
        let low = self.read_byte_silent(address) as u16;
        // Address increment must not affect page number
        let high_address = (address & 0xFF00) | (address.wrapping_add(1) & 0x00FF);
        let high = self.read_byte_silent(high_address) as u16;
        (high << 8) | low
    }

    pub fn write_word(&mut self, address: u16, value: u16) {
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

    pub fn stack_push_word(&mut self, value: u16) {
        self.stack_push_byte((value >> 8) as u8);
        self.stack_push_byte((value & 0x00FF) as u8);
    }

    pub fn stack_pull_word(&mut self) -> u16 {
        let low = self.stack_pull_byte();
        let high = self.stack_pull_byte();
        (high as u16) << 8 | low as u16
    }

    pub fn tick(&mut self) {
        // This tick ordering is important
        self.ppu.tick(self.cartridge.as_mut());
        self.cpu.tick();
        if self.cpu.cycle_tick_offset == 0 {
            self.apu.cpu_cycle_tick(self.cpu.is_put_cycle);
            self.tick_dmc_dma_cycle();
        }
        if let Some(cartridge) = &mut self.cartridge {
            cartridge.tick();
        }

        if let Some(instruction) = self.cpu.pending_instruction {
            let cycles_needed = instruction.cycles_needed();
            if self.cpu.delay_cycles == 0 && self.cpu.cycles_available >= cycles_needed {
                self.cpu.cycles_available -= cycles_needed;
                self.cpu.pending_instruction = None;

                if let Some(mut disassembly) = self.debug_disassembly.take() {
                    let size_bytes = instruction.size_bytes();
                    if size_bytes > 0 {
                        let program_counter = self.cpu.program_counter;
                        disassembly.entry(program_counter).or_insert_with(|| (
                            size_bytes,
                            format!("${program_counter:04X}: {}", instruction.disassemble(self, program_counter)),
                        ));
                    }
                    self.debug_disassembly = Some(disassembly);
                }
                if self.debug_printing {
                    let program_counter = self.cpu.program_counter;
                    println!("${program_counter:04X}: {}", instruction.disassemble(self, program_counter));
                }

                instruction.execute(self);
            }
        }
        else if self.ppu.check_vblank_nmi() {
            self.cpu.pending_instruction = Some(Instruction::meta_nmi());
        }
        else if !self.cpu.interrupt_disable_flag && self.check_irq() {
            self.cpu.pending_instruction = Some(Instruction::meta_irq());
        }
        else if self.cpu.oam_dma_active {
            self.tick_oam_dma();
        }
        else {
            let opcode = self.read_byte(self.cpu.program_counter);
            self.cpu.pending_instruction = Some(Instruction::decode(opcode));
        }
    }

    fn check_irq(&mut self) -> bool {
        self.apu.irq_asserted() || self.cartridge.as_mut().is_some_and(Cartridge::check_irq)
    }

    fn tick_oam_dma(&mut self) {
        // Perform a get/put on each CPU clock cycle
        if self.cpu.delay_cycles == 0 && self.cpu.cycles_available >= 1 {
            self.cpu.cycles_available -= 1;
        }
        else {
            return;
        }

        if self.cpu.is_put_cycle {
            if let Some(value) = self.cpu.oam_dma_fetch.take() {
                self.ppu.write_oam_data(value);
                if self.cpu.oam_dma_address & 0xFF == 0x00 {
                    self.cpu.oam_dma_active = false;
                    if self.debug_printing {
                        println!("[OAM DMA finished]");
                    }
                }
            }
        }
        else {
            self.cpu.oam_dma_fetch = Some(self.read_byte(self.cpu.oam_dma_address));
            self.cpu.oam_dma_address = self.cpu.oam_dma_address.wrapping_add(1);
        }
    }

    fn tick_dmc_dma_cycle(&mut self) {
        let Some((dma_address, is_reload)) = self.apu.dmc_dma_request() else {
            // DMC DMA is not currently being requested
            return;
        };

        // TODO: accuracy
        if !self.cpu.dmc_dma_active {
            if is_reload == self.cpu.is_put_cycle {
                self.cpu.dmc_dma_active = true;
                self.cpu.delay_cycles += 1;
            }
        }
        else if !self.cpu.is_put_cycle {
            let dma_read = self.cartridge.as_ref().map_or(OPEN_BUS, |cartridge| {
                cartridge.read_cpu_byte(dma_address)
            });
            self.apu.load_dmc_sample_buffer(dma_read);
            self.cpu.delay_cycles += 1;
            self.cpu.dmc_dma_active = false;
        }
        else {
            self.cpu.delay_cycles += 1;
        }
    }

    pub fn debug_step(&mut self) -> &Instruction {
        self.debug_printing = true;
        loop {
            let last_instruction = self.cpu.pending_instruction;
            self.tick();
            if let (Some(instruction), None) = (last_instruction, self.cpu.pending_instruction) {
                self.debug_printing = false;
                break instruction;
            }
        }
    }
}

impl PullState for Machine {
    fn pull_state_from(&mut self, source: &Self) {
        self.cpu.pull_state_from(&source.cpu);
        self.ppu.pull_state_from(&source.ppu);
        self.apu.pull_state_from(&source.apu);
        self.cartridge.pull_state_from(&source.cartridge);
        self.joypads.pull_state_from(&source.joypads);
        self.internal_ram.pull_state_from(&source.internal_ram);
        // Debug state is ignored
    }
}
