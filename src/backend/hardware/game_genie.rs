// Logic based on https://tuxnes.sourceforge.net/gamegenie.html

use serde::{Deserialize, Serialize};

pub fn parse_char(ch: char) -> Option<u16> {
    match ch {
        'A' | 'a' => Some(0x0),
        'P' | 'p' => Some(0x1),
        'Z' | 'z' => Some(0x2),
        'L' | 'l' => Some(0x3),
        'G' | 'g' => Some(0x4),
        'I' | 'i' => Some(0x5),
        'T' | 't' => Some(0x6),
        'Y' | 'y' => Some(0x7),
        'E' | 'e' => Some(0x8),
        'O' | 'o' => Some(0x9),
        'X' | 'x' => Some(0xA),
        'U' | 'u' => Some(0xB),
        'K' | 'k' => Some(0xC),
        'S' | 's' => Some(0xD),
        'V' | 'v' => Some(0xE),
        'N' | 'n' => Some(0xF),
        _ => None
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameGenie {
    pub address: u16,
    pub value: u8,
    pub compare: Option<u8>,
}

impl GameGenie {
    pub fn parse_code(code: &str) -> Option<Self> {
        let mut nybbles = code.chars().filter_map(parse_char);
        let nybble_0 = nybbles.next()?;
        let nybble_1 = nybbles.next()?;
        let nybble_2 = nybbles.next()?;
        let nybble_3 = nybbles.next()?;
        let nybble_4 = nybbles.next()?;
        let nybble_5 = nybbles.next()?;

        let address = 0x8000
            | ((nybble_3 & 0b0111) << 12)
            | ((nybble_5 & 0b0111) << 8) | ((nybble_4 & 0b1000) << 8)
            | ((nybble_2 & 0b0111) << 4) | ((nybble_1 & 0b1000) << 4)
            | ((nybble_4 & 0b0111) << 0) | ((nybble_3 & 0b1000) << 0);

        let value;
        let compare;
        if let Some(nybble_6) = nybbles.next() {
            let nybble_7 = nybbles.next()?;
            if nybbles.next().is_some() {
                return None;
            }

            value =
                  ((nybble_1 & 0b0111) << 4) | ((nybble_0 & 0b1000) << 4)
                | ((nybble_0 & 0b0111) << 0) | ((nybble_7 & 0b1000) << 0);
            compare = Some(
                  ((nybble_7 & 0b0111) << 4) | ((nybble_6 & 0b1000) << 4)
                | ((nybble_6 & 0b0111) << 0) | ((nybble_5 & 0b1000) << 0));
        }
        else {
            value =
                  ((nybble_1 & 0b0111) << 4) | ((nybble_0 & 0b1000) << 4)
                | ((nybble_0 & 0b0111) << 0) | ((nybble_5 & 0b1000) << 0);
            compare = None;
        }

        Some(Self {
            address,
            value: value as u8,
            compare: compare.map(|x| x as u8),
        })
    }

    pub fn read_byte(&self, address: u16, read_default_byte: impl FnOnce() -> u8) -> u8 {
        if address != self.address {
            read_default_byte()
        }
        else if let Some(compare) = self.compare {
            let default_byte = read_default_byte();
            if default_byte == compare {
                self.value
            }
            else {
                default_byte
            }
        }
        else {
            self.value
        }
    }
}
