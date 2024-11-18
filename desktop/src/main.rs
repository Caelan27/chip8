use std::env;
fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = &args[1];
    let mut emu = chip8_core::Emulator::new();
    let mut canvas = emu.start_game(String::from(file_path));
    emu.game_loop(&mut canvas);
}
