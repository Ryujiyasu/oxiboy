/// Game Boy Joypad input
pub struct Joypad {
    /// Button state (active low)
    pub buttons: u8, // Start, Select, B, A
    pub dpad: u8,    // Down, Up, Left, Right
    /// Select bits (0xFF00 write)
    pub select: u8,
    /// Interrupt flag
    pub interrupt: bool,
}

#[derive(Clone, Copy)]
pub enum JoypadButton {
    A,
    B,
    Select,
    Start,
    Right,
    Left,
    Up,
    Down,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad {
            buttons: 0x0F,
            dpad: 0x0F,
            select: 0x30,
            interrupt: false,
        }
    }

    pub fn read(&self) -> u8 {
        let mut result = self.select | 0xC0;
        if self.select & 0x20 == 0 {
            result = (result & 0xF0) | self.buttons;
        }
        if self.select & 0x10 == 0 {
            result = (result & 0xF0) | self.dpad;
        }
        result
    }

    pub fn write(&mut self, val: u8) {
        self.select = val & 0x30;
    }

    pub fn press(&mut self, button: JoypadButton) {
        match button {
            JoypadButton::A => self.buttons &= !0x01,
            JoypadButton::B => self.buttons &= !0x02,
            JoypadButton::Select => self.buttons &= !0x04,
            JoypadButton::Start => self.buttons &= !0x08,
            JoypadButton::Right => self.dpad &= !0x01,
            JoypadButton::Left => self.dpad &= !0x02,
            JoypadButton::Up => self.dpad &= !0x04,
            JoypadButton::Down => self.dpad &= !0x08,
        }
        self.interrupt = true;
    }

    pub fn release(&mut self, button: JoypadButton) {
        match button {
            JoypadButton::A => self.buttons |= 0x01,
            JoypadButton::B => self.buttons |= 0x02,
            JoypadButton::Select => self.buttons |= 0x04,
            JoypadButton::Start => self.buttons |= 0x08,
            JoypadButton::Right => self.dpad |= 0x01,
            JoypadButton::Left => self.dpad |= 0x02,
            JoypadButton::Up => self.dpad |= 0x04,
            JoypadButton::Down => self.dpad |= 0x08,
        }
    }
}
