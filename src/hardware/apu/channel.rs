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

const PULSE_DUTY_LEVELS: [[u8; 8]; 4] = [
    [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xF],
    [0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xF, 0xF],
    [0x0, 0x0, 0x0, 0x0, 0xF, 0xF, 0xF, 0xF],
    [0xF, 0xF, 0xF, 0xF, 0xF, 0xF, 0x0, 0x0],
];

pub struct PulseChannel {
    is_enabled: bool,
    length_counter: u8,
    envelope_parameter: u8,
    constant_volume: bool,
    halt_length_counter: bool,
    restart_envelope: bool,
    duty: u8,
    init_sequence_period: u16,
    target_sequence_period: u16,
    sequence_period: u16,
    sequence_timer: u16,
    sequence_index: u8,
    decay_level: u8,
    decay_timer: u8,
    sweep_enabled: bool,
    sweep_carry_in: bool,
    sweep_shift_amount: u8,
    sweep_negate: bool,
    sweep_period: u8,
    sweep_timer: u8,
    sweep_reload: bool,
    sweep_is_muting: bool,
    output_level: u8,
}

impl PulseChannel {
    pub fn new(is_pulse_2: bool) -> Self {
        Self {
            is_enabled: false,
            length_counter: 0,
            envelope_parameter: 0,
            constant_volume: false,
            halt_length_counter: false,
            restart_envelope: false,
            duty: 0,
            init_sequence_period: 0,
            target_sequence_period: 0,
            sequence_period: 0,
            sequence_timer: 0,
            sequence_index: 0,
            decay_level: 0,
            decay_timer: 0,
            sweep_enabled: false,
            sweep_carry_in: is_pulse_2,
            sweep_shift_amount: 0,
            sweep_negate: false,
            sweep_period: 0,
            sweep_timer: 0,
            sweep_reload: false,
            sweep_is_muting: false,
            output_level: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_enabled && self.length_counter > 0
    }

    pub fn set_enabled(&mut self, enable: bool) {
        self.is_enabled = enable;
        if !enable {
            self.length_counter = 0;
        }
    }

    pub fn output_level(&self) -> u8 {
        if self.is_active() && !self.sweep_is_muting {
            self.output_level
        }
        else {
            0
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address & 0b11 {
            0b00 => { // $4000, $4004
                self.envelope_parameter = value & 0b1111;
                self.constant_volume = value & 0b10000 != 0;
                self.halt_length_counter = value & 0b100000 != 0;
                self.duty = value >> 6;
            }
            0b01 => { // $4001, $4005
                self.sweep_shift_amount = value & 0b111;
                self.sweep_negate = value & 0b1000 != 0;
                self.sweep_period = (value >> 4) & 0b111;
                self.sweep_enabled = self.sweep_shift_amount != 0 && value & 0b10000000 != 0;
                self.sweep_reload = true;
                self.update_sweep();
            }
            0b10 => { // $4002, $4006
                let period_low = value as u16;
                self.init_sequence_period &= 0b111_00000000;
                self.init_sequence_period |= period_low;
                self.sequence_period &= 0b111_00000000;
                self.sequence_period |= period_low;
                self.update_sweep();
            }
            0b11 => { // $4003, $4007
                let period_high = (value as u16 & 0b111) << 8;
                self.init_sequence_period &= 0b000_11111111;
                self.init_sequence_period |= period_high;
                self.sequence_period &= 0b000_11111111;
                self.sequence_period |= period_high;
                if self.is_enabled {
                    self.length_counter = LENGTH_TABLE[(value >> 3) as usize];
                }
                self.sequence_index = 0;
                self.restart_envelope = true;
                self.update_sweep();
            }
            _ => unreachable!()
        }
    }

    pub fn clock_apu_cycle(&mut self) {
        if self.sequence_timer == 0 {
            self.sequence_timer = self.sequence_period;
            self.sequence_index = self.sequence_index.wrapping_add(1) & 0b111;
            self.update_output_level();
        }
        else {
            self.sequence_timer -= 1;
        }
    }

    pub fn clock_quarter_frame(&mut self) {
        if self.restart_envelope {
            self.restart_envelope = false;
            self.decay_timer = self.envelope_parameter;
            self.decay_level = 15;
        }
        else if self.decay_timer == 0 {
            self.decay_timer = self.envelope_parameter;
            if self.decay_level > 0 {
                self.decay_level -= 1;
            }
            else if self.halt_length_counter {
                self.decay_level = 15;
            }
            self.update_output_level();
        }
        else {
            self.decay_timer -= 1;
        }
    }

    pub fn clock_half_frame(&mut self) {
        self.length_counter = self.length_counter.saturating_sub(!self.halt_length_counter as u8);

        if self.sweep_reload {
            self.sweep_reload = false;
            self.sweep_timer = self.sweep_period;
        }
        else if self.sweep_timer == 0 {
            self.sweep_timer = self.sweep_period;
            if self.sweep_enabled && !self.sweep_is_muting {
                self.sequence_period = self.target_sequence_period;
                self.update_sweep();
            }
        }
        else {
            self.sweep_timer -= 1;
        }
    }

    fn update_sweep(&mut self) {
        let mut change_amount = (self.init_sequence_period >> self.sweep_shift_amount) as i16;
        if self.sweep_negate {
            change_amount = (!change_amount).wrapping_add(self.sweep_carry_in as i16);
        }
        self.target_sequence_period = self.sequence_period.saturating_add_signed(change_amount);
        self.sweep_is_muting = self.sequence_period < 0x008 || self.target_sequence_period > 0x7FF;
    }

    fn update_output_level(&mut self) {
        self.output_level = PULSE_DUTY_LEVELS[self.duty as usize][self.sequence_index as usize];
        if self.constant_volume {
            self.output_level &= self.envelope_parameter;
        }
        else {
            self.output_level &= self.decay_level;
        }
    }

    pub fn debug_print_state(&self) {
        println!("        Enabled: {}", self.is_enabled);
        println!("        Length counter: {}", self.length_counter);
        println!("        Length counter halted: {}", self.halt_length_counter);
    }
}

const TRIANGLE_LEVELS: [u8; 32] = [
    0xF, 0xE, 0xD, 0xC, 0xB, 0xA, 0x9, 0x8, 0x7, 0x6, 0x5, 0x4, 0x3, 0x2, 0x1, 0x0,
    0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xA, 0xB, 0xC, 0xD, 0xE, 0xF,
];

pub struct TriangleChannel {
    is_enabled: bool,
    length_counter: u8,
    linear_counter: u8,
    halt_counters: bool,
    linear_counter_reload_value: u8,
    reload_linear_counter: bool,
    sequence_period: u16,
    sequence_timer: u16,
    sequence_index: u8,
    output_level: u8,
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
            sequence_period: 0,
            sequence_timer: 0,
            sequence_index: 0,
            output_level: TRIANGLE_LEVELS[0],
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

    pub fn output_level(&self) -> u8 {
        if self.is_enabled {
            self.output_level
        }
        else {
            0
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address & 0b11 {
            0b00 => { // $4008
                self.linear_counter_reload_value = value & 0b1111111;
                self.halt_counters = value & 0b10000000 != 0;
            }
            0b01 => { // $4009
                // Unused
            }
            0b10 => { // $400A
                self.sequence_period &= 0b111_00000000;
                self.sequence_period |= value as u16;
            }
            0b11 => { // $400B
                self.sequence_period &= 0b000_11111111;
                self.sequence_period |= (value as u16 & 0b111) << 8;
                if self.is_enabled {
                    self.length_counter = LENGTH_TABLE[(value >> 3) as usize];
                }
                self.reload_linear_counter = true;
            }
            _ => unreachable!()
        }
    }

    pub fn clock_cpu_cycle(&mut self) {
        if self.sequence_timer == 0 {
            self.sequence_timer = self.sequence_period;
            if self.length_counter > 0 && self.linear_counter > 0 {
                self.sequence_index = self.sequence_index.wrapping_add(1) & 0b11111;
                self.output_level = TRIANGLE_LEVELS[self.sequence_index as usize];
            }
        }
        else {
            self.sequence_timer -= 1;
        }
    }

    pub fn clock_quarter_frame(&mut self) {
        if self.reload_linear_counter {
            self.linear_counter = self.linear_counter_reload_value;
            self.reload_linear_counter = self.halt_counters;
        }
        else {
            self.linear_counter = self.linear_counter.saturating_sub(!self.halt_counters as u8);
        }
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

const NOISE_PERIODS: [u16; 16] = [
    0x002, 0x004, 0x008, 0x010, 0x020, 0x030, 0x040, 0x050,
    0x065, 0x07F, 0x0BE, 0x0FE, 0x17D, 0x1FC, 0x3F9, 0x7F2,
];

pub struct NoiseChannel {
    is_enabled: bool,
    length_counter: u8,
    envelope_parameter: u8,
    constant_volume: bool,
    halt_length_counter: bool,
    restart_envelope: bool,
    noise_loop_mode: bool,
    noise_period: u16,
    noise_timer: u16,
    decay_timer: u8,
    decay_level: u8,
    shift_register: u16,
    output_level: u8,
}

impl NoiseChannel {
    pub fn new() -> Self {
        Self {
            is_enabled: false,
            length_counter: 0,
            envelope_parameter: 0,
            constant_volume: false,
            halt_length_counter: false,
            restart_envelope: false,
            noise_loop_mode: false,
            noise_period: NOISE_PERIODS[0],
            noise_timer: 0,
            decay_timer: 0,
            decay_level: 0,
            shift_register: 1,
            output_level: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_enabled && self.length_counter > 0
    }

    pub fn set_enabled(&mut self, enable: bool) {
        self.is_enabled = enable;
        if !enable {
            self.length_counter = 0;
        }
    }

    pub fn output_level(&self) -> u8 {
        if self.is_active() {
            self.output_level
        }
        else {
            0
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        match address & 0b11 {
            0b00 => { // $400C
                self.envelope_parameter = value & 0b1111;
                self.constant_volume = value & 0b10000 != 0;
                self.halt_length_counter = value & 0b100000 != 0;
            }
            0b01 => { // $400D
                // Unused
            }
            0b10 => { // $400E
                self.noise_period = NOISE_PERIODS[(value & 0b1111) as usize];
                self.noise_loop_mode = value & 0b10000000 != 0;
            }
            0b11 => { // $400F
                if self.is_enabled {
                    self.length_counter = LENGTH_TABLE[(value >> 3) as usize];
                }
                self.restart_envelope = true;
            }
            _ => unreachable!()
        }
    }

    pub fn clock_apu_cycle(&mut self) {
        if self.noise_timer == 0 {
            self.noise_timer = self.noise_period;
            let feedback_bit = if self.noise_loop_mode { 6 } else { 1 };
            let feedback = (self.shift_register & 1) ^ (self.shift_register >> feedback_bit & 1);
            self.shift_register >>= 1;
            self.shift_register |= feedback << 14;
            self.update_output_level();
        }
        else {
            self.noise_timer -= 1;
        }
    }

    pub fn clock_quarter_frame(&mut self) {
        if self.restart_envelope {
            self.restart_envelope = false;
            self.decay_timer = self.envelope_parameter;
            self.decay_level = 15;
        }
        else if self.decay_timer == 0 {
            self.decay_timer = self.envelope_parameter;
            if self.decay_level > 0 {
                self.decay_level -= 1;
            }
            else if self.halt_length_counter {
                self.decay_level = 15;
            }
        }
        else {
            self.decay_timer -= 1;
        }
    }

    pub fn clock_half_frame(&mut self) {
        self.length_counter = self.length_counter.saturating_sub(!self.halt_length_counter as u8);
    }

    fn update_output_level(&mut self) {
        if self.shift_register & 1 == 0 {
            if self.constant_volume {
                self.output_level = self.envelope_parameter;
            }
            else {
                self.output_level = self.decay_level;
            }
        }
        else {
            self.output_level = 0;
        }
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
    sample_shifter: Option<u8>,
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
            sample_shifter: None,
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
            self.dma_is_reload = false;
        }
    }

    pub fn output_level(&self) -> u8 {
        if self.is_enabled {
            self.pcm_counter
        }
        else {
            0
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
        if let Some(shifter) = self.sample_shifter.take() {
            // Process the next bit of the sample
            if shifter & 1 == 0 {
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
            self.sample_shifter = Some(shifter >> 1);
        }

        self.sample_shifter_bits_left = self.sample_shifter_bits_left.saturating_sub(1);

        if self.sample_shifter_bits_left == 0 {
            // Start a new output cycle
            self.sample_shifter_bits_left = 8;
            // Try to grab a new sample byte from the sample buffer
            if let Some(buffer) = self.sample_buffer.take() {
                // Empty the sample buffer into the sample shifter
                self.sample_shifter = Some(buffer);
                self.dma_is_reload = true;
            }
            else {
                self.sample_shifter = None;
            }
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
