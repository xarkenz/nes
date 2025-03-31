use crate::loader::{Cartridge, ColorConverter};

pub const PPU_CONTROL: u16 = 0;
pub const PPU_MASK: u16 = 1;
pub const PPU_STATUS: u16 = 2;
pub const PPU_OAM_ADDRESS: u16 = 3;
pub const PPU_OAM_DATA: u16 = 4;
pub const PPU_SCROLL: u16 = 5;
pub const PPU_VRAM_ADDRESS: u16 = 6;
pub const PPU_VRAM_DATA: u16 = 7;
pub const TICKS_PER_PPU_CYCLE: u16 = 1;
const ATTRIBUTE_OFFSET: u16 = 0x03C0;
const OAM_SIZE: usize = 0x100;
// const PPU_COLORS: [u32; PPU_COLOR_COUNT] = [
//     0x916D00, 0x6D48DA, 0x009191, 0xDADA00, 0x000000, 0xFFB6B6, 0x002491, 0xDA6D00,
//     0xB6B6B6, 0x6D2400, 0x00FF00, 0x00006D, 0xFFDA91, 0xFFFF00, 0x009100, 0xB6FF48,
//     0xFF6DFF, 0x480000, 0x0048FF, 0xFF91FF, 0x000000, 0x484848, 0xB62400, 0xFF9100,
//     0xDAB66D, 0x00B66D, 0x9191FF, 0x249100, 0x91006D, 0x000000, 0x91FF6D, 0x6DB6FF,
//     0xB6006D, 0x006D24, 0x914800, 0x0000DA, 0x9100FF, 0xB600FF, 0x6D6D6D, 0xFF0091,
//     0x004848, 0xDADADA, 0x006DDA, 0x004800, 0x242424, 0xFFFF6D, 0x919191, 0xFF00FF,
//     0xFFB6FF, 0xFFFFFF, 0x6D4800, 0xFF0000, 0xFFDA00, 0x48FFDA, 0xFFFFFF, 0x91DAFF,
//     0x000000, 0xFFB600, 0xDA6DFF, 0xB6DAFF, 0x6DDA00, 0xDAB6FF, 0x00FFFF, 0x244800,
// ];
pub const PPU_COLOR_COUNT: usize = 64;
// const HORIZONTAL_BITS: u16 = 0b000_01_00000_11111;
// const VERTICAL_BITS: u16 = 0b111_10_11111_00000;
const VBLANK_START_SCANLINE: u16 = 241;
const MAX_SCANLINE: u16 = 261;
const MAX_DOT: u16 = 341;

pub struct PictureProcessingUnit {
    // PPU_CONTROL
    vram_address_increment: u16,
    sprite_ptable_address: u16,
    background_ptable_address: u16,
    tall_sprites: bool,
    vblank_nmi_enabled: bool,
    // PPU_MASK
    greyscale: bool,
    mask_background: bool,
    mask_sprites: bool,
    show_background: bool,
    show_sprites: bool,
    color_emphasis: u16,
    // PPU_STATUS
    vblank: bool,
    next_vblank: bool, // For the purposes of emulating hardware multiplexing
    sprite_0_hit: bool,
    sprite_overflow: bool,
    // PPU_OAM_ADDRESS
    oam_address: u8,
    // PPU_SCROLL
    fine_scroll_x: u8,
    origin_vram_address: u16,
    // PPU_VRAM_ADDRESS
    vram_address: u16,
    // Internal
    write_latch: bool,
    scanline: u16,
    dot: u16,
    vblank_nmi_triggered: bool, // true if pulling /NMI low
    pub palette_ram: [u8; 32],
    primary_oam: Box<[u8; OAM_SIZE]>,
    pub color_converter: ColorConverter,
}

impl PictureProcessingUnit {
    pub fn new() -> Self {
        Self {
            vram_address_increment: 1,
            sprite_ptable_address: 0,
            background_ptable_address: 0,
            tall_sprites: false,
            vblank_nmi_enabled: false,
            greyscale: false,
            mask_background: false,
            mask_sprites: false,
            show_background: false,
            show_sprites: false,
            color_emphasis: 0,
            vblank: false,
            next_vblank: false,
            sprite_0_hit: false,
            sprite_overflow: false,
            oam_address: 0,
            fine_scroll_x: 0,
            origin_vram_address: 0,
            vram_address: 0,
            write_latch: false,
            scanline: 0,
            dot: 0,
            vblank_nmi_triggered: false,
            palette_ram: [0; 32],
            primary_oam: Box::new([0; OAM_SIZE]),
            color_converter: ColorConverter::new(),
        }
    }

    pub fn write_control(&mut self, value: u8) {
        self.origin_vram_address = (self.origin_vram_address & !0b11_00000_00000)
            | ((value & 0b11) as u16) << 10;
        self.vram_address_increment = if value & 0b100 != 0 { 32 } else { 1 };
        self.sprite_ptable_address = if value & 0b1000 != 0 { 0x1000 } else { 0x0000 };
        self.background_ptable_address = if value & 0b10000 != 0 { 0x1000 } else { 0x0000 };
        self.tall_sprites = value & 0b100000 != 0;
        let next_vblank_nmi_enabled = value & 0b10000000 != 0;
        self.vblank_nmi_triggered |= self.next_vblank && next_vblank_nmi_enabled && !self.vblank_nmi_enabled;
        self.vblank_nmi_enabled = next_vblank_nmi_enabled;
    }

    pub fn write_mask(&mut self, value: u8) {
        self.greyscale = value & 0b1 != 0;
        self.mask_background = value & 0b10 != 0;
        self.mask_sprites = value & 0b100 != 0;
        self.show_background = value & 0b1000 != 0;
        self.show_sprites = value & 0b10000 != 0;
        self.color_emphasis = ((value & 0b11100000) as u16) << 1;
    }

    pub fn read_status(&mut self) -> u8 {
        let status = (self.vblank as u8) << 7
            | (self.sprite_0_hit as u8) << 6
            | (self.sprite_overflow as u8) << 5;
        self.next_vblank = false;
        self.write_latch = false;
        status
    }

    pub fn write_oam_address(&mut self, value: u8) {
        self.oam_address = value;
    }

    pub fn read_oam_data(&mut self) -> u8 {
        // Unlike PPU_VRAM_DATA, the address is not incremented after reading
        self.primary_oam[self.oam_address as usize]
    }

    pub fn write_oam_data(&mut self, value: u8) {
        self.primary_oam[self.oam_address as usize] = value;
        self.oam_address = self.oam_address.wrapping_add(1);
    }

    pub fn write_scroll(&mut self, value: u8) {
        if self.write_latch {
            self.origin_vram_address = (self.origin_vram_address & !0b111_00_11111_00000)
                | ((value & 0b00000111) as u16) << 12
                | ((value & 0b11111000) as u16) << 5;
            self.write_latch = false;
        }
        else {
            self.origin_vram_address = (self.origin_vram_address & !0b11111)
                | (value >> 3) as u16;
            self.fine_scroll_x = value & 0b111;
            self.write_latch = true;
        }
    }

    pub fn write_vram_address(&mut self, value: u8) {
        if self.write_latch {
            self.origin_vram_address = (self.origin_vram_address & 0xFF00)
                | value as u16;
            self.vram_address = self.origin_vram_address;
            self.write_latch = false;
        }
        else {
            self.origin_vram_address = (self.origin_vram_address & 0x00FF)
                | (((value & 0x3F) as u16) << 8);
            self.write_latch = true;
        }
    }

    pub fn read_vram_data(&mut self, cartridge: &Cartridge) -> u8 {
        let value = match self.vram_address & 0x3FFF {
            0x0000 ..= 0x3EFF => {
                cartridge.read_ppu_byte(self.vram_address)
            }
            address => {
                self.palette_ram[(address & 0b11111) as usize]
            }
        };
        self.vram_address = self.vram_address.wrapping_add(self.vram_address_increment);
        value
    }

    pub fn write_vram_data(&mut self, value: u8, cartridge: &mut Cartridge) {
        // println!("writing ${:04X} = #${value:02X}", self.vram_address);
        match self.vram_address & 0x3FFF {
            0x0000 ..= 0x3EFF => {
                cartridge.write_ppu_byte(self.vram_address, value);
            }
            address if address & 0b11 == 0 => {
                self.palette_ram[(address & 0b1111) as usize] = value;
                self.palette_ram[((address & 0b1111) | 0b10000) as usize] = value;
            }
            address => {
                self.palette_ram[(address & 0b11111) as usize] = value;
            }
        }
        self.vram_address = self.vram_address.wrapping_add(self.vram_address_increment);
    }

    pub fn check_vblank_nmi(&mut self) -> bool {
        std::mem::replace(&mut self.vblank_nmi_triggered, false)
    }
    
    // TODO: temporary
    pub fn is_at_top_left(&self) -> bool {
        self.scanline == 0 && self.dot == 0
    }

    pub fn tick(&mut self) {
        self.vblank = self.next_vblank;

        match (self.scanline, self.dot) {
            (VBLANK_START_SCANLINE, 0) => {
                self.next_vblank = true;
            }
            (VBLANK_START_SCANLINE, 1) => {
                self.vblank_nmi_triggered |= self.vblank_nmi_enabled && self.vblank;
            }
            (MAX_SCANLINE, 0) => {
                self.next_vblank = false;
                self.sprite_0_hit = false;
            }
            (scanline, dot) => {
                if scanline == self.primary_oam[0] as u16 && dot == self.primary_oam[3] as u16 {
                    self.sprite_0_hit = true;
                }
            }
        }

        if self.dot < MAX_DOT {
            self.dot = self.dot.wrapping_add(1);
        }
        else {
            self.dot = 0;
            if self.scanline < MAX_SCANLINE {
                self.scanline = self.scanline.wrapping_add(1);
            }
            else {
                self.scanline = 0;
            }
        }
    }

    pub fn get_tile_sliver(&self, base_nametable_address: u16, x: u8, y: u8, cartridge: &Cartridge) -> [u8; 8] {
        // 10 NN YYYYY XXXXX
        let tile_address = base_nametable_address
            | (y as u16) >> 3 << 5
            | (x as u16) >> 3;
        let pattern_number = cartridge.read_ppu_byte(tile_address);
        let pattern_address = self.background_ptable_address | (pattern_number as u16) << 4;
        let plane_0_row_address = pattern_address | (y & 0b111) as u16;
        let plane_1_row_address = plane_0_row_address | 0b1000;
        let plane_0_row = cartridge.read_ppu_byte(plane_0_row_address);
        let plane_1_row = cartridge.read_ppu_byte(plane_1_row_address);

        // 10 NN 1111 YYY XXX
        let attribute_address = base_nametable_address
            | ATTRIBUTE_OFFSET
            | (y as u16) >> 5 << 3
            | (x as u16) >> 5;
        let attribute_byte = cartridge.read_ppu_byte(attribute_address);
        let attribute_bit = (y & 0b10000) >> 2 | (x & 0b10000) >> 3;
        let palette_base = ((attribute_byte >> attribute_bit) & 0b11) << 2;

        let mut sliver = [0; 8];
        for fine_x in 0 .. sliver.len() {
            let color_bit_0 = (plane_0_row >> (7 - fine_x)) & 1;
            let color_bit_1 = (plane_1_row >> (7 - fine_x)) & 1;
            if color_bit_0 == 0 && color_bit_1 == 0 {
                sliver[fine_x] = self.palette_ram[0];
            }
            else {
                let color_index = palette_base | color_bit_1 << 1 | color_bit_0;
                sliver[fine_x] = self.palette_ram[color_index as usize];
            }
        }
        sliver
    }

    pub fn get_color_rgb(&self, index: u8) -> u32 {
        self.color_converter.get_rgb(self.color_emphasis | (index & 0b111111) as u16)
    }
}
