pub const CARRY_FLAG: usize = 0;
pub const ZERO_FLAG: usize = 1;
pub const INTERRUPT_DISABLE_FLAG: usize = 2;
pub const DECIMAL_FLAG: usize = 3; // NOTE: BCD mode is disabled in the Ricoh 2A03
pub const BREAK_FLAG: usize = 4;
pub const OVERFLOW_FLAG: usize = 5;
pub const NEGATIVE_FLAG: usize = 6;
pub const OAM_DMA_REGISTER: u16 = 0x4014;

pub struct CentralProcessingUnit {
    pub program_counter: u16,
    pub accumulator: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub stack_pointer: u8,
    pub status_byte: u8,
    delay_cycles: u16,
    oam_dma_address: u16,
    oam_dma_active: bool,
}

impl CentralProcessingUnit {
    pub fn new() -> Self {
        Self {
            program_counter: 0,
            accumulator: 0,
            register_x: 0,
            register_y: 0,
            stack_pointer: 0,
            status_byte: 0,
            delay_cycles: 0,
            oam_dma_address: 0,
            oam_dma_active: false,
        }
    }

    pub fn get_flag(&self, flag: usize) -> bool {
        (self.status_byte >> flag) & 1 != 0
    }

    pub fn set_flag(&mut self, flag: usize, value: bool) {
        let mask = 1 << flag;
        self.status_byte = (self.status_byte & !mask) | ((value as u8) << flag);
    }

    pub fn set_result_flags(&mut self, result: u8) {
        self.set_flag(ZERO_FLAG, result == 0);
        self.set_flag(NEGATIVE_FLAG, (result & 0x80) != 0);
    }

    pub fn start_oam_dma(&mut self, page: u8) {
        self.oam_dma_active = true;
        self.oam_dma_address = (page as u16) << 8;
    }

    pub fn debug_print_state(&self) {
        println!("CPU state:");
        println!("    PC:     ${:04X}", self.program_counter);
        println!("    SP:     $01{:02X}", self.stack_pointer);
        println!("    A:      ${:02X}", self.accumulator);
        println!("    X:      ${:02X}", self.register_x);
        println!("    Y:      ${:02X}", self.register_y);
        println!("    Status:  CZIDBVN-");
        println!("            %{:08b}", self.get_status_byte());
    }
}
