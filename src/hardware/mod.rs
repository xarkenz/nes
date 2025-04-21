use std::collections::BTreeMap;
use std::io::Write;
use crate::loader::Cartridge;
use instructions::*;
use cpu::*;
use ppu::*;
use apu::*;

pub mod instructions;
pub mod cpu;
pub mod ppu;
pub mod apu;
pub mod map;

#[derive(Copy, Clone, Debug)]
pub struct DelayedFlag<const N: u16> {
    shifter: u16,
    current_flag: bool,
}

impl<const N: u16> DelayedFlag<N> {
    pub fn new(initial_flag: bool) -> Self {
        Self {
            shifter: ((0b10 << N) - 1) * initial_flag as u16,
            current_flag: initial_flag,
        }
    }

    pub fn reset(&mut self, flag: bool) {
        self.shifter = ((0b10 << N) - 1) * flag as u16;
        self.current_flag = flag;
    }

    pub fn get_current(&self) -> bool {
        self.current_flag
    }

    pub fn set_current(&mut self, flag: bool) {
        self.current_flag = flag;
        self.shifter |= (flag as u16) << N;
    }
    
    pub fn pulse(&mut self, flag: bool) {
        self.shifter |= (flag as u16) << N;
    }

    pub fn get_delayed(&self) -> bool {
        self.shifter & 1 != 0
    }

    pub fn tick(&mut self) {
        self.shifter >>= 1;
        self.shifter |= (self.current_flag as u16) << N;
    }
}

pub const NTSC_FRAMES_PER_SECOND: usize = 60;
pub const OPEN_BUS: u8 = 0x00;
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
    pub cartridge: Option<Cartridge>,
    pub cpu: CentralProcessingUnit,
    pub ppu: PictureProcessingUnit,
    pub apu: AudioProcessingUnit,
    internal_ram: Box<[u8; 0x0800]>,
    pub controller_1: [bool; BUTTON_COUNT],
    pub controller_2: [bool; BUTTON_COUNT],
    controller_strobe: bool,
    controller_1_button: usize,
    controller_2_button: usize,
    pub debug_printing: bool,
    debug_disassembly: Option<BTreeMap<u16, (u16, String)>>,
}

impl Machine {
    pub fn new() -> Self {
        Self {
            cartridge: None,
            cpu: CentralProcessingUnit::new(),
            ppu: PictureProcessingUnit::new(),
            apu: AudioProcessingUnit::new(),
            internal_ram: Box::new([0; 0x0800]),
            controller_1: [false; BUTTON_COUNT],
            controller_2: [false; BUTTON_COUNT],
            controller_strobe: false,
            controller_1_button: BUTTON_COUNT,
            controller_2_button: BUTTON_COUNT,
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
                PPU_VRAM_DATA => if let Some(cartridge) = &mut self.cartridge {
                    self.ppu.read_vram_data(cartridge)
                } else {
                    OPEN_BUS
                }
                _ => OPEN_BUS
            }
            APU_STATUS => {
                self.apu.read_status()
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
            _ => self.cartridge.as_ref().map_or(OPEN_BUS, |cartridge| {
                cartridge.read_cpu_byte(address)
            })
        }
    }

    pub fn read_byte_silent(&self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x1FFF => {
                self.internal_ram[(address & 0x07FF) as usize]
            }
            _ => self.cartridge.as_ref().map_or(OPEN_BUS, |cartridge| {
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
                PPU_VRAM_ADDRESS => if let Some(cartridge) = &mut self.cartridge {
                    self.ppu.write_vram_address(value, cartridge);
                },
                PPU_VRAM_DATA => if let Some(cartridge) = &mut self.cartridge {
                    self.ppu.write_vram_data(value, cartridge);
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
            CONTROLLER_1_REGISTER => {
                self.controller_strobe = value & 1 != 0;
                self.controller_1_button = 0;
                self.controller_2_button = 0;
            }
            APU_FRAME_COUNTER => {
                self.apu.write_frame_counter(value);
            }
            _ => if let Some(cartridge) = &mut self.cartridge {
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

        // TODO
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
