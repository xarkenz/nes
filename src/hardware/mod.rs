use instructions::*;

pub mod instructions;

pub struct Machine {
    pub debug: bool,
    pub program_counter: u16,
    pub accumulator: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub stack_pointer: u8,
    status_flags: [bool; Machine::STATUS_FLAG_COUNT],
    internal_ram: [u8; 0x0800], // $0000-$07FF, ..., $1800-$1FFF (mirrored)
    ppu_registers: [u8; 0x0008], // $2000-$2007, ..., $3FF8-$3FFF (mirrored)
    main_memory: [u8; 0xC000], // $4000-$FFFF
}

impl Machine {
    pub const STATUS_FLAG_COUNT: usize = u8::BITS as usize;
    pub const CARRY_FLAG: usize = 0;
    pub const ZERO_FLAG: usize = 1;
    pub const INTERRUPT_DISABLE_FLAG: usize = 2;
    pub const DECIMAL_FLAG: usize = 3; // NOTE: BCD mode is disabled in the Ricoh 2A03
    pub const BREAK_FLAG: usize = 4;
    pub const OVERFLOW_FLAG: usize = 5;
    pub const NEGATIVE_FLAG: usize = 6;
    pub const RESET_HANDLER_ADDRESS: u16 = 0xFFFC;
    pub const IRQ_ADDRESS: u16 = 0xFFFE;
    pub const STACK_PAGE: u16 = 0x0100;

    pub fn new() -> Self {
        Self {
            debug: false,
            program_counter: 0,
            accumulator: 0,
            register_x: 0,
            register_y: 0,
            stack_pointer: 0,
            status_flags: [false; Machine::STATUS_FLAG_COUNT],
            internal_ram: [0; 0x0800],
            ppu_registers: [0; 0x0008],
            main_memory: [0; 0xC000],
        }
    }

    pub fn reset(&mut self) {
        self.program_counter = self.fetch_pair(Self::RESET_HANDLER_ADDRESS);
    }

    pub fn fetch_byte(&self, address: u16) -> u8 {
        match address {
            0x0000 ..= 0x1FFF => {
                self.internal_ram[(address & 0x07FF) as usize]
            }
            0x2000 ..= 0x3FFF => {
                self.ppu_registers[(address & 0x0007) as usize]
            }
            _ => {
                self.main_memory[(address - 0x4000) as usize]
            }
        }
    }

    pub fn store_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000 ..= 0x1FFF => {
                self.internal_ram[(address & 0x07FF) as usize] = value;
            }
            0x2000 ..= 0x3FFF => {
                self.ppu_registers[(address & 0x0007) as usize] = value;
            }
            _ => {
                self.main_memory[(address - 0x4000) as usize] = value;
            }
        }
    }

    pub fn fetch_pair(&self, address: u16) -> u16 {
        let low = self.fetch_byte(address) as u16;
        let high = self.fetch_byte(address.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    pub fn store_pair(&mut self, address: u16, value: u16) {
        let low = (value & 0xFF) as u8;
        let high = (value >> 8) as u8;
        self.store_byte(address, low);
        self.store_byte(address.wrapping_add(1), high);
    }

    pub fn load_memory_bank(&mut self, start_address: u16, bank: &[u8]) {
        if start_address < 0x4000 || start_address as usize + bank.len() > 0x10000 {
            panic!("invalid memory bank start address: ${start_address:04X}");
        }
        let start_index = (start_address - 0x4000) as usize;
        self.main_memory[start_index..(start_index + bank.len())].copy_from_slice(bank);
    }

    pub fn get_flag(&self, flag: usize) -> bool {
        self.status_flags[flag]
    }

    pub fn set_flag(&mut self, flag: usize, value: bool) {
        self.status_flags[flag] = value;
    }

    pub fn get_status_byte(&self) -> u8 {
        let mut status = 0;
        for flag in self.status_flags {
            status <<= 1;
            status |= flag as u8;
        }
        status
    }

    pub fn set_status_byte(&mut self, mut status: u8) {
        for flag in self.status_flags.iter_mut() {
            *flag = (status & 1) != 0;
            status >>= 1;
        }
    }

    pub fn set_result_flags(&mut self, result: u8) {
        self.set_flag(Self::ZERO_FLAG, result == 0);
        self.set_flag(Self::NEGATIVE_FLAG, (result & 0x80) != 0);
    }

    pub fn stack_push_byte(&mut self, value: u8) {
        self.store_byte(Self::STACK_PAGE | self.stack_pointer as u16, value);
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    pub fn stack_pull_byte(&mut self) -> u8 {
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        self.fetch_byte(Self::STACK_PAGE | self.stack_pointer as u16)
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
        let opcode = self.fetch_byte(self.program_counter);
        let instruction = Instruction::decode(opcode);
        if self.debug {
            println!("Opcode: ${opcode:02X}");
            println!("Disassembly: {}", instruction.disassemble(self, self.program_counter));
        }
        instruction.execute(self);
        if self.debug {
            self.print_cpu_state();
        }
    }

    pub fn print_cpu_state(&self) {
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

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}
