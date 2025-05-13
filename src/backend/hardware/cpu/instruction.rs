use crate::hardware::*;

#[derive(Copy, Clone, Debug)]
struct Operation {
    mnemonic: &'static str,
    function: fn(AddressingMode, &mut Machine),
}

#[derive(Copy, Clone, Debug)]
enum AddressingMode {
    /// This is a meta-operation, and is not part of the instruction set.
    Meta,
    Implied,
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
            Meta => format!("[{mnemonic}]"),
            Implied => format!("{mnemonic}"),
            Accumulator => format!("{mnemonic} A"),
            Relative => format!("{mnemonic} ${:04X}", opcode_address.wrapping_add(2)
                .wrapping_add_signed(machine.read_byte_silent(operand_address) as i8 as i16)),
            Immediate => format!("{mnemonic} #${:02X}", machine.read_byte_silent(operand_address)),
            ZeroPage => format!("{mnemonic} ${:02X}", machine.read_byte_silent(operand_address)),
            ZeroPageX => format!("{mnemonic} ${:02X},X", machine.read_byte_silent(operand_address)),
            ZeroPageY => format!("{mnemonic} ${:02X},Y", machine.read_byte_silent(operand_address)),
            Absolute => format!("{mnemonic} ${:04X}", machine.read_word_silent(operand_address)),
            AbsoluteX => format!("{mnemonic} ${:04X},X", machine.read_word_silent(operand_address)),
            AbsoluteY => format!("{mnemonic} ${:04X},Y", machine.read_word_silent(operand_address)),
            Indirect => format!("{mnemonic} (${:04X})", machine.read_word_silent(operand_address)),
            IndirectX => format!("{mnemonic} (${:02X},X)", machine.read_byte_silent(operand_address)),
            IndirectY => format!("{mnemonic} (${:02X}),Y", machine.read_byte_silent(operand_address)),
        }
    }

    fn get_instruction_size(self) -> u16 {
        match self {
            Meta
                => 0,
            Implied | Accumulator
                => 1,
            Relative | Immediate | ZeroPage | ZeroPageX | ZeroPageY | IndirectX | IndirectY
                => 2,
            Absolute | AbsoluteX | AbsoluteY | Indirect
                => 3,
        }
    }

    fn calculate_address(self, machine: &mut Machine) -> (u16, bool) {
        match self {
            Immediate => {
                (machine.cpu.program_counter.wrapping_sub(1), false)
            }
            ZeroPage => {
                (machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)) as u16, false)
            }
            ZeroPageX => {
                (machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)).wrapping_add(machine.cpu.register_x) as u16, false)
            }
            ZeroPageY => {
                (machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)).wrapping_add(machine.cpu.register_y) as u16, false)
            }
            Absolute => {
                (machine.read_word(machine.cpu.program_counter.wrapping_sub(2)), false)
            }
            AbsoluteX => {
                page_crossing_add(machine.read_word(machine.cpu.program_counter.wrapping_sub(2)), machine.cpu.register_x as u16)
            }
            AbsoluteY => {
                page_crossing_add(machine.read_word(machine.cpu.program_counter.wrapping_sub(2)), machine.cpu.register_y as u16)
            }
            Indirect => {
                let address = machine.read_word(machine.cpu.program_counter.wrapping_sub(2));
                (machine.read_word_paged(address), false)
            }
            IndirectX => {
                let address = machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)).wrapping_add(machine.cpu.register_x) as u16;
                (machine.read_word_paged(address), false)
            }
            IndirectY => {
                let address = machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)) as u16;
                page_crossing_add(machine.read_word_paged(address), machine.cpu.register_y as u16)
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

    pub fn meta_irq() -> &'static Self {
        &IRQ_INSTRUCTION
    }

    pub fn meta_nmi() -> &'static Self {
        &NMI_INSTRUCTION
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

impl PartialEq for Instruction {
    fn eq(&self, other: &Self) -> bool {
        // Just compare addresses, all instances should have come from the INSTRUCTIONS table
        self as *const _ == other as *const _
    }
}

// Meta-instructions
const IRQ_INSTRUCTION: Instruction = Instruction::new(0x00, IRQ, Meta, 7);
const NMI_INSTRUCTION: Instruction = Instruction::new(0x00, NMI, Meta, 7);

// Cycle counts are adapted from https://github.com/jslepicka/nemulator/blob/5dccc9ca8cdd8a8593303ecce2b433ae14f437ca/nes/cpu.cpp#L12
const INSTRUCTIONS: [Instruction; 0x100] = [
    Instruction::new(0x00, BRK, Immediate, 7),
    Instruction::new(0x01, ORA, IndirectX, 6),
    Instruction::new(0x02, KIL_X, Implied, 1),
    Instruction::new(0x03, SLO_U, IndirectX, 8),
    Instruction::new(0x04, IGN_U, ZeroPage, 3),
    Instruction::new(0x05, ORA, ZeroPage, 3),
    Instruction::new(0x06, ASL, ZeroPage, 5),
    Instruction::new(0x07, SLO_U, ZeroPage, 5),
    Instruction::new(0x08, PHP, Implied, 3),
    Instruction::new(0x09, ORA, Immediate, 2),
    Instruction::new(0x0A, ASL, Accumulator, 2),
    Instruction::new(0x0B, ANC_U, Immediate, 2),
    Instruction::new(0x0C, IGN_U, Absolute, 4),
    Instruction::new(0x0D, ORA, Absolute, 4),
    Instruction::new(0x0E, ASL, Absolute, 6),
    Instruction::new(0x0F, SLO_U, Absolute, 6),
    Instruction::new(0x10, BPL, Relative, 2),
    Instruction::new(0x11, ORA, IndirectY, 5),
    Instruction::new(0x12, KIL_X, Implied, 1),
    Instruction::new(0x13, SLO_U, IndirectY, 8),
    Instruction::new(0x14, IGN_U, ZeroPageX, 4),
    Instruction::new(0x15, ORA, ZeroPageX, 4),
    Instruction::new(0x16, ASL, ZeroPageX, 6),
    Instruction::new(0x17, SLO_U, ZeroPageX, 6),
    Instruction::new(0x18, CLC, Implied, 2),
    Instruction::new(0x19, ORA, AbsoluteY, 4),
    Instruction::new(0x1A, NOP_U, Implied, 2),
    Instruction::new(0x1B, SLO_U, AbsoluteY, 7),
    Instruction::new(0x1C, IGN_U, AbsoluteX, 4),
    Instruction::new(0x1D, ORA, AbsoluteX, 4),
    Instruction::new(0x1E, ASL, AbsoluteX, 7),
    Instruction::new(0x1F, SLO_U, AbsoluteX, 7),
    Instruction::new(0x20, JSR, Absolute, 6),
    Instruction::new(0x21, AND, IndirectX, 6),
    Instruction::new(0x22, KIL_X, Implied, 1),
    Instruction::new(0x23, RLA_U, IndirectX, 8),
    Instruction::new(0x24, BIT, ZeroPage, 3),
    Instruction::new(0x25, AND, ZeroPage, 3),
    Instruction::new(0x26, ROL, ZeroPage, 5),
    Instruction::new(0x27, RLA_U, ZeroPage, 5),
    Instruction::new(0x28, PLP, Implied, 4),
    Instruction::new(0x29, AND, Immediate, 2),
    Instruction::new(0x2A, ROL, Accumulator, 2),
    Instruction::new(0x2B, ANC_U, Immediate, 2),
    Instruction::new(0x2C, BIT, Absolute, 4),
    Instruction::new(0x2D, AND, Absolute, 4),
    Instruction::new(0x2E, ROL, Absolute, 6),
    Instruction::new(0x2F, RLA_U, Absolute, 6),
    Instruction::new(0x30, BMI, Relative, 2),
    Instruction::new(0x31, AND, IndirectY, 5),
    Instruction::new(0x32, KIL_X, Implied, 1),
    Instruction::new(0x33, RLA_U, IndirectY, 8),
    Instruction::new(0x34, IGN_U, ZeroPageX, 4),
    Instruction::new(0x35, AND, ZeroPageX, 4),
    Instruction::new(0x36, ROL, ZeroPageX, 6),
    Instruction::new(0x37, RLA_U, ZeroPageX, 6),
    Instruction::new(0x38, SEC, Implied, 2),
    Instruction::new(0x39, AND, AbsoluteY, 4),
    Instruction::new(0x3A, NOP_U, Implied, 2),
    Instruction::new(0x3B, RLA_U, AbsoluteY, 7),
    Instruction::new(0x3C, IGN_U, AbsoluteX, 4),
    Instruction::new(0x3D, AND, AbsoluteX, 4),
    Instruction::new(0x3E, ROL, AbsoluteX, 7),
    Instruction::new(0x3F, RLA_U, AbsoluteX, 7),
    Instruction::new(0x40, RTI, Implied, 6),
    Instruction::new(0x41, EOR, IndirectX, 6),
    Instruction::new(0x42, KIL_X, Implied, 1),
    Instruction::new(0x43, SRE_U, IndirectX, 8),
    Instruction::new(0x44, IGN_U, ZeroPage, 3),
    Instruction::new(0x45, EOR, ZeroPage, 3),
    Instruction::new(0x46, LSR, ZeroPage, 5),
    Instruction::new(0x47, SRE_U, ZeroPage, 5),
    Instruction::new(0x48, PHA, Implied, 3),
    Instruction::new(0x49, EOR, Immediate, 2),
    Instruction::new(0x4A, LSR, Accumulator, 2),
    Instruction::new(0x4B, ALR_U, Immediate, 2),
    Instruction::new(0x4C, JMP, Absolute, 3),
    Instruction::new(0x4D, EOR, Absolute, 4),
    Instruction::new(0x4E, LSR, Absolute, 6),
    Instruction::new(0x4F, SRE_U, Absolute, 6),
    Instruction::new(0x50, BVC, Relative, 2),
    Instruction::new(0x51, EOR, IndirectY, 5),
    Instruction::new(0x52, KIL_X, Implied, 1),
    Instruction::new(0x53, SRE_U, IndirectY, 8),
    Instruction::new(0x54, IGN_U, ZeroPageX, 4),
    Instruction::new(0x55, EOR, ZeroPageX, 4),
    Instruction::new(0x56, LSR, ZeroPageX, 6),
    Instruction::new(0x57, SRE_U, ZeroPageX, 6),
    Instruction::new(0x58, CLI, Implied, 2),
    Instruction::new(0x59, EOR, AbsoluteY, 4),
    Instruction::new(0x5A, NOP_U, Implied, 2),
    Instruction::new(0x5B, SRE_U, AbsoluteY, 7),
    Instruction::new(0x5C, IGN_U, AbsoluteX, 4),
    Instruction::new(0x5D, EOR, AbsoluteX, 4),
    Instruction::new(0x5E, LSR, AbsoluteX, 7),
    Instruction::new(0x5F, SRE_U, AbsoluteX, 7),
    Instruction::new(0x60, RTS, Implied, 6),
    Instruction::new(0x61, ADC, IndirectX, 6),
    Instruction::new(0x62, KIL_X, Implied, 1),
    Instruction::new(0x63, RRA_U, IndirectX, 8),
    Instruction::new(0x64, IGN_U, ZeroPage, 3),
    Instruction::new(0x65, ADC, ZeroPage, 3),
    Instruction::new(0x66, ROR, ZeroPage, 5),
    Instruction::new(0x67, RRA_U, ZeroPage, 5),
    Instruction::new(0x68, PLA, Implied, 4),
    Instruction::new(0x69, ADC, Immediate, 2),
    Instruction::new(0x6A, ROR, Accumulator, 2),
    Instruction::new(0x6B, ARR_U, Immediate, 2),
    Instruction::new(0x6C, JMP, Indirect, 5),
    Instruction::new(0x6D, ADC, Absolute, 4),
    Instruction::new(0x6E, ROR, Absolute, 6),
    Instruction::new(0x6F, RRA_U, Absolute, 6),
    Instruction::new(0x70, BVS, Relative, 2),
    Instruction::new(0x71, ADC, IndirectY, 5),
    Instruction::new(0x72, KIL_X, Implied, 1),
    Instruction::new(0x73, RRA_U, IndirectY, 8),
    Instruction::new(0x74, IGN_U, ZeroPageX, 4),
    Instruction::new(0x75, ADC, ZeroPageX, 4),
    Instruction::new(0x76, ROR, ZeroPageX, 6),
    Instruction::new(0x77, RRA_U, ZeroPageX, 6),
    Instruction::new(0x78, SEI, Implied, 2),
    Instruction::new(0x79, ADC, AbsoluteY, 4),
    Instruction::new(0x7A, NOP_U, Implied, 2),
    Instruction::new(0x7B, RRA_U, AbsoluteY, 7),
    Instruction::new(0x7C, IGN_U, AbsoluteX, 4),
    Instruction::new(0x7D, ADC, AbsoluteX, 4),
    Instruction::new(0x7E, ROR, AbsoluteX, 7),
    Instruction::new(0x7F, RRA_U, AbsoluteX, 7),
    Instruction::new(0x80, SKB_U, Immediate, 2),
    Instruction::new(0x81, STA, IndirectX, 6),
    Instruction::new(0x82, SKB_U, Immediate, 2),
    Instruction::new(0x83, SAX_U, IndirectX, 6),
    Instruction::new(0x84, STY, ZeroPage, 3),
    Instruction::new(0x85, STA, ZeroPage, 3),
    Instruction::new(0x86, STX, ZeroPage, 3),
    Instruction::new(0x87, SAX_U, ZeroPage, 3),
    Instruction::new(0x88, DEY, Implied, 2),
    Instruction::new(0x89, SKB_U, Immediate, 2),
    Instruction::new(0x8A, TXA, Implied, 2),
    Instruction::new(0x8B, XAA_U, Immediate, 2),
    Instruction::new(0x8C, STY, Absolute, 4),
    Instruction::new(0x8D, STA, Absolute, 4),
    Instruction::new(0x8E, STX, Absolute, 4),
    Instruction::new(0x8F, SAX_U, Absolute, 4),
    Instruction::new(0x90, BCC, Relative, 2),
    Instruction::new(0x91, STA, IndirectY, 6),
    Instruction::new(0x92, KIL_X, Implied, 1),
    Instruction::new(0x93, SHA_U, IndirectY, 6),
    Instruction::new(0x94, STY, ZeroPageX, 4),
    Instruction::new(0x95, STA, ZeroPageX, 4),
    Instruction::new(0x96, STX, ZeroPageY, 4),
    Instruction::new(0x97, SAX_U, ZeroPageY, 4),
    Instruction::new(0x98, TYA, Implied, 2),
    Instruction::new(0x99, STA, AbsoluteY, 5),
    Instruction::new(0x9A, TXS, Implied, 2),
    Instruction::new(0x9B, TAS_U, AbsoluteY, 5),
    Instruction::new(0x9C, SHY_U, AbsoluteX, 5),
    Instruction::new(0x9D, STA, AbsoluteX, 5),
    Instruction::new(0x9E, SHX_U, AbsoluteY, 5),
    Instruction::new(0x9F, SHA_U, AbsoluteY, 5),
    Instruction::new(0xA0, LDY, Immediate, 2),
    Instruction::new(0xA1, LDA, IndirectX, 6),
    Instruction::new(0xA2, LDX, Immediate, 2),
    Instruction::new(0xA3, LAX_U, IndirectX, 6),
    Instruction::new(0xA4, LDY, ZeroPage, 3),
    Instruction::new(0xA5, LDA, ZeroPage, 3),
    Instruction::new(0xA6, LDX, ZeroPage, 3),
    Instruction::new(0xA7, LAX_U, ZeroPage, 3),
    Instruction::new(0xA8, TAY, Implied, 2),
    Instruction::new(0xA9, LDA, Immediate, 2),
    Instruction::new(0xAA, TAX, Implied, 2),
    Instruction::new(0xAB, LAX_U, Immediate, 2),
    Instruction::new(0xAC, LDY, Absolute, 4),
    Instruction::new(0xAD, LDA, Absolute, 4),
    Instruction::new(0xAE, LDX, Absolute, 4),
    Instruction::new(0xAF, LAX_U, Absolute, 4),
    Instruction::new(0xB0, BCS, Relative, 2),
    Instruction::new(0xB1, LDA, IndirectY, 5),
    Instruction::new(0xB2, KIL_X, Implied, 1),
    Instruction::new(0xB3, LAX_U, IndirectY, 5),
    Instruction::new(0xB4, LDY, ZeroPageX, 4),
    Instruction::new(0xB5, LDA, ZeroPageX, 4),
    Instruction::new(0xB6, LDX, ZeroPageY, 4),
    Instruction::new(0xB7, LAX_U, ZeroPageY, 4),
    Instruction::new(0xB8, CLV, Implied, 2),
    Instruction::new(0xB9, LDA, AbsoluteY, 4),
    Instruction::new(0xBA, TSX, Implied, 2),
    Instruction::new(0xBB, LAS_U, AbsoluteY, 4),
    Instruction::new(0xBC, LDY, AbsoluteX, 4),
    Instruction::new(0xBD, LDA, AbsoluteX, 4),
    Instruction::new(0xBE, LDX, AbsoluteY, 4),
    Instruction::new(0xBF, LAX_U, AbsoluteY, 4),
    Instruction::new(0xC0, CPY, Immediate, 2),
    Instruction::new(0xC1, CMP, IndirectX, 6),
    Instruction::new(0xC2, SKB_U, Immediate, 2),
    Instruction::new(0xC3, DCP_U, IndirectX, 8),
    Instruction::new(0xC4, CPY, ZeroPage, 3),
    Instruction::new(0xC5, CMP, ZeroPage, 3),
    Instruction::new(0xC6, DEC, ZeroPage, 5),
    Instruction::new(0xC7, DCP_U, ZeroPage, 5),
    Instruction::new(0xC8, INY, Implied, 2),
    Instruction::new(0xC9, CMP, Immediate, 2),
    Instruction::new(0xCA, DEX, Implied, 2),
    Instruction::new(0xCB, AXS_U, Immediate, 2),
    Instruction::new(0xCC, CPY, Absolute, 4),
    Instruction::new(0xCD, CMP, Absolute, 4),
    Instruction::new(0xCE, DEC, Absolute, 6),
    Instruction::new(0xCF, DCP_U, Absolute, 6),
    Instruction::new(0xD0, BNE, Relative, 2),
    Instruction::new(0xD1, CMP, IndirectY, 5),
    Instruction::new(0xD2, KIL_X, Implied, 1),
    Instruction::new(0xD3, DCP_U, IndirectY, 8),
    Instruction::new(0xD4, IGN_U, ZeroPageX, 4),
    Instruction::new(0xD5, CMP, ZeroPageX, 4),
    Instruction::new(0xD6, DEC, ZeroPageX, 6),
    Instruction::new(0xD7, DCP_U, ZeroPageX, 6),
    Instruction::new(0xD8, CLD, Implied, 2),
    Instruction::new(0xD9, CMP, AbsoluteY, 4),
    Instruction::new(0xDA, NOP_U, Implied, 2),
    Instruction::new(0xDB, DCP_U, AbsoluteY, 7),
    Instruction::new(0xDC, IGN_U, AbsoluteX, 4),
    Instruction::new(0xDD, CMP, AbsoluteX, 4),
    Instruction::new(0xDE, DEC, AbsoluteX, 7),
    Instruction::new(0xDF, DCP_U, AbsoluteX, 7),
    Instruction::new(0xE0, CPX, Immediate, 2),
    Instruction::new(0xE1, SBC, IndirectX, 6),
    Instruction::new(0xE2, SKB_U, Immediate, 2),
    Instruction::new(0xE3, ISC_U, IndirectX, 8),
    Instruction::new(0xE4, CPX, ZeroPage, 3),
    Instruction::new(0xE5, SBC, ZeroPage, 3),
    Instruction::new(0xE6, INC, ZeroPage, 5),
    Instruction::new(0xE7, ISC_U, ZeroPage, 5),
    Instruction::new(0xE8, INX, Implied, 2),
    Instruction::new(0xE9, SBC, Immediate, 2),
    Instruction::new(0xEA, NOP, Implied, 2),
    Instruction::new(0xEB, SBC_U, Immediate, 2),
    Instruction::new(0xEC, CPX, Absolute, 4),
    Instruction::new(0xED, SBC, Absolute, 4),
    Instruction::new(0xEE, INC, Absolute, 6),
    Instruction::new(0xEF, ISC_U, Absolute, 6),
    Instruction::new(0xF0, BEQ, Relative, 2),
    Instruction::new(0xF1, SBC, IndirectY, 5),
    Instruction::new(0xF2, KIL_X, Implied, 1),
    Instruction::new(0xF3, ISC_U, IndirectY, 8),
    Instruction::new(0xF4, IGN_U, ZeroPageX, 4),
    Instruction::new(0xF5, SBC, ZeroPageX, 4),
    Instruction::new(0xF6, INC, ZeroPageX, 6),
    Instruction::new(0xF7, ISC_U, ZeroPageX, 6),
    Instruction::new(0xF8, SED, Implied, 2),
    Instruction::new(0xF9, SBC, AbsoluteY, 4),
    Instruction::new(0xFA, NOP_U, Implied, 2),
    Instruction::new(0xFB, ISC_U, AbsoluteY, 7),
    Instruction::new(0xFC, IGN_U, AbsoluteX, 4),
    Instruction::new(0xFD, SBC, AbsoluteX, 4),
    Instruction::new(0xFE, INC, AbsoluteX, 7),
    Instruction::new(0xFF, ISC_U, AbsoluteX, 7),
];

const fn carrying_add(lhs: u8, rhs: u8, carry: bool) -> (u8, bool) {
    let (intermediate, carry_1) = lhs.overflowing_add(rhs);
    let (result, carry_2) = intermediate.overflowing_add(carry as u8);
    (result, carry_1 || carry_2)
}

const fn add_overflowed(lhs: u8, rhs: u8, result: u8) -> bool {
    // Overflow occurred if result's sign bit differs from that of both lhs and rhs
    ((result ^ lhs) & (result ^ rhs) & 0b10000000) != 0
}

const fn page_crossing_add(lhs: u16, rhs: u16) -> (u16, bool) {
    let result = lhs.wrapping_add(rhs);
    (result, result >> 8 != lhs >> 8)
}

fn process_branch(machine: &mut Machine) {
    let start_page = machine.cpu.program_counter & 0xFF00;
    let offset = machine.read_byte(machine.cpu.program_counter.wrapping_sub(1));
    // Offset is sign-extended
    machine.cpu.program_counter = machine.cpu.program_counter.wrapping_add_signed(offset as i8 as i16);
    // Delay 1 cycle since the branch is being taken
    machine.cpu.delay_cycles += 1;
    // If a page boundary was crossed, delay an extra cycle
    if machine.cpu.program_counter & 0xFF00 != start_page {
        machine.cpu.delay_cycles += 1;
    }
}

// Interrupt Request
// Meta
const IRQ: Operation = Operation {
    mnemonic: "IRQ",
    function: |_, machine| {
        machine.stack_push_word(machine.cpu.program_counter);
        machine.stack_push_byte(machine.cpu.get_status_byte(false));
        machine.cpu.interrupt_disable_flag = true;
        machine.cpu.program_counter = machine.read_word(IRQ_VECTOR);
    },
};

// Non-Maskable Interrupt
// Meta
const NMI: Operation = Operation {
    mnemonic: "NMI",
    function: |_, machine| {
        machine.stack_push_word(machine.cpu.program_counter);
        machine.stack_push_byte(machine.cpu.get_status_byte(false));
        machine.cpu.program_counter = machine.read_word(NMI_VECTOR);
    },
};

// Add with Carry
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const ADC: Operation = Operation {
    mnemonic: "ADC",
    function: |addressing_mode, machine| {
        let (address, page_crossed) = addressing_mode.calculate_address(machine);
        let addend = machine.read_byte(address);
        let carry_in = machine.cpu.carry_flag;
        let (result, carry_out) = carrying_add(machine.cpu.accumulator, addend, carry_in);
        let overflowed = add_overflowed(machine.cpu.accumulator, addend, result);
        machine.cpu.accumulator = result;

        machine.cpu.carry_flag = carry_out;
        machine.cpu.overflow_flag = overflowed;
        machine.cpu.set_result_flags(result);

        // If a page boundary was crossed, delay an extra cycle
        if page_crossed {
            machine.cpu.delay_cycles += 1;
        }
    },
};

// Bitwise AND
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const AND: Operation = Operation {
    mnemonic: "AND",
    function: |addressing_mode, machine| {
        let (address, page_crossed) = addressing_mode.calculate_address(machine);
        machine.cpu.accumulator &= machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.accumulator);

        // If a page boundary was crossed, delay an extra cycle
        if page_crossed {
            machine.cpu.delay_cycles += 1;
        }
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
            carry_out = (machine.cpu.accumulator & 0b10000000) != 0;
            result = machine.cpu.accumulator << 1;
            machine.cpu.accumulator = result;
        }
        else {
            let (address, _page_crossed) = addressing_mode.calculate_address(machine);
            let value = machine.read_byte(address);
            carry_out = (value & 0b10000000) != 0;
            result = value << 1;
            machine.write_byte(address, result);
        }

        machine.cpu.carry_flag = carry_out;
        machine.cpu.set_result_flags(result);
    },
};

// Branch if Carry Clear
// Relative
const BCC: Operation = Operation {
    mnemonic: "BCC",
    function: |_, machine| {
        if !machine.cpu.carry_flag {
            process_branch(machine);
        }
    },
};

// Branch if Carry Set
// Relative
const BCS: Operation = Operation {
    mnemonic: "BCS",
    function: |_, machine| {
        if machine.cpu.carry_flag {
            process_branch(machine);
        }
    },
};

// Branch if Equal
// Relative
const BEQ: Operation = Operation {
    mnemonic: "BEQ",
    function: |_, machine| {
        if machine.cpu.zero_flag {
            process_branch(machine);
        }
    },
};

// Bit Test
// ZeroPage, Absolute
const BIT: Operation = Operation {
    mnemonic: "BIT",
    function: |addressing_mode, machine| {
        let (address, _) = addressing_mode.calculate_address(machine);
        let value = machine.read_byte(address);

        machine.cpu.zero_flag = (machine.cpu.accumulator & value) == 0;
        machine.cpu.overflow_flag = (value & 0b01000000) != 0;
        machine.cpu.negative_flag = (value & 0b10000000) != 0;
    },
};

// Branch if Minus
// Relative
const BMI: Operation = Operation {
    mnemonic: "BMI",
    function: |_, machine| {
        if machine.cpu.negative_flag {
            process_branch(machine);
        }
    },
};

// Branch if Not Equal
// Relative
const BNE: Operation = Operation {
    mnemonic: "BNE",
    function: |_, machine| {
        if !machine.cpu.zero_flag {
            process_branch(machine);
        }
    },
};

// Branch if Plus
// Relative
const BPL: Operation = Operation {
    mnemonic: "BPL",
    function: |_, machine| {
        if !machine.cpu.negative_flag {
            process_branch(machine);
        }
    },
};

// Force Interrupt
// Implied
const BRK: Operation = Operation {
    mnemonic: "BRK",
    function: |_, machine| {
        machine.stack_push_word(machine.cpu.program_counter);
        machine.stack_push_byte(machine.cpu.get_status_byte(true));
        machine.cpu.interrupt_disable_flag = true;
        machine.cpu.program_counter = machine.read_word(IRQ_VECTOR);
    },
};

// Branch if Overflow Clear
// Relative
const BVC: Operation = Operation {
    mnemonic: "BVC",
    function: |_, machine| {
        if !machine.cpu.overflow_flag {
            process_branch(machine);
        }
    },
};

// Branch if Overflow Set
// Relative
const BVS: Operation = Operation {
    mnemonic: "BVS",
    function: |_, machine| {
        if machine.cpu.overflow_flag {
            process_branch(machine);
        }
    },
};

// Clear Carry Flag
// Implied
const CLC: Operation = Operation {
    mnemonic: "CLC",
    function: |_, machine| {
        machine.cpu.carry_flag = false;
    },
};

// Clear Decimal Flag
// Implied
const CLD: Operation = Operation {
    mnemonic: "CLD",
    function: |_, machine| {
        // NOTE: BCD mode is disabled in the Ricoh 2A03
        machine.cpu.decimal_mode_flag = false;
    },
};

// Clear Interrupt Disable Flag
// Implied
const CLI: Operation = Operation {
    mnemonic: "CLI",
    function: |_, machine| {
        machine.cpu.interrupt_disable_flag = false;
    },
};

// Clear Overflow Flag
// Implied
const CLV: Operation = Operation {
    mnemonic: "CLV",
    function: |_, machine| {
        machine.cpu.overflow_flag = false;
    },
};

// Compare
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const CMP: Operation = Operation {
    mnemonic: "CMP",
    function: |addressing_mode, machine| {
        let (address, page_crossed) = addressing_mode.calculate_address(machine);
        let subtrahend = machine.read_byte(address);
        let (result, borrowed) = machine.cpu.accumulator.overflowing_sub(subtrahend);

        machine.cpu.carry_flag = !borrowed;
        machine.cpu.set_result_flags(result);

        // If a page boundary was crossed, delay an extra cycle
        if page_crossed {
            machine.cpu.delay_cycles += 1;
        }
    },
};

// Compare X Register
// Immediate, ZeroPage, Absolute
const CPX: Operation = Operation {
    mnemonic: "CPX",
    function: |addressing_mode, machine| {
        let (address, _) = addressing_mode.calculate_address(machine);
        let subtrahend = machine.read_byte(address);
        let (result, borrowed) = machine.cpu.register_x.overflowing_sub(subtrahend);

        machine.cpu.carry_flag = !borrowed;
        machine.cpu.set_result_flags(result);
    },
};

// Compare Y Register
// Immediate, ZeroPage, Absolute
const CPY: Operation = Operation {
    mnemonic: "CPY",
    function: |addressing_mode, machine| {
        let (address, _) = addressing_mode.calculate_address(machine);
        let subtrahend = machine.read_byte(address);
        let (result, borrowed) = machine.cpu.register_y.overflowing_sub(subtrahend);

        machine.cpu.carry_flag = !borrowed;
        machine.cpu.set_result_flags(result);
    },
};

// Decrement Memory
// ZeroPage, ZeroPageX, Absolute, AbsoluteX
const DEC: Operation = Operation {
    mnemonic: "DEC",
    function: |addressing_mode, machine| {
        let (address, _page_crossed) = addressing_mode.calculate_address(machine);
        let result = machine.read_byte(address).wrapping_sub(1);
        machine.write_byte(address, result);

        machine.cpu.set_result_flags(result);
    },
};

// Decrement X Register
// Implied
const DEX: Operation = Operation {
    mnemonic: "DEX",
    function: |_, machine| {
        machine.cpu.register_x = machine.cpu.register_x.wrapping_sub(1);

        machine.cpu.set_result_flags(machine.cpu.register_x);
    },
};

// Decrement Y Register
// Implied
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
        let (address, page_crossed) = addressing_mode.calculate_address(machine);
        machine.cpu.accumulator ^= machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.accumulator);

        // If a page boundary was crossed, delay an extra cycle
        if page_crossed {
            machine.cpu.delay_cycles += 1;
        }
    },
};

// Increment Memory
// ZeroPage, ZeroPageX, Absolute, AbsoluteX
const INC: Operation = Operation {
    mnemonic: "INC",
    function: |addressing_mode, machine| {
        let (address, _page_crossed) = addressing_mode.calculate_address(machine);
        let result = machine.read_byte(address).wrapping_add(1);
        machine.write_byte(address, result);

        machine.cpu.set_result_flags(result);
    },
};

// Increment X Register
// Implied
const INX: Operation = Operation {
    mnemonic: "INX",
    function: |_, machine| {
        machine.cpu.register_x = machine.cpu.register_x.wrapping_add(1);

        machine.cpu.set_result_flags(machine.cpu.register_x);
    },
};

// Increment Y Register
// Implied
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
        let (address, _) = addressing_mode.calculate_address(machine);
        machine.cpu.program_counter = address;
    },
};

// Jump to Subroutine
// Absolute
const JSR: Operation = Operation {
    mnemonic: "JSR",
    function: |addressing_mode, machine| {
        let (address, _) = addressing_mode.calculate_address(machine);
        machine.stack_push_word(machine.cpu.program_counter.wrapping_sub(1));
        machine.cpu.program_counter = address;
    },
};

// Load Accumulator
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const LDA: Operation = Operation {
    mnemonic: "LDA",
    function: |addressing_mode, machine| {
        let (address, page_crossed) = addressing_mode.calculate_address(machine);
        machine.cpu.accumulator = machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.accumulator);

        // If a page boundary was crossed, delay an extra cycle
        if page_crossed {
            machine.cpu.delay_cycles += 1;
        }
    },
};

// Load X Register
// Immediate, ZeroPage, ZeroPageY, Absolute, AbsoluteY
const LDX: Operation = Operation {
    mnemonic: "LDX",
    function: |addressing_mode, machine| {
        let (address, page_crossed) = addressing_mode.calculate_address(machine);
        machine.cpu.register_x = machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.register_x);

        // If a page boundary was crossed, delay an extra cycle
        if page_crossed {
            machine.cpu.delay_cycles += 1;
        }
    },
};

// Load Y Register
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const LDY: Operation = Operation {
    mnemonic: "LDY",
    function: |addressing_mode, machine| {
        let (address, page_crossed) = addressing_mode.calculate_address(machine);
        machine.cpu.register_y = machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.register_y);

        // If a page boundary was crossed, delay an extra cycle
        if page_crossed {
            machine.cpu.delay_cycles += 1;
        }
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
            carry_out = (machine.cpu.accumulator & 0b00000001) != 0;
            result = machine.cpu.accumulator >> 1;
            machine.cpu.accumulator = result;
        }
        else {
            let (address, _page_crossed) = addressing_mode.calculate_address(machine);
            let value = machine.read_byte(address);
            carry_out = (value & 0b00000001) != 0;
            result = value >> 1;
            machine.write_byte(address, result);
        }

        machine.cpu.carry_flag = carry_out;
        machine.cpu.set_result_flags(result);
    },
};

// No Operation
// Implied
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
        let (address, page_crossed) = addressing_mode.calculate_address(machine);
        machine.cpu.accumulator |= machine.read_byte(address);

        machine.cpu.set_result_flags(machine.cpu.accumulator);

        // If a page boundary was crossed, delay an extra cycle
        if page_crossed {
            machine.cpu.delay_cycles += 1;
        }
    },
};

// Push Accumulator
// Implied
const PHA: Operation = Operation {
    mnemonic: "PHA",
    function: |_, machine| {
        machine.stack_push_byte(machine.cpu.accumulator);
    },
};

// Push Processor Status
// Implied
const PHP: Operation = Operation {
    mnemonic: "PHP",
    function: |_, machine| {
        machine.stack_push_byte(machine.cpu.get_status_byte(true));
    },
};

// Pull Accumulator
// Implied
const PLA: Operation = Operation {
    mnemonic: "PLA",
    function: |_, machine| {
        machine.cpu.accumulator = machine.stack_pull_byte();

        machine.cpu.set_result_flags(machine.cpu.accumulator);
    },
};

// Pull Processor Status
// Implied
const PLP: Operation = Operation {
    mnemonic: "PLP",
    function: |_, machine| {
        let status_byte = machine.stack_pull_byte();
        machine.cpu.set_status_byte(status_byte);
    },
};

// Rotate Left
// Accumulator, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const ROL: Operation = Operation {
    mnemonic: "ROL",
    function: |addressing_mode, machine| {
        let carry_in = machine.cpu.carry_flag;
        let carry_out;
        let result;
        if let Accumulator = addressing_mode {
            carry_out = (machine.cpu.accumulator & 0b10000000) != 0;
            result = machine.cpu.accumulator << 1 | carry_in as u8;
            machine.cpu.accumulator = result;
        }
        else {
            let (address, _page_crossed) = addressing_mode.calculate_address(machine);
            let value = machine.read_byte(address);
            carry_out = (value & 0b10000000) != 0;
            result = value << 1 | carry_in as u8;
            machine.write_byte(address, result);
        }

        machine.cpu.carry_flag = carry_out;
        machine.cpu.set_result_flags(result);
    },
};

// Rotate Right
// Accumulator, ZeroPage, ZeroPageX, Absolute, AbsoluteX
const ROR: Operation = Operation {
    mnemonic: "ROR",
    function: |addressing_mode, machine| {
        let carry_in = machine.cpu.carry_flag;
        let carry_out;
        let result;
        if let Accumulator = addressing_mode {
            carry_out = (machine.cpu.accumulator & 0b00000001) != 0;
            result = machine.cpu.accumulator >> 1 | (carry_in as u8) << 7;
            machine.cpu.accumulator = result;
        }
        else {
            let (address, _page_crossed) = addressing_mode.calculate_address(machine);
            let value = machine.read_byte(address);
            carry_out = (value & 0b00000001) != 0;
            result = value >> 1 | (carry_in as u8) << 7;
            machine.write_byte(address, result);
        }

        machine.cpu.carry_flag = carry_out;
        machine.cpu.set_result_flags(result);
    },
};

// Return from Interrupt
// Implied
const RTI: Operation = Operation {
    mnemonic: "RTI",
    function: |_, machine| {
        let status_byte = machine.stack_pull_byte();
        let program_counter = machine.stack_pull_word();
        machine.cpu.set_status_byte(status_byte);
        machine.cpu.program_counter = program_counter;
    },
};

// Return from Subroutine
// Implied
const RTS: Operation = Operation {
    mnemonic: "RTS",
    function: |_, machine| {
        let program_counter = machine.stack_pull_word().wrapping_add(1);
        machine.cpu.program_counter = program_counter;
    },
};

// Subtract with Carry
// Immediate, ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const SBC: Operation = Operation {
    mnemonic: "SBC",
    function: |addressing_mode, machine| {
        let (address, page_crossed) = addressing_mode.calculate_address(machine);
        let addend = !machine.read_byte(address); // Invert for subtraction
        let carry_in = machine.cpu.carry_flag;
        let (result, carry_out) = carrying_add(machine.cpu.accumulator, addend, carry_in);
        let overflowed = add_overflowed(machine.cpu.accumulator, addend, result);
        machine.cpu.accumulator = result;

        machine.cpu.carry_flag = carry_out;
        machine.cpu.overflow_flag = overflowed;
        machine.cpu.set_result_flags(result);

        // If a page boundary was crossed, delay an extra cycle
        if page_crossed {
            machine.cpu.delay_cycles += 1;
        }
    },
};

// Set Carry Flag
// Implied
const SEC: Operation = Operation {
    mnemonic: "SEC",
    function: |_, machine| {
        machine.cpu.carry_flag = true;
    },
};

// Set Decimal Flag
// Implied
const SED: Operation = Operation {
    mnemonic: "SED",
    function: |_, machine| {
        // NOTE: BCD mode is disabled in the Ricoh 2A03
        machine.cpu.decimal_mode_flag = true;
    },
};

// Set Interrupt Disable Flag
// Implied
const SEI: Operation = Operation {
    mnemonic: "SEI",
    function: |_, machine| {
        machine.cpu.interrupt_disable_flag = true;
    },
};

// Store Accumulator
// ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const STA: Operation = Operation {
    mnemonic: "STA",
    function: |addressing_mode, machine| {
        let (address, _page_crossed) = addressing_mode.calculate_address(machine);
        machine.write_byte(address, machine.cpu.accumulator);
    },
};

// Store X Register
// ZeroPage, ZeroPageY, Absolute
const STX: Operation = Operation {
    mnemonic: "STX",
    function: |addressing_mode, machine| {
        let (address, _) = addressing_mode.calculate_address(machine);
        machine.write_byte(address, machine.cpu.register_x);
    },
};

// Store Y Register
// ZeroPage, ZeroPageX, Absolute
const STY: Operation = Operation {
    mnemonic: "STY",
    function: |addressing_mode, machine| {
        let (address, _) = addressing_mode.calculate_address(machine);
        machine.write_byte(address, machine.cpu.register_y);
    },
};

// Transfer Accumulator to X Register
// Implied
const TAX: Operation = Operation {
    mnemonic: "TAX",
    function: |_, machine| {
        machine.cpu.register_x = machine.cpu.accumulator;

        machine.cpu.set_result_flags(machine.cpu.register_x);
    },
};

// Transfer Accumulator to Y Register
// Implied
const TAY: Operation = Operation {
    mnemonic: "TAY",
    function: |_, machine| {
        machine.cpu.register_y = machine.cpu.accumulator;

        machine.cpu.set_result_flags(machine.cpu.register_y);
    },
};

// Transfer Stack Pointer to X Register
// Implied
const TSX: Operation = Operation {
    mnemonic: "TSX",
    function: |_, machine| {
        machine.cpu.register_x = machine.cpu.stack_pointer;

        machine.cpu.set_result_flags(machine.cpu.register_x);
    },
};

// Transfer X Register to Accumulator
// Implied
const TXA: Operation = Operation {
    mnemonic: "TXA",
    function: |_, machine| {
        machine.cpu.accumulator = machine.cpu.register_x;

        machine.cpu.set_result_flags(machine.cpu.accumulator);
    },
};

// Transfer X Register to Stack Pointer
// Implied
const TXS: Operation = Operation {
    mnemonic: "TXS",
    function: |_, machine| {
        machine.cpu.stack_pointer = machine.cpu.register_x;
    },
};

// Transfer Y Register to Accumulator
// Implied
const TYA: Operation = Operation {
    mnemonic: "TYA",
    function: |_, machine| {
        machine.cpu.accumulator = machine.cpu.register_y;

        machine.cpu.set_result_flags(machine.cpu.accumulator);
    },
};

// UNOFFICIAL OPCODES

// Crash CPU (freeze until console is reset)
// Implied (x12)
const KIL_X: Operation = Operation {
    mnemonic: "KIL*",
    function: |_, machine| {
        let opcode = machine.read_byte_silent(machine.cpu.program_counter.wrapping_sub(1));
        eprintln!("Warning: CPU crashed due to unofficial opcode ${opcode:02X} (KIL*).");
        // machine.cpu.is_halted = true;
    }
};

// Duplicate NOP, for the purpose of having a distinct mnemonic
// Implied (x6)
const NOP_U: Operation = Operation {
    mnemonic: "NOP*",
    function: NOP.function,
};

// Duplicate SBC, for the purpose of having a distinct mnemonic
// Immediate
const SBC_U: Operation = Operation {
    mnemonic: "SBC*",
    function: SBC.function,
};

// Skip Byte (unofficial NOP with discarded immediate)
// Immediate (x5)
const SKB_U: Operation = Operation {
    mnemonic: "SKB*",
    function: NOP.function,
};

// Dummy Read (read and discard byte from memory)
// ZeroPage (x3), ZeroPageX (x6), Absolute, AbsoluteX (x6)
const IGN_U: Operation = Operation {
    mnemonic: "IGN*",
    function: |addressing_mode, machine| {
        let address;
        let page_crossed;
        if let ZeroPageX = addressing_mode {
            // Also reads from the base ZeroPage address as well for some reason
            let base_address = machine.read_byte(machine.cpu.program_counter.wrapping_sub(1));
            machine.read_byte(base_address as u16);
            address = base_address.wrapping_add(machine.cpu.register_x) as u16;
            page_crossed = false;
        }
        else {
            (address, page_crossed) = addressing_mode.calculate_address(machine);
        }
        machine.read_byte(address);

        // If a page boundary was crossed, delay an extra cycle
        if page_crossed {
            machine.cpu.delay_cycles += 1;
        }
    },
};

// Decrement then Compare
// ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const DCP_U: Operation = Operation {
    mnemonic: "DCP*",
    function: |addressing_mode, machine| {
        (DEC.function)(addressing_mode, machine);
        (CMP.function)(addressing_mode, machine);
        machine.cpu.delay_cycles = 0;
    },
};

// Increment then Subtract with Carry
// ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const ISC_U: Operation = Operation {
    mnemonic: "ISC*",
    function: |addressing_mode, machine| {
        (INC.function)(addressing_mode, machine);
        (SBC.function)(addressing_mode, machine);
        machine.cpu.delay_cycles = 0;
    },
};

// Rotate Left then Bitwise AND
// ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const RLA_U: Operation = Operation {
    mnemonic: "RLA*",
    function: |addressing_mode, machine| {
        (ROL.function)(addressing_mode, machine);
        (AND.function)(addressing_mode, machine);
        machine.cpu.delay_cycles = 0;
    },
};

// Rotate Right then Add with Carry
// ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const RRA_U: Operation = Operation {
    mnemonic: "RRA*",
    function: |addressing_mode, machine| {
        (ROR.function)(addressing_mode, machine);
        (ADC.function)(addressing_mode, machine);
        machine.cpu.delay_cycles = 0;
    },
};

// Arithmetic Shift Left then Bitwise Inclusive OR
// ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const SLO_U: Operation = Operation {
    mnemonic: "SLO*",
    function: |addressing_mode, machine| {
        (ASL.function)(addressing_mode, machine);
        (ORA.function)(addressing_mode, machine);
        machine.cpu.delay_cycles = 0;
    },
};

// Logical Shift Right then Bitwise Exclusive OR
// ZeroPage, ZeroPageX, Absolute, AbsoluteX, AbsoluteY, IndirectX, IndirectY
const SRE_U: Operation = Operation {
    mnemonic: "SRE*",
    function: |addressing_mode, machine| {
        (LSR.function)(addressing_mode, machine);
        (EOR.function)(addressing_mode, machine);
        machine.cpu.delay_cycles = 0;
    },
};

// Buggy STX abs,Y
// AbsoluteY
const SHX_U: Operation = Operation {
    mnemonic: "SHX*",
    function: |_, machine| {
        // Compute address manually since the literal address is important
        let literal_address = machine.read_word(machine.cpu.program_counter.wrapping_sub(2));
        let (address, page_crossed) = page_crossing_add(literal_address, machine.cpu.register_y as u16);
        let mask = ((if page_crossed { address } else { literal_address.wrapping_add(0x100) }) >> 8) as u8;
        machine.write_byte(address, machine.cpu.register_x & mask);
    },
};

// Buggy STY abs,X
// AbsoluteX
const SHY_U: Operation = Operation {
    mnemonic: "SHY*",
    function: |_, machine| {
        // Compute address manually since the literal address is important
        let literal_address = machine.read_word(machine.cpu.program_counter.wrapping_sub(2));
        let (address, page_crossed) = page_crossing_add(literal_address, machine.cpu.register_x as u16);
        let mask = ((if page_crossed { address } else { literal_address.wrapping_add(0x100) }) >> 8) as u8;
        machine.write_byte(address, machine.cpu.register_y & mask);
    },
};

// AND #imm; LSR A
// Immediate
const ALR_U: Operation = Operation {
    mnemonic: "ALR*",
    function: |_, machine| {
        (AND.function)(Immediate, machine);
        (LSR.function)(Accumulator, machine);
    },
};

// AND #imm; C <- N
// Immediate (x2)
const ANC_U: Operation = Operation {
    mnemonic: "ANC*",
    function: |_, machine| {
        (AND.function)(Immediate, machine);
        machine.cpu.carry_flag = machine.cpu.negative_flag;
    },
};

// AND #imm; ROR A; C <- A[6]; V <- C ^ A[5]
// Immediate
const ARR_U: Operation = Operation {
    mnemonic: "ARR*",
    function: |_, machine| {
        (AND.function)(Immediate, machine);
        (ROR.function)(Accumulator, machine);
        let result = machine.cpu.accumulator;
        machine.cpu.set_result_flags(result);
        machine.cpu.carry_flag = result & 0b01000000 != 0;
        machine.cpu.overflow_flag = machine.cpu.carry_flag != (result & 0b00100000 != 0);
    },
};

// X,N,Z,C <- (A & X) - #imm
// Immediate
const AXS_U: Operation = Operation {
    mnemonic: "AXS*",
    function: |addressing_mode, machine| {
        let (address, _) = addressing_mode.calculate_address(machine);
        let lhs = machine.cpu.accumulator & machine.cpu.register_x;
        let rhs = !machine.read_byte(address); // Invert for subtraction
        let (result, carry_out) = carrying_add(lhs, rhs, true);
        machine.cpu.register_x = result;
        machine.cpu.set_result_flags(result);
        machine.cpu.carry_flag = carry_out;
    },
};

// LDA _; TAX
// ZeroPage, ZeroPageY, Absolute, AbsoluteY, IndirectX, IndirectY
const LAX_U: Operation = Operation {
    mnemonic: "LAX*",
    function: |addressing_mode, machine| {
        if let Immediate = addressing_mode {
            eprintln!("Warning: Executing unstable opcode $AB (LAX* #imm).");
        }
        (LDA.function)(addressing_mode, machine);
        (TAX.function)(Implied, machine);
    },
};

// _ <- A & X
// ZeroPage, ZeroPageY, Absolute, IndirectX
const SAX_U: Operation = Operation {
    mnemonic: "SAX*",
    function: |addressing_mode, machine| {
        let (address, _page_crossed) = addressing_mode.calculate_address(machine);
        machine.write_byte(address, machine.cpu.accumulator & machine.cpu.register_x);
    },
};

// SHX* _ but with A & X instead of just X
// AbsoluteY, IndirectY
const SHA_U: Operation = Operation {
    mnemonic: "SHA*",
    function: |addressing_mode, machine| {
        // Compute address manually since the literal address is important
        let literal_address;
        if let IndirectY = addressing_mode {
            let address = machine.read_byte(machine.cpu.program_counter.wrapping_sub(1)) as u16;
            literal_address = machine.read_word_paged(address);
        }
        else {
            literal_address = machine.read_word(machine.cpu.program_counter.wrapping_sub(2));
        }
        let (address, page_crossed) = page_crossing_add(literal_address, machine.cpu.register_y as u16);
        let mask = ((if page_crossed { address } else { literal_address.wrapping_add(0x100) }) >> 8) as u8;
        machine.write_byte(address, machine.cpu.accumulator & machine.cpu.register_x & mask);
    },
};

// S <- A & X; M <- A & X & H
// AbsoluteY
const TAS_U: Operation = Operation {
    mnemonic: "TAS*",
    function: |addressing_mode, machine| {
        let (address, _) = addressing_mode.calculate_address(machine);
        let result = machine.cpu.accumulator & machine.cpu.register_x;
        machine.cpu.stack_pointer = result & (address >> 8) as u8;
        machine.write_byte(address, result);
    },
};

// LDA _ & S; TAX; TXS
// AbsoluteY
const LAS_U: Operation = Operation {
    mnemonic: "LAS*",
    function: |addressing_mode, machine| {
        (LDA.function)(addressing_mode, machine);
        // Cheat a little to do bitwise AND with stack pointer (TAX will reset result flags)
        machine.cpu.accumulator &= machine.cpu.stack_pointer;
        (TAX.function)(Implied, machine);
        (TXS.function)(Implied, machine);
    },
};

// Unstable instruction I don't feel like implementing properly
// Immediate
const XAA_U: Operation = Operation {
    mnemonic: "XAA*",
    function: |_, machine| {
        eprintln!("Warning: Executing unstable opcode $8B (XAA* #imm).");
        (TXA.function)(Implied, machine);
        (AND.function)(Immediate, machine);
    },
};
