use crate::hardware::instructions::Instruction;

pub const OAM_DMA_REGISTER: u16 = 0x4014;
pub const TICKS_PER_CPU_CYCLE: u16 = 3;

pub struct CentralProcessingUnit {
    pub program_counter: u16,
    pub accumulator: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub stack_pointer: u8,
    pub carry_flag: bool,
    pub zero_flag: bool,
    pub interrupt_disable_flag: bool,
    pub decimal_mode_flag: bool,
    pub overflow_flag: bool,
    pub negative_flag: bool,
    pub oam_dma_address: u16,
    pub oam_dma_active: bool,
    pub oam_dma_fetch: Option<u8>,
    pub cycle_tick_count: u16,
    pub is_put_cycle: bool,
    pub ticks_available: u16,
    pub pending_instruction: Option<&'static Instruction>,
}

impl CentralProcessingUnit {
    pub fn new() -> Self {
        Self {
            program_counter: 0,
            accumulator: 0,
            register_x: 0,
            register_y: 0,
            stack_pointer: 0,
            carry_flag: false,
            zero_flag: false,
            interrupt_disable_flag: false,
            decimal_mode_flag: false,
            overflow_flag: false,
            negative_flag: false,
            oam_dma_address: 0,
            oam_dma_active: false,
            oam_dma_fetch: None,
            cycle_tick_count: 0,
            is_put_cycle: false,
            ticks_available: 0,
            pending_instruction: None,
        }
    }

    pub fn get_status_byte(&self, break_flag: bool) -> u8 {
        0b00100000 // Bit 5 is always set
            | self.carry_flag as u8
            | (self.zero_flag as u8) << 1
            | (self.interrupt_disable_flag as u8) << 2
            | (self.decimal_mode_flag as u8) << 3
            | (break_flag as u8) << 4
            | (self.overflow_flag as u8) << 6
            | (self.negative_flag as u8) << 7
    }

    pub fn set_status_byte(&mut self, status: u8) {
        self.carry_flag = status & 0b00000001 != 0;
        self.zero_flag = status & 0b00000010 != 0;
        self.interrupt_disable_flag = status & 0b00000100 != 0;
        self.decimal_mode_flag = status & 0b00001000 != 0;
        self.overflow_flag = status & 0b01000000 != 0;
        self.negative_flag = status & 0b10000000 != 0;
    }

    pub fn set_result_flags(&mut self, result: u8) {
        self.zero_flag = result == 0;
        self.negative_flag = (result & 0b10000000) != 0;
    }

    pub fn start_oam_dma(&mut self, page: u8) {
        self.oam_dma_active = true;
        self.oam_dma_address = (page as u16) << 8;
    }

    pub fn tick(&mut self) {
        self.ticks_available += 1;
        self.cycle_tick_count += 1;
        if self.cycle_tick_count >= TICKS_PER_CPU_CYCLE {
            self.cycle_tick_count = 0;
            self.is_put_cycle = !self.is_put_cycle;
        }
    }

    pub fn debug_print_state(&self) {
        println!("CPU state:");
        println!("    PC:     ${:04X}", self.program_counter);
        println!("    SP:     $01{:02X}", self.stack_pointer);
        println!("    A:      ${:02X}", self.accumulator);
        println!("    X:      ${:02X}", self.register_x);
        println!("    Y:      ${:02X}", self.register_y);
        println!("    Status:  NV.BDIZC");
        println!("            %{:08b}", self.get_status_byte(false));
    }
}
