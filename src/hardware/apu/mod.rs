use crate::hardware::DelayedFlag;
use channel::*;

pub mod channel;

pub const APU_CHANNEL_START: u16 = 0x4000;
pub const APU_CHANNEL_END: u16 = 0x4013;
pub const APU_STATUS: u16 = 0x4015;
pub const APU_FRAME_COUNTER: u16 = 0x4017;
const FOUR_STEP_COUNTER_LIMIT: u16 = 29830;
const FIVE_STEP_COUNTER_LIMIT: u16 = 37282;

#[derive(Copy, Clone, Debug)]
pub enum SequencerMode {
    FourStep,
    FiveStep,
}

impl std::fmt::Display for SequencerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SequencerMode::FourStep => write!(f, "four-step"),
            SequencerMode::FiveStep => write!(f, "five-step"),
        }
    }
}

pub struct AudioProcessingUnit {
    pulse_channel_1: PulseChannel,
    pulse_channel_2: PulseChannel,
    triangle_channel: TriangleChannel,
    noise_channel: NoiseChannel,
    delta_modulation_channel: DeltaModulationChannel,
    sequencer_mode: SequencerMode,
    second_half_cycle: bool,
    frame_irq_inhibited: bool,
    frame_irq_asserted: bool,
    frame_counter: u16,
    reset_frame_counter: DelayedFlag<4>,
}

impl AudioProcessingUnit {
    pub fn new() -> Self {
        Self {
            pulse_channel_1: PulseChannel::new(),
            pulse_channel_2: PulseChannel::new(),
            triangle_channel: TriangleChannel::new(),
            noise_channel: NoiseChannel::new(),
            delta_modulation_channel: DeltaModulationChannel::new(),
            sequencer_mode: SequencerMode::FourStep,
            frame_irq_inhibited: false,
            second_half_cycle: false,
            frame_irq_asserted: false,
            frame_counter: 0,
            reset_frame_counter: DelayedFlag::new(false),
        }
    }
    
    pub fn irq_asserted(&self) -> bool {
        self.frame_irq_asserted || self.delta_modulation_channel.irq_asserted()
    }

    pub fn reset(&mut self) {
        self.pulse_channel_1.set_enabled(false);
        self.pulse_channel_2.set_enabled(false);
        self.triangle_channel.set_enabled(false);
        self.noise_channel.set_enabled(false);
        self.delta_modulation_channel.set_enabled(false);
        self.delta_modulation_channel.clear_irq();
    }

    pub fn write_channel_register(&mut self, address: u16, value: u8) {
        match address & 0b11100 {
            0b00000 => {
                self.pulse_channel_1.write_register(address, value);
            }
            0b00100 => {
                self.pulse_channel_2.write_register(address, value);
            }
            0b01000 => {
                self.triangle_channel.write_register(address, value);
            }
            0b01100 => {
                self.noise_channel.write_register(address, value);
            }
            0b10000 => {
                self.delta_modulation_channel.write_register(address, value);
            }
            _ => {}
        }
    }
    
    pub fn read_status(&mut self) -> u8 {
        // TODO: same time flag behavior?
        let status = (self.pulse_channel_1.is_active() as u8)
            | (self.pulse_channel_2.is_active() as u8) << 1
            | (self.triangle_channel.is_active() as u8) << 2
            | (self.noise_channel.is_active() as u8) << 3
            | (self.delta_modulation_channel.is_active() as u8) << 4
            | (self.frame_irq_asserted as u8) << 6
            | (self.delta_modulation_channel.irq_asserted() as u8) << 7;
        self.frame_irq_asserted = false;
        status
    }
    
    pub fn write_status(&mut self, value: u8) {
        self.pulse_channel_1.set_enabled(value & 0b1 != 0);
        self.pulse_channel_2.set_enabled(value & 0b10 != 0);
        self.triangle_channel.set_enabled(value & 0b100 != 0);
        self.noise_channel.set_enabled(value & 0b1000 != 0);
        self.delta_modulation_channel.set_enabled(value & 0b10000 != 0);
        self.delta_modulation_channel.clear_irq();
    }

    pub fn write_frame_counter(&mut self, value: u8) {
        self.frame_irq_inhibited = value & 0b1000000 != 0;
        self.sequencer_mode = if value & 0b10000000 == 0 {
            SequencerMode::FourStep
        } else {
            SequencerMode::FiveStep
        };
        self.frame_irq_asserted &= !self.frame_irq_inhibited;

        // Reset the frame counter in 3 CPU cycles if in APU cycle, or 4 CPU cycles otherwise
        // TODO: does "in" APU cycle mean first or second half?
        self.reset_frame_counter.pulse(true);
        if self.second_half_cycle {
            self.reset_frame_counter.tick();
        }
    }
    
    pub fn dmc_dma_request(&self) -> Option<(u16, bool)> {
        self.delta_modulation_channel.dma_request()
    }

    pub fn load_dmc_sample_buffer(&mut self, dma_read: u8) {
        self.delta_modulation_channel.load_sample_buffer(dma_read);
    }
    
    pub fn cpu_cycle_tick(&mut self, second_half_cycle: bool) {
        self.second_half_cycle = second_half_cycle;
        self.reset_frame_counter.tick();

        self.pulse_channel_1.clock_cpu_cycle();
        self.pulse_channel_2.clock_cpu_cycle();
        self.triangle_channel.clock_cpu_cycle();
        self.noise_channel.clock_cpu_cycle();
        self.delta_modulation_channel.clock_cpu_cycle();

        if self.reset_frame_counter.get_delayed() {
            self.frame_counter = 0;
            if let SequencerMode::FiveStep = self.sequencer_mode {
                self.clock_half_frame();
            }
        }
        else {
            self.frame_counter = self.frame_counter.wrapping_add(1);
            match self.sequencer_mode {
                SequencerMode::FourStep => {
                    if self.frame_counter >= FOUR_STEP_COUNTER_LIMIT - 2 {
                        self.frame_irq_asserted |= !self.frame_irq_inhibited;
                        if self.frame_counter >= FOUR_STEP_COUNTER_LIMIT {
                            self.frame_counter = 0;
                        }
                    }
                    match self.frame_counter {
                        7457 | 22371 => self.clock_quarter_frame(),
                        14913 | 29829 => self.clock_half_frame(),
                        _ => {}
                    }
                }
                SequencerMode::FiveStep => {
                    if self.frame_counter >= FIVE_STEP_COUNTER_LIMIT {
                        self.frame_counter = 0;
                    }
                    match self.frame_counter {
                        7457 | 22371 => self.clock_quarter_frame(),
                        14913 | 37281 => self.clock_half_frame(),
                        _ => {}
                    }
                }
            }
        }
    }
    
    fn clock_quarter_frame(&mut self) {
        self.pulse_channel_1.clock_quarter_frame();
        self.pulse_channel_2.clock_quarter_frame();
        self.triangle_channel.clock_quarter_frame();
        self.noise_channel.clock_quarter_frame();
    }
    
    fn clock_half_frame(&mut self) {
        self.clock_quarter_frame();
        self.pulse_channel_1.clock_half_frame();
        self.pulse_channel_2.clock_half_frame();
        self.triangle_channel.clock_half_frame();
        self.noise_channel.clock_half_frame();
    }
    
    pub fn debug_print_state(&self) {
        println!("APU state:");
        println!("    Sequencer mode: {}", self.sequencer_mode);
        println!("    Frame counter: {}", self.frame_counter);
        println!("    Frame IRQ inhibited: {}", self.frame_irq_inhibited);
        println!("    Frame IRQ asserted: {}", self.frame_irq_asserted);
        println!("    Cycle half: {}", if self.second_half_cycle { "second" } else { "first" });
        println!("    Pulse 1:");
        self.pulse_channel_1.debug_print_state();
        println!("    Pulse 2:");
        self.pulse_channel_2.debug_print_state();
        println!("    Triangle:");
        self.triangle_channel.debug_print_state();
        println!("    Noise:");
        self.noise_channel.debug_print_state();
        println!("    DMC:");
        self.delta_modulation_channel.debug_print_state();
    }
}