#![allow(dead_code)]
#![allow(clippy::new_without_default)]
#![allow(unused_variables)]
#![allow(clippy::single_match)]
use rand::Rng;
use sdl2::{keyboard::Scancode, pixels::Color, render::Canvas, video::Window};
use std::{fs, thread, time};

pub const SCREEN_WIDTH: usize = 64;
pub const SCREEN_HEIGHT: usize = 32;

const RAM_SIZE: usize = 4096;
const NUM_REGS: usize = 16;
const STACK_SIZE: usize = 16;
const NUM_KEYS: usize = 16;
const START_ADDR: u16 = 0x200;
const FONTS: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

#[derive(Debug)]
pub enum Instruction {
    ClearScreen,         // 00E0 - Clears screen
    SubroutineReturn,    // 00EE - Returns from a subroutine
    Jump(u16),           // 1NNN - Set PC to NNN
    CallSubRoutine(u16), // 2NNN - Calls subroutine at memory location NNN

    SkipIfEqual(u8, u8), // 3XNN - Skips an instruction if value in VX is equal to NN
    SkipIfNotEqual(u8, u8), // 3XNN - Skips an instruction if value in VX is not equal to NN
    SkipIfRegistersEqual(u8, u8), // 5XY0 - Skips an instruction if values in VX and VY are equal
    SkipIfRegistersNotEqual(u8, u8), // 9XY0 - Skips an instruction if values in VX and VY are not equal

    SetRegister(u8, u8),   // 6XNN - Set register VX to value NN
    AddToRegister(u8, u8), // 7XNN - Add the value NN to VX

    Set(u8, u8), // 8XY0 - Sets VX to the value of VY
    Or(u8, u8),  // 8XY1 - VX is set to bitwise OR of VX and VY. VY is not affected
    And(u8, u8), // 8XY2 - VX is set to bitwise AND of VX and VY. VY is not affected
    Xor(u8, u8), // 8XY3 - VX is set to bitwise XOR of VX and VY. VY is not affected

    AddRegisters(u8, u8), // 8XY4 - VX is set to the value of VX plus VY.

    Subtract(u8, u8),        // 8XY5 - Sets VX to VX - VY
    ReverseSubtract(u8, u8), // 8XY7 - Sets VX to VY - VX

    ShiftRight(u8, u8), // 8XY6 - Puts the value of VY in VX and shifts the value in VX 1 bit right
    ShiftLeft(u8, u8),  // 8XYE - Puts the value of VY in VX and shifts the value in VX 1 bit left

    SetIndexRegister(u16), // ANNN - Set index register to value NNN
    JumpWithOffset(u16),   // BNNN - Jumps to address NNN plus value in register V0
    Random(u8, u8),        // CXNN - Stores a random number, ANDed with the value NN in VX

    Draw(u8, u8, u8), // DXYN - Draws an N pixels tall sprite from the memory location the index
    // register is holding to the screen, at the horizontal X coordinate in VX
    // and Y coordinate in VY
    SkipIfKey(u8), // EX9E - Skips an instruction if key corresponding to value VX is pressed
    SkipIfNotKey(u8), // EXA1 - Skips an instruction if key corresponding to value VX is not pressed
    GetKey(u8),    // FX0A - Stops executing instructions and waits until a key is pressed

    GetTimer(u8),      // FX07 - Sets VX to the current value of the delay timer
    SetDelayTimer(u8), // FX15 - Sets delay timer to value in VX
    SetSoundTimer(u8), // FX18 - Sets sound timer to value in VX

    AddToIndex(u8),              // FX1E - Index register gets value in VX added to it
    FontCharacter(u8),           // FX29 - Index register set to address of hex character in VX
    BinaryDecimalConversion(u8), // FX33 - Takes the number in VXm converts it to 3 decimal digits

    StoreMemory(u8), // FX55 - Value of each register from V0 to VX inclusive stored in memory
    LoadMemory(u8),  // FX65 - Loads the value stored at memory addresses to registers

    PlaceHolder,
}

pub struct Emulator {
    // SDL Context - Used for input / output
    sdl_context: sdl2::Sdl,

    // Program Counter - Keeps track of current place in the game
    pc: u16,

    // Random Access Memory - Entire game is transferred to RAM - 4KB
    ram: [u8; RAM_SIZE],

    // Screen is monochrome (1 bit per pixel)
    screen: [bool; SCREEN_WIDTH * SCREEN_HEIGHT],

    // V Registers - referenced from V0 to VF (0 - 15) in Hex
    v_registers: [u8; NUM_REGS],

    // I register - Used for indexing into RAM for reads and writes
    i_register: u16,

    // Stack - An awway of 16-bit values the CPU can read and write to - Last in, First Out
    // Only used when you are entering or exiting a subroutine
    // An array with a Stack Pointer to know where the top is
    stack_pointer: u16,
    stack: [u16; STACK_SIZE],

    // Chip-8 supports 16 different keys, numbered in hex from 0-9, A-F
    // Arranged in a 4x4 grid
    keys: [bool; NUM_KEYS],

    // Delay timer - A typical timer, counts down each cycle, performs an action if it hits zero
    delay_timer: u8,

    // Sound Timer - Counts down every cycle, emits a noise at zero
    sound_timer: u8,

    // Used for the instruction FX0A - (waiting for key?, register to store key in)
    waiting_for_key: (bool, u8),
}

impl Emulator {
    pub fn new() -> Self {
        Self {
            sdl_context: sdl2::init().unwrap(),
            pc: START_ADDR,
            ram: [0; RAM_SIZE],
            screen: [false; SCREEN_WIDTH * SCREEN_HEIGHT],
            v_registers: [0; NUM_REGS],
            i_register: 0,
            stack_pointer: 0,
            stack: [0; STACK_SIZE],
            keys: [false; NUM_KEYS],
            delay_timer: 0,
            sound_timer: 0,
            waiting_for_key: (false, 0),
        }
    }

    // Loads fonts for hex characters 0-F into memory from index 0x50-0x9F
    pub fn load_fonts(&mut self) {
        for (i, byte) in FONTS.iter().enumerate() {
            self.ram[i + 0x50] = *byte;
        }
    }

    pub fn load_rom(&mut self, rom_path: String) {
        let f: Vec<u8> = fs::read(rom_path).unwrap_or_else(|e| {
            eprintln!("Failed to load ROM: {}", e);
            std::process::exit(1);
        });

        if f.len() <= 4096 {
            for (i, byte) in f.iter().enumerate() {
                self.ram[i + 0x200] = *byte;
            }
        } else {
            eprintln!("Maximum ROM size exceeded");
            std::process::exit(1);
        }
    }

    pub fn fetch_instruction(&mut self) -> u16 {
        let instruction: u16 =
            ((self.ram[self.pc as usize] as u16) << 8) | (self.ram[self.pc as usize + 1] as u16);
        self.pc += 2;
        instruction
    }

    pub fn decode_instruction(&mut self, instruction: u16) -> Instruction {
        // First hex digit = (instruction >> 12) as u8
        // Last two hex digits = (instruction & 0xFF) as u8
        // Second hex digit = ((instruction >> 8) & 0xF) as u8
        // Last three hex digits = instruction & 0xFFF
        // Third hex digit = ((instruction >> 4) & 0xF) as u8,
        match (instruction >> 12) as u8 {
            0x0 => match instruction {
                0x00E0 => Instruction::ClearScreen,

                0x00EE => Instruction::SubroutineReturn,

                _ => Instruction::PlaceHolder,
            },

            0x1 => Instruction::Jump(instruction & 0x0FFF),

            0x2 => Instruction::CallSubRoutine(instruction & 0xFFF),

            0x3 => Instruction::SkipIfEqual(
                ((instruction >> 8) & 0xF) as u8,
                (instruction & 0xFF) as u8,
            ),

            0x4 => Instruction::SkipIfNotEqual(
                ((instruction >> 8) & 0xF) as u8,
                (instruction & 0xFF) as u8,
            ),

            0x5 => Instruction::SkipIfRegistersEqual(
                ((instruction >> 8) & 0xF) as u8,
                ((instruction >> 4) & 0xF) as u8,
            ),

            0x6 => Instruction::SetRegister(
                ((instruction >> 8) & 0xF) as u8,
                (instruction & 0xFF) as u8,
            ),

            0x7 => Instruction::AddToRegister(
                ((instruction >> 8) & 0x0F) as u8,
                (instruction & 0xFF) as u8,
            ),

            0x8 => {
                let second = ((instruction >> 8) & 0xF) as u8;
                let third = ((instruction >> 4) & 0xF) as u8;
                match instruction & 0xF {
                    0x0 => Instruction::Set(second, third),
                    0x1 => Instruction::Or(second, third),
                    0x2 => Instruction::And(second, third),
                    0x3 => Instruction::Xor(second, third),
                    0x4 => Instruction::AddRegisters(second, third),
                    0x5 => Instruction::Subtract(second, third),
                    0x6 => Instruction::ShiftRight(second, third),
                    0x7 => Instruction::ReverseSubtract(second, third),
                    0xE => Instruction::ShiftLeft(second, third),
                    _ => Instruction::PlaceHolder,
                }
            }

            0x9 => Instruction::SkipIfRegistersNotEqual(
                ((instruction >> 8) & 0xF) as u8,
                ((instruction >> 4) & 0xF) as u8,
            ),

            0xA => Instruction::SetIndexRegister(instruction & 0x0FFF),

            0xB => Instruction::JumpWithOffset(instruction & 0x0FFF),

            0xC => {
                Instruction::Random(((instruction >> 8) & 0xF) as u8, (instruction & 0xFF) as u8)
            }

            0xD => Instruction::Draw(
                ((instruction >> 8) & 0x0F) as u8,
                ((instruction >> 4) & 0x0F) as u8,
                (instruction & 0x0F) as u8,
            ),

            0xE => match instruction & 0xF {
                0xE => Instruction::SkipIfKey(((instruction >> 8) & 0xF) as u8),
                0x1 => Instruction::SkipIfNotKey(((instruction >> 8) & 0xF) as u8),
                _ => Instruction::PlaceHolder,
            },
            0xF => {
                let second = ((instruction >> 8) & 0xF) as u8;
                match instruction & 0xFF {
                    0x07 => Instruction::GetTimer(second),
                    0x0A => Instruction::GetKey(second),
                    0x15 => Instruction::SetDelayTimer(second),
                    0x18 => Instruction::SetSoundTimer(second),
                    0x1E => Instruction::AddToIndex(second),
                    0x29 => Instruction::FontCharacter(second),
                    0x33 => Instruction::BinaryDecimalConversion(second),
                    0x55 => Instruction::StoreMemory(second),
                    0x65 => Instruction::LoadMemory(second),
                    _ => Instruction::PlaceHolder,
                }
            }
            _ => Instruction::PlaceHolder,
        }
    }

    pub fn execute_instruction(&mut self, instruction: Instruction) {
        match instruction {
            Instruction::ClearScreen => {
                self.screen = [false; SCREEN_WIDTH * SCREEN_HEIGHT];
            }

            Instruction::CallSubRoutine(nnn) => {
                self.stack[self.stack_pointer as usize] = self.pc;
                self.stack_pointer += 1;
                self.pc = nnn;
            }

            Instruction::SubroutineReturn => {
                self.stack_pointer -= 1;
                self.pc = self.stack[self.stack_pointer as usize];
            }

            Instruction::Jump(nnn) => {
                self.pc = nnn;
            }

            Instruction::SkipIfEqual(vx, nn) => {
                if self.v_registers[vx as usize] == nn {
                    self.pc += 2;
                }
            }

            Instruction::SkipIfNotEqual(vx, nn) => {
                if self.v_registers[vx as usize] != nn {
                    self.pc += 2;
                }
            }

            Instruction::SkipIfRegistersEqual(vx, vy) => {
                if self.v_registers[vx as usize] == self.v_registers[vy as usize] {
                    self.pc += 2;
                }
            }

            Instruction::SkipIfRegistersNotEqual(vx, vy) => {
                if self.v_registers[vx as usize] != self.v_registers[vy as usize] {
                    self.pc += 2;
                }
            }

            Instruction::Set(vx, vy) => {
                self.v_registers[vx as usize] = self.v_registers[vy as usize];
            }

            Instruction::Or(vx, vy) => {
                self.v_registers[vx as usize] |= self.v_registers[vy as usize];
            }

            Instruction::And(vx, vy) => {
                self.v_registers[vx as usize] &= self.v_registers[vy as usize];
            }

            Instruction::Xor(vx, vy) => {
                self.v_registers[vx as usize] ^= self.v_registers[vy as usize];
            }

            Instruction::AddRegisters(vx, vy) => {
                let (result, overflow) =
                    self.v_registers[vx as usize].overflowing_add(self.v_registers[vy as usize]);
                self.v_registers[vx as usize] = result;
                self.v_registers[0xF] = if overflow { 1 } else { 0 }
            }

            Instruction::Subtract(vx, vy) => {
                self.v_registers[vx as usize] =
                    self.v_registers[vx as usize].wrapping_sub(self.v_registers[vy as usize]);
            }

            Instruction::ReverseSubtract(vx, vy) => {
                self.v_registers[vx as usize] =
                    self.v_registers[vy as usize].wrapping_sub(self.v_registers[vx as usize]);
            }

            Instruction::ShiftLeft(vx, vy) => {
                if (self.v_registers[vx as usize] >> 7) & 1 == 1 {
                    self.v_registers[0xF] = 1;
                } else {
                    self.v_registers[0xF] = 0;
                }
                self.v_registers[vx as usize] = self.v_registers[vy as usize] << 1;
            }

            Instruction::ShiftRight(vx, vy) => {
                if self.v_registers[vx as usize] & 1 == 1 {
                    self.v_registers[0xF] = 1;
                } else {
                    self.v_registers[0xF] = 0;
                }
                self.v_registers[vx as usize] = self.v_registers[vy as usize] >> 1;
            }

            Instruction::JumpWithOffset(nnn) => {
                self.pc = nnn + self.v_registers[0] as u16;
            }

            Instruction::Random(vx, nn) => {
                let mut rng = rand::thread_rng();

                self.v_registers[vx as usize] = rng.gen::<u8>() & nn;
            }

            Instruction::SetRegister(vx, nn) => {
                self.v_registers[vx as usize] = nn;
            }

            Instruction::SetIndexRegister(nnn) => {
                self.i_register = nnn;
            }

            Instruction::AddToRegister(vx, nn) => {
                self.v_registers[vx as usize] = self.v_registers[vx as usize].wrapping_add(nn);
            }

            Instruction::SkipIfKey(vx) => {
                if self.keys[self.v_registers[vx as usize] as usize] {
                    self.pc += 2;
                }
            }

            Instruction::SkipIfNotKey(vx) => {
                if !self.keys[self.v_registers[vx as usize] as usize] {
                    self.pc += 2;
                }
            }

            Instruction::GetTimer(vx) => {
                self.v_registers[vx as usize] = self.delay_timer;
            }

            Instruction::SetSoundTimer(vx) => {
                self.sound_timer = self.v_registers[vx as usize];
            }

            Instruction::SetDelayTimer(vx) => {
                self.delay_timer = self.v_registers[vx as usize];
            }

            Instruction::AddToIndex(vx) => {
                self.i_register += self.v_registers[vx as usize] as u16;
            }

            Instruction::GetKey(vx) => {
                self.waiting_for_key = (true, vx);
            }

            Instruction::FontCharacter(vx) => {
                self.i_register = 0x200 + 5 * (self.v_registers[vx as usize] & 0xF) as u16;
            }

            Instruction::StoreMemory(vx) => {
                for register in 0..=vx {
                    self.ram[self.i_register as usize + register as usize] =
                        self.v_registers[register as usize];
                }
            }

            Instruction::LoadMemory(vx) => {
                for register in 0..=vx {
                    self.v_registers[register as usize] =
                        self.ram[self.i_register as usize + register as usize];
                }
            }

            Instruction::BinaryDecimalConversion(vx) => {
                let number = self.v_registers[vx as usize];
                let hundreds = number / 100;
                let tens = (number % 100) / 10;
                let ones = number % 10;
                self.ram[self.i_register as usize] = hundreds;
                self.ram[self.i_register as usize + 1] = tens;
                self.ram[self.i_register as usize + 2] = ones;
            }

            Instruction::Draw(vx, vy, height) => {
                let mut y = self.v_registers[vy as usize] % (SCREEN_HEIGHT as u8);
                self.v_registers[0xF] = 0;
                for sprite_row in 0..height {
                    if y as usize >= SCREEN_HEIGHT {
                        break;
                    }
                    let mut x = self.v_registers[vx as usize] % (SCREEN_WIDTH as u8);
                    let row_data = self.ram[self.i_register as usize + sprite_row as usize];

                    for sprite_column in 0..8 {
                        if x as usize >= SCREEN_WIDTH {
                            break;
                        }

                        let position_on_screen = SCREEN_WIDTH * y as usize + x as usize;

                        let bit = (row_data >> (7 - sprite_column)) & 1;

                        if bit == 1 {
                            if self.screen[position_on_screen] {
                                self.screen[position_on_screen] = false;
                                self.v_registers[0xF] = 1;
                            } else {
                                self.screen[position_on_screen] = true;
                            }
                        }
                        x += 1;
                    }
                    y += 1;
                }
            }
            Instruction::PlaceHolder => todo!(),
        }
    }

    pub fn new_window(&self) -> Canvas<Window> {
        let video_subsystem = self.sdl_context.video().unwrap();
        let window = video_subsystem
            .window(
                "Chip-8 Emulator",
                (SCREEN_WIDTH * 20) as u32,
                (SCREEN_HEIGHT * 20) as u32,
            )
            .position_centered()
            .build()
            .unwrap();
        let mut canvas = window.into_canvas().accelerated().build().unwrap();
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.present();
        canvas
    }

    pub fn update_screen(&mut self, canvas: &mut Canvas<Window>) {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.set_draw_color(Color::RGB(255, 255, 255));
        for (position, pixel) in self.screen.iter().enumerate() {
            if *pixel {
                let x = (position % SCREEN_WIDTH) as i32 * 20;
                let y = (position / SCREEN_WIDTH) as i32 * 20;
                canvas
                    .fill_rect(sdl2::rect::Rect::new(x, y, 20, 20))
                    .unwrap();
            }
        }
        canvas.present();
    }

    pub fn game_loop(&mut self, canvas: &mut Canvas<Window>) {
        let mut last_timer_update = time::Instant::now();
        let timer_interval = time::Duration::from_millis(16);

        loop {
            let start_time = time::Instant::now();

            if start_time.duration_since(last_timer_update) >= timer_interval {
                if self.sound_timer > 0 {
                    self.sound_timer -= 1;
                }
                if self.delay_timer > 0 {
                    self.delay_timer -= 1;
                }
                last_timer_update = start_time;
            }

            let mut event_pump = self.sdl_context.event_pump().unwrap();
            let pressed_keys = event_pump.keyboard_state();
            self.keys[0] = pressed_keys.is_scancode_pressed(Scancode::X);
            self.keys[1] = pressed_keys.is_scancode_pressed(Scancode::Num1);
            self.keys[2] = pressed_keys.is_scancode_pressed(Scancode::Num2);
            self.keys[3] = pressed_keys.is_scancode_pressed(Scancode::Num3);
            self.keys[4] = pressed_keys.is_scancode_pressed(Scancode::Q);
            self.keys[5] = pressed_keys.is_scancode_pressed(Scancode::W);
            self.keys[6] = pressed_keys.is_scancode_pressed(Scancode::E);
            self.keys[7] = pressed_keys.is_scancode_pressed(Scancode::A);
            self.keys[8] = pressed_keys.is_scancode_pressed(Scancode::S);
            self.keys[9] = pressed_keys.is_scancode_pressed(Scancode::D);
            self.keys[0xA] = pressed_keys.is_scancode_pressed(Scancode::Z);
            self.keys[0xB] = pressed_keys.is_scancode_pressed(Scancode::C);
            self.keys[0xC] = pressed_keys.is_scancode_pressed(Scancode::Num4);
            self.keys[0xD] = pressed_keys.is_scancode_pressed(Scancode::R);
            self.keys[0xE] = pressed_keys.is_scancode_pressed(Scancode::F);
            self.keys[0xF] = pressed_keys.is_scancode_pressed(Scancode::V);

            for event in event_pump.poll_iter() {
                match event {
                    sdl2::event::Event::Quit { .. } => return,
                    _ => {}
                }
            }

            if self.waiting_for_key.0 {
                for (key, pressed) in self.keys.iter().enumerate() {
                    if *pressed {
                        self.v_registers[self.waiting_for_key.1 as usize] = key as u8;
                        self.waiting_for_key.0 = false;
                        break;
                    }
                }
                if self.waiting_for_key.0 {
                    continue;
                }
            }

            let instruction_code = self.fetch_instruction();
            let instruction = self.decode_instruction(instruction_code);
            println!("{:X}, {:?}", instruction_code, instruction);
            self.execute_instruction(instruction);

            self.update_screen(canvas);

            thread::sleep(time::Duration::new(0, 1000))
        }
    }

    fn handle_input(&mut self) {
        let mut event_pump = self.sdl_context.event_pump().unwrap();
        let pressed_keys = event_pump.keyboard_state();
        self.keys[0] = pressed_keys.is_scancode_pressed(Scancode::X);
        self.keys[1] = pressed_keys.is_scancode_pressed(Scancode::Num1);
        self.keys[2] = pressed_keys.is_scancode_pressed(Scancode::Num2);
        self.keys[3] = pressed_keys.is_scancode_pressed(Scancode::Num3);
        self.keys[4] = pressed_keys.is_scancode_pressed(Scancode::Q);
        self.keys[5] = pressed_keys.is_scancode_pressed(Scancode::W);
        self.keys[6] = pressed_keys.is_scancode_pressed(Scancode::E);
        self.keys[7] = pressed_keys.is_scancode_pressed(Scancode::A);
        self.keys[8] = pressed_keys.is_scancode_pressed(Scancode::S);
        self.keys[9] = pressed_keys.is_scancode_pressed(Scancode::D);
        self.keys[0xA] = pressed_keys.is_scancode_pressed(Scancode::Z);
        self.keys[0xB] = pressed_keys.is_scancode_pressed(Scancode::C);
        self.keys[0xC] = pressed_keys.is_scancode_pressed(Scancode::Num4);
        self.keys[0xD] = pressed_keys.is_scancode_pressed(Scancode::R);
        self.keys[0xE] = pressed_keys.is_scancode_pressed(Scancode::F);
        self.keys[0xF] = pressed_keys.is_scancode_pressed(Scancode::V);

        for event in event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } => return,
                _ => {}
            }
        }
    }

    pub fn start_game(&mut self, rom_path: String) -> Canvas<Window> {
        self.load_fonts();
        self.load_rom(rom_path);
        self.new_window()
    }
}
