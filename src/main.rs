use std::io::Write;
use minifb::{Scale, Window, WindowOptions};
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
    let mut machine = Machine::new();
    let mut user_input = String::new();

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
        else if command.eq_ignore_ascii_case("Quit") {
            break;
        }
        else if command.eq_ignore_ascii_case("Load") {
            let mut file = match std::fs::File::open(argument) {
                Ok(file) => file,
                Err(error) => {
                    eprintln!("Error: failed to open: {error}");
                    continue;
                }
            };

            let cartridge = match Cartridge::parse_ines(&mut file) {
                Ok(file) => file,
                Err(error) => {
                    eprintln!("Error: failed to parse: {error}");
                    continue;
                }
            };

            machine.cartridge_slot = Some(cartridge);
            println!("Successfully loaded cartridge.");
        }
        else if command.eq_ignore_ascii_case("Reset") {
            machine.reset();
            println!("Successfully reset.");
        }
        else if command.eq_ignore_ascii_case("Step") {
            let opcode = machine.read_byte_silent(machine.cpu.program_counter);
            let instruction = instructions::Instruction::decode(opcode);
            println!("Opcode: ${opcode:02X}");
            println!("Disassembly: {}", instruction.disassemble(&machine, machine.cpu.program_counter));
            // machine.execute_instruction();
        }
        else if command.eq_ignore_ascii_case("State") {
            machine.cpu.debug_print_state();
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
        else if command.eq_ignore_ascii_case("Play") {
            let mut buffer = vec![0_u32; 256 * 240];
            let mut window_options = WindowOptions::default();
            window_options.scale = Scale::X4;
            let mut window = Window::new("NES", 256, 240, window_options).unwrap();
            window.set_target_fps(60);
            while window.is_open() {
                machine.tick();
                while !machine.ppu.is_at_top_left() {
                    machine.tick();
                }
                for (pos, sliver) in buffer.chunks_exact_mut(8).enumerate() {
                    let base_nametable_address = 0x2000;
                    let x = ((pos as u8) & 0b11111) << 3;
                    let y = (pos >> 5) as u8;
                    let computed_sliver = machine.ppu.get_tile_sliver(base_nametable_address, x, y, &machine.cartridge_slot.as_ref().unwrap())
                        .map(|index| machine.ppu.get_color_rgb(index));
                    sliver.copy_from_slice(&computed_sliver);
                }
                window.update_with_buffer(&buffer, 256, 240).unwrap();
            }
        }
        else {
            println!("Error: unknown command: {}", command);
        }
    }
}
