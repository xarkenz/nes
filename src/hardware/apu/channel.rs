const LENGTH_TABLE: [u8; 32] = [
    0x0A,
    0xFE,
    0x14,
    0x02,
    0x28,
    0x04,
    0x50,
    0x06,
    0xA0,
    0x08,
    0x3C,
    0x0A,
    0x0E,
    0x0C,
    0x1A,
    0x0E,
    0x0C,
    0x10,
    0x18,
    0x12,
    0x30,
    0x14,
    0x60,
    0x16,
    0xC0,
    0x18,
    0x48,
    0x1A,
    0x10,
    0x1C,
    0x20,
    0x1E,
];

pub struct PulseChannel {
    is_enabled: bool,
    length_counter: u8,
    envelope: u8,
    constant_volume: bool,
    halt_length_counter: bool,
    duty: u8,
    timer: u16,
    sweep_enabled: bool,
    sweep_shift: u8,
    sweep_negate: bool,
    sweep_period: u8,
}

impl PulseChannel {
    pub fn new() -> Self {
        Self {
            is_enabled: false,
            length_counter: 0,
            envelope: 0,
            constant_volume: false,
            halt_length_counter: false,
            duty: 0,
            timer: 0,
            sweep_enabled: false,
            sweep_shift: 0,
            sweep_negate: false,
            sweep_period: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.length_counter > 0
    }

    pub fn set_enabled(&mut self, enable: bool) {
        self.is_enabled = enable;
        if !enable {
            self.length_counter = 0;
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address & 0b11 {
            0b00 => {
                self.envelope = value & 0b1111;
                self.constant_volume = value & 0b10000 != 0;
                self.halt_length_counter = value & 0b100000 != 0;
                self.duty = value >> 6;
            }
            0b01 => {
                self.sweep_shift = value & 0b111;
                self.sweep_negate = value & 0b1000 != 0;
                self.sweep_period = (value >> 4) & 0b111;
                self.sweep_enabled = value & 0b10000000 != 0;
            }
            0b10 => {
                self.timer &= 0b111_00000000;
                self.timer |= value as u16;
            }
            0b11 => {
                self.timer &= 0b000_11111111;
                self.timer |= (value as u16 & 0b111) << 8;
                if self.is_enabled {
                    self.length_counter = LENGTH_TABLE[(value >> 3) as usize];
                }
            }
            _ => unreachable!()
        }
    }

    pub fn clock_cpu_cycle(&mut self) {
        //
    }

    pub fn clock_quarter_frame(&mut self) {
        //
    }

    pub fn clock_half_frame(&mut self) {
        self.length_counter = self.length_counter.saturating_sub(!self.halt_length_counter as u8);
    }

    pub fn debug_print_state(&self) {
        println!("        Enabled: {}", self.is_enabled);
        println!("        Length counter: {}", self.length_counter);
        println!("        Length counter halted: {}", self.halt_length_counter);
    }
}

pub struct TriangleChannel {
    is_enabled: bool,
    length_counter: u8,
    linear_counter: u8,
    halt_counters: bool,
    linear_counter_reload_value: u8,
    reload_linear_counter: bool,
    timer: u16,
}

impl TriangleChannel {
    pub fn new() -> Self {
        Self {
            is_enabled: false,
            length_counter: 0,
            linear_counter: 0,
            halt_counters: false,
            linear_counter_reload_value: 0,
            reload_linear_counter: false,
            timer: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.length_counter > 0
    }

    pub fn set_enabled(&mut self, enable: bool) {
        self.is_enabled = enable;
        if !enable {
            self.length_counter = 0;
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address & 0b11 {
            0b00 => {
                self.linear_counter_reload_value = value & 0b1111111;
                self.halt_counters = value & 0b10000000 != 0;
            }
            0b01 => {
                // Unused
            }
            0b10 => {
                self.timer &= 0b111_00000000;
                self.timer |= value as u16;
            }
            0b11 => {
                self.timer &= 0b000_11111111;
                self.timer |= (value as u16 & 0b111) << 8;
                if self.is_enabled {
                    self.length_counter = LENGTH_TABLE[(value >> 3) as usize];
                }
                self.reload_linear_counter = true;
            }
            _ => unreachable!()
        }
    }

    pub fn clock_cpu_cycle(&mut self) {
        //
    }

    pub fn clock_quarter_frame(&mut self) {
        if self.reload_linear_counter {
            self.linear_counter = self.linear_counter_reload_value;
        }
        else {
            self.linear_counter = self.linear_counter.saturating_sub(!self.halt_counters as u8);
        }
        self.reload_linear_counter &= self.halt_counters;
    }

    pub fn clock_half_frame(&mut self) {
        self.length_counter = self.length_counter.saturating_sub(!self.halt_counters as u8);
    }

    pub fn debug_print_state(&self) {
        println!("        Enabled: {}", self.is_enabled);
        println!("        Length counter: {}", self.length_counter);
        println!("        Linear counter: {}", self.linear_counter);
        println!("        Length/linear counters halted: {}", self.halt_counters);
    }
}

pub struct NoiseChannel {
    is_enabled: bool,
    length_counter: u8,
    envelope: u8,
    constant_volume: bool,
    halt_length_counter: bool,
    noise_period: u8,
    loop_noise: bool,
}

impl NoiseChannel {
    pub fn new() -> Self {
        Self {
            is_enabled: false,
            length_counter: 0,
            envelope: 0,
            constant_volume: false,
            halt_length_counter: false,
            noise_period: 0,
            loop_noise: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.length_counter > 0
    }

    pub fn set_enabled(&mut self, enable: bool) {
        self.is_enabled = enable;
        if !enable {
            self.length_counter = 0;
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address & 0b11 {
            0b00 => {
                self.envelope = value & 0b1111;
                self.constant_volume = value & 0b10000 != 0;
                self.halt_length_counter = value & 0b100000 != 0;
            }
            0b01 => {
                // Unused
            }
            0b10 => {
                self.noise_period = value & 0b1111;
                self.loop_noise = value & 0b10000000 != 0;
            }
            0b11 => {
                if self.is_enabled {
                    self.length_counter = LENGTH_TABLE[(value >> 3) as usize];
                }
            }
            _ => unreachable!()
        }
    }

    pub fn clock_cpu_cycle(&mut self) {
        //
    }

    pub fn clock_quarter_frame(&mut self) {
        //
    }

    pub fn clock_half_frame(&mut self) {
        self.length_counter = self.length_counter.saturating_sub(!self.halt_length_counter as u8);
    }

    pub fn debug_print_state(&self) {
        println!("        Enabled: {}", self.is_enabled);
        println!("        Length counter: {}", self.length_counter);
        println!("        Length counter halted: {}", self.halt_length_counter);
    }
}

const DMC_START_ADDRESS_BASE: u16 = 0xC000;
const DMC_POINTER_BASE: u16 = 0x8000;
const DMC_PERIOD_TABLE: [u16; 16] = [
    0x1AC,
    0x17C,
    0x154,
    0x140,
    0x11E,
    0x0FE,
    0x0E2,
    0x0D6,
    0x0BE,
    0x0A0,
    0x08E,
    0x080,
    0x06A,
    0x054,
    0x048,
    0x036,
];

pub struct DeltaModulationChannel {
    is_enabled: bool,
    period: u16,
    loop_enabled: bool,
    irq_enabled: bool,
    irq_asserted: bool,
    dma_requested: bool,
    dma_is_reload: bool,
    sample_start_address: u16,
    sample_length: u16,
    sample_pointer: u16,
    sample_bytes_left: u16,
    sample_buffer: Option<u8>,
    sample_shifter: u8,
    sample_shifter_bits_left: u8,
    timer: u16,
    pcm_counter: u8,
}

impl DeltaModulationChannel {
    pub fn new() -> Self {
        Self {
            is_enabled: false,
            period: DMC_PERIOD_TABLE[0],
            loop_enabled: false,
            irq_enabled: false,
            irq_asserted: false,
            dma_requested: false,
            dma_is_reload: false,
            sample_start_address: DMC_START_ADDRESS_BASE,
            sample_length: 1,
            sample_pointer: DMC_POINTER_BASE,
            sample_bytes_left: 0,
            sample_buffer: None,
            sample_shifter: 0,
            sample_shifter_bits_left: 0,
            timer: 0,
            pcm_counter: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.sample_bytes_left > 0
    }

    pub fn set_enabled(&mut self, enable: bool) {
        self.is_enabled = enable;
        if !enable {
            self.sample_bytes_left = 0;
            self.sample_buffer = None;
        }
        else if self.sample_bytes_left == 0 {
            self.sample_pointer = self.sample_start_address;
            self.sample_bytes_left = self.sample_length;
            self.timer = self.period;
            self.dma_is_reload = false;
        }
    }

    pub fn irq_asserted(&self) -> bool {
        self.irq_asserted
    }

    pub fn clear_irq(&mut self) {
        self.irq_asserted = false;
    }

    pub fn dma_request(&self) -> Option<(u16, bool)> {
        self.dma_requested.then_some((self.sample_pointer, self.dma_is_reload))
    }
    
    pub fn load_sample_buffer(&mut self, dma_read: u8) {
        self.sample_buffer = Some(dma_read);
        self.sample_pointer = DMC_POINTER_BASE | self.sample_pointer.wrapping_add(1);
        self.sample_bytes_left = self.sample_bytes_left.saturating_sub(1);
        self.dma_requested = false;

        if self.sample_bytes_left == 0 {
            if self.loop_enabled {
                self.sample_pointer = self.sample_start_address;
                self.sample_bytes_left = self.sample_length;
            }
            else {
                self.irq_asserted |= self.irq_enabled;
            }
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address & 0b11 {
            0b00 => { // $4010
                self.period = DMC_PERIOD_TABLE[(value & 0b1111) as usize];
                self.loop_enabled = value & 0b1000000 != 0;
                self.irq_enabled = value & 0b10000000 != 0;
                self.irq_asserted &= self.irq_enabled;
            }
            0b01 => { // $4011
                self.pcm_counter = value & 0b1111111;
            }
            0b10 => { // $4012
                self.sample_start_address = DMC_START_ADDRESS_BASE | (value as u16) << 6;
            }
            0b11 => { // $4013
                self.sample_length = 1 | (value as u16) << 4;
            }
            _ => unreachable!()
        }
    }

    pub fn clock_cpu_cycle(&mut self) {
        if self.timer == 0 {
            self.clock_timer();
            // If the sample shifter is empty, try to grab a new sample byte from the sample buffer
            if self.sample_shifter_bits_left == 0 {
                if let Some(buffer) = self.sample_buffer.take() {
                    // Empty the sample buffer into the sample shifter
                    self.sample_shifter = buffer;
                    self.sample_shifter_bits_left = 8;
                    self.dma_is_reload = true;
                }
            }
            // Restart the timer
            self.timer = self.period;
        }
        self.timer -= 1;

        // If the sample buffer needs to be loaded, request DMA to fetch the next byte of the sample
        if self.sample_buffer.is_none() && self.sample_bytes_left > 0 {
            self.dma_requested = true;
        }
    }

    fn clock_timer(&mut self) {
        if self.sample_shifter_bits_left > 0 {
            // Process the next bit of the sample
            if self.sample_shifter & 1 == 0 {
                // Decrement the 7-bit PCM counter by 2 unless underflow would occur
                if self.pcm_counter >= 2 {
                    self.pcm_counter -= 2;
                }
            }
            else {
                // Increment the 7-bit PCM counter by 2 unless overflow would occur
                if self.pcm_counter <= 125 {
                    self.pcm_counter += 2;
                }
            }
            // Consume the bit
            self.sample_shifter >>= 1;
            self.sample_shifter_bits_left -= 1;
        }
    }

    pub fn debug_print_state(&self) {
        println!("        Enabled: {}", self.is_enabled);
        println!("        Loop: {}", self.loop_enabled);
        println!("        IRQ enabled: {}", self.irq_enabled);
        println!("        IRQ asserted: {}", self.irq_asserted);
        println!("        Sample address: ${:04X}", self.sample_start_address);
        println!("        Sample length: ${:03X}", self.sample_length);
        println!("        Pointer: ${:04X}", self.sample_pointer);
        println!("        Bytes remaining: {}", self.sample_bytes_left);
        if let Some(buffer) = self.sample_buffer {
            println!("        Sample buffer: ${:02X}", buffer);
        }
        else {
            println!("        Sample buffer: (empty)");
        }
        println!("        DMA requested: {}", self.dma_requested);
        println!("        Shifter bits remaining: {}", self.sample_shifter_bits_left);
        println!("        Period: ${:03X}", self.period);
        println!("        Timer: ${:03X}", self.timer);
    }
}
