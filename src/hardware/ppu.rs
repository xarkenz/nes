use crate::loader::Cartridge;

pub const PPU_CONTROL: u16 = 0;
pub const PPU_MASK: u16 = 1;
pub const PPU_STATUS: u16 = 2;
pub const PPU_OAM_ADDRESS: u16 = 3;
pub const PPU_OAM_DATA: u16 = 4;
pub const PPU_SCROLL: u16 = 5;
pub const PPU_VRAM_ADDRESS: u16 = 6;
pub const PPU_VRAM_DATA: u16 = 7;
const BASE_NAMETABLE_ADDRESSES: [u16; 4] = [0x2000, 0x2400, 0x2800, 0x2C00];
const OAM_SIZE: usize = 0x100;

pub struct PictureProcessingUnit {
    // PPU_CONTROL
    base_nametable_address: u16,
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
    emphasize_red: bool,
    emphasize_green: bool,
    emphasize_blue: bool,
    // PPU_STATUS
    vblank: bool,
    sprite_0_hit: bool,
    sprite_overflow: bool,
    // PPU_OAM_ADDRESS
    oam_address: u8,
    // PPU_SCROLL
    scroll_x: u8,
    scroll_y: u8,
    // PPU_VRAM_ADDRESS
    vram_address: u16,
    // Internal
    write_latch: bool,
    internal_oam: Box<[u8; OAM_SIZE]>,
}

impl PictureProcessingUnit {
    pub fn new() -> Self {
        Self {
            base_nametable_address: BASE_NAMETABLE_ADDRESSES[0],
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
            emphasize_red: false,
            emphasize_green: false,
            emphasize_blue: false,
            vblank: false,
            sprite_0_hit: false,
            sprite_overflow: false,
            oam_address: 0,
            scroll_x: 0,
            scroll_y: 0,
            vram_address: 0,
            write_latch: false,
            internal_oam: Box::new([0; OAM_SIZE]),
        }
    }

    pub fn write_control(&mut self, value: u8) {
        self.base_nametable_address = BASE_NAMETABLE_ADDRESSES[(value & 0b11) as usize];
        self.vram_address_increment = if value & 0b100 != 0 { 32 } else { 1 };
        self.sprite_ptable_address = if value & 0b1000 != 0 { 0x1000 } else { 0x0000 };
        self.background_ptable_address = if value & 0b10000 != 0 { 0x1000 } else { 0x0000 };
        self.tall_sprites = value & 0b100000 != 0;
        self.vblank_nmi_enabled = value & 0b10000000 != 0;
    }

    pub fn write_mask(&mut self, value: u8) {
        self.greyscale = value & 0b1 != 0;
        self.mask_background = value & 0b10 != 0;
        self.mask_sprites = value & 0b100 != 0;
        self.show_background = value & 0b1000 != 0;
        self.show_sprites = value & 0b10000 != 0;
        self.emphasize_red = value & 0b100000 != 0;
        self.emphasize_green = value & 0b1000000 != 0;
        self.emphasize_blue = value & 0b10000000 != 0;
    }

    pub fn read_status(&mut self) -> u8 {
        let status = (self.vblank as u8) << 7
            | (self.sprite_0_hit as u8) << 6
            | (self.sprite_overflow as u8) << 5;
        self.vblank = false;
        self.write_latch = false;
        status
    }

    pub fn write_oam_address(&mut self, value: u8) {
        self.oam_address = value;
    }

    pub fn read_oam_data(&mut self) -> u8 {
        // Unlike PPU_VRAM_DATA, the address is not incremented after reading
        self.internal_oam[self.oam_address as usize]
    }

    pub fn write_oam_data(&mut self, value: u8) {
        self.internal_oam[self.oam_address as usize] = value;
        self.oam_address = self.oam_address.wrapping_add(1);
    }

    pub fn write_scroll(&mut self, value: u8) {
        if self.write_latch {
            self.scroll_y = value;
            self.write_latch = false;
        }
        else {
            self.scroll_x = value;
            self.write_latch = true;
        }
    }

    pub fn write_vram_address(&mut self, value: u8) {
        if self.write_latch {
            self.vram_address = (self.vram_address & 0xFF00) | value as u16;
            self.write_latch = false;
        }
        else {
            self.vram_address = (self.vram_address & 0x00FF) | ((value as u16) << 8);
            self.write_latch = true;
        }
    }

    pub fn read_vram_data(&mut self, cartridge: &Cartridge) -> u8 {
        let value = cartridge.read_ppu_byte(self.vram_address);
        self.vram_address = self.vram_address.wrapping_add(self.vram_address_increment);
        value
    }

    pub fn write_vram_data(&mut self, value: u8, cartridge: &mut Cartridge) {
        cartridge.write_ppu_byte(self.vram_address, value);
        self.vram_address = self.vram_address.wrapping_add(self.vram_address_increment);
    }
}
