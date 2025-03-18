use std::io::Write;
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
    let mut machine = Box::new(Machine::new());
    machine.debug = true;
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

        if command.eq_ignore_ascii_case("Quit") {
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

            cartridge.load_into(&mut machine);
            println!("Successfully loaded cartridge.");
        }
        else if command.eq_ignore_ascii_case("Reset") {
            machine.reset();
            println!("Successfully reset.");
        }
        else if command.eq_ignore_ascii_case("Step") {
            machine.execute_instruction();
        }
        else if command.eq_ignore_ascii_case("State") {
            machine.print_cpu_state();
        }
        else if command.eq_ignore_ascii_case("Byte") {
            let address = match parse_int(argument) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: invalid address: {error}");
                    continue;
                }
            };
            let value = machine.fetch_byte(address);
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
            let value = machine.fetch_pair(address);
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
            let old_value = machine.fetch_byte(address);
            machine.store_byte(address, value);
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
            let old_value = machine.fetch_pair(address);
            machine.store_pair(address, value);
            println!("Pair at address ${address:04X}: ${old_value:04X} -> ${value:04X}");
        }
        else {
            println!("Error: unknown command: {}", command);
        }
    }
}
