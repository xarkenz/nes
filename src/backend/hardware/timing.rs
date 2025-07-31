use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct DelayedFlag<const N: u16> {
    shifter: u16,
    current_flag: bool,
}

impl<const N: u16> DelayedFlag<N> {
    pub fn new(initial_flag: bool) -> Self {
        Self {
            shifter: ((0b10 << N) - 1) * initial_flag as u16,
            current_flag: initial_flag,
        }
    }

    pub fn reset(&mut self, flag: bool) {
        self.shifter = ((0b10 << N) - 1) * flag as u16;
        self.current_flag = flag;
    }

    pub fn get_current(&self) -> bool {
        self.current_flag
    }

    pub fn set_current(&mut self, flag: bool) {
        self.current_flag = flag;
        self.shifter |= (flag as u16) << N;
    }
    
    pub fn pulse(&mut self, flag: bool) {
        self.shifter |= (flag as u16) << N;
    }

    pub fn get_delayed(&self) -> bool {
        self.shifter & 1 != 0
    }

    pub fn tick(&mut self) {
        self.shifter >>= 1;
        self.shifter |= (self.current_flag as u16) << N;
    }
}