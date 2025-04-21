use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use cpal::traits::HostTrait;
use minifb::{Key, Scale, Window, WindowOptions};
use hardware::*;
use loader::*;
use util::*;
use crate::audio::ReceiverSignal;

pub mod hardware;
pub mod loader;
pub mod audio;
pub mod util;

pub fn main() {
    let keyboard_interrupt_flag = Arc::new(AtomicBool::new(false));
    {
        // Set up Ctrl+C handler
        let keyboard_interrupt_flag = keyboard_interrupt_flag.clone();
        let result = ctrlc::set_handler(move || {
            keyboard_interrupt_flag.store(true, Ordering::Relaxed);
            println!("Stopping...");
        });
        if let Err(error) = result {
            eprintln!("Warning: Failed to setup Ctrl+C handler: {}", error);
        }
    }
    let keyboard_interrupt = || {
        keyboard_interrupt_flag.swap(false, Ordering::Relaxed)
    };

    let mut machine = Machine::new();
    let mut audio_runtime = audio::AudioRuntime::new(cpal::default_host().default_output_device().unwrap());
    let mut user_input = String::new();
    let mut target_fps = NTSC_FRAMES_PER_SECOND;
    let mut log_sound = false;

    {
        let mut pal_file = std::fs::File::open("2C02G_wiki.pal")
            .expect("failed to open pal file");
        machine.ppu.color_converter.parse_pal(&mut pal_file).unwrap();
    }

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
        else if command.eq_ignore_ascii_case("Warmup") {
            machine.ppu.resetting = false;
            println!("PPU forced out of warm-up phase.");
        }
        else if command.eq_ignore_ascii_case("Tick") {
            machine.tick();
            println!("Tick completed.");
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
                    eprintln!("Error: Invalid address: {error}");
                    continue;
                }
            };
            while !keyboard_interrupt() && machine.cpu.program_counter != address {
                machine.debug_step();
            }
            if machine.cpu.program_counter == address {
                println!("Breakpoint reached.");
            }
        }
        else if command.eq_ignore_ascii_case("StepNMI") {
            let nmi_handler_address = machine.read_pair(NMI_VECTOR);
            while !keyboard_interrupt() && machine.cpu.program_counter != nmi_handler_address {
                machine.tick();
            }
            if machine.cpu.program_counter == nmi_handler_address {
                println!("NMI detected. (PC = ${nmi_handler_address:04X})");
            }
        }
        else if command.eq_ignore_ascii_case("NextFrame") {
            while !machine.ppu.is_entering_vblank() {
                machine.tick();
            }
            machine.tick();
            println!("Entered vblank of next frame.");
        }
        else if command.eq_ignore_ascii_case("ResetStats") {
            machine.cpu.debug_cycle_counter = 0;
            machine.ppu.debug_cycle_counter = 0;
            println!("Reset counters to zero.");
        }
        else if command.eq_ignore_ascii_case("Stats") {
            println!("CPU cycles: {}", machine.cpu.debug_cycle_counter);
            println!("PPU cycles: {}", machine.ppu.debug_cycle_counter);
        }
        else if command.eq_ignore_ascii_case("CPU") {
            machine.cpu.debug_print_state();
        }
        else if command.eq_ignore_ascii_case("PPU") {
            machine.ppu.debug_print_state();
        }
        else if command.eq_ignore_ascii_case("APU") {
            machine.apu.debug_print_state();
        }
        else if command.eq_ignore_ascii_case("Mapper") {
            if let Some(cartridge) = &machine.cartridge {
                cartridge.debug_print_mapper_state();
            }
            else {
                println!("Error: No cartridge loaded.");
            }
        }
        else if command.eq_ignore_ascii_case("SetPC") {
            let address = match parse_int(argument) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: Invalid address: {error}");
                    continue;
                }
            };
            machine.cpu.program_counter = address;
        }
        else if command.eq_ignore_ascii_case("Byte") {
            let address = match parse_int(argument) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: Invalid address: {error}");
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
                    eprintln!("Error: Invalid address: {error}");
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
                    eprintln!("Error: Invalid address: {error}");
                    continue;
                }
            };
            let value = match parse_int(value.trim_start()) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("Error: Invalid byte value: {error}");
                    continue;
                }
            };
            let old_value = machine.read_byte_silent(address);
            machine.write_byte(address, value);
            println!("Byte at address ${address:04X}: ${old_value:02X} -> ${value:02X}");
        }
        else if command.eq_ignore_ascii_case("SetPair") {
            let Some((address, value)) = argument.split_once('=') else {
                eprintln!("Error: Expected '=' for assignment.");
                continue;
            };
            let address = match parse_int(address.trim_end()) {
                Ok(address) => address,
                Err(error) => {
                    eprintln!("Error: Invalid address: {error}");
                    continue;
                }
            };
            let value = match parse_int(value.trim_start()) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("Error: Invalid pair value: {error}");
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
                    eprintln!("Error: Invalid address: {error}");
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
        else if command.eq_ignore_ascii_case("Sprite") {
            let index = match parse_int::<u8>(argument) {
                Ok(index) if index & 0b11000000 == 0 => index,
                Ok(..) => {
                    eprintln!("Error: Sprite index out of range.");
                    continue;
                }
                Err(error) => {
                    eprintln!("Error: Invalid sprite index: {error}");
                    continue;
                }
            };
            let address = (index << 2) as usize;
            let byte_0 = machine.ppu.primary_oam[address + 0];
            let byte_1 = machine.ppu.primary_oam[address + 1];
            let byte_2 = machine.ppu.primary_oam[address + 2];
            let byte_3 = machine.ppu.primary_oam[address + 3];
            println!("Sprite {index}:");
            println!("    OAM at ${address:02X}: {byte_0:02X} {byte_1:02X} {byte_2:02X} {byte_3:02X}");
            println!("    Screen X: {}", byte_3);
            println!("    Screen Y: {}", byte_0 as u16 + 1);
            println!("    Tile number: ${:02X}", byte_1);
            println!("    Palette: {}", (byte_2 & 0b11) | 0b100);
            println!("    Priority: {}", if byte_2 & 0b100000 == 0 { "In front of background" } else { "Behind background" });
            println!("    Flip X: {}", byte_2 & 0b1000000 != 0);
            println!("    Flip Y: {}", byte_2 & 0b10000000 != 0);
        }
        else if command.eq_ignore_ascii_case("PTables") {
            let Some(cartridge) = &mut machine.cartridge else {
                println!("Error: No cartridge loaded.");
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
            let mut window = Window::new("NES Pattern Tables", WIDTH, HEIGHT, window_options).unwrap();
            window.set_target_fps(10);
            while window.is_open() {
                window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
            }
        }
        else if command.eq_ignore_ascii_case("NTables") {
            let Some(cartridge) = &mut machine.cartridge else {
                println!("Error: No cartridge loaded.");
                continue;
            };

            let mut buffer = vec![0_u32; ppu::SCREEN_WIDTH * ppu::SCREEN_HEIGHT * 4];
            for (pos, sliver) in buffer.chunks_exact_mut(8).enumerate() {
                let x = ((pos & 0b11111) as u8) << 3;
                let y = ((pos >> 6) % ppu::SCREEN_HEIGHT) as u8;
                let mut base_nametable_address = 0x2000;
                if pos & 0b100000 != 0 {
                    base_nametable_address |= 0x0400;
                }
                if pos >> 6 >= ppu::SCREEN_HEIGHT {
                    base_nametable_address |= 0x0800;
                }
                let computed_sliver = machine.ppu.get_tile_sliver(base_nametable_address, x, y, cartridge)
                    .map(|index| machine.ppu.get_palette_color_rgb(index));
                sliver.copy_from_slice(&computed_sliver);
            }

            let mut window_options = WindowOptions::default();
            window_options.scale = Scale::X1;
            let mut window = Window::new("NES Nametables", ppu::SCREEN_WIDTH * 2, ppu::SCREEN_HEIGHT * 2, window_options).unwrap();
            window.set_target_fps(10);
            while window.is_open() {
                window.update_with_buffer(&buffer, ppu::SCREEN_WIDTH * 2, ppu::SCREEN_HEIGHT * 2).unwrap();
            }
        }
        else if command.eq_ignore_ascii_case("FPS") {
            if argument.is_empty() {
                println!("Target FPS is {target_fps}.");
                continue;
            }
            match parse_int::<u16>(argument) {
                Ok(fps) => {
                    target_fps = fps as usize;
                }
                Err(error) => {
                    eprintln!("Error: Invalid FPS value: {error}");
                }
            }
        }
        else if command.eq_ignore_ascii_case("LogSound") {
            log_sound = true;
            println!("Sound logging is now enabled.");
        }
        else if command.eq_ignore_ascii_case("NoLogSound") {
            log_sound = false;
            println!("Sound logging is now disabled.");
        }
        else if command.eq_ignore_ascii_case("Play") {
            use dasp::Signal;
            let (mixer_sender, mixer_receiver) = std::sync::mpsc::channel();
            machine.apu.connect_mixer_output(mixer_sender);
            if log_sound {
                let mut log_file = std::fs::File::create("target/sndlog.txt").unwrap();
                let mut last_frame = 0.0;
                audio_runtime.connect(
                    ReceiverSignal::new(mixer_receiver).map(move |frame| {
                        let frame = frame * 2.0 - 1.0;
                        if frame != last_frame {
                            writeln!(log_file, "{frame}").ok();
                            last_frame = frame;
                        }
                        frame
                    }),
                    machine.apu.mixer_samples_per_frame() * target_fps as f64,
                );
            }
            else {
                audio_runtime.connect(
                    ReceiverSignal::new(mixer_receiver).map(|frame| frame * 2.0 - 1.0),
                    machine.apu.mixer_samples_per_frame() * target_fps as f64,
                );
            }

            let mut window_options = WindowOptions::default();
            window_options.scale = Scale::X2;
            let mut window = Window::new("NES", ppu::SCREEN_WIDTH, ppu::SCREEN_HEIGHT, window_options).unwrap();

            window.update_with_buffer(machine.ppu.screen_buffer.as_slice(), ppu::SCREEN_WIDTH, ppu::SCREEN_HEIGHT).unwrap();
            window.set_target_fps(target_fps);

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

            machine.apu.disconnect_mixer_output();
            audio_runtime.disconnect();
        }
        else {
            println!("Error: Unknown command: {}", command);
        }
    }
}
