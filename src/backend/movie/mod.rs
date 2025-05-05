use std::io::{BufRead, BufReader, Read, Seek};
use zip::ZipArchive;
use crate::hardware::joypad::*;
use crate::hardware::Machine;

#[derive(Copy, Clone, Debug)]
pub struct MovieFrame {
    pub power_pressed: bool,
    pub reset_pressed: bool,
    pub player_1_inputs: u8,
    pub player_2_inputs: u8,
}

impl MovieFrame {
    pub fn apply_inputs(&self, machine: &mut Machine) {
        for button in 0 .. BUTTON_COUNT {
            machine.joypads.player_1[button] = self.player_1_inputs >> button & 1 != 0;
            machine.joypads.player_2[button] = self.player_2_inputs >> button & 1 != 0;
        }
        if self.power_pressed || self.reset_pressed {
            machine.reset();
        }
    }
}

pub struct Movie {
    frames: Vec<MovieFrame>,
    pub frame_offset: i64,
}

impl Movie {
    pub fn parse_bk2(reader: impl Read + Seek) -> Result<Self, String> {
        let mut archive = ZipArchive::new(reader).map_err(|error| error.to_string())?;

        let Ok(mut input_log) = archive.by_name("Input Log.txt") else {
            return Err("Failed to find the input log for the given BK2 archive.".to_string());
        };
        let frames = Self::parse_bk2_input_log(&mut input_log)?;

        Ok(Self {
            frames,
            frame_offset: 0,
        })
    }

    pub fn parse_bk2_input_log(reader: impl Read) -> Result<Vec<MovieFrame>, String> {
        const BUTTON_MAP: [usize; BUTTON_COUNT] = [
            BUTTON_UP,
            BUTTON_DOWN,
            BUTTON_LEFT,
            BUTTON_RIGHT,
            BUTTON_START,
            BUTTON_SELECT,
            BUTTON_B,
            BUTTON_A,
        ];

        fn pack_player_inputs(inputs: Option<&[bool]>) -> u8 {
            if let Some(inputs) = inputs {
                inputs
                    .iter()
                    .take(BUTTON_COUNT)
                    .copied()
                    .zip(BUTTON_MAP)
                    .fold(0, |inputs_byte: u8, (is_pressed, button)| {
                        inputs_byte | (is_pressed as u8) << button
                    })
            }
            else {
                0
            }
        }

        let mut frames = Vec::new();
        let mut in_input_block = false;

        for line in BufReader::new(reader).lines() {
            let line = line.map_err(|error| error.to_string())?;
            let line = line.trim();

            if !in_input_block {
                if line.eq_ignore_ascii_case("[Input]") {
                    in_input_block = true;
                }
            }
            else if line.eq_ignore_ascii_case("[/Input]") {
                in_input_block = false;
            }
            else if line.starts_with('|') {
                let line = line.split_once(',').map_or(line, |split| split.1);
                let inputs: Vec<bool> = line
                    .chars()
                    .filter_map(|ch| (ch != '|' && !ch.is_whitespace()).then_some(ch != '.'))
                    .collect();

                frames.push(MovieFrame {
                    power_pressed: inputs[0],
                    reset_pressed: inputs[1],
                    player_1_inputs: pack_player_inputs(inputs.get(2 ..)),
                    player_2_inputs: pack_player_inputs(inputs.get(2 + BUTTON_COUNT ..)),
                });
            }
        }

        Ok(frames)
    }

    pub fn next_frame(&mut self) -> Option<MovieFrame> {
        let frame = usize::try_from(self.frame_offset).ok()
            .and_then(|index| self.frames.get(index).copied());
        self.frame_offset = self.frame_offset.saturating_add(1);
        frame
    }
}
