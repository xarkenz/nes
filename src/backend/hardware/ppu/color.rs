use std::io::Read;
use crate::hardware::ppu::PPU_COLOR_COUNT;

pub struct ColorConverter {
    table: Box<[u32; PPU_COLOR_COUNT * 8]>,
}

impl ColorConverter {
    pub fn new() -> Self {
        Self {
            table: Box::new([0; PPU_COLOR_COUNT * 8]),
        }
    }

    pub fn parse_pal(&mut self, reader: &mut impl Read) -> std::io::Result<()> {
        for color in self.table.as_mut_slice() {
            let mut rgb = [0_u8; 3];
            if let Err(error) = reader.read_exact(&mut rgb) {
                if let std::io::ErrorKind::UnexpectedEof = error.kind() {
                    // Assume all color emphases use the same palette
                    for section in 1 .. 8 {
                        self.table.copy_within(0 .. PPU_COLOR_COUNT, section * PPU_COLOR_COUNT);
                    }
                    break;
                }
                return Err(error);
            }

            *color = (rgb[0] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[2] as u32;
        }

        Ok(())
    }

    pub fn get_rgb(&self, index: u16) -> u32 {
        // 3 bits for emphasis, 6 bits for palette color
        self.table[(index & 0b111_111111) as usize]
    }
}
