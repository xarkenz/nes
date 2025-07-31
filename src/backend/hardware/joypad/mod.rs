use serde::{Deserialize, Serialize};

pub const JOYPAD_1_REGISTER: u16 = 0x4016;
pub const JOYPAD_2_REGISTER: u16 = 0x4017;
pub const BUTTON_A: usize = 0;
pub const BUTTON_B: usize = 1;
pub const BUTTON_SELECT: usize = 2;
pub const BUTTON_START: usize = 3;
pub const BUTTON_UP: usize = 4;
pub const BUTTON_DOWN: usize = 5;
pub const BUTTON_LEFT: usize = 6;
pub const BUTTON_RIGHT: usize = 7;
pub const BUTTON_COUNT: usize = 8;
const CONTROLLER_EXCESS_READ: u8 = 0x01;

#[derive(Clone, Serialize, Deserialize)]
pub struct Joypads {
    pub player_1: [bool; BUTTON_COUNT],
    pub player_2: [bool; BUTTON_COUNT],
    strobe: bool,
    read_index_1: usize,
    read_index_2: usize,
}

impl Joypads {
    pub fn new() -> Self {
        Self {
            player_1: [false; BUTTON_COUNT],
            player_2: [false; BUTTON_COUNT],
            strobe: false,
            read_index_1: 0,
            read_index_2: 0,
        }
    }

    pub fn read_player_1(&mut self) -> u8 {
        if self.read_index_1 >= BUTTON_COUNT {
            CONTROLLER_EXCESS_READ
        }
        else {
            let joypad_read = self.player_1[self.read_index_1] as u8;
            if !self.strobe {
                self.read_index_1 += 1;
            }
            joypad_read
        }
    }

    pub fn read_player_2(&mut self) -> u8 {
        if self.read_index_2 >= BUTTON_COUNT {
            CONTROLLER_EXCESS_READ
        }
        else {
            let joypad_read = self.player_2[self.read_index_2] as u8;
            if !self.strobe {
                self.read_index_2 += 1;
            }
            joypad_read
        }
    }

    pub fn write_strobe(&mut self, value: u8) {
        self.strobe = value & 1 != 0;
        self.read_index_1 = 0;
        self.read_index_2 = 0;
    }
}

impl Default for Joypads {
    fn default() -> Self {
        Self::new()
    }
}
