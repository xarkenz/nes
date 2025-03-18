use super::*;

#[derive(Copy, Clone, Debug)]
struct Operation {
    mnemonic: &'static str,
    function: fn(AddressingMode, &mut Machine),
}

#[derive(Copy, Clone, Debug)]
enum AddressingMode {
    Implicit,
    Accumulator, // A
    Relative, // ${pc+op}
    Immediate, // #${op}
    ZeroPage, // $00{op}
    ZeroPageX, // $00{op},X
    ZeroPageY, // $00{op},Y
    Absolute, // ${op1}{op0}
    AbsoluteX, // ${op1}{op0},X
    AbsoluteY, // ${op1}{op0},Y
    Indirect, // ($00{op})
    IndirectX, // ($00{op},X)
    IndirectY, // ($00{op}),Y
}

use AddressingMode::*;

impl AddressingMode {
    fn format_instruction(&self, mnemonic: &str, machine: &Machine, opcode_address: u16) -> String {
        let operand_address = opcode_address.wrapping_add(1);
        match self {
            Implicit => format!("{mnemonic}"),
            Accumulator => format!("{mnemonic} A"),
            Relative => format!("{mnemonic} ${:04X}", opcode_address.wrapping_add(2)
                .wrapping_add_signed(machine.fetch_byte(operand_address) as i8 as i16)),
            Immediate => format!("{mnemonic} #${:02X}", machine.fetch_byte(operand_address)),
            ZeroPage => format!("{mnemonic} ${:02X}", machine.fetch_byte(operand_address)),
            ZeroPageX => format!("{mnemonic} ${:02X},X", machine.fetch_byte(operand_address)),
            ZeroPageY => format!("{mnemonic} ${:02X},Y", machine.fetch_byte(operand_address)),
            Absolute => format!("{mnemonic} ${:04X}", machine.fetch_pair(operand_address)),
            AbsoluteX => format!("{mnemonic} ${:04X},X", machine.fetch_pair(operand_address)),
            AbsoluteY => format!("{mnemonic} ${:04X},Y", machine.fetch_pair(operand_address)),
            Indirect => format!("{mnemonic} (${:04X})", machine.fetch_pair(operand_address)),
            IndirectX => format!("{mnemonic} (${:02X},X)", machine.fetch_byte(operand_address)),
            IndirectY => format!("{mnemonic} (${:02X}),Y", machine.fetch_byte(operand_address)),
        }
    }

    fn program_counter_increment(self) -> u16 {
        match self {
            Implicit => 1,
            Accumulator => 1,
            Relative => 2,
            Immediate => 2,
            ZeroPage => 2,
            ZeroPageX => 2,
            ZeroPageY => 2,
            Absolute => 3,
            AbsoluteX => 3,
            AbsoluteY => 3,
            Indirect => 3,
            IndirectX => 2,
            IndirectY => 2,
        }
    }

    fn calculate_address(self, machine: &Machine) -> u16 {
        match self {
            Immediate => {
                machine.program_counter.wrapping_sub(1)
            }
            ZeroPage => {
                machine.fetch_byte(machine.program_counter.wrapping_sub(1)) as u16
            }
            ZeroPageX => {
                machine.fetch_byte(machine.program_counter.wrapping_sub(1)).wrapping_add(machine.register_x) as u16
            }
            ZeroPageY => {
                machine.fetch_byte(machine.program_counter.wrapping_sub(1)).wrapping_add(machine.register_y) as u16
            }
            Absolute => {
                machine.fetch_pair(machine.program_counter.wrapping_sub(2))
            }
            AbsoluteX => {
                machine.fetch_pair(machine.program_counter.wrapping_sub(2)).wrapping_add(machine.register_x as u16)
            }
            AbsoluteY => {
                machine.fetch_pair(machine.program_counter.wrapping_sub(2)).wrapping_add(machine.register_y as u16)
            }
            Indirect => {
                // TODO: Does NES have page boundary bug?
                machine.fetch_pair(machine.fetch_pair(machine.program_counter.wrapping_sub(2)))
            }
            IndirectX => {
                machine.fetch_pair(machine.fetch_byte(machine.program_counter.wrapping_sub(1)).wrapping_add(machine.register_x) as u16)
            }
            IndirectY => {
                machine.fetch_pair(machine.fetch_byte(machine.program_counter.wrapping_sub(1)) as u16).wrapping_add(machine.register_y as u16)
            }
            _ => panic!("cannot calculate address for addressing mode")
        }
    }
}

pub struct Instruction(u8, Operation, AddressingMode);

impl Instruction {
    pub fn decode(opcode: u8) -> &'static Self {
        &INSTRUCTION_SPACE[opcode as usize]
    }

    pub fn disassemble(&self, machine: &Machine, opcode_address: u16) -> String {
        let Self(_opcode, operation, addressing_mode) = *self;
        addressing_mode.format_instruction(operation.mnemonic, machine, opcode_address)
    }

    pub fn execute(&self, machine: &mut Machine) {
        let Self(_opcode, operation, addressing_mode) = *self;
        machine.program_counter = machine.program_counter.wrapping_add(addressing_mode.program_counter_increment());
        (operation.function)(addressing_mode, machine);
    }
}

const INSTRUCTION_SPACE: [Instruction; 0x100] = [
    Instruction(0x00, BRK, Implicit),
    Instruction(0x01, ORA, IndirectX),
    Instruction(0x02, UNMAPPED, Implicit),
    Instruction(0x03, UNMAPPED, Implicit),
    Instruction(0x04, UNMAPPED, Implicit),
    Instruction(0x05, ORA, ZeroPage),
    Instruction(0x06, ASL, ZeroPage),
    Instruction(0x07, UNMAPPED, Implicit),
    Instruction(0x08, PHP, Implicit),
    Instruction(0x09, ORA, Immediate),
    Instruction(0x0A, ASL, Accumulator),
    Instruction(0x0B, UNMAPPED, Implicit),
    Instruction(0x0C, UNMAPPED, Implicit),
    Instruction(0x0D, ORA, Absolute),
    Instruction(0x0E, ASL, Absolute),
    Instruction(0x0F, UNMAPPED, Implicit),
    Instruction(0x10, BPL, Relative),
    Instruction(0x11, ORA, IndirectY),
    Instruction(0x12, UNMAPPED, Implicit),
    Instruction(0x13, UNMAPPED, Implicit),
    Instruction(0x14, UNMAPPED, Implicit),
    Instruction(0x15, ORA, ZeroPageX),
    Instruction(0x16, ASL, ZeroPageX),
    Instruction(0x17, UNMAPPED, Implicit),
    Instruction(0x18, CLC, Implicit),
    Instruction(0x19, ORA, AbsoluteY),
    Instruction(0x1A, UNMAPPED, Implicit),
    Instruction(0x1B, UNMAPPED, Implicit),
    Instruction(0x1C, UNMAPPED, Implicit),
    Instruction(0x1D, ORA, AbsoluteX),
    Instruction(0x1E, ASL, AbsoluteX),
    Instruction(0x1F, UNMAPPED, Implicit),
    Instruction(0x20, JSR, Absolute),
    Instruction(0x21, AND, IndirectX),
    Instruction(0x22, UNMAPPED, Implicit),
    Instruction(0x23, UNMAPPED, Implicit),
    Instruction(0x24, BIT, ZeroPage),
    Instruction(0x25, AND, ZeroPage),
    Instruction(0x26, ROL, ZeroPage),
    Instruction(0x27, UNMAPPED, Implicit),
    Instruction(0x28, PLP, Implicit),
    Instruction(0x29, AND, Immediate),
    Instruction(0x2A, ROL, Accumulator),
    Instruction(0x2B, UNMAPPED, Implicit),
    Instruction(0x2C, BIT, Absolute),
    Instruction(0x2D, AND, Absolute),
    Instruction(0x2E, ROL, Absolute),
    Instruction(0x2F, UNMAPPED, Implicit),
    Instruction(0x30, BMI, Relative),
    Instruction(0x31, AND, IndirectY),
    Instruction(0x32, UNMAPPED, Implicit),
    Instruction(0x33, UNMAPPED, Implicit),
    Instruction(0x34, UNMAPPED, Implicit),
    Instruction(0x35, AND, ZeroPageX),
    Instruction(0x36, ROL, ZeroPageX),
    Instruction(0x37, UNMAPPED, Implicit),
    Instruction(0x38, SEC, Implicit),
    Instruction(0x39, AND, AbsoluteY),
    Instruction(0x3A, UNMAPPED, Implicit),
    Instruction(0x3B, UNMAPPED, Implicit),
    Instruction(0x3C, UNMAPPED, Implicit),
    Instruction(0x3D, AND, AbsoluteX),
    Instruction(0x3E, ROL, AbsoluteX),
    Instruction(0x3F, UNMAPPED, Implicit),
    Instruction(0x40, RTI, Implicit),
    Instruction(0x41, EOR, IndirectX),
    Instruction(0x42, UNMAPPED, Implicit),
    Instruction(0x43, UNMAPPED, Implicit),
    Instruction(0x44, UNMAPPED, Implicit),
    Instruction(0x45, EOR, ZeroPage),
    Instruction(0x46, LSR, ZeroPage),
    Instruction(0x47, UNMAPPED, Implicit),
    Instruction(0x48, PHA, Implicit),
    Instruction(0x49, EOR, Immediate),
    Instruction(0x4A, LSR, Accumulator),
    Instruction(0x4B, UNMAPPED, Implicit),
    Instruction(0x4C, JMP, Absolute),
    Instruction(0x4D, EOR, Absolute),
    Instruction(0x4E, LSR, Absolute),
    Instruction(0x4F, UNMAPPED, Implicit),
    Instruction(0x50, BVC, Relative),
    Instruction(0x51, EOR, IndirectY),
    Instruction(0x52, UNMAPPED, Implicit),
    Instruction(0x53, UNMAPPED, Implicit),
    Instruction(0x54, UNMAPPED, Implicit),
    Instruction(0x55, EOR, ZeroPageX),
    Instruction(0x56, LSR, ZeroPageX),
    Instruction(0x57, UNMAPPED, Implicit),
    Instruction(0x58, CLI, Implicit),
    Instruction(0x59, EOR, AbsoluteY),
    Instruction(0x5A, UNMAPPED, Implicit),
    Instruction(0x5B, UNMAPPED, Implicit),
    Instruction(0x5C, UNMAPPED, Implicit),
    Instruction(0x5D, EOR, AbsoluteX),
    Instruction(0x5E, LSR, AbsoluteX),
    Instruction(0x5F, UNMAPPED, Implicit),
    Instruction(0x60, RTS, Implicit),
    Instruction(0x61, ADC, IndirectX),
    Instruction(0x62, UNMAPPED, Implicit),
    Instruction(0x63, UNMAPPED, Implicit),
    Instruction(0x64, UNMAPPED, Implicit),
    Instruction(0x65, ADC, ZeroPage),
    Instruction(0x66, ROR, ZeroPage),
    Instruction(0x67, UNMAPPED, Implicit),
    Instruction(0x68, PLA, Implicit),
    Instruction(0x69, ADC, Immediate),
    Instruction(0x6A, ROR, Accumulator),
    Instruction(0x6B, UNMAPPED, Implicit),
    Instruction(0x6C, JMP, Indirect),
    Instruction(0x6D, ADC, Absolute),
    Instruction(0x6E, ROR, Absolute),
    Instruction(0x6F, UNMAPPED, Implicit),
    Instruction(0x70, BVS, Relative),
    Instruction(0x71, ADC, IndirectY),
    Instruction(0x72, UNMAPPED, Implicit),
    Instruction(0x73, UNMAPPED, Implicit),
    Instruction(0x74, UNMAPPED, Implicit),
    Instruction(0x75, ADC, ZeroPageX),
    Instruction(0x76, ROR, ZeroPageX),
    Instruction(0x77, UNMAPPED, Implicit),
    Instruction(0x78, SEI, Implicit),
    Instruction(0x79, ADC, AbsoluteY),
    Instruction(0x7A, UNMAPPED, Implicit),
    Instruction(0x7B, UNMAPPED, Implicit),
    Instruction(0x7C, UNMAPPED, Implicit),
    Instruction(0x7D, ADC, AbsoluteX),
    Instruction(0x7E, ROR, AbsoluteX),
    Instruction(0x7F, UNMAPPED, Implicit),
    Instruction(0x80, UNMAPPED, Implicit),
    Instruction(0x81, STA, IndirectX),
    Instruction(0x82, UNMAPPED, Implicit),
    Instruction(0x83, UNMAPPED, Implicit),
    Instruction(0x84, STY, ZeroPage),
    Instruction(0x85, STA, ZeroPage),
    Instruction(0x86, STX, ZeroPage),
    Instruction(0x87, UNMAPPED, Implicit),
    Instruction(0x88, DEY, Implicit),
    Instruction(0x89, UNMAPPED, Implicit),
    Instruction(0x8A, TXA, Implicit),
    Instruction(0x8B, UNMAPPED, Implicit),
    Instruction(0x8C, STY, Absolute),
    Instruction(0x8D, STA, Absolute),
    Instruction(0x8E, STX, Absolute),
    Instruction(0x8F, UNMAPPED, Implicit),
    Instruction(0x90, BCC, Relative),
    Instruction(0x91, STA, IndirectY),
    Instruction(0x92, UNMAPPED, Implicit),
    Instruction(0x93, UNMAPPED, Implicit),
    Instruction(0x94, STY, ZeroPageX),
    Instruction(0x95, STA, ZeroPageX),
    Instruction(0x96, STX, ZeroPageY),
    Instruction(0x97, UNMAPPED, Implicit),
    Instruction(0x98, TYA, Implicit),
    Instruction(0x99, STA, AbsoluteY),
    Instruction(0x9A, TXS, Implicit),
    Instruction(0x9B, UNMAPPED, Implicit),
    Instruction(0x9C, UNMAPPED, Implicit),
    Instruction(0x9D, STA, AbsoluteX),
    Instruction(0x9E, UNMAPPED, Implicit),
    Instruction(0x9F, UNMAPPED, Implicit),
    Instruction(0xA0, LDY, Immediate),
    Instruction(0xA1, LDA, IndirectX),
    Instruction(0xA2, LDX, Immediate),
    Instruction(0xA3, UNMAPPED, Implicit),
    Instruction(0xA4, LDY, ZeroPage),
    Instruction(0xA5, LDA, ZeroPage),
    Instruction(0xA6, LDX, ZeroPage),
    Instruction(0xA7, UNMAPPED, Implicit),
    Instruction(0xA8, TAY, Implicit),
    Instruction(0xA9, LDA, Immediate),
    Instruction(0xAA, TAX, Implicit),
    Instruction(0xAB, UNMAPPED, Implicit),
    Instruction(0xAC, LDY, Absolute),
    Instruction(0xAD, LDA, Absolute),
    Instruction(0xAE, LDX, Absolute),
    Instruction(0xAF, UNMAPPED, Implicit),
    Instruction(0xB0, BCS, Relative),
    Instruction(0xB1, LDA, IndirectY),
    Instruction(0xB2, UNMAPPED, Implicit),
    Instruction(0xB3, UNMAPPED, Implicit),
    Instruction(0xB4, LDY, ZeroPageX),
    Instruction(0xB5, LDA, ZeroPageX),
    Instruction(0xB6, LDX, ZeroPageY),
    Instruction(0xB7, UNMAPPED, Implicit),
    Instruction(0xB8, CLV, Implicit),
    Instruction(0xB9, LDA, AbsoluteY),
    Instruction(0xBA, TSX, Implicit),
    Instruction(0xBB, UNMAPPED, Implicit),
    Instruction(0xBC, LDY, AbsoluteX),
    Instruction(0xBD, LDA, AbsoluteX),
    Instruction(0xBE, LDX, AbsoluteY),
    Instruction(0xBF, UNMAPPED, Implicit),
    Instruction(0xC0, CPY, Immediate),
    Instruction(0xC1, CMP, IndirectX),
    Instruction(0xC2, UNMAPPED, Implicit),
    Instruction(0xC3, UNMAPPED, Implicit),
    Instruction(0xC4, CPY, ZeroPage),
    Instruction(0xC5, CMP, ZeroPage),
    Instruction(0xC6, DEC, ZeroPage),
    Instruction(0xC7, UNMAPPED, Implicit),
    Instruction(0xC8, INY, Implicit),
    Instruction(0xC9, CMP, Immediate),
    Instruction(0xCA, DEX, Implicit),
    Instruction(0xCB, UNMAPPED, Implicit),
    Instruction(0xCC, CPY, Absolute),
    Instruction(0xCD, CMP, Absolute),
    Instruction(0xCE, DEC, Absolute),
    Instruction(0xCF, UNMAPPED, Implicit),
    Instruction(0xD0, BNE, Relative),
    Instruction(0xD1, CMP, IndirectY),
    Instruction(0xD2, UNMAPPED, Implicit),
    Instruction(0xD3, UNMAPPED, Implicit),
    Instruction(0xD4, UNMAPPED, Implicit),
    Instruction(0xD5, CMP, ZeroPageX),
    Instruction(0xD6, DEC, ZeroPageX),
    Instruction(0xD7, UNMAPPED, Implicit),
    Instruction(0xD8, CLD, Implicit),
    Instruction(0xD9, CMP, AbsoluteY),
    Instruction(0xDA, UNMAPPED, Implicit),
    Instruction(0xDB, UNMAPPED, Implicit),
    Instruction(0xDC, UNMAPPED, Implicit),
    Instruction(0xDD, CMP, AbsoluteX),
    Instruction(0xDE, DEC, AbsoluteX),
    Instruction(0xDF, UNMAPPED, Implicit),
    Instruction(0xE0, CPX, Immediate),
    Instruction(0xE1, SBC, IndirectX),
    Instruction(0xE2, UNMAPPED, Implicit),
    Instruction(0xE3, UNMAPPED, Implicit),
    Instruction(0xE4, CPX, ZeroPage),
    Instruction(0xE5, SBC, ZeroPage),
    Instruction(0xE6, INC, ZeroPage),
    Instruction(0xE7, UNMAPPED, Implicit),
    Instruction(0xE8, INX, Implicit),
    Instruction(0xE9, SBC, Immediate),
    Instruction(0xEA, NOP, Implicit),
    Instruction(0xEB, UNMAPPED, Implicit),
    Instruction(0xEC, CPX, Absolute),
    Instruction(0xED, SBC, Absolute),
    Instruction(0xEE, INC, Absolute),
    Instruction(0xEF, UNMAPPED, Implicit),
    Instruction(0xF0, BEQ, Relative),
    Instruction(0xF1, SBC, IndirectY),
    Instruction(0xF2, UNMAPPED, Implicit),
    Instruction(0xF3, UNMAPPED, Implicit),
    Instruction(0xF4, UNMAPPED, Implicit),
    Instruction(0xF5, SBC, ZeroPageX),
    Instruction(0xF6, INC, ZeroPageX),
    Instruction(0xF7, UNMAPPED, Implicit),
    Instruction(0xF8, SED, Implicit),
    Instruction(0xF9, SBC, AbsoluteY),
    Instruction(0xFA, UNMAPPED, Implicit),
    Instruction(0xFB, UNMAPPED, Implicit),
    Instruction(0xFC, UNMAPPED, Implicit),
    Instruction(0xFD, SBC, AbsoluteX),
    Instruction(0xFE, INC, AbsoluteX),
    Instruction(0xFF, UNMAPPED, Implicit),
];

// TODO: these four functions are almost certainly not correct

const fn carrying_add(lhs: u8, rhs: u8, carry: bool) -> (u8, bool) {
    let (intermediate, carry_1) = lhs.overflowing_add(rhs);
    let (result, carry_2) = intermediate.overflowing_add(carry as u8);
    (result, carry_1 || carry_2)
}

const fn carrying_add_overflows(lhs: u8, rhs: u8, carry: bool) -> bool {
    let (intermediate, carry_1) = (lhs as i8).overflowing_add(rhs as i8);
    let (_, carry_2) = intermediate.overflowing_add(carry as i8);
    carry_1 != carry_2
}

const fn carrying_sub(lhs: u8, rhs: u8, carry: bool) -> (u8, bool) {
    let (intermediate, carry_1) = lhs.overflowing_sub(rhs);
    let (result, carry_2) = intermediate.overflowing_sub(carry as u8);
    (result, carry_1 || carry_2)
}

const fn carrying_sub_overflows(lhs: u8, rhs: u8, carry: bool) -> bool {
    let (intermediate, carry_1) = (lhs as i8).overflowing_sub(rhs as i8);
    let (_, carry_2) = intermediate.overflowing_sub(carry as i8);
    carry_1 != carry_2
}

fn process_branch(machine: &mut Machine) {
    let offset = machine.fetch_byte(machine.program_counter.wrapping_sub(1));
    // Offset is sign-extended
    machine.program_counter = machine.program_counter.wrapping_add_signed(offset as i8 as i16);
}

const UNMAPPED: Operation = Operation {
    mnemonic: "???",
    function: |_, _| {
        // Uh oh
    },
};

// Add with Carry
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const ADC: Operation = Operation {
    mnemonic: "ADC",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let addend = machine.fetch_byte(address);
        let carry_in = machine.get_flag(Machine::CARRY_FLAG);
        let (result, carry_out) = carrying_add(machine.accumulator, addend, carry_in);
        let overflows = carrying_add_overflows(machine.accumulator, addend, carry_in);
        machine.accumulator = result;

        machine.set_flag(Machine::CARRY_FLAG, carry_out);
        machine.set_flag(Machine::OVERFLOW_FLAG, overflows);
        machine.set_result_flags(result);
    },
};

// Bitwise AND
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const AND: Operation = Operation {
    mnemonic: "AND",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.accumulator &= machine.fetch_byte(address);

        machine.set_result_flags(machine.accumulator);
    },
};

// Arithmetic Shift Left
// Accumulator, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const ASL: Operation = Operation {
    mnemonic: "ASL",
    function: |addressing_mode, machine| {
        let carry_out;
        let result;
        if let Accumulator = addressing_mode {
            carry_out = (machine.accumulator & 0x80) != 0;
            result = machine.accumulator << 1;
            machine.accumulator = result;
        }
        else {
            let address = addressing_mode.calculate_address(machine);
            let value = machine.fetch_byte(address);
            carry_out = (value & 0x80) != 0;
            result = value << 1;
            machine.store_byte(address, result);
        }

        machine.set_flag(Machine::CARRY_FLAG, carry_out);
        machine.set_result_flags(result);
    },
};

// Branch if Carry Clear
// Relative
const BCC: Operation = Operation {
    mnemonic: "BCC",
    function: |_, machine| {
        if !machine.get_flag(Machine::CARRY_FLAG) {
            process_branch(machine);
        }
    },
};

// Branch if Carry Set
// Relative
const BCS: Operation = Operation {
    mnemonic: "BCS",
    function: |_, machine| {
        if machine.get_flag(Machine::CARRY_FLAG) {
            process_branch(machine);
        }
    },
};

// Branch if Equal
// Relative
const BEQ: Operation = Operation {
    mnemonic: "BEQ",
    function: |_, machine| {
        if machine.get_flag(Machine::ZERO_FLAG) {
            process_branch(machine);
        }
    },
};

// Bit Test
// ZeroPage, Absolute
const BIT: Operation = Operation {
    mnemonic: "BIT",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let result = machine.fetch_byte(address) & machine.accumulator;

        machine.set_flag(Machine::OVERFLOW_FLAG, (result & 0x40) != 0);
        machine.set_result_flags(result);
    },
};

// Branch if Minus
// Relative
const BMI: Operation = Operation {
    mnemonic: "BMI",
    function: |_, machine| {
        if machine.get_flag(Machine::NEGATIVE_FLAG) {
            process_branch(machine);
        }
    },
};

// Branch if Not Equal
// Relative
const BNE: Operation = Operation {
    mnemonic: "BNE",
    function: |_, machine| {
        if !machine.get_flag(Machine::ZERO_FLAG) {
            process_branch(machine);
        }
    },
};

// Branch if Plus
// Relative
const BPL: Operation = Operation {
    mnemonic: "BPL",
    function: |_, machine| {
        if !machine.get_flag(Machine::NEGATIVE_FLAG) {
            process_branch(machine);
        }
    },
};

// Force Interrupt
// Implicit
const BRK: Operation = Operation {
    mnemonic: "BRK",
    function: |_, machine| {
        machine.stack_push_pair(machine.program_counter);
        machine.stack_push_byte(machine.get_status_byte());
        machine.program_counter = machine.fetch_pair(Machine::IRQ_ADDRESS);
        machine.set_flag(Machine::BREAK_FLAG, true);
    },
};

// Branch if Overflow Clear
// Relative
const BVC: Operation = Operation {
    mnemonic: "BVC",
    function: |_, machine| {
        if !machine.get_flag(Machine::OVERFLOW_FLAG) {
            process_branch(machine);
        }
    },
};

// Branch if Overflow Set
// Relative
const BVS: Operation = Operation {
    mnemonic: "BVS",
    function: |_, machine| {
        if machine.get_flag(Machine::OVERFLOW_FLAG) {
            process_branch(machine);
        }
    },
};

// Clear Carry Flag
// Implicit
const CLC: Operation = Operation {
    mnemonic: "CLC",
    function: |_, machine| {
        machine.set_flag(Machine::CARRY_FLAG, false);
    },
};

// Clear Decimal Flag
// Implicit
const CLD: Operation = Operation {
    mnemonic: "CLD",
    function: |_, machine| {
        // NOTE: BCD mode is disabled in the Ricoh 2A03
        machine.set_flag(Machine::DECIMAL_FLAG, false);
    },
};

// Clear Interrupt Disable Flag
// Implicit
const CLI: Operation = Operation {
    mnemonic: "CLI",
    function: |_, machine| {
        machine.set_flag(Machine::INTERRUPT_DISABLE_FLAG, false);
    },
};

// Clear Overflow Flag
// Implicit
const CLV: Operation = Operation {
    mnemonic: "CLV",
    function: |_, machine| {
        machine.set_flag(Machine::OVERFLOW_FLAG, false);
    },
};

// Compare
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const CMP: Operation = Operation {
    mnemonic: "CMP",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let value = machine.fetch_byte(address);
        let result = machine.accumulator.wrapping_sub(value);

        machine.set_flag(Machine::CARRY_FLAG, machine.accumulator as i8 >= value as i8);
        machine.set_result_flags(result);
    },
};

// Compare X Register
// Immediate, ZeroPage, Absolute
const CPX: Operation = Operation {
    mnemonic: "CPX",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let value = machine.fetch_byte(address);
        let result = machine.register_x.wrapping_sub(value);

        machine.set_flag(Machine::CARRY_FLAG, machine.register_x as i8 >= value as i8);
        machine.set_result_flags(result);
    },
};

// Compare Y Register
// Immediate, ZeroPage, Absolute
const CPY: Operation = Operation {
    mnemonic: "CPY",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let value = machine.fetch_byte(address);
        let result = machine.register_y.wrapping_sub(value);

        machine.set_flag(Machine::CARRY_FLAG, machine.register_y as i8 >= value as i8);
        machine.set_result_flags(result);
    },
};

// Decrement Memory
// ZeroPage, ZeroPageX, Absolute, AbsoluteX
const DEC: Operation = Operation {
    mnemonic: "DEC",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let result = machine.fetch_byte(address).wrapping_sub(1);
        machine.store_byte(address, result);

        machine.set_result_flags(result);
    },
};

// Decrement X Register
// Implicit
const DEX: Operation = Operation {
    mnemonic: "DEX",
    function: |_, machine| {
        machine.register_x = machine.register_x.wrapping_sub(1);

        machine.set_result_flags(machine.register_x);
    },
};

// Decrement Y Register
// Implicit
const DEY: Operation = Operation {
    mnemonic: "DEY",
    function: |_, machine| {
        machine.register_y = machine.register_y.wrapping_sub(1);

        machine.set_result_flags(machine.register_y);
    },
};

// Bitwise Exclusive OR
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const EOR: Operation = Operation {
    mnemonic: "EOR",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.accumulator ^= machine.fetch_byte(address);

        machine.set_result_flags(machine.accumulator);
    },
};

// Increment Memory
// ZeroPage, ZeroPageX, Absolute, AbsoluteX
const INC: Operation = Operation {
    mnemonic: "INC",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let result = machine.fetch_byte(address).wrapping_add(1);
        machine.store_byte(address, result);

        machine.set_result_flags(result);
    },
};

// Increment X Register
// Implicit
const INX: Operation = Operation {
    mnemonic: "INX",
    function: |_, machine| {
        machine.register_x = machine.register_x.wrapping_add(1);

        machine.set_result_flags(machine.register_x);
    },
};

// Increment Y Register
// Implicit
const INY: Operation = Operation {
    mnemonic: "INY",
    function: |_, machine| {
        machine.register_y = machine.register_y.wrapping_add(1);

        machine.set_result_flags(machine.register_y);
    },
};

// Jump
// Absolute, Indirect
const JMP: Operation = Operation {
    mnemonic: "JMP",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.program_counter = address;
    },
};

// Jump to Subroutine
// Absolute
const JSR: Operation = Operation {
    mnemonic: "JSR",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.stack_push_pair(machine.program_counter);
        machine.program_counter = address;
    },
};

// Load Accumulator
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const LDA: Operation = Operation {
    mnemonic: "LDA",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.accumulator = machine.fetch_byte(address);

        machine.set_result_flags(machine.accumulator);
    },
};

// Load X Register
// Immediate, ZeroPage, ZeroPageY, Absolute, AbsoluteY
const LDX: Operation = Operation {
    mnemonic: "LDX",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.register_x = machine.fetch_byte(address);

        machine.set_result_flags(machine.register_x);
    },
};

// Load Y Register
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const LDY: Operation = Operation {
    mnemonic: "LDY",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.register_y = machine.fetch_byte(address);

        machine.set_result_flags(machine.register_y);
    },
};

// Logical Shift Right
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const LSR: Operation = Operation {
    mnemonic: "LSR",
    function: |addressing_mode, machine| {
        let carry_out;
        let result;
        if let Accumulator = addressing_mode {
            carry_out = (machine.accumulator & 1) != 0;
            result = machine.accumulator >> 1;
            machine.accumulator = result;
        }
        else {
            let address = addressing_mode.calculate_address(machine);
            let value = machine.fetch_byte(address);
            carry_out = (value & 1) != 0;
            result = value >> 1;
            machine.store_byte(address, result);
        }

        machine.set_flag(Machine::CARRY_FLAG, carry_out);
        machine.set_result_flags(result);
    },
};

// No Operation
// Implicit
const NOP: Operation = Operation {
    mnemonic: "NOP",
    function: |_, _| {
        // Astonishingly, absolutely nothing happens
    },
};

// Bitwise Inclusive OR
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const ORA: Operation = Operation {
    mnemonic: "ORA",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.accumulator |= machine.fetch_byte(address);

        machine.set_result_flags(machine.accumulator);
    },
};

// Push Accumulator
// Implicit
const PHA: Operation = Operation {
    mnemonic: "PHA",
    function: |_, machine| {
        machine.stack_push_byte(machine.accumulator);
    },
};

// Push Processor Status
// Implicit
const PHP: Operation = Operation {
    mnemonic: "PHP",
    function: |_, machine| {
        machine.stack_push_byte(machine.get_status_byte());
    },
};

// Pull Accumulator
// Implicit
const PLA: Operation = Operation {
    mnemonic: "PLA",
    function: |_, machine| {
        machine.accumulator = machine.stack_pull_byte();

        machine.set_result_flags(machine.accumulator);
    },
};

// Pull Processor Status
// Implicit
const PLP: Operation = Operation {
    mnemonic: "PLP",
    function: |_, machine| {
        let status_byte = machine.stack_pull_byte();
        machine.set_status_byte(status_byte);
    },
};

// Rotate Left
// Accumulator, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const ROL: Operation = Operation {
    mnemonic: "ROL",
    function: |addressing_mode, machine| {
        let carry_in = machine.get_flag(Machine::CARRY_FLAG);
        let carry_out;
        let result;
        if let Accumulator = addressing_mode {
            carry_out = (machine.accumulator & 0x80) != 0;
            result = machine.accumulator << 1 | carry_in as u8;
            machine.accumulator = result;
        }
        else {
            let address = addressing_mode.calculate_address(machine);
            let value = machine.fetch_byte(address);
            carry_out = (value & 0x80) != 0;
            result = value << 1 | carry_in as u8;
            machine.store_byte(address, result);
        }

        machine.set_flag(Machine::CARRY_FLAG, carry_out);
        machine.set_result_flags(result);
    },
};

// Rotate Right
// Accumulator, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const ROR: Operation = Operation {
    mnemonic: "ROR",
    function: |addressing_mode, machine| {
        let carry_in = machine.get_flag(Machine::CARRY_FLAG);
        let carry_out;
        let result;
        if let Accumulator = addressing_mode {
            carry_out = (machine.accumulator & 1) != 0;
            result = machine.accumulator >> 1 | (carry_in as u8) << 7;
            machine.accumulator = result;
        }
        else {
            let address = addressing_mode.calculate_address(machine);
            let value = machine.fetch_byte(address);
            carry_out = (value & 1) != 0;
            result = value >> 1 | (carry_in as u8) << 7;
            machine.store_byte(address, result);
        }

        machine.set_flag(Machine::CARRY_FLAG, carry_out);
        machine.set_result_flags(result);
    },
};

// Return from Interrupt
// Implicit
const RTI: Operation = Operation {
    mnemonic: "RTI",
    function: |_, machine| {
        let status_byte = machine.stack_pull_byte();
        let program_counter = machine.stack_pull_pair();
        machine.set_status_byte(status_byte);
        machine.program_counter = program_counter;
    },
};

// Return from Subroutine
// Implicit
const RTS: Operation = Operation {
    mnemonic: "RTS",
    function: |_, machine| {
        // TODO: minus one?
        let program_counter = machine.stack_pull_pair();
        machine.program_counter = program_counter;
    },
};

// Subtract with Carry
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const SBC: Operation = Operation {
    mnemonic: "SBC",
    function: |addressing_mode, machine| {
        // TODO: definitely wrong
        let address = addressing_mode.calculate_address(machine);
        let addend = machine.fetch_byte(address);
        let carry_in = !machine.get_flag(Machine::CARRY_FLAG);
        let (result, carry_out) = carrying_sub(machine.accumulator, addend, carry_in);
        let overflows = carrying_sub_overflows(machine.accumulator, addend, carry_in);
        machine.accumulator = result;

        machine.set_flag(Machine::CARRY_FLAG, carry_out);
        machine.set_flag(Machine::OVERFLOW_FLAG, overflows);
        machine.set_result_flags(result);
    },
};

// Set Carry Flag
// Implicit
const SEC: Operation = Operation {
    mnemonic: "SEC",
    function: |_, machine| {
        machine.set_flag(Machine::CARRY_FLAG, true);
    },
};

// Set Decimal Flag
// Implicit
const SED: Operation = Operation {
    mnemonic: "SED",
    function: |_, machine| {
        // NOTE: BCD mode is disabled in the Ricoh 2A03
        machine.set_flag(Machine::DECIMAL_FLAG, true);
    },
};

// Set Interrupt Disable Flag
// Implicit
const SEI: Operation = Operation {
    mnemonic: "SEI",
    function: |_, machine| {
        machine.set_flag(Machine::INTERRUPT_DISABLE_FLAG, true);
    },
};

// Store Accumulator
// ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const STA: Operation = Operation {
    mnemonic: "STA",
    function: |addressing_mode, machine| {
    let address = addressing_mode.calculate_address(machine);
        machine.store_byte(address, machine.accumulator);
    },
};

// Store X Register
// ZeroPage, ZeroPageY, Absolute
const STX: Operation = Operation {
    mnemonic: "STX",
    function: |addressing_mode, machine| {
    let address = addressing_mode.calculate_address(machine);
        machine.store_byte(address, machine.register_x);
    },
};

// Store Y Register
// ZeroPage, ZeroPageX, Absolute
const STY: Operation = Operation {
    mnemonic: "STY",
    function: |addressing_mode, machine| {
    let address = addressing_mode.calculate_address(machine);
        machine.store_byte(address, machine.register_y);
    },
};

// Transfer Accumulator to X Register
// Implicit
const TAX: Operation = Operation {
    mnemonic: "TAX",
    function: |_, machine| {
        machine.register_x = machine.accumulator;

        machine.set_result_flags(machine.register_x);
    },
};

// Transfer Accumulator to Y Register
// Implicit
const TAY: Operation = Operation {
    mnemonic: "TAY",
    function: |_, machine| {
        machine.register_y = machine.accumulator;

        machine.set_result_flags(machine.register_y);
    },
};

// Transfer Stack Pointer to X Register
// Implicit
const TSX: Operation = Operation {
    mnemonic: "TSX",
    function: |_, machine| {
        machine.register_x = machine.stack_pointer;

        machine.set_result_flags(machine.register_x);
    },
};

// Transfer X Register to Accumulator
// Implicit
const TXA: Operation = Operation {
    mnemonic: "TXA",
    function: |_, machine| {
        machine.accumulator = machine.register_x;

        machine.set_result_flags(machine.accumulator);
    },
};

// Transfer X Register to Stack Pointer
// Implicit
const TXS: Operation = Operation {
    mnemonic: "TXS",
    function: |_, machine| {
        machine.stack_pointer = machine.register_x;
    },
};

// Transfer Y Register to Accumulator
// Implicit
const TYA: Operation = Operation {
    mnemonic: "TYA",
    function: |_, machine| {
        machine.accumulator = machine.register_y;

        machine.set_result_flags(machine.accumulator);
    },
};
