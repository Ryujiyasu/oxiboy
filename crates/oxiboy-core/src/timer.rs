/// Game Boy Timer — DIV, TIMA, TMA, TAC
pub struct Timer {
    /// Internal divider counter (incremented every T-cycle)
    div_counter: u16,
    /// Timer counter (0xFF05)
    pub tima: u8,
    /// Timer modulo (0xFF06)
    pub tma: u8,
    /// Timer control (0xFF07)
    pub tac: u8,
    /// Internal timer counter
    timer_counter: u32,
    /// Interrupt flag
    pub interrupt: bool,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            div_counter: 0xABCC,
            tima: 0,
            tma: 0,
            tac: 0,
            timer_counter: 0,
            interrupt: false,
        }
    }

    /// DIV register (upper 8 bits of internal counter)
    pub fn div(&self) -> u8 {
        (self.div_counter >> 8) as u8
    }

    /// Reset DIV
    pub fn reset_div(&mut self) {
        self.div_counter = 0;
    }

    fn timer_enabled(&self) -> bool {
        self.tac & 0x04 != 0
    }

    fn clock_select(&self) -> u32 {
        match self.tac & 0x03 {
            0 => 1024, // 4096 Hz
            1 => 16,   // 262144 Hz
            2 => 64,   // 65536 Hz
            3 => 256,  // 16384 Hz
            _ => unreachable!(),
        }
    }

    /// Step timer by T-cycles
    pub fn step(&mut self, cycles: u32) {
        self.interrupt = false;
        self.div_counter = self.div_counter.wrapping_add(cycles as u16);

        if !self.timer_enabled() {
            return;
        }

        self.timer_counter += cycles;
        let freq = self.clock_select();

        while self.timer_counter >= freq {
            self.timer_counter -= freq;
            let (new_tima, overflow) = self.tima.overflowing_add(1);
            if overflow {
                self.tima = self.tma;
                self.interrupt = true;
            } else {
                self.tima = new_tima;
            }
        }
    }
}
