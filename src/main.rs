use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use minifb::{Key, Scale, Window, WindowOptions};
use hardware::*;
use loader::*;

pub mod hardware;
pub mod loader;

pub trait FromStrRadix {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, std::num::ParseIntError>
    where Self: Sized;
}

impl FromStrRadix for u8 {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, std::num::ParseIntError> {
        u8::from_str_radix(src, radix)
    }
}

impl FromStrRadix for u16 {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, std::num::ParseIntError> {
        u16::from_str_radix(src, radix)
    }
}

impl FromStrRadix for u32 {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, std::num::ParseIntError> {
        u32::from_str_radix(src, radix)
    }
}

pub fn parse_int<T: FromStrRadix>(string: &str) -> Result<T, std::num::ParseIntError> {
    let string = string.trim_start_matches('#');
    if let Some(hex) = string.strip_prefix("0x") {
        T::from_str_radix(hex, 16)
    }
    else if let Some(hex) = string.strip_prefix("$") {
        T::from_str_radix(hex, 16)
    }
    else if let Some(bin) = string.strip_prefix("0b") {
        T::from_str_radix(bin, 2)
    }
    else if let Some(bin) = string.strip_prefix("%") {
        T::from_str_radix(bin, 2)
    }
    else {
        T::from_str_radix(string, 10)
    }
}

pub fn main() {
    let keyboard_interrupt_flag = Arc::new(AtomicBool::new(false));
    {
        // Set up Ctrl+C handler
        let keyboard_interrupt_flag = keyboard_interrupt_flag.clone();
        let result = ctrlc::set_handler(move || {
            keyboard_interrupt_flag.store(true, Ordering::Relaxed);
            println!("Stopping...")
        });
        if let Err(error) = result {
            eprintln!("Warning: Failed to setup Ctrl+C handler: {}", error);
        }
    }
    let keyboard_interrupt = || {
        keyboard_interrupt_flag.swap(false, Ordering::Relaxed)
    };

    let mut machine = Machine::new();
    let mut user_input = String::new();

    let mut pal_file = std::fs::File::open("2C02G_wiki.pal")
        .expect("failed to open pal file");
    machine.ppu.color_converter.parse_pal(&mut pal_file).unwrap();

    loop {
        print!("> ");
        std::io::stdout().flush().unwrap();

        user_input.clear();
        std::io::stdin().read_line(&mut user_input).unwrap();

        let (command, argument) = match user_input.trim().split_once(' ') {
            Some((command, argument)) => (command.trim_end(), argument.trim_start()),
            None => (user_input.trim(), ""),
        };

        if command.is_empty() {
            continue;
        }
        else if command.eq_ignore_ascii_case("Quit") || command.eq_ignore_ascii_case("Exit") {
            break;
        }
        else if command.eq_ignore_ascii_case("Load") {
            let mut file = match std::fs::File::open(argument) {
                Ok(file) => file,
                Err(error) => {
                    eprintln!("Error: Failed to open file: {error}");
                    continue;
                }
            };

            let cartridge = match Cartridge::parse_nes(&mut file) {
                Ok(file) => file,
                Err(error) => {
                    eprintln!("Error: Failed to parse file: {error}");
                    continue;
                }
            };

            machine.cartridge = Some(cartridge);
            println!("Successfully loaded cartridge.");
            machine.reset();
            println!("Console reset.");
        }
        else if command.eq_ignore_ascii_case("Reset") {
            machine.reset();
            println!("Console reset.");
        }
        else if command.eq_ignore_ascii_case("Step") {
            machine.debug_step();
            println!("Step completed.");
        }
        else if command.eq_ignore_ascii_case("StepOver") {
            // Ignore JSR if it is the first instruction
            if machine.debug_step().mnemonic() != "RTS" {
                let mut nesting_level = 0_u64;
                while !keyboard_interrupt() {
                    match machine.debug_step().mnemonic() {
                        "JSR" => {
                            let overflowed;
                            (nesting_level, overflowed) = nesting_level.overflowing_add(1);
                            if overflowed {
                                eprintln!("Error: Nesting level overflowed.");
                                break;
                            }
                        }
                        "RTS" => {
                            let underflowed;
                            (nesting_level, underflowed) = nesting_level.overflowing_sub(1);
                            if underflowed {
                                println!("Subroutine completed.");
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        else if command.eq_ignore_ascii_case("StepBkpt") {
            let address = match parse_int(argument) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: invalid address: {error}");
                    continue;
                }
            };
            while !keyboard_interrupt() && machine.cpu.program_counter != address {
                machine.debug_step();
            }
            println!("Breakpoint reached.");
        }
        else if command.eq_ignore_ascii_case("NextFrame") {
            while !machine.ppu.is_entering_vblank() {
                machine.tick();
            }
            machine.tick();
            println!("Entered vblank of next frame.");
        }
        else if command.eq_ignore_ascii_case("StartCounter") {
            machine.cpu.debug_cycle_counter = 0;
            machine.ppu.debug_cycle_counter = 0;
            println!("Reset cycle counters to zero.");
        }
        else if command.eq_ignore_ascii_case("Counter") {
            println!("CPU cycles: {}", machine.cpu.debug_cycle_counter);
            println!("PPU cycles: {}", machine.ppu.debug_cycle_counter);
        }
        else if command.eq_ignore_ascii_case("State") {
            machine.cpu.debug_print_state();
        }
        else if command.eq_ignore_ascii_case("SetPC") {
            let address = match parse_int(argument) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: invalid address: {error}");
                    continue;
                }
            };
            machine.cpu.program_counter = address;
        }
        else if command.eq_ignore_ascii_case("Byte") {
            let address = match parse_int(argument) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: invalid address: {error}");
                    continue;
                }
            };
            let value = machine.read_byte(address);
            println!("Byte at address ${address:04X}: ${value:02X}");
        }
        else if command.eq_ignore_ascii_case("Pair") {
            let address = match parse_int(argument) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: invalid address: {error}");
                    continue;
                }
            };
            let value = machine.read_pair(address);
            println!("Pair at address ${address:04X}: ${value:04X}");
        }
        else if command.eq_ignore_ascii_case("SetByte") {
            let Some((address, value)) = argument.split_once('=') else {
                eprintln!("Error: expected '=' for assignment.");
                continue;
            };
            let address = match parse_int(address.trim_end()) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: invalid address: {error}");
                    continue;
                }
            };
            let value = match parse_int(value.trim_start()) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("Error: invalid byte value: {error}");
                    continue;
                }
            };
            let old_value = machine.read_byte_silent(address);
            machine.write_byte(address, value);
            println!("Byte at address ${address:04X}: ${old_value:02X} -> ${value:02X}");
        }
        else if command.eq_ignore_ascii_case("SetPair") {
            let Some((address, value)) = argument.split_once('=') else {
                eprintln!("Error: expected '=' for assignment.");
                continue;
            };
            let address = match parse_int(address.trim_end()) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: invalid address: {error}");
                    continue;
                }
            };
            let value = match parse_int(value.trim_start()) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("Error: invalid pair value: {error}");
                    continue;
                }
            };
            let old_value = machine.read_pair_silent(address);
            machine.write_pair(address, value);
            println!("Pair at address ${address:04X}: ${old_value:04X} -> ${value:04X}");
        }
        else if command.eq_ignore_ascii_case("Dis") {
            let address = match parse_int(argument) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: invalid address: {error}");
                    continue;
                }
            };
            let opcode = machine.read_byte(address);
            let instruction = instructions::Instruction::decode(opcode);
            let disassembly = instruction.disassemble(&machine, address);
            match instruction.size_bytes() {
                2 => {
                    let op0 = machine.read_byte(address.wrapping_add(1));
                    println!("${address:04X}: {opcode:02X} {op0:02X}    ; {disassembly}");
                }
                3 => {
                    let op0 = machine.read_byte(address.wrapping_add(1));
                    let op1 = machine.read_byte(address.wrapping_add(2));
                    println!("${address:04X}: {opcode:02X} {op0:02X} {op1:02X} ; {disassembly}");
                }
                _ => {
                    println!("${address:04X}: {opcode:02X}       ; {disassembly}");
                }
            }
        }
        else if command.eq_ignore_ascii_case("StartDis") {
            machine.start_debug_disassembly();
            println!("Debug disassembly is now active.");
        }
        else if command.eq_ignore_ascii_case("EndDis") {
            if argument.is_empty() {
                machine.cancel_debug_disassembly();
                println!("Debug disassembly canceled.");
                continue;
            }
            
            let mut file = match std::fs::File::create(argument) {
                Ok(file) => file,
                Err(error) => {
                    eprintln!("Error: Failed to create file: {error}");
                    continue;
                }
            };

            if let Err(error) = machine.end_debug_disassembly(&mut file) {
                eprintln!("Error: Failed to write to file: {error}");
                let _ = std::fs::remove_file(argument);
                continue;
            }
            
            println!("Debug disassembly successfully dumped to file.");
        }
        else if command.eq_ignore_ascii_case("Palette") {
            print!("BG: ");
            for index in 0..16 {
                if index & 0b11 == 0 {
                    print!("| ");
                }
                print!("{:02X} ", machine.ppu.palette_ram[index]);
            }
            println!();
            print!("SP: ");
            for index in 16..32 {
                if index & 0b11 == 0 {
                    print!("| ");
                }
                print!("{:02X} ", machine.ppu.palette_ram[index]);
            }
            println!();
        }
        else if command.eq_ignore_ascii_case("PTables") {
            let Some(cartridge) = &mut machine.cartridge else {
                println!("Error: no cartridge loaded.");
                continue;
            };
            
            const GAP: usize = 8;
            const TABLE_SIZE: usize = 128;
            const WIDTH: usize = TABLE_SIZE + GAP + TABLE_SIZE;
            const HEIGHT: usize = TABLE_SIZE;
            const COLORS: [u32; 4] = [0x000000, 0xFFFFFF, 0x999999, 0x444444];
            let mut buffer = vec![0xFF00FF_u32; WIDTH * HEIGHT];
            
            for (base_address, start_x) in [(0x0000, 0), (0x1000, TABLE_SIZE + GAP)] {
                for coarse_y in 0b0000 ..= 0b1111 {
                    for coarse_x in 0b0000 ..= 0b1111 {
                        let pattern_address = base_address | coarse_y << 8 | coarse_x << 4;
                        let tile_x = start_x + ((coarse_x as usize) << 3);
                        let tile_y = (coarse_y as usize) << 3;
                        for row in 0b000 ..= 0b111 {
                            let plane_0_row_address = pattern_address | row;
                            let plane_1_row_address = plane_0_row_address | 0b1000;
                            let plane_0_row = cartridge.read_ppu_byte(plane_0_row_address);
                            let plane_1_row = cartridge.read_ppu_byte(plane_1_row_address);
                            let start_index = (tile_y + row as usize) * WIDTH + tile_x;
                            for column in 0b000 ..= 0b111 {
                                let color_bit_0 = (plane_0_row >> (7 - column)) & 1;
                                let color_bit_1 = (plane_1_row >> (7 - column)) & 1;
                                let color_index = color_bit_1 << 1 | color_bit_0;
                                buffer[start_index + column] = COLORS[color_index as usize];
                            }
                        }
                    }
                }
            }
            
            let mut window_options = WindowOptions::default();
            window_options.scale = Scale::X4;
            let mut window = Window::new("NES CHR View", WIDTH, HEIGHT, window_options).unwrap();
            window.set_target_fps(10);
            while window.is_open() {
                window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
            }
        }
        else if command.eq_ignore_ascii_case("Play") {
            let mut window_options = WindowOptions::default();
            window_options.scale = Scale::X2;
            let mut window = Window::new("NES", ppu::SCREEN_WIDTH, ppu::SCREEN_HEIGHT, window_options).unwrap();
            window.update_with_buffer(machine.ppu.screen_buffer.as_slice(), ppu::SCREEN_WIDTH, ppu::SCREEN_HEIGHT).unwrap();
            window.set_target_fps(60);
            
            while window.is_open() {
                machine.controller_1.fill(false);
                for key in window.get_keys() {
                    match key {
                        Key::K => machine.controller_1[BUTTON_A] = true,
                        Key::J => machine.controller_1[BUTTON_B] = true,
                        Key::Tab => machine.controller_1[BUTTON_SELECT] = true,
                        Key::Space => machine.controller_1[BUTTON_START] = true,
                        Key::W => machine.controller_1[BUTTON_UP] = true,
                        Key::S => machine.controller_1[BUTTON_DOWN] = true,
                        Key::A => machine.controller_1[BUTTON_LEFT] = true,
                        Key::D => machine.controller_1[BUTTON_RIGHT] = true,
                        _ => {}
                    }
                }
                
                while !machine.ppu.is_entering_vblank() {
                    machine.tick();
                }
                machine.tick();
                
                window.update_with_buffer(machine.ppu.screen_buffer.as_slice(), ppu::SCREEN_WIDTH, ppu::SCREEN_HEIGHT).unwrap();
            }
        }
        else {
            println!("Error: unknown command: {}", command);
        }
    }
}
