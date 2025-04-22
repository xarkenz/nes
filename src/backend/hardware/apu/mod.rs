use std::sync::mpsc::Sender;
use crate::hardware::timing::DelayedFlag;
use channel::*;

pub mod channel;

pub const APU_CHANNEL_START: u16 = 0x4000;
pub const APU_CHANNEL_END: u16 = 0x4013;
pub const APU_STATUS: u16 = 0x4015;
pub const APU_FRAME_COUNTER: u16 = 0x4017;
const FOUR_STEP_COUNTER_LIMIT: u16 = 29830;
const FIVE_STEP_COUNTER_LIMIT: u16 = 37282;

// Rust won't let me generate these tables as compile time constants. Literally 1984
fn mixer_pulse_table() -> Box<[f32; 31]> {
    let mut table = Box::new([0.0; 31]);
    for (index, entry) in table.iter_mut().enumerate() {
        *entry = (95.52 / (8128.0 / index as f64 + 100.0)) as f32;
    }
    table
}

fn mixer_tnd_table() -> Box<[f32; 203]> {
    let mut table = Box::new([0.0; 203]);
    for (index, entry) in table.iter_mut().enumerate() {
        *entry = (163.67 / (24329.0 / index as f64 + 100.0)) as f32;
    }
    table
}

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
    mixer_output: Option<Sender<f32>>,
    mixer_sample_interval: u16,
    mixer_sample_timer: u16,
    mixer_pulse_table: Box<[f32; 31]>,
    mixer_tnd_table: Box<[f32; 203]>,
}

impl AudioProcessingUnit {
    pub fn new() -> Self {
        Self {
            pulse_channel_1: PulseChannel::new(false),
            pulse_channel_2: PulseChannel::new(true),
            triangle_channel: TriangleChannel::new(),
            noise_channel: NoiseChannel::new(),
            delta_modulation_channel: DeltaModulationChannel::new(),
            sequencer_mode: SequencerMode::FourStep,
            frame_irq_inhibited: true,
            second_half_cycle: false,
            frame_irq_asserted: false,
            frame_counter: 0,
            reset_frame_counter: DelayedFlag::new(false),
            mixer_output: None,
            mixer_sample_interval: 1,
            mixer_sample_timer: 0,
            mixer_pulse_table: mixer_pulse_table(),
            mixer_tnd_table: mixer_tnd_table(),
        }
    }

    pub fn mixer_samples_per_frame(&self) -> f64 {
        use crate::hardware::ppu::{PRE_RENDER_SCANLINE, LAST_DOT};
        use crate::hardware::cpu::TICKS_PER_CPU_CYCLE;
        // Calculate CPU cycles per second
        let ppu_cycles_per_frame = (PRE_RENDER_SCANLINE + 1) as f64 * (LAST_DOT + 1) as f64;
        let cpu_cycles_per_frame = ppu_cycles_per_frame / TICKS_PER_CPU_CYCLE as f64;
        cpu_cycles_per_frame / self.mixer_sample_interval as f64
    }

    pub fn set_mixer_sample_interval(&mut self, cycles: u16) {
        self.mixer_sample_interval = cycles;
    }

    pub fn connect_mixer_output(&mut self, mixer_output: Sender<f32>) {
        self.send_mixer_output(&mixer_output);
        self.mixer_output = Some(mixer_output);
    }

    pub fn disconnect_mixer_output(&mut self) {
        self.mixer_output = None;
    }

    pub fn irq_asserted(&self) -> bool {
        self.frame_irq_asserted || self.delta_modulation_channel.irq_asserted()
    }

    pub fn reset(&mut self) {
        self.frame_irq_inhibited = true;
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

        // Reset the frame counter in 3 CPU cycles if 2nd half APU cycle, or 4 CPU cycles otherwise
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

        if self.second_half_cycle {
            self.pulse_channel_1.clock_apu_cycle();
            self.pulse_channel_2.clock_apu_cycle();
            self.noise_channel.clock_apu_cycle();
        }
        self.triangle_channel.clock_cpu_cycle();
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

        if let Some(mixer_output) = &self.mixer_output {
            self.mixer_sample_timer = self.mixer_sample_timer.saturating_sub(1);
            if self.mixer_sample_timer == 0 {
                self.mixer_sample_timer = self.mixer_sample_interval;
                self.send_mixer_output(mixer_output);
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

    fn send_mixer_output(&self, mixer_output: &Sender<f32>) {
        let pulse_1 = self.pulse_channel_1.output_level() as usize;
        let pulse_2 = self.pulse_channel_2.output_level() as usize;
        let triangle = self.triangle_channel.output_level() as usize;
        let noise = self.noise_channel.output_level() as usize;
        let dmc = self.delta_modulation_channel.output_level() as usize;

        // Do some mixing
        let pulse_output = self.mixer_pulse_table[pulse_1 + pulse_2];
        let tnd_output = self.mixer_tnd_table[3 * triangle + 2 * noise + dmc];

        // Send the mixer output; not really problematic if it fails, though
        mixer_output.send(pulse_output + tnd_output).ok();
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
