use std::io::Read;
use crate::hardware::ppu::PPU_COLOR_COUNT;

#[derive(Clone, Debug)]
pub struct ColorOptions {
    pub saturation: f64,
    pub contrast: f64,
    pub brightness: f64,
    pub hue_tweak: f64,
}

impl ColorOptions {
    pub fn new() -> Self {
        Self {
            saturation: 1.0,
            contrast: 1.0,
            brightness: 1.0,
            hue_tweak: 0.0,
        }
    }
}

impl Default for ColorOptions {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ColorConverter {
    table: Box<[u32; PPU_COLOR_COUNT * 8]>,
}

impl ColorConverter {
    pub fn new() -> Self {
        Self {
            table: Box::new([0; PPU_COLOR_COUNT * 8]),
        }
    }

    pub fn get_rgb(&self, index: u16) -> u32 {
        // 3 bits for emphasis, 6 bits for palette color
        self.table[(index & 0b111_111111) as usize]
    }

    pub fn parse_pal(&mut self, reader: &mut impl Read) -> std::io::Result<()> {
        for entry in self.table.iter_mut() {
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

            *entry = 0xFF000000 | (rgb[0] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[2] as u32;
        }

        Ok(())
    }

    // Based on https://github.com/jslepicka/nemulator/blob/d1e87d556a2592a7b7dcf5a316106944ee33b2d8/nes/ppu.cpp#L42
    pub fn generate_palette(&mut self, color_options: ColorOptions) {
        const PHASE_COUNT: usize = 12;
        // LOW_VOLTAGES[0] is color $0D, the "blacker than black" signal that causes issues on some
        // real TVs which interpret it as a blanking signal, hence why BLACK_VOLTAGE is index 1
        const LOW_VOLTAGES: [f64; 4] = [0.350, 0.518, 0.962, 1.550];
        const HIGH_VOLTAGES: [f64; 4] = [1.094, 1.506, 1.962, 1.962];
        const BLACK_VOLTAGE: f64 = LOW_VOLTAGES[1];
        const WHITE_VOLTAGE: f64 = HIGH_VOLTAGES[2];
        const ATTENUATION: f64 = 0.746;

        for (index, entry) in self.table.iter_mut().enumerate() {
            let hue = index & 0b1111;
            let value = match hue {
                0x0 ..= 0xD => index >> 4 & 0b11,
                _ => 1
            };
            let emphasis_r = index & 0b001_000000 != 0;
            let emphasis_g = index & 0b010_000000 != 0;
            let emphasis_b = index & 0b100_000000 != 0;

            let [low_voltage, high_voltage] = match hue {
                0x0 => {
                    // Light gray: constant high voltage
                    [HIGH_VOLTAGES[value]; 2]
                }
                0xD ..= 0xF => {
                    // Dark gray / black: constant low voltage
                    [LOW_VOLTAGES[value]; 2]
                }
                _ => {
                    // Chromatic hues: phased low/high voltage wave
                    [LOW_VOLTAGES[value], HIGH_VOLTAGES[value]]
                }
            };

            // Start computation in the YIQ color space
            let mut y = 0.0; // Luma
            let mut i = 0.0; // Orange-blue contrast
            let mut q = 0.0; // Purple-green contrast

            // Sample all 12 phases and accumulate the average YIQ components
            for phase in 0 .. PHASE_COUNT {
                let is_in_phase = move |hue| (phase + hue + 8) % PHASE_COUNT < 6;

                let mut voltage = if is_in_phase(hue) { high_voltage } else { low_voltage };
                // The PPU_MASK emphasis bits cause an attenuator to be active in the phase which
                // is complementary to the emphasis color:
                // - Red emphasis enables attenuation during the phase for hue C (cyan)
                // - Green emphasis enables attenuation during the phase for hue 4 (magenta)
                // - Blue emphasis enables attenuation during the phase for hue 8 (yellow)
                // So, "emphasizing" a color is really just "de-emphasizing" its complement.
                let attenuate = (emphasis_r && is_in_phase(0xC))
                    || (emphasis_g && is_in_phase(0x4))
                    || (emphasis_b && is_in_phase(0x8));
                if attenuate {
                    voltage *= ATTENUATION;
                }
                // Convert voltage to value, where 0 is black and 1 is white
                let mut value = (voltage - BLACK_VOLTAGE) / (WHITE_VOLTAGE - BLACK_VOLTAGE);
                // Apply contrast and brightness options
                value = color_options.contrast * (value - 0.5) + 0.5;
                value *= color_options.brightness;
                // Divide by PHASE_COUNT for the purpose of averaging the results
                value /= PHASE_COUNT as f64;

                use std::f64::consts::FRAC_PI_6;
                let tweaked_phase = phase as f64 + color_options.hue_tweak;
                let (phase_sin, phase_cos) = (tweaked_phase * FRAC_PI_6).sin_cos();
                y += value;
                i += value * phase_cos;
                q += value * phase_sin;
            }

            // Apply saturation option
            i *= color_options.saturation;
            q *= color_options.saturation;

            // Convert YIQ to NTSC RGB
            let ntsc_r = y + 0.956 * i + 0.620 * q;
            let ntsc_g = y - 0.272 * i - 0.647 * q;
            let ntsc_b = y - 1.108 * i + 1.705 * q;

            // Convert NTSC RGB to linear NTSC RGB
            let ntsc_r = ntsc_r.clamp(0.0, 1.0).powf(2.2);
            let ntsc_g = ntsc_g.clamp(0.0, 1.0).powf(2.2);
            let ntsc_b = ntsc_b.clamp(0.0, 1.0).powf(2.2);

            // Convert linear NTSC RGB to linear sRGB
            let srgb_r = 0.9320460 * ntsc_r + 0.0411137 * ntsc_g + 0.0215592 * ntsc_b;
            let srgb_g = 0.0136330 * ntsc_r + 0.9710000 * ntsc_g + 0.0147336 * ntsc_b;
            let srgb_b = 0.0055574 * ntsc_r - 0.0143139 * ntsc_g + 1.0082900 * ntsc_b;

            // Convert linear sRGB to sRGB
            let srgb_r = srgb_r.powf(1.0 / 2.2).clamp(0.0, 1.0);
            let srgb_g = srgb_g.powf(1.0 / 2.2).clamp(0.0, 1.0);
            let srgb_b = srgb_b.powf(1.0 / 2.2).clamp(0.0, 1.0);

            // Convert sRGB to 24-bit RGB color
            let r = (255.0 * srgb_r).round() as u32;
            let g = (255.0 * srgb_g).round() as u32;
            let b = (255.0 * srgb_b).round() as u32;

            // At long last, we have our actual palette entry
            *entry = 0xFF000000 | r << 16 | g << 8 | b;
        }
    }
}
