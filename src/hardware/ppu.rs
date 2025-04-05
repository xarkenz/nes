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
pub const PPU_COLOR_COUNT: usize = 64;
pub const HORIZONTAL_BITS: u16 = 0b000_01_00000_11111;
pub const VERTICAL_BITS: u16 = 0b111_10_11111_00000;
pub const SCREEN_WIDTH: usize = 256;
pub const SCREEN_HEIGHT: usize = 240;
pub const LAST_VISIBLE_SCANLINE: u16 = 239;
pub const VBLANK_START_SCANLINE: u16 = 241;
pub const PRE_RENDER_SCANLINE: u16 = 261;
pub const LAST_DOT: u16 = 341;
const NORMAL_COLOR_MASK: u16 = 0b111111;
const GREYSCALE_COLOR_MASK: u16 = 0b110000;
const NAMETABLES_START_ADDRESS: u16 = 0x2000;
const ATTRIBUTE_OFFSET: u16 = 0x03C0;
const SECONDARY_OAM_SIZE: u8 = 32;
const OAM_BYTE_2_MASK: u8 = 0b11100011;

pub struct PictureProcessingUnit {
    // PPU_CONTROL
    vram_address_increment: u16,
    sprite_ptable_address: u16,
    background_ptable_address: u16,
    tall_sprites: bool,
    vblank_nmi_enabled: bool,
    // PPU_MASK
    mask_background: bool,
    mask_sprites: bool,
    show_background: bool,
    show_sprites: bool,
    color_emphasis: u16,
    color_mask: u16,
    // PPU_STATUS
    vblank_flag: bool,
    next_vblank_flag: bool, // For the purposes of emulating hardware multiplexing
    sprite_0_hit: bool,
    sprite_overflow: bool,
    // PPU_OAM_ADDRESS
    oam_address: u8,
    // PPU_SCROLL
    fine_scroll_x: u8,
    origin_vram_address: u16,
    // PPU_VRAM_ADDRESS
    pub vram_address: u16,
    // PPU_VRAM_DATA
    vram_read_buffer: u8,
    // Internal
    resetting: bool,
    rendering: bool,
    write_latch: bool,
    pub scanline: u16,
    pub dot: u16,
    pub in_vblank: bool,
    vblank_nmi_triggered: bool, // true if actively pulling /NMI low
    sliver_shifter: [u8; 16], // epic variable name
    pub palette_ram: [u8; 32],
    primary_oam: Box<[u8; 0x100]>,
    secondary_oam_address: u8,
    secondary_oam: [u8; SECONDARY_OAM_SIZE as usize],
    sprite_evaluation_finished: bool,
    sprite_0_in_range: bool,
    sprite_output_buffer: Box<[u8; SCREEN_WIDTH]>,
    pub color_converter: ColorConverter,
    pub screen_buffer: Box<[u32; SCREEN_WIDTH * SCREEN_HEIGHT]>,
}

impl PictureProcessingUnit {
    pub fn new() -> Self {
        Self {
            vram_address_increment: 1,
            sprite_ptable_address: 0,
            background_ptable_address: 0,
            tall_sprites: false,
            vblank_nmi_enabled: false,
            mask_background: false,
            mask_sprites: false,
            show_background: false,
            show_sprites: false,
            color_emphasis: 0,
            color_mask: NORMAL_COLOR_MASK,
            vblank_flag: false,
            next_vblank_flag: false,
            sprite_0_hit: false,
            sprite_overflow: false,
            oam_address: 0,
            fine_scroll_x: 0,
            origin_vram_address: 0,
            vram_address: 0,
            vram_read_buffer: 0,
            resetting: false,
            rendering: false,
            write_latch: false,
            scanline: 0,
            dot: 0,
            in_vblank: false,
            vblank_nmi_triggered: false,
            sliver_shifter: [0; 16],
            palette_ram: [0; 32],
            primary_oam: Box::new([0; 0x100]),
            secondary_oam_address: 0,
            secondary_oam: [0; SECONDARY_OAM_SIZE as usize],
            sprite_evaluation_finished: false,
            sprite_0_in_range: false,
            sprite_output_buffer: Box::new([0; SCREEN_WIDTH]),
            color_converter: ColorConverter::new(),
            screen_buffer: Box::new([0; SCREEN_WIDTH * SCREEN_HEIGHT]),
        }
    }

    pub fn reset(&mut self) {
        self.resetting = true;

        self.vram_address_increment = 1;
        self.sprite_ptable_address = 0;
        self.background_ptable_address = 0;
        self.tall_sprites = false;
        self.vblank_nmi_enabled = false;
        self.mask_background = false;
        self.mask_sprites = false;
        self.show_background = false;
        self.show_sprites = false;
        self.color_emphasis = 0;
        self.color_mask = NORMAL_COLOR_MASK;
        self.fine_scroll_x = 0;
        self.origin_vram_address = 0;
        self.vram_read_buffer = 0;
        self.rendering = false;
        self.write_latch = false;
        self.scanline = 0;
        self.dot = 0;
        self.vblank_nmi_triggered = false;
    }

    pub fn write_control(&mut self, value: u8) {
        if self.resetting {
            return;
        }

        self.origin_vram_address = (self.origin_vram_address & !0b11_00000_00000)
            | ((value & 0b11) as u16) << 10;
        self.vram_address_increment = if value & 0b100 != 0 { 32 } else { 1 };
        self.sprite_ptable_address = if value & 0b1000 != 0 { 0x1000 } else { 0x0000 };
        self.background_ptable_address = if value & 0b10000 != 0 { 0x1000 } else { 0x0000 };
        self.tall_sprites = value & 0b100000 != 0;
        let next_vblank_nmi_enabled = value & 0b10000000 != 0;
        self.vblank_nmi_triggered |= self.next_vblank_flag && next_vblank_nmi_enabled && !self.vblank_nmi_enabled;
        self.vblank_nmi_enabled = next_vblank_nmi_enabled;
    }

    pub fn write_mask(&mut self, value: u8) {
        if self.resetting {
            return;
        }

        self.color_mask = if value & 0b1 != 0 { GREYSCALE_COLOR_MASK } else { NORMAL_COLOR_MASK };
        self.mask_background = value & 0b10 != 0;
        self.mask_sprites = value & 0b100 != 0;
        self.show_background = value & 0b1000 != 0;
        self.show_sprites = value & 0b10000 != 0;
        self.rendering = self.show_background || self.show_sprites;
        self.color_emphasis = ((value & 0b11100000) as u16) << 1;
    }

    pub fn read_status(&mut self) -> u8 {
        let status = (self.vblank_flag as u8) << 7
            | (self.sprite_0_hit as u8) << 6
            | (self.sprite_overflow as u8) << 5;
        self.next_vblank_flag = false;
        self.write_latch = false;
        status
    }

    pub fn write_oam_address(&mut self, value: u8) {
        self.oam_address = value;
    }

    pub fn read_oam_data(&mut self) -> u8 {
        let mut value;
        if self.scanline <= LAST_VISIBLE_SCANLINE && self.dot >= 1 && self.dot <= 64 {
            // Secondary OAM is being cleared to $FF by latching all reads from primary OAM to $FF
            value = 0xFF;
        }
        else {
            // Unlike PPU_VRAM_DATA, the address is not incremented after reading
            value = self.primary_oam[self.oam_address as usize];
        }

        if self.oam_address & 0b11 == 0b10 {
            // Bits 4:2 of each byte 2 in OAM aren't really supposed to exist and always read 0
            value &= OAM_BYTE_2_MASK;
        }

        value
    }

    pub fn write_oam_data(&mut self, value: u8) {
        self.primary_oam[self.oam_address as usize] = value;
        self.oam_address = self.oam_address.wrapping_add(1);
    }

    pub fn write_scroll(&mut self, value: u8) {
        if self.resetting {
            return;
        }

        if self.write_latch {
            self.origin_vram_address = (self.origin_vram_address & !0b111_00_11111_00000)
                | ((value & 0b00000111) as u16) << 12
                | ((value & 0b11111000) as u16) << 2;
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
        if self.resetting {
            return;
        }

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
        if self.resetting {
            return 0;
        }

        let value = self.vram_read_buffer;
        self.vram_read_buffer = match self.vram_address & 0x3FFF {
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

    pub fn is_at_top_left(&self) -> bool {
        self.scanline == 0 && self.dot == 0
    }

    pub fn is_entering_vblank(&self) -> bool {
        self.scanline == VBLANK_START_SCANLINE && self.dot == 0
    }

    pub fn get_tile_sliver(&self, base_nametable_address: u16, x: u8, y: u8, cartridge: &Cartridge) -> [u8; 8] {
        // yyy NN YYYYY XXXXX
        let vram_address = (base_nametable_address & 0b11_00000_00000) // Nametable select
            | (y as u16) >> 3 << 5 // Coarse Y
            | ((y as u16) & 0b111) << 12 // Fine Y
            | (x as u16) >> 3; // Coarse X

        let mut sliver = [0; 8];
        self.compute_background_sliver(&mut sliver, vram_address, cartridge);
        sliver
    }

    pub fn get_color_rgb(&self, index: u8) -> u32 {
        self.color_converter.get_rgb(self.color_emphasis | (index as u16 & self.color_mask))
    }

    pub fn tick(&mut self, cartridge: Option<&Cartridge>) {
        self.vblank_flag = self.next_vblank_flag;

        // Update VRAM address and flags if necessary
        match (self.scanline, self.dot) {
            (VBLANK_START_SCANLINE, 0) => {
                self.next_vblank_flag = true;
            }
            (VBLANK_START_SCANLINE, 1) => {
                self.in_vblank = true;
                self.vblank_nmi_triggered |= self.vblank_nmi_enabled && self.vblank_flag;
            }
            (PRE_RENDER_SCANLINE, 1) => {
                self.resetting = false;
                self.next_vblank_flag = false;
                self.sprite_0_hit = false;
                self.sprite_overflow = false;
                self.in_vblank = false;
            }
            _ => {}
        }
        
        if self.rendering {
            self.tick_rendering(cartridge);
        }

        if self.dot < LAST_DOT {
            self.dot = self.dot.wrapping_add(1);
        }
        else {
            self.dot = 0;
            if self.scanline < PRE_RENDER_SCANLINE {
                self.scanline = self.scanline.wrapping_add(1);
            }
            else {
                self.scanline = 0;
            }
        }
    }
    
    fn tick_rendering(&mut self, cartridge: Option<&Cartridge>) {
        // Update VRAM address as necessary for tile fetches
        match (self.scanline, self.dot) {
            (0 ..= 239 | PRE_RENDER_SCANLINE, 8 ..= 256) if self.dot & 0b111 == 0 => {
                self.draw_sliver();
                self.load_next_sliver(cartridge);
                self.increment_coarse_x();
                if self.dot == 256 {
                    self.increment_fine_y();
                }
            }
            (0 ..= 239 | PRE_RENDER_SCANLINE, 257) => {
                self.reset_coarse_x();
            }
            (0 ..= 239 | PRE_RENDER_SCANLINE, 328 | 336) => {
                self.load_next_sliver(cartridge);
                self.increment_coarse_x();
            }
            (PRE_RENDER_SCANLINE, 280 ..= 304) => {
                self.reset_fine_y();
            }
            _ => {}
        }

        // Handle sprite evaluation if on a visible scanline
        if self.scanline <= LAST_VISIBLE_SCANLINE {
            match self.dot {
                1 => {
                    // Clear secondary OAM
                    // This technically happens gradually between 1 ..= 64, but whatever
                    self.secondary_oam.fill(0xFF);
                    self.secondary_oam_address = 0;
                    self.oam_address = 0;
                    self.sprite_evaluation_finished = false;
                    self.sprite_0_in_range = false;
                }
                // TODO: this is awful
                65 ..= 256 if self.dot & 1 != 0 => {
                    // Perform sprite evaluation (reading from primary OAM in realtime, as the CPU
                    // can mess with that; writing to secondary OAM can happen simultaneously)
                    if self.sprite_evaluation_finished {
                        self.oam_address = self.oam_address.wrapping_add(4);
                    }
                    else if self.secondary_oam_address & 0b11 == 0 {
                        // Check if the current sprite is present on this scanline
                        let start_y = self.primary_oam[self.oam_address as usize] as u16;
                        let end_y = start_y + if self.tall_sprites { 16 } else { 8 };
                        if self.scanline >= start_y && self.scanline < end_y {
                            // Sprite is in range
                            if self.secondary_oam_address == SECONDARY_OAM_SIZE {
                                // Theoretically found a 9th sprite which triggers overflow
                                // (though, with the hardware bug, probably wrong)
                                self.sprite_overflow = true;
                                self.secondary_oam_address += 1; // Stop sprite range-checking
                            }
                            else {
                                if self.oam_address == 0 {
                                    self.sprite_0_in_range = true;
                                }
                                self.secondary_oam[self.secondary_oam_address as usize] = start_y as u8;
                                self.secondary_oam_address += 1;
                                self.oam_address = self.oam_address.wrapping_add(1);
                            }
                        }
                        else {
                            // Sprite is not in range
                            if self.secondary_oam_address == SECONDARY_OAM_SIZE {
                                // Perform the double-increment hardware bug (yippee!)
                                let increment_amount = if self.oam_address & 0b11 == 0b11 { 1 } else { 5 };
                                (self.oam_address, self.sprite_evaluation_finished) = self.oam_address.overflowing_add(increment_amount);
                            }
                            else {
                                (self.oam_address, self.sprite_evaluation_finished) = self.oam_address.overflowing_add(4);
                            }
                        }
                    }
                    else if self.secondary_oam_address < SECONDARY_OAM_SIZE {
                        // Transfer a byte from primary OAM to secondary OAM
                        let oam_byte = self.primary_oam[self.oam_address as usize];
                        self.secondary_oam[self.secondary_oam_address as usize] = oam_byte;
                        self.secondary_oam_address += 1;
                        (self.oam_address, self.sprite_evaluation_finished) = self.oam_address.overflowing_add(1);
                    }
                }
                257 => {
                    // Screw sprite output units, let's just load a buffer since it's basically
                    // invisible behavior to the CPU anyway
                    // Clear the buffer first
                    self.sprite_output_buffer.fill(0);
                    if let Some(cartridge) = cartridge {
                        self.load_sprite_output_buffer(self.scanline as u8, cartridge);
                    }
                }
                _ => {}
            }
        }
    }

    fn reset_coarse_x(&mut self) {
        // Retain the bits related to vertical position
        self.vram_address &= VERTICAL_BITS;
        // Copy coarse X and horizontal nametable bit from the origin VRAM address
        self.vram_address |= self.origin_vram_address & HORIZONTAL_BITS;
    }

    fn reset_fine_y(&mut self) {
        // Retain the bits related to horizontal position
        self.vram_address &= HORIZONTAL_BITS;
        // Copy fine Y, coarse Y, and vertical nametable bit from the origin VRAM address
        self.vram_address |= self.origin_vram_address & VERTICAL_BITS;
    }

    fn increment_coarse_x(&mut self) {
        if self.vram_address & 0b11111 == 0b11111 {
            // Coarse X will overflow the 5-bit field
            // Switch to the horizontally adjacent nametable
            self.vram_address ^= 0b01_00000_00000;
            // Simulate the overflow by setting the field to 0
            self.vram_address &= !0b11111;
        }
        else {
            // No overflow will occur, so add 1 to coarse X
            self.vram_address = self.vram_address.wrapping_add(0b00001);
        }
    }

    fn increment_coarse_y(&mut self) {
        if self.vram_address & 0b11101_00000 == 0b11101_00000 {
            // Coarse Y is either 29 (last row of the screen) or will overflow the 5-bit field
            // If coarse Y is 29, switch to the vertically adjacent nametable
            self.vram_address ^= (self.vram_address & 0b00010_00000) << 5;
            // Simulate the overflow by setting the field to 0
            self.vram_address &= !0b11111_00000;
        }
        else {
            // No overflow will occur, so add 1 to coarse Y
            self.vram_address = self.vram_address.wrapping_add(0b00001_00000);
        }
    }

    fn increment_fine_y(&mut self) {
        if self.vram_address & 0b111_00_00000_00000 == 0b111_00_00000_00000 {
            // Fine Y will overflow the 3-bit field
            // The fine Y field carries to the coarse Y field, so increment that instead
            self.increment_coarse_y();
            // Simulate the overflow by setting the field to 0
            self.vram_address &= !0b111_00_00000_00000;
        }
        else {
            // No overflow will occur, so add 1 to fine Y
            self.vram_address = self.vram_address.wrapping_add(0b001_00_00000_00000);
        }
    }

    fn load_next_sliver(&mut self, cartridge: Option<&Cartridge>) {
        // Make room for the next sliver to be stored by shifting the right sliver to the left
        self.sliver_shifter.copy_within(8.., 0);
        // Compute sliver color indices
        if let Some(cartridge) = cartridge {
            let mut sliver = [0; 8];
            if self.show_background {
                self.compute_background_sliver(&mut sliver, self.vram_address, cartridge);
            }
            self.sliver_shifter[8..].copy_from_slice(&sliver);
        }
        else {
            self.sliver_shifter[8..].fill(0);
        }
    }

    fn draw_sliver(&mut self) {
        // Make sure we are on a render scanline (dot is guaranteed to be valid unless there is a
        // logic error in the tick() method)
        if self.scanline as usize >= SCREEN_HEIGHT {
            return;
        }

        // The current dot is at the pixel immediately after the sliver being drawn, hence minus 8
        let base_pixel_x = self.dot as usize - 8;
        let base_pixel_index = self.scanline as usize * SCREEN_WIDTH + base_pixel_x;
        let shift_amount = self.fine_scroll_x as usize;
        for offset_x in 0 .. 8 {
            // Retrieve background pixel
            let mut color_index = self.sliver_shifter[shift_amount + offset_x];

            // Possibly modify pixel based on sprite output
            let buffer_value = self.sprite_output_buffer[base_pixel_x + offset_x];
            if self.show_sprites && buffer_value != 0 {
                // These flags are made up but we kinda need them in order to do it this way
                let behind_background = buffer_value & 0b10000000 != 0;
                let is_sprite_0 = buffer_value & 0b1000000 != 0;
                let sprite_color_index = buffer_value & 0b11111;

                if color_index == 0 {
                    // Background is transparent, so sprite pixel will always be used
                    color_index = sprite_color_index;
                }
                else {
                    // Check for sprite 0 hit (happens if opaque pixel of sprite 0 is being compared
                    // with opaque pixel of background)
                    if is_sprite_0 && base_pixel_x >= 2 {
                        self.sprite_0_hit = true;
                    }
                    // Overwrite background pixel unless the "behind background" flag is set
                    if !behind_background {
                        color_index = sprite_color_index;
                    }
                }
            }

            // Compute sRGB pixel color based on palette data and the color converter
            let palette_index = self.palette_ram[color_index as usize];
            self.screen_buffer[base_pixel_index + offset_x] = self.get_color_rgb(palette_index);
        }
    }

    fn compute_background_sliver(&self, sliver: &mut [u8], vram_address: u16, cartridge: &Cartridge) {
        // 10 NN YYYYY XXXXX
        let tile_address = NAMETABLES_START_ADDRESS | (vram_address & 0b11_11111_11111);
        // yyy
        let fine_y = (vram_address >> 12) & 0b111;

        let pattern_number = cartridge.read_ppu_byte(tile_address) as u16;
        // 0_ PPPPPPPP 0 000
        let pattern_address = self.background_ptable_address | pattern_number << 4;
        // 0_ PPPPPPPP 0 yyy
        let plane_0_row_address = pattern_address | fine_y;
        // 0_ PPPPPPPP 1 yyy
        let plane_1_row_address = plane_0_row_address | 0b1000;
        let plane_0_row = cartridge.read_ppu_byte(plane_0_row_address);
        let plane_1_row = cartridge.read_ppu_byte(plane_1_row_address);

        // 10 NN 1111 YYY XXX
        let attribute_address = (tile_address & 0b11_11_00000_00000)
            | ATTRIBUTE_OFFSET
            | (tile_address & 0b11100_00000) >> 4
            | (tile_address & 0b00000_11100) >> 2;
        let attribute_byte = cartridge.read_ppu_byte(attribute_address);
        // YX0
        let attribute_bit = (tile_address & 0b00010_00000) >> 4 | (tile_address & 0b00000_00010);
        // 0 pp 00
        let palette_base = ((attribute_byte >> attribute_bit) & 0b11) << 2;

        for offset_x in 0 .. sliver.len() {
            let color_bit_0 = (plane_0_row >> (7 - offset_x)) & 1;
            let color_bit_1 = (plane_1_row >> (7 - offset_x)) & 1;
            if color_bit_0 == 0 && color_bit_1 == 0 {
                sliver[offset_x] = 0;
            }
            else {
                // 0 pp cc
                sliver[offset_x] = palette_base | color_bit_1 << 1 | color_bit_0;
            }
        }
    }

    fn load_sprite_output_buffer(&mut self, y: u8, cartridge: &Cartridge) {
        for (sprite_index, sprite_data) in self.secondary_oam.chunks_exact(4).enumerate() {
            // Process byte 2 first since we need to know about flipping early on
            let attributes = sprite_data[2];
            let palette_base = 0b10000 | (attributes & 0b11) << 2;
            // Not realistic to hardware, but useful with this hacky approach. Tracks which pixels
            // belong to sprite 0 as well as whether the background should take precedence
            // Bit 7 = behind background?
            // Bit 6 = sprite 0?
            let is_sprite_0 = self.sprite_0_in_range && sprite_index == 0;
            let rendering_flags = (attributes & 0b100000) << 2 | (is_sprite_0 as u8) << 6;
            let flip_x = attributes & 0b1000000 != 0;
            let flip_y = attributes & 0b10000000 != 0;

            let top_y = sprite_data[0];
            if top_y >= 0xEF {
                // This sprite is offscreen (probably an empty sprite slot)
                continue;
            }
            let mut y_offset = y - top_y;
            if flip_y {
                y_offset = if self.tall_sprites { 15 - y_offset } else { 7 - y_offset };
            }

            let pattern_number = sprite_data[1] as u16;
            let pattern_address;
            if self.tall_sprites {
                let ptable_address = (pattern_number & 1) << 12;
                if y_offset >= 8 {
                    pattern_address = ptable_address | (pattern_number | 1) << 4;
                }
                else {
                    pattern_address = ptable_address | (pattern_number & !1) << 4;
                }
            }
            else {
                pattern_address = self.sprite_ptable_address | pattern_number << 4;
            }
            // 0_ PPPPPPPP 0 yyy
            let plane_0_row_address = pattern_address | (y_offset & 0b111) as u16;
            // 0_ PPPPPPPP 1 yyy
            let plane_1_row_address = plane_0_row_address | 0b1000;
            let plane_0_row = cartridge.read_ppu_byte(plane_0_row_address);
            let plane_1_row = cartridge.read_ppu_byte(plane_1_row_address);

            let left_x = sprite_data[3];

            for x_offset in 0 .. 8 {
                let (x, offscreen) = left_x.overflowing_add(x_offset);
                if offscreen {
                    break;
                }
                else if self.sprite_output_buffer[x as usize] != 0 {
                    // Get out-prioritied lol
                    continue;
                }
                
                let shift_amount = if flip_x { x_offset } else { 7 - x_offset };
                let color_bit_0 = (plane_0_row >> shift_amount) & 1;
                let color_bit_1 = (plane_1_row >> shift_amount) & 1;
                if color_bit_0 != 0 || color_bit_1 != 0 {
                    // BZ0 1 pp cc
                    self.sprite_output_buffer[x as usize] = rendering_flags
                        | palette_base
                        | color_bit_1 << 1
                        | color_bit_0;
                }
            }
        }
    }
}
