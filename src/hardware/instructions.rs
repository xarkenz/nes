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
                .wrapping_add_signed(machine.read_byte_silent(operand_address) as i8 as i16)),
            Immediate => format!("{mnemonic} #${:02X}", machine.read_byte_silent(operand_address)),
            ZeroPage => format!("{mnemonic} ${:02X}", machine.read_byte_silent(operand_address)),
            ZeroPageX => format!("{mnemonic} ${:02X},X", machine.read_byte_silent(operand_address)),
            ZeroPageY => format!("{mnemonic} ${:02X},Y", machine.read_byte_silent(operand_address)),
            Absolute => format!("{mnemonic} ${:04X}", machine.read_pair_silent(operand_address)),
            AbsoluteX => format!("{mnemonic} ${:04X},X", machine.read_pair_silent(operand_address)),
            AbsoluteY => format!("{mnemonic} ${:04X},Y", machine.read_pair_silent(operand_address)),
            Indirect => format!("{mnemonic} (${:04X})", machine.read_pair_silent(operand_address)),
            IndirectX => format!("{mnemonic} (${:02X},X)", machine.read_byte_silent(operand_address)),
            IndirectY => format!("{mnemonic} (${:02X}),Y", machine.read_byte_silent(operand_address)),
        }
    }

    fn get_instruction_size(self) -> u16 {
        match self {
            Implicit | Accumulator
                => 1,
            Relative | Immediate | ZeroPage | ZeroPageX | ZeroPageY | IndirectX | IndirectY
                => 2,
            Absolute | AbsoluteX | AbsoluteY | Indirect
                => 3,
        }
    }

    fn calculate_address(self, machine: &mut Machine) -> u16 {
        match self {
            Immediate => {
                machine.cpu.program_counter.wrapping_sub(1)
            }
            ZeroPage => {
                machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)) as u16
            }
            ZeroPageX => {
                machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)).wrapping_add(machine.cpu.register_x) as u16
            }
            ZeroPageY => {
                machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)).wrapping_add(machine.cpu.register_y) as u16
            }
            Absolute => {
                machine.read_pair(machine.cpu.program_counter.wrapping_sub(2))
            }
            AbsoluteX => {
                machine.read_pair(machine.cpu.program_counter.wrapping_sub(2)).wrapping_add(machine.cpu.register_x as u16)
            }
            AbsoluteY => {
                machine.read_pair(machine.cpu.program_counter.wrapping_sub(2)).wrapping_add(machine.cpu.register_y as u16)
            }
            Indirect => {
                // TODO: Does NES have page boundary bug?
                let address = machine.read_pair(machine.cpu.program_counter.wrapping_sub(2));
                machine.read_pair(address)
            }
            IndirectX => {
                let address = machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)).wrapping_add(machine.cpu.register_x) as u16;
                machine.read_pair(address)
            }
            IndirectY => {
                let address = machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)) as u16;
                machine.read_pair(address).wrapping_add(machine.cpu.register_y as u16)
            }
            _ => panic!("cannot calculate address for addressing mode")
        }
    }
}

pub struct Instruction {
    opcode: u8,
    operation: Operation,
    addressing_mode: AddressingMode,
    cycles_needed: u16,
}

impl Instruction {
    const fn new(opcode: u8, operation: Operation, addressing_mode: AddressingMode, cycles_needed: u16) -> Self {
        Self {
            opcode,
            operation,
            addressing_mode,
            cycles_needed,
        }
    }

    pub fn decode(opcode: u8) -> &'static Self {
        &INSTRUCTIONS[opcode as usize]
    }

    pub fn opcode(&self) -> u8 {
        self.opcode
    }

    pub fn mnemonic(&self) -> &'static str {
        self.operation.mnemonic
    }
    
    pub fn size_bytes(&self) -> u16 {
        self.addressing_mode.get_instruction_size()
    }

    pub fn cycles_needed(&self) -> u16 {
        self.cycles_needed
    }

    pub fn disassemble(&self, machine: &Machine, opcode_address: u16) -> String {
        self.addressing_mode.format_instruction(self.mnemonic(), machine, opcode_address)
    }

    pub fn execute(&self, machine: &mut Machine) {
        // println!("${:04X}: {}", machine.cpu.program_counter, self.disassemble(&machine, machine.cpu.program_counter));
        machine.cpu.program_counter = machine.cpu.program_counter.wrapping_add(self.size_bytes());
        (self.operation.function)(self.addressing_mode, machine);
    }
}

// Cycle counts are from https://github.com/jslepicka/nemulator/blob/5dccc9ca8cdd8a8593303ecce2b433ae14f437ca/nes/cpu.cpp#L12
// TODO: some of the indexed ones add 1 cycle if crossing a page boundary, but not all
const INSTRUCTIONS: [Instruction; 0x100] = [
    Instruction::new(0x00, BRK, Implicit, 7),
    Instruction::new(0x01, ORA, IndirectX, 6),
    Instruction::new(0x02, UNMAPPED, Implicit, 1),
    Instruction::new(0x03, UNMAPPED, Implicit, 8),
    Instruction::new(0x04, UNMAPPED, Implicit, 3),
    Instruction::new(0x05, ORA, ZeroPage, 3),
    Instruction::new(0x06, ASL, ZeroPage, 5),
    Instruction::new(0x07, UNMAPPED, Implicit, 5),
    Instruction::new(0x08, PHP, Implicit, 3),
    Instruction::new(0x09, ORA, Immediate, 2),
    Instruction::new(0x0A, ASL, Accumulator, 2),
    Instruction::new(0x0B, UNMAPPED, Implicit, 2),
    Instruction::new(0x0C, UNMAPPED, Implicit, 4),
    Instruction::new(0x0D, ORA, Absolute, 4),
    Instruction::new(0x0E, ASL, Absolute, 6),
    Instruction::new(0x0F, UNMAPPED, Implicit, 6),
    Instruction::new(0x10, BPL, Relative, 2),
    Instruction::new(0x11, ORA, IndirectY, 5),
    Instruction::new(0x12, UNMAPPED, Implicit, 1),
    Instruction::new(0x13, UNMAPPED, Implicit, 8),
    Instruction::new(0x14, UNMAPPED, Implicit, 4),
    Instruction::new(0x15, ORA, ZeroPageX, 4),
    Instruction::new(0x16, ASL, ZeroPageX, 6),
    Instruction::new(0x17, UNMAPPED, Implicit, 6),
    Instruction::new(0x18, CLC, Implicit, 2),
    Instruction::new(0x19, ORA, AbsoluteY, 4),
    Instruction::new(0x1A, UNMAPPED, Implicit, 2),
    Instruction::new(0x1B, UNMAPPED, Implicit, 7),
    Instruction::new(0x1C, UNMAPPED, Implicit, 4),
    Instruction::new(0x1D, ORA, AbsoluteX, 4),
    Instruction::new(0x1E, ASL, AbsoluteX, 7),
    Instruction::new(0x1F, UNMAPPED, Implicit, 7),
    Instruction::new(0x20, JSR, Absolute, 6),
    Instruction::new(0x21, AND, IndirectX, 6),
    Instruction::new(0x22, UNMAPPED, Implicit, 1),
    Instruction::new(0x23, UNMAPPED, Implicit, 8),
    Instruction::new(0x24, BIT, ZeroPage, 3),
    Instruction::new(0x25, AND, ZeroPage, 3),
    Instruction::new(0x26, ROL, ZeroPage, 5),
    Instruction::new(0x27, UNMAPPED, Implicit, 5),
    Instruction::new(0x28, PLP, Implicit, 4),
    Instruction::new(0x29, AND, Immediate, 2),
    Instruction::new(0x2A, ROL, Accumulator, 2),
    Instruction::new(0x2B, UNMAPPED, Implicit, 2),
    Instruction::new(0x2C, BIT, Absolute, 4),
    Instruction::new(0x2D, AND, Absolute, 4),
    Instruction::new(0x2E, ROL, Absolute, 6),
    Instruction::new(0x2F, UNMAPPED, Implicit, 6),
    Instruction::new(0x30, BMI, Relative, 2),
    Instruction::new(0x31, AND, IndirectY, 5),
    Instruction::new(0x32, UNMAPPED, Implicit, 1),
    Instruction::new(0x33, UNMAPPED, Implicit, 8),
    Instruction::new(0x34, UNMAPPED, Implicit, 4),
    Instruction::new(0x35, AND, ZeroPageX, 4),
    Instruction::new(0x36, ROL, ZeroPageX, 6),
    Instruction::new(0x37, UNMAPPED, Implicit, 6),
    Instruction::new(0x38, SEC, Implicit, 2),
    Instruction::new(0x39, AND, AbsoluteY, 4),
    Instruction::new(0x3A, UNMAPPED, Implicit, 2),
    Instruction::new(0x3B, UNMAPPED, Implicit, 7),
    Instruction::new(0x3C, UNMAPPED, Implicit, 4),
    Instruction::new(0x3D, AND, AbsoluteX, 4),
    Instruction::new(0x3E, ROL, AbsoluteX, 7),
    Instruction::new(0x3F, UNMAPPED, Implicit, 7),
    Instruction::new(0x40, RTI, Implicit, 6),
    Instruction::new(0x41, EOR, IndirectX, 6),
    Instruction::new(0x42, UNMAPPED, Implicit, 1),
    Instruction::new(0x43, UNMAPPED, Implicit, 8),
    Instruction::new(0x44, UNMAPPED, Implicit, 3),
    Instruction::new(0x45, EOR, ZeroPage, 3),
    Instruction::new(0x46, LSR, ZeroPage, 5),
    Instruction::new(0x47, UNMAPPED, Implicit, 5),
    Instruction::new(0x48, PHA, Implicit, 3),
    Instruction::new(0x49, EOR, Immediate, 2),
    Instruction::new(0x4A, LSR, Accumulator, 2),
    Instruction::new(0x4B, UNMAPPED, Implicit, 2),
    Instruction::new(0x4C, JMP, Absolute, 3),
    Instruction::new(0x4D, EOR, Absolute, 4),
    Instruction::new(0x4E, LSR, Absolute, 6),
    Instruction::new(0x4F, UNMAPPED, Implicit, 6),
    Instruction::new(0x50, BVC, Relative, 2),
    Instruction::new(0x51, EOR, IndirectY, 5),
    Instruction::new(0x52, UNMAPPED, Implicit, 1),
    Instruction::new(0x53, UNMAPPED, Implicit, 8),
    Instruction::new(0x54, UNMAPPED, Implicit, 4),
    Instruction::new(0x55, EOR, ZeroPageX, 4),
    Instruction::new(0x56, LSR, ZeroPageX, 6),
    Instruction::new(0x57, UNMAPPED, Implicit, 6),
    Instruction::new(0x58, CLI, Implicit, 2),
    Instruction::new(0x59, EOR, AbsoluteY, 4),
    Instruction::new(0x5A, UNMAPPED, Implicit, 2),
    Instruction::new(0x5B, UNMAPPED, Implicit, 7),
    Instruction::new(0x5C, UNMAPPED, Implicit, 4),
    Instruction::new(0x5D, EOR, AbsoluteX, 4),
    Instruction::new(0x5E, LSR, AbsoluteX, 7),
    Instruction::new(0x5F, UNMAPPED, Implicit, 7),
    Instruction::new(0x60, RTS, Implicit, 6),
    Instruction::new(0x61, ADC, IndirectX, 6),
    Instruction::new(0x62, UNMAPPED, Implicit, 1),
    Instruction::new(0x63, UNMAPPED, Implicit, 8),
    Instruction::new(0x64, UNMAPPED, Implicit, 3),
    Instruction::new(0x65, ADC, ZeroPage, 3),
    Instruction::new(0x66, ROR, ZeroPage, 5),
    Instruction::new(0x67, UNMAPPED, Implicit, 5),
    Instruction::new(0x68, PLA, Implicit, 4),
    Instruction::new(0x69, ADC, Immediate, 2),
    Instruction::new(0x6A, ROR, Accumulator, 2),
    Instruction::new(0x6B, UNMAPPED, Implicit, 2),
    Instruction::new(0x6C, JMP, Indirect, 5),
    Instruction::new(0x6D, ADC, Absolute, 4),
    Instruction::new(0x6E, ROR, Absolute, 6),
    Instruction::new(0x6F, UNMAPPED, Implicit, 6),
    Instruction::new(0x70, BVS, Relative, 2),
    Instruction::new(0x71, ADC, IndirectY, 5),
    Instruction::new(0x72, UNMAPPED, Implicit, 1),
    Instruction::new(0x73, UNMAPPED, Implicit, 8),
    Instruction::new(0x74, UNMAPPED, Implicit, 4),
    Instruction::new(0x75, ADC, ZeroPageX, 4),
    Instruction::new(0x76, ROR, ZeroPageX, 6),
    Instruction::new(0x77, UNMAPPED, Implicit, 6),
    Instruction::new(0x78, SEI, Implicit, 2),
    Instruction::new(0x79, ADC, AbsoluteY, 4),
    Instruction::new(0x7A, UNMAPPED, Implicit, 2),
    Instruction::new(0x7B, UNMAPPED, Implicit, 7),
    Instruction::new(0x7C, UNMAPPED, Implicit, 4),
    Instruction::new(0x7D, ADC, AbsoluteX, 4),
    Instruction::new(0x7E, ROR, AbsoluteX, 7),
    Instruction::new(0x7F, UNMAPPED, Implicit, 7),
    Instruction::new(0x80, UNMAPPED, Implicit, 2),
    Instruction::new(0x81, STA, IndirectX, 6),
    Instruction::new(0x82, UNMAPPED, Implicit, 2),
    Instruction::new(0x83, UNMAPPED, Implicit, 6),
    Instruction::new(0x84, STY, ZeroPage, 3),
    Instruction::new(0x85, STA, ZeroPage, 3),
    Instruction::new(0x86, STX, ZeroPage, 3),
    Instruction::new(0x87, UNMAPPED, Implicit, 3),
    Instruction::new(0x88, DEY, Implicit, 2),
    Instruction::new(0x89, UNMAPPED, Implicit, 2),
    Instruction::new(0x8A, TXA, Implicit, 2),
    Instruction::new(0x8B, UNMAPPED, Implicit, 2),
    Instruction::new(0x8C, STY, Absolute, 4),
    Instruction::new(0x8D, STA, Absolute, 4),
    Instruction::new(0x8E, STX, Absolute, 4),
    Instruction::new(0x8F, UNMAPPED, Implicit, 4),
    Instruction::new(0x90, BCC, Relative, 2),
    Instruction::new(0x91, STA, IndirectY, 6),
    Instruction::new(0x92, UNMAPPED, Implicit, 1),
    Instruction::new(0x93, UNMAPPED, Implicit, 6),
    Instruction::new(0x94, STY, ZeroPageX, 4),
    Instruction::new(0x95, STA, ZeroPageX, 4),
    Instruction::new(0x96, STX, ZeroPageY, 4),
    Instruction::new(0x97, UNMAPPED, Implicit, 4),
    Instruction::new(0x98, TYA, Implicit, 2),
    Instruction::new(0x99, STA, AbsoluteY, 5),
    Instruction::new(0x9A, TXS, Implicit, 2),
    Instruction::new(0x9B, UNMAPPED, Implicit, 5),
    Instruction::new(0x9C, UNMAPPED, Implicit, 5),
    Instruction::new(0x9D, STA, AbsoluteX, 5),
    Instruction::new(0x9E, UNMAPPED, Implicit, 5),
    Instruction::new(0x9F, UNMAPPED, Implicit, 5),
    Instruction::new(0xA0, LDY, Immediate, 2),
    Instruction::new(0xA1, LDA, IndirectX, 6),
    Instruction::new(0xA2, LDX, Immediate, 2),
    Instruction::new(0xA3, UNMAPPED, Implicit, 6),
    Instruction::new(0xA4, LDY, ZeroPage, 3),
    Instruction::new(0xA5, LDA, ZeroPage, 3),
    Instruction::new(0xA6, LDX, ZeroPage, 3),
    Instruction::new(0xA7, UNMAPPED, Implicit, 3),
    Instruction::new(0xA8, TAY, Implicit, 2),
    Instruction::new(0xA9, LDA, Immediate, 2),
    Instruction::new(0xAA, TAX, Implicit, 2),
    Instruction::new(0xAB, UNMAPPED, Implicit, 2),
    Instruction::new(0xAC, LDY, Absolute, 4),
    Instruction::new(0xAD, LDA, Absolute, 4),
    Instruction::new(0xAE, LDX, Absolute, 4),
    Instruction::new(0xAF, UNMAPPED, Implicit, 4),
    Instruction::new(0xB0, BCS, Relative, 2),
    Instruction::new(0xB1, LDA, IndirectY, 5),
    Instruction::new(0xB2, UNMAPPED, Implicit, 1),
    Instruction::new(0xB3, UNMAPPED, Implicit, 5),
    Instruction::new(0xB4, LDY, ZeroPageX, 4),
    Instruction::new(0xB5, LDA, ZeroPageX, 4),
    Instruction::new(0xB6, LDX, ZeroPageY, 4),
    Instruction::new(0xB7, UNMAPPED, Implicit, 4),
    Instruction::new(0xB8, CLV, Implicit, 2),
    Instruction::new(0xB9, LDA, AbsoluteY, 4),
    Instruction::new(0xBA, TSX, Implicit, 2),
    Instruction::new(0xBB, UNMAPPED, Implicit, 4),
    Instruction::new(0xBC, LDY, AbsoluteX, 4),
    Instruction::new(0xBD, LDA, AbsoluteX, 4),
    Instruction::new(0xBE, LDX, AbsoluteY, 4),
    Instruction::new(0xBF, UNMAPPED, Implicit, 4),
    Instruction::new(0xC0, CPY, Immediate, 2),
    Instruction::new(0xC1, CMP, IndirectX, 6),
    Instruction::new(0xC2, UNMAPPED, Implicit, 2),
    Instruction::new(0xC3, UNMAPPED, Implicit, 8),
    Instruction::new(0xC4, CPY, ZeroPage, 3),
    Instruction::new(0xC5, CMP, ZeroPage, 3),
    Instruction::new(0xC6, DEC, ZeroPage, 5),
    Instruction::new(0xC7, UNMAPPED, Implicit, 5),
    Instruction::new(0xC8, INY, Implicit, 2),
    Instruction::new(0xC9, CMP, Immediate, 2),
    Instruction::new(0xCA, DEX, Implicit, 2),
    Instruction::new(0xCB, UNMAPPED, Implicit, 2),
    Instruction::new(0xCC, CPY, Absolute, 4),
    Instruction::new(0xCD, CMP, Absolute, 4),
    Instruction::new(0xCE, DEC, Absolute, 6),
    Instruction::new(0xCF, UNMAPPED, Implicit, 6),
    Instruction::new(0xD0, BNE, Relative, 2),
    Instruction::new(0xD1, CMP, IndirectY, 5),
    Instruction::new(0xD2, UNMAPPED, Implicit, 1),
    Instruction::new(0xD3, UNMAPPED, Implicit, 8),
    Instruction::new(0xD4, UNMAPPED, Implicit, 4),
    Instruction::new(0xD5, CMP, ZeroPageX, 4),
    Instruction::new(0xD6, DEC, ZeroPageX, 6),
    Instruction::new(0xD7, UNMAPPED, Implicit, 6),
    Instruction::new(0xD8, CLD, Implicit, 2),
    Instruction::new(0xD9, CMP, AbsoluteY, 4),
    Instruction::new(0xDA, UNMAPPED, Implicit, 2),
    Instruction::new(0xDB, UNMAPPED, Implicit, 7),
    Instruction::new(0xDC, UNMAPPED, Implicit, 4),
    Instruction::new(0xDD, CMP, AbsoluteX, 4),
    Instruction::new(0xDE, DEC, AbsoluteX, 7),
    Instruction::new(0xDF, UNMAPPED, Implicit, 7),
    Instruction::new(0xE0, CPX, Immediate, 2),
    Instruction::new(0xE1, SBC, IndirectX, 6),
    Instruction::new(0xE2, UNMAPPED, Implicit, 2),
    Instruction::new(0xE3, UNMAPPED, Implicit, 8),
    Instruction::new(0xE4, CPX, ZeroPage, 3),
    Instruction::new(0xE5, SBC, ZeroPage, 3),
    Instruction::new(0xE6, INC, ZeroPage, 5),
    Instruction::new(0xE7, UNMAPPED, Implicit, 5),
    Instruction::new(0xE8, INX, Implicit, 2),
    Instruction::new(0xE9, SBC, Immediate, 2),
    Instruction::new(0xEA, NOP, Implicit, 2),
    Instruction::new(0xEB, UNMAPPED, Implicit, 2),
    Instruction::new(0xEC, CPX, Absolute, 4),
    Instruction::new(0xED, SBC, Absolute, 4),
    Instruction::new(0xEE, INC, Absolute, 6),
    Instruction::new(0xEF, UNMAPPED, Implicit, 6),
    Instruction::new(0xF0, BEQ, Relative, 2),
    Instruction::new(0xF1, SBC, IndirectY, 5),
    Instruction::new(0xF2, UNMAPPED, Implicit, 1),
    Instruction::new(0xF3, UNMAPPED, Implicit, 8),
    Instruction::new(0xF4, UNMAPPED, Implicit, 4),
    Instruction::new(0xF5, SBC, ZeroPageX, 4),
    Instruction::new(0xF6, INC, ZeroPageX, 6),
    Instruction::new(0xF7, UNMAPPED, Implicit, 6),
    Instruction::new(0xF8, SED, Implicit, 2),
    Instruction::new(0xF9, SBC, AbsoluteY, 4),
    Instruction::new(0xFA, UNMAPPED, Implicit, 2),
    Instruction::new(0xFB, UNMAPPED, Implicit, 7),
    Instruction::new(0xFC, UNMAPPED, Implicit, 4),
    Instruction::new(0xFD, SBC, AbsoluteX, 4),
    Instruction::new(0xFE, INC, AbsoluteX, 7),
    Instruction::new(0xFF, UNMAPPED, Implicit, 7),
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
    let offset = machine.read_byte(machine.cpu.program_counter.wrapping_sub(1));
    // Offset is sign-extended
    machine.cpu.program_counter = machine.cpu.program_counter.wrapping_add_signed(offset as i8 as i16);
}

const UNMAPPED: Operation = Operation {
    mnemonic: "???",
    function: |_, _| {
        // Uh oh
        panic!("yeah no can do sorry");
    },
};

// Add with Carry
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const ADC: Operation = Operation {
    mnemonic: "ADC",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let addend = machine.read_byte(address);
        let carry_in = machine.cpu.get_flag(CARRY_FLAG);
        let (result, carry_out) = carrying_add(machine.cpu.accumulator, addend, carry_in);
        let overflows = carrying_add_overflows(machine.cpu.accumulator, addend, carry_in);
        machine.cpu.accumulator = result;

        machine.cpu.set_flag(CARRY_FLAG, carry_out);
        machine.cpu.set_flag(OVERFLOW_FLAG, overflows);
        machine.cpu.set_result_flags(result);
    },
};

// Bitwise AND
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const AND: Operation = Operation {
    mnemonic: "AND",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.cpu.accumulator &= machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.accumulator);
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
            carry_out = (machine.cpu.accumulator & 0x80) != 0;
            result = machine.cpu.accumulator << 1;
            machine.cpu.accumulator = result;
        }
        else {
            let address = addressing_mode.calculate_address(machine);
            let value = machine.read_byte(address);
            carry_out = (value & 0x80) != 0;
            result = value << 1;
            machine.write_byte(address, result);
        }

        machine.cpu.set_flag(CARRY_FLAG, carry_out);
        machine.cpu.set_result_flags(result);
    },
};

// Branch if Carry Clear
// Relative
const BCC: Operation = Operation {
    mnemonic: "BCC",
    function: |_, machine| {
        if !machine.cpu.get_flag(CARRY_FLAG) {
            process_branch(machine);
        }
    },
};

// Branch if Carry Set
// Relative
const BCS: Operation = Operation {
    mnemonic: "BCS",
    function: |_, machine| {
        if machine.cpu.get_flag(CARRY_FLAG) {
            process_branch(machine);
        }
    },
};

// Branch if Equal
// Relative
const BEQ: Operation = Operation {
    mnemonic: "BEQ",
    function: |_, machine| {
        if machine.cpu.get_flag(ZERO_FLAG) {
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
        let result = machine.read_byte(address) & machine.cpu.accumulator;

        machine.cpu.set_flag(OVERFLOW_FLAG, (result & 0x40) != 0);
        machine.cpu.set_result_flags(result);
    },
};

// Branch if Minus
// Relative
const BMI: Operation = Operation {
    mnemonic: "BMI",
    function: |_, machine| {
        if machine.cpu.get_flag(NEGATIVE_FLAG) {
            process_branch(machine);
        }
    },
};

// Branch if Not Equal
// Relative
const BNE: Operation = Operation {
    mnemonic: "BNE",
    function: |_, machine| {
        if !machine.cpu.get_flag(ZERO_FLAG) {
            process_branch(machine);
        }
    },
};

// Branch if Plus
// Relative
const BPL: Operation = Operation {
    mnemonic: "BPL",
    function: |_, machine| {
        if !machine.cpu.get_flag(NEGATIVE_FLAG) {
            process_branch(machine);
        }
    },
};

// Force Interrupt
// Implicit
const BRK: Operation = Operation {
    mnemonic: "BRK",
    function: |_, machine| {
        machine.stack_push_pair(machine.cpu.program_counter);
        machine.stack_push_byte(machine.cpu.status_byte);
        machine.cpu.program_counter = machine.read_pair(IRQ_VECTOR);
        machine.cpu.set_flag(BREAK_FLAG, true);
    },
};

// Branch if Overflow Clear
// Relative
const BVC: Operation = Operation {
    mnemonic: "BVC",
    function: |_, machine| {
        if !machine.cpu.get_flag(OVERFLOW_FLAG) {
            process_branch(machine);
        }
    },
};

// Branch if Overflow Set
// Relative
const BVS: Operation = Operation {
    mnemonic: "BVS",
    function: |_, machine| {
        if machine.cpu.get_flag(OVERFLOW_FLAG) {
            process_branch(machine);
        }
    },
};

// Clear Carry Flag
// Implicit
const CLC: Operation = Operation {
    mnemonic: "CLC",
    function: |_, machine| {
        machine.cpu.set_flag(CARRY_FLAG, false);
    },
};

// Clear Decimal Flag
// Implicit
const CLD: Operation = Operation {
    mnemonic: "CLD",
    function: |_, machine| {
        // NOTE: BCD mode is disabled in the Ricoh 2A03
        machine.cpu.set_flag(DECIMAL_FLAG, false);
    },
};

// Clear Interrupt Disable Flag
// Implicit
const CLI: Operation = Operation {
    mnemonic: "CLI",
    function: |_, machine| {
        machine.cpu.set_flag(INTERRUPT_DISABLE_FLAG, false);
    },
};

// Clear Overflow Flag
// Implicit
const CLV: Operation = Operation {
    mnemonic: "CLV",
    function: |_, machine| {
        machine.cpu.set_flag(OVERFLOW_FLAG, false);
    },
};

// Compare
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const CMP: Operation = Operation {
    mnemonic: "CMP",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let value = machine.read_byte(address);
        let result = machine.cpu.accumulator.wrapping_sub(value);

        machine.cpu.set_flag(CARRY_FLAG, machine.cpu.accumulator >= value);
        machine.cpu.set_result_flags(result);
    },
};

// Compare X Register
// Immediate, ZeroPage, Absolute
const CPX: Operation = Operation {
    mnemonic: "CPX",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let value = machine.read_byte(address);
        let result = machine.cpu.register_x.wrapping_sub(value);

        machine.cpu.set_flag(CARRY_FLAG, machine.cpu.register_x >= value);
        machine.cpu.set_result_flags(result);
    },
};

// Compare Y Register
// Immediate, ZeroPage, Absolute
const CPY: Operation = Operation {
    mnemonic: "CPY",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let value = machine.read_byte(address);
        let result = machine.cpu.register_y.wrapping_sub(value);

        machine.cpu.set_flag(CARRY_FLAG, machine.cpu.register_y >= value);
        machine.cpu.set_result_flags(result);
    },
};

// Decrement Memory
// ZeroPage, ZeroPageX, Absolute, AbsoluteX
const DEC: Operation = Operation {
    mnemonic: "DEC",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let result = machine.read_byte(address).wrapping_sub(1);
        machine.write_byte(address, result);

        machine.cpu.set_result_flags(result);
    },
};

// Decrement X Register
// Implicit
const DEX: Operation = Operation {
    mnemonic: "DEX",
    function: |_, machine| {
        machine.cpu.register_x = machine.cpu.register_x.wrapping_sub(1);

        machine.cpu.set_result_flags(machine.cpu.register_x);
    },
};

// Decrement Y Register
// Implicit
const DEY: Operation = Operation {
    mnemonic: "DEY",
    function: |_, machine| {
        machine.cpu.register_y = machine.cpu.register_y.wrapping_sub(1);

        machine.cpu.set_result_flags(machine.cpu.register_y);
    },
};

// Bitwise Exclusive OR
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const EOR: Operation = Operation {
    mnemonic: "EOR",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.cpu.accumulator ^= machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.accumulator);
    },
};

// Increment Memory
// ZeroPage, ZeroPageX, Absolute, AbsoluteX
const INC: Operation = Operation {
    mnemonic: "INC",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        let result = machine.read_byte(address).wrapping_add(1);
        machine.write_byte(address, result);

        machine.cpu.set_result_flags(result);
    },
};

// Increment X Register
// Implicit
const INX: Operation = Operation {
    mnemonic: "INX",
    function: |_, machine| {
        machine.cpu.register_x = machine.cpu.register_x.wrapping_add(1);

        machine.cpu.set_result_flags(machine.cpu.register_x);
    },
};

// Increment Y Register
// Implicit
const INY: Operation = Operation {
    mnemonic: "INY",
    function: |_, machine| {
        machine.cpu.register_y = machine.cpu.register_y.wrapping_add(1);

        machine.cpu.set_result_flags(machine.cpu.register_y);
    },
};

// Jump
// Absolute, Indirect
const JMP: Operation = Operation {
    mnemonic: "JMP",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.cpu.program_counter = address;
    },
};

// Jump to Subroutine
// Absolute
const JSR: Operation = Operation {
    mnemonic: "JSR",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.stack_push_pair(machine.cpu.program_counter.wrapping_sub(1));
        machine.cpu.program_counter = address;
    },
};

// Load Accumulator
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const LDA: Operation = Operation {
    mnemonic: "LDA",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.cpu.accumulator = machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.accumulator);
    },
};

// Load X Register
// Immediate, ZeroPage, ZeroPageY, Absolute, AbsoluteY
const LDX: Operation = Operation {
    mnemonic: "LDX",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.cpu.register_x = machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.register_x);
    },
};

// Load Y Register
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const LDY: Operation = Operation {
    mnemonic: "LDY",
    function: |addressing_mode, machine| {
        let address = addressing_mode.calculate_address(machine);
        machine.cpu.register_y = machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.register_y);
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
            carry_out = (machine.cpu.accumulator & 1) != 0;
            result = machine.cpu.accumulator >> 1;
            machine.cpu.accumulator = result;
        }
        else {
            let address = addressing_mode.calculate_address(machine);
            let value = machine.read_byte(address);
            carry_out = (value & 1) != 0;
            result = value >> 1;
            machine.write_byte(address, result);
        }

        machine.cpu.set_flag(CARRY_FLAG, carry_out);
        machine.cpu.set_result_flags(result);
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
        machine.cpu.accumulator |= machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.accumulator);
    },
};

// Push Accumulator
// Implicit
const PHA: Operation = Operation {
    mnemonic: "PHA",
    function: |_, machine| {
        machine.stack_push_byte(machine.cpu.accumulator);
    },
};

// Push Processor Status
// Implicit
const PHP: Operation = Operation {
    mnemonic: "PHP",
    function: |_, machine| {
        machine.stack_push_byte(machine.cpu.status_byte);
    },
};

// Pull Accumulator
// Implicit
const PLA: Operation = Operation {
    mnemonic: "PLA",
    function: |_, machine| {
        machine.cpu.accumulator = machine.stack_pull_byte();

        machine.cpu.set_result_flags(machine.cpu.accumulator);
    },
};

// Pull Processor Status
// Implicit
const PLP: Operation = Operation {
    mnemonic: "PLP",
    function: |_, machine| {
        let status_byte = machine.stack_pull_byte();
        machine.cpu.status_byte = status_byte;
    },
};

// Rotate Left
// Accumulator, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const ROL: Operation = Operation {
    mnemonic: "ROL",
    function: |addressing_mode, machine| {
        let carry_in = machine.cpu.get_flag(CARRY_FLAG);
        let carry_out;
        let result;
        if let Accumulator = addressing_mode {
            carry_out = (machine.cpu.accumulator & 0x80) != 0;
            result = machine.cpu.accumulator << 1 | carry_in as u8;
            machine.cpu.accumulator = result;
        }
        else {
            let address = addressing_mode.calculate_address(machine);
            let value = machine.read_byte(address);
            carry_out = (value & 0x80) != 0;
            result = value << 1 | carry_in as u8;
            machine.write_byte(address, result);
        }

        machine.cpu.set_flag(CARRY_FLAG, carry_out);
        machine.cpu.set_result_flags(result);
    },
};

// Rotate Right
// Accumulator, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const ROR: Operation = Operation {
    mnemonic: "ROR",
    function: |addressing_mode, machine| {
        let carry_in = machine.cpu.get_flag(CARRY_FLAG);
        let carry_out;
        let result;
        if let Accumulator = addressing_mode {
            carry_out = (machine.cpu.accumulator & 1) != 0;
            result = machine.cpu.accumulator >> 1 | (carry_in as u8) << 7;
            machine.cpu.accumulator = result;
        }
        else {
            let address = addressing_mode.calculate_address(machine);
            let value = machine.read_byte(address);
            carry_out = (value & 1) != 0;
            result = value >> 1 | (carry_in as u8) << 7;
            machine.write_byte(address, result);
        }

        machine.cpu.set_flag(CARRY_FLAG, carry_out);
        machine.cpu.set_result_flags(result);
    },
};

// Return from Interrupt
// Implicit
const RTI: Operation = Operation {
    mnemonic: "RTI",
    function: |_, machine| {
        let status_byte = machine.stack_pull_byte();
        let program_counter = machine.stack_pull_pair();
        machine.cpu.status_byte = status_byte;
        machine.cpu.program_counter = program_counter;
    },
};

// Return from Subroutine
// Implicit
const RTS: Operation = Operation {
    mnemonic: "RTS",
    function: |_, machine| {
        let program_counter = machine.stack_pull_pair().wrapping_add(1);
        machine.cpu.program_counter = program_counter;
    },
};

// Subtract with Carry
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const SBC: Operation = Operation {
    mnemonic: "SBC",
    function: |addressing_mode, machine| {
        // TODO: definitely wrong
        let address = addressing_mode.calculate_address(machine);
        let addend = machine.read_byte(address);
        let carry_in = !machine.cpu.get_flag(CARRY_FLAG);
        let (result, carry_out) = carrying_sub(machine.cpu.accumulator, addend, carry_in);
        let overflows = carrying_sub_overflows(machine.cpu.accumulator, addend, carry_in);
        machine.cpu.accumulator = result;

        machine.cpu.set_flag(CARRY_FLAG, carry_out);
        machine.cpu.set_flag(OVERFLOW_FLAG, overflows);
        machine.cpu.set_result_flags(result);
    },
};

// Set Carry Flag
// Implicit
const SEC: Operation = Operation {
    mnemonic: "SEC",
    function: |_, machine| {
        machine.cpu.set_flag(CARRY_FLAG, true);
    },
};

// Set Decimal Flag
// Implicit
const SED: Operation = Operation {
    mnemonic: "SED",
    function: |_, machine| {
        // NOTE: BCD mode is disabled in the Ricoh 2A03
        machine.cpu.set_flag(DECIMAL_FLAG, true);
    },
};

// Set Interrupt Disable Flag
// Implicit
const SEI: Operation = Operation {
    mnemonic: "SEI",
    function: |_, machine| {
        machine.cpu.set_flag(INTERRUPT_DISABLE_FLAG, true);
    },
};

// Store Accumulator
// ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const STA: Operation = Operation {
    mnemonic: "STA",
    function: |addressing_mode, machine| {
    let address = addressing_mode.calculate_address(machine);
        machine.write_byte(address, machine.cpu.accumulator);
    },
};

// Store X Register
// ZeroPage, ZeroPageY, Absolute
const STX: Operation = Operation {
    mnemonic: "STX",
    function: |addressing_mode, machine| {
    let address = addressing_mode.calculate_address(machine);
        machine.write_byte(address, machine.cpu.register_x);
    },
};

// Store Y Register
// ZeroPage, ZeroPageX, Absolute
const STY: Operation = Operation {
    mnemonic: "STY",
    function: |addressing_mode, machine| {
    let address = addressing_mode.calculate_address(machine);
        machine.write_byte(address, machine.cpu.register_y);
    },
};

// Transfer Accumulator to X Register
// Implicit
const TAX: Operation = Operation {
    mnemonic: "TAX",
    function: |_, machine| {
        machine.cpu.register_x = machine.cpu.accumulator;

        machine.cpu.set_result_flags(machine.cpu.register_x);
    },
};

// Transfer Accumulator to Y Register
// Implicit
const TAY: Operation = Operation {
    mnemonic: "TAY",
    function: |_, machine| {
        machine.cpu.register_y = machine.cpu.accumulator;

        machine.cpu.set_result_flags(machine.cpu.register_y);
    },
};

// Transfer Stack Pointer to X Register
// Implicit
const TSX: Operation = Operation {
    mnemonic: "TSX",
    function: |_, machine| {
        machine.cpu.register_x = machine.cpu.stack_pointer;

        machine.cpu.set_result_flags(machine.cpu.register_x);
    },
};

// Transfer X Register to Accumulator
// Implicit
const TXA: Operation = Operation {
    mnemonic: "TXA",
    function: |_, machine| {
        machine.cpu.accumulator = machine.cpu.register_x;

        machine.cpu.set_result_flags(machine.cpu.accumulator);
    },
};

// Transfer X Register to Stack Pointer
// Implicit
const TXS: Operation = Operation {
    mnemonic: "TXS",
    function: |_, machine| {
        machine.cpu.stack_pointer = machine.cpu.register_x;
    },
};

// Transfer Y Register to Accumulator
// Implicit
const TYA: Operation = Operation {
    mnemonic: "TYA",
    function: |_, machine| {
        machine.cpu.accumulator = machine.cpu.register_y;

        machine.cpu.set_result_flags(machine.cpu.accumulator);
    },
};
