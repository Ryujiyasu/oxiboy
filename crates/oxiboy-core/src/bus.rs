use crate::cartridge::Cartridge;
use crate::ppu::Ppu;
use crate::timer::Timer;
use crate::joypad::Joypad;

/// Memory Bus — maps addresses to hardware components
pub struct Bus {
    pub cartridge: Cartridge,
    pub ppu: Ppu,
    pub timer: Timer,
    pub joypad: Joypad,
    /// Work RAM (8KB)
    wram: [u8; 0x2000],
    /// High RAM (127 bytes)
    hram: [u8; 0x7F],
    /// Interrupt Enable register (0xFFFF)
    pub ie: u8,
    /// Interrupt Flag register (0xFF0F)
    pub if_reg: u8,
    /// Serial transfer data (stub)
    serial_data: u8,
    serial_control: u8,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Bus {
            cartridge,
            ppu: Ppu::new(),
            timer: Timer::new(),
            joypad: Joypad::new(),
            wram: [0; 0x2000],
            hram: [0; 0x7F],
            ie: 0,
            if_reg: 0xE1,
            serial_data: 0,
            serial_control: 0,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // ROM
            0x0000..=0x7FFF => self.cartridge.read(addr),
            // VRAM
            0x8000..=0x9FFF => self.ppu.vram[(addr - 0x8000) as usize],
            // External RAM
            0xA000..=0xBFFF => self.cartridge.read(addr),
            // WRAM
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            // Echo RAM
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize],
            // OAM
            0xFE00..=0xFE9F => self.ppu.oam[(addr - 0xFE00) as usize],
            // Unusable
            0xFEA0..=0xFEFF => 0xFF,
            // I/O Registers
            0xFF00..=0xFF7F => self.read_io(addr),
            // HRAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            // IE
            0xFFFF => self.ie,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            // ROM (MBC register writes)
            0x0000..=0x7FFF => self.cartridge.write(addr, val),
            // VRAM
            0x8000..=0x9FFF => self.ppu.vram[(addr - 0x8000) as usize] = val,
            // External RAM
            0xA000..=0xBFFF => self.cartridge.write(addr, val),
            // WRAM
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            // Echo RAM
            0xE000..=0xFDFF => self.wram[(addr - 0xE000) as usize] = val,
            // OAM
            0xFE00..=0xFE9F => self.ppu.oam[(addr - 0xFE00) as usize] = val,
            // Unusable
            0xFEA0..=0xFEFF => {}
            // I/O
            0xFF00..=0xFF7F => self.write_io(addr, val),
            // HRAM
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            // IE
            0xFFFF => self.ie = val,
        }
    }

    fn read_io(&self, addr: u16) -> u8 {
        match addr {
            0xFF00 => self.joypad.read(),
            0xFF01 => self.serial_data,
            0xFF02 => self.serial_control,
            0xFF04 => self.timer.div(),
            0xFF05 => self.timer.tima,
            0xFF06 => self.timer.tma,
            0xFF07 => self.timer.tac,
            0xFF0F => self.if_reg,
            // Sound registers (stub — return 0)
            0xFF10..=0xFF3F => 0,
            0xFF40 => self.ppu.lcdc,
            0xFF41 => self.ppu.stat | 0x80,
            0xFF42 => self.ppu.scy,
            0xFF43 => self.ppu.scx,
            0xFF44 => self.ppu.ly,
            0xFF45 => self.ppu.lyc,
            // DMA (write-only)
            0xFF46 => 0xFF,
            0xFF47 => self.ppu.bgp,
            0xFF48 => self.ppu.obp0,
            0xFF49 => self.ppu.obp1,
            0xFF4A => self.ppu.wy,
            0xFF4B => self.ppu.wx,
            _ => 0xFF,
        }
    }

    fn write_io(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF00 => self.joypad.write(val),
            0xFF01 => self.serial_data = val,
            0xFF02 => self.serial_control = val,
            0xFF04 => self.timer.reset_div(),
            0xFF05 => self.timer.tima = val,
            0xFF06 => self.timer.tma = val,
            0xFF07 => self.timer.tac = val,
            0xFF0F => self.if_reg = val,
            // Sound registers (stub)
            0xFF10..=0xFF3F => {}
            0xFF40 => self.ppu.lcdc = val,
            0xFF41 => self.ppu.stat = (self.ppu.stat & 0x07) | (val & 0x78),
            0xFF42 => self.ppu.scy = val,
            0xFF43 => self.ppu.scx = val,
            0xFF44 => {} // LY is read-only
            0xFF45 => self.ppu.lyc = val,
            // DMA Transfer
            0xFF46 => self.dma_transfer(val),
            0xFF47 => self.ppu.bgp = val,
            0xFF48 => self.ppu.obp0 = val,
            0xFF49 => self.ppu.obp1 = val,
            0xFF4A => self.ppu.wy = val,
            0xFF4B => self.ppu.wx = val,
            _ => {}
        }
    }

    /// OAM DMA Transfer: copies 160 bytes from XX00-XX9F to OAM
    fn dma_transfer(&mut self, source: u8) {
        let base = (source as u16) << 8;
        for i in 0..0xA0u16 {
            let val = self.read(base + i);
            self.ppu.oam[i as usize] = val;
        }
    }
}
