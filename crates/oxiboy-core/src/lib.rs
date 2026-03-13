pub mod cpu;
pub mod bus;
pub mod ppu;
pub mod timer;
pub mod cartridge;
pub mod joypad;
pub mod gb;

pub use gb::GameBoy;
pub use cartridge::Cartridge;
pub use joypad::JoypadButton;

/// Screen dimensions
pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

/// Clock speed: 4.194304 MHz
pub const CLOCK_SPEED: u32 = 4_194_304;

/// Cycles per frame (at ~59.7 fps)
pub const CYCLES_PER_FRAME: u32 = 70224;
