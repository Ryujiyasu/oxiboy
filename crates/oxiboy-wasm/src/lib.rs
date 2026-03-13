use wasm_bindgen::prelude::*;
use oxiboy_core::{GameBoy, JoypadButton, SCREEN_WIDTH, SCREEN_HEIGHT};

#[wasm_bindgen]
pub struct OxiBoy {
    gb: GameBoy,
}

#[wasm_bindgen]
impl OxiBoy {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> OxiBoy {
        OxiBoy {
            gb: GameBoy::new(rom.to_vec()),
        }
    }

    pub fn run_frame(&mut self) {
        self.gb.run_frame();
    }

    pub fn framebuffer(&self) -> Vec<u8> {
        self.gb.framebuffer().to_vec()
    }

    pub fn screen_width(&self) -> usize {
        SCREEN_WIDTH
    }

    pub fn screen_height(&self) -> usize {
        SCREEN_HEIGHT
    }

    pub fn title(&self) -> String {
        self.gb.title().to_string()
    }

    pub fn press(&mut self, key: &str) {
        if let Some(btn) = str_to_button(key) {
            self.gb.press(btn);
        }
    }

    pub fn release(&mut self, key: &str) {
        if let Some(btn) = str_to_button(key) {
            self.gb.release(btn);
        }
    }
}

fn str_to_button(key: &str) -> Option<JoypadButton> {
    match key {
        "a" | "z" => Some(JoypadButton::A),
        "b" | "x" => Some(JoypadButton::B),
        "select" | "Shift" => Some(JoypadButton::Select),
        "start" | "Enter" => Some(JoypadButton::Start),
        "right" | "ArrowRight" => Some(JoypadButton::Right),
        "left" | "ArrowLeft" => Some(JoypadButton::Left),
        "up" | "ArrowUp" => Some(JoypadButton::Up),
        "down" | "ArrowDown" => Some(JoypadButton::Down),
        _ => None,
    }
}
