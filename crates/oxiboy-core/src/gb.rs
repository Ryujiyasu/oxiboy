use crate::bus::Bus;
use crate::cartridge::Cartridge;
use crate::cpu::Cpu;
use crate::joypad::JoypadButton;
use crate::{SCREEN_WIDTH, SCREEN_HEIGHT, CYCLES_PER_FRAME};

/// Main Game Boy emulator
pub struct GameBoy {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl GameBoy {
    pub fn new(rom: Vec<u8>) -> Self {
        let cartridge = Cartridge::new(rom);
        GameBoy {
            cpu: Cpu::new(),
            bus: Bus::new(cartridge),
        }
    }

    /// Run one frame (~70224 T-cycles). Returns true when frame is ready.
    pub fn run_frame(&mut self) -> bool {
        let mut cycles_this_frame: u32 = 0;
        let mut frame_ready = false;

        while cycles_this_frame < CYCLES_PER_FRAME {
            let cycles = self.cpu.step(&mut self.bus);
            cycles_this_frame += cycles;

            // Update timer
            self.bus.timer.step(cycles);
            if self.bus.timer.interrupt {
                self.bus.if_reg |= 0x04; // Timer interrupt
            }

            // Update PPU
            self.bus.ppu.step(cycles);
            self.bus.if_reg |= self.bus.ppu.interrupt_flags;
            if self.bus.ppu.frame_ready {
                frame_ready = true;
            }

            // Joypad interrupt
            if self.bus.joypad.interrupt {
                self.bus.if_reg |= 0x10;
                self.bus.joypad.interrupt = false;
            }
        }

        frame_ready
    }

    /// Get framebuffer as RGBA bytes
    pub fn framebuffer(&self) -> &[u8] {
        &self.bus.ppu.framebuffer
    }

    /// Get screen dimensions
    pub fn screen_size(&self) -> (usize, usize) {
        (SCREEN_WIDTH, SCREEN_HEIGHT)
    }

    /// Get ROM title
    pub fn title(&self) -> &str {
        &self.bus.cartridge.title
    }

    pub fn press(&mut self, button: JoypadButton) {
        self.bus.joypad.press(button);
    }

    pub fn release(&mut self, button: JoypadButton) {
        self.bus.joypad.release(button);
    }
}
