use super::*;

pub const PRG_RAM_SIZE: usize = 0x2000;

#[derive(Copy, Clone, PartialEq, Debug)]
enum IRQTrigger {
    /// IRQ is triggered if counter is 0 after decrementing/reloading.
    Level,
    /// IRQ is triggered if counter newly becomes 0 while decrementing/reloading.
    Edge,
}

impl std::fmt::Display for IRQTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            IRQTrigger::Level => write!(f, "Level"),
            IRQTrigger::Edge => write!(f, "Edge"),
        }
    }
}

pub struct Mapper004 {
    nametables: BuiltinNametables,
    prg_chunks: Vec<PrgChunk>,
    chr_chunks: Vec<ChrChunk>,
    prg_ram: Box<[u8; PRG_RAM_SIZE]>,
    bank_registers: [u8; 8],
    register_select: usize,
    prg_bank_mode_1: bool,
    chr_bank_mode_1: bool,
    prg_bank_mask: u8,
    chr_bank_mask: u8,
    prg_bank_indices: [(usize, usize); 4],
    chr_bank_indices: [(usize, usize); 8],
    irq_trigger: IRQTrigger,
    irq_enabled: bool,
    irq_counter: u8,
    irq_reload_value: u8,
    irq_reload_requested: bool,
    irq_asserted: bool,
    irq_triggered: bool,
    ticks_since_a12_high: u16,
}

impl Mapper004 {
    pub fn new(header: &NESFileHeader, prg_chunks: Vec<PrgChunk>, chr_chunks: Vec<ChrChunk>) -> Result<Self, String> {
        let mut prg_bank_mask = 0xFF;
        while prg_bank_mask as usize >= (prg_chunks.len() << 1) {
            prg_bank_mask >>= 1;
        }
        let mut chr_bank_mask = 0xFF;
        while chr_bank_mask as usize >= (chr_chunks.len() << 3) {
            chr_bank_mask >>= 1;
        }

        let mut mapper = Self {
            nametables: BuiltinNametables::new(header.nametable_arrangement),
            prg_chunks,
            chr_chunks,
            prg_ram: Box::new([0; PRG_RAM_SIZE]),
            bank_registers: [0; 8],
            register_select: 0,
            prg_bank_mode_1: false,
            chr_bank_mode_1: false,
            prg_bank_mask,
            chr_bank_mask,
            prg_bank_indices: [(0, 0); 4],
            chr_bank_indices: [(0, 0); 8],
            irq_trigger: match header.submapper_number {
                2 => IRQTrigger::Edge,
                _ => IRQTrigger::Level
            },
            irq_enabled: false,
            irq_counter: 0,
            irq_reload_value: 0,
            irq_reload_requested: false,
            irq_asserted: false,
            irq_triggered: false,
            ticks_since_a12_high: 0,
        };

        mapper.update_prg_banks();
        mapper.update_chr_banks();

        Ok(mapper)
    }

    fn update_prg_banks(&mut self) {
        // I love pointlessly doing branchless programming
        let swappable_bank_index = (self.prg_bank_mode_1 as usize) << 1;
        let fixed_bank_index = swappable_bank_index ^ 0b10;

        self.prg_bank_indices[swappable_bank_index] = self.prg_indices(self.bank_registers[6]);
        self.prg_bank_indices[1] = self.prg_indices(self.bank_registers[7]);
        self.prg_bank_indices[fixed_bank_index] = self.prg_indices(self.prg_bank_mask - 1);
        self.prg_bank_indices[3] = self.prg_indices(self.prg_bank_mask);
    }

    fn prg_indices(&self, number: u8) -> (usize, usize) {
        let number = (number & self.prg_bank_mask) as usize;
        let chunk_index = number >> 1;
        let byte_offset = (number & 1) << 13;
        (chunk_index, byte_offset)
    }

    fn update_chr_banks(&mut self) {
        // I love pointlessly doing branchless programming
        let big_chunks_offset = (self.chr_bank_mode_1 as usize) << 2;
        let small_chunks_offset = big_chunks_offset ^ 0b100;

        self.chr_bank_indices[big_chunks_offset + 0] = self.chr_indices(self.bank_registers[0] & !1);
        self.chr_bank_indices[big_chunks_offset + 1] = self.chr_indices(self.bank_registers[0] | 1);
        self.chr_bank_indices[big_chunks_offset + 2] = self.chr_indices(self.bank_registers[1] & !1);
        self.chr_bank_indices[big_chunks_offset + 3] = self.chr_indices(self.bank_registers[1] | 1);

        self.chr_bank_indices[small_chunks_offset + 0] = self.chr_indices(self.bank_registers[2]);
        self.chr_bank_indices[small_chunks_offset + 1] = self.chr_indices(self.bank_registers[3]);
        self.chr_bank_indices[small_chunks_offset + 2] = self.chr_indices(self.bank_registers[4]);
        self.chr_bank_indices[small_chunks_offset + 3] = self.chr_indices(self.bank_registers[5]);
    }

    fn chr_indices(&self, number: u8) -> (usize, usize) {
        let number = (number & self.chr_bank_mask) as usize;
        let chunk_index = number >> 3;
        let byte_offset = (number & 0b111) << 10;
        (chunk_index, byte_offset)
    }

    fn check_ppu_address(&mut self, address: u16) {
        if address & 0x1000 == 0 {
            return;
        }

        if self.ticks_since_a12_high > 4 * crate::hardware::cpu::TICKS_PER_CPU_CYCLE {
            // End of scanline detected, decrement counter
            let previous_irq_counter = self.irq_counter;
            if self.irq_counter == 0 || self.irq_reload_requested {
                self.irq_counter = self.irq_reload_value;
                self.irq_reload_requested = false;
            }
            else {
                self.irq_counter -= 1;
            }
            // println!("MMC3 counter: ${address:04X} => {}", self.irq_counter);
            if self.irq_enabled {
                let trigger_condition = match self.irq_trigger {
                    IRQTrigger::Level => self.irq_counter == 0,
                    IRQTrigger::Edge => self.irq_counter == 0 && previous_irq_counter != 0,
                };
                if trigger_condition {
                    self.irq_triggered |= !self.irq_asserted;
                    self.irq_asserted = true;
                }
            }
        }

        self.ticks_since_a12_high = 0;
    }
}

impl Mapper for Mapper004 {
    fn name(&self) -> &'static str {
        "Mapper 004 (MMC3/MMC6)"
    }

    fn tick(&mut self) {
        self.ticks_since_a12_high = self.ticks_since_a12_high.saturating_add(1);
    }

    fn check_irq(&mut self) -> bool {
        std::mem::replace(&mut self.irq_triggered, false)
    }

    fn read_cpu_byte(&self, address: u16) -> u8 {
        match address {
            0x6000 ..= 0x7FFF => {
                self.prg_ram[(address & 0x1FFF) as usize]
            }
            0x8000 ..= 0xFFFF => {
                let bank_number = (address >> 13) & 0b11;
                let (chunk_index, byte_offset) = self.prg_bank_indices[bank_number as usize];
                self.prg_chunks[chunk_index][byte_offset + (address & 0x1FFF) as usize]
            }
            _ => crate::hardware::OPEN_BUS
        }
    }

    fn write_cpu_byte(&mut self, address: u16, value: u8) {
        match address {
            0x6000 ..= 0x7FFF => {
                self.prg_ram[(address & 0x1FFF) as usize] = value;
            }
            0x8000 ..= 0x9FFF => {
                // Even: Bank Select, Odd: Bank Data
                if address & 1 == 0 {
                    self.register_select = (value & 0b111) as usize;
                    let prg_bank_mode_1 = value & 0b01000000 != 0;
                    if prg_bank_mode_1 != self.prg_bank_mode_1 {
                        self.prg_bank_mode_1 = prg_bank_mode_1;
                        self.update_prg_banks();
                    }
                    let chr_bank_mode_1 = value & 0b10000000 != 0;
                    if chr_bank_mode_1 != self.chr_bank_mode_1 {
                        self.chr_bank_mode_1 = chr_bank_mode_1;
                        self.update_chr_banks();
                    }
                }
                else if self.bank_registers[self.register_select] != value {
                    self.bank_registers[self.register_select] = value;
                    if self.register_select < 6 {
                        self.update_chr_banks();
                    }
                    else {
                        self.update_prg_banks();
                    }
                }
            }
            0xA000 ..= 0xBFFF => {
                // Even: Nametable Mirroring, Odd: PRG-RAM Protect
                if address & 1 == 0 {
                    self.nametables.arrangement = if value & 1 != 0 {
                        NametableArrangement::Vertical
                    } else {
                        NametableArrangement::Horizontal
                    };
                }
                else {
                    // Ignore this one...
                }
            }
            0xC000 ..= 0xDFFF => {
                // Even: IRQ Latch, Odd: IRQ Reload
                if address & 1 == 0 {
                    self.irq_reload_value = value;
                }
                else {
                    self.irq_reload_requested = true;
                }
            }
            0xE000 ..= 0xFFFF => {
                // Even: IRQ Disable, Odd: IRQ Enable
                self.irq_enabled = address & 1 != 0;
                if !self.irq_enabled {
                    self.irq_triggered = false;
                    self.irq_asserted = false;
                }
            }
            _ => {}
        }
    }

    fn read_ppu_byte(&mut self, address: u16) -> u8 {
        self.check_ppu_address(address);
        match address {
            0x0000 ..= 0x1FFF => {
                let bank_number = (address >> 10) & 0b111;
                let (chunk_index, byte_offset) = self.chr_bank_indices[bank_number as usize];
                self.chr_chunks[chunk_index][byte_offset + (address & 0x03FF) as usize]
            }
            _ => {
                self.nametables.read_byte(address)
            }
        }
    }

    fn write_ppu_byte(&mut self, address: u16, value: u8) {
        self.check_ppu_address(address);
        match address {
            0x0000 ..= 0x1FFF => {
                // TODO: how to detect CHR-RAM?
                let bank_number = (address >> 10) & 0b111;
                let (chunk_index, byte_offset) = self.chr_bank_indices[bank_number as usize];
                self.chr_chunks[chunk_index][byte_offset + (address & 0x03FF) as usize] = value;
            }
            _ => {
                self.nametables.write_byte(address, value)
            }
        }
    }

    fn debug_print_state(&self) {
        println!("{}:", self.name());
        let bank_select = self.register_select as u8
            | (self.prg_bank_mode_1 as u8) << 6
            | (self.chr_bank_mode_1 as u8) << 7;
        println!("                  CP---RRR");
        println!("    Bank select: %{:08b}", bank_select);
        println!("    Bank registers: {:?}", self.bank_registers);
        println!("    Nametable arrangement: {}", self.nametables.arrangement);
        println!("    IRQ trigger type: {}", self.irq_trigger);
        println!("    IRQ enabled: {}", self.irq_enabled);
        println!("    IRQ reload value: {}", self.irq_reload_value);
        println!("    IRQ reload requested: {}", self.irq_reload_requested);
        println!("    IRQ counter: {}", self.irq_counter);
    }
}
