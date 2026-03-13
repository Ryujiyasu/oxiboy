/// Game Boy cartridge — ROM header parsing and MBC emulation
pub struct Cartridge {
    pub rom: Vec<u8>,
    pub ram: Vec<u8>,
    pub mbc: MbcType,
    pub rom_bank: usize,
    pub ram_bank: usize,
    pub ram_enabled: bool,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MbcType {
    None,
    Mbc1,
    Mbc3,
    Mbc5,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Self {
        let title = String::from_utf8_lossy(&rom[0x134..0x143])
            .trim_end_matches('\0')
            .to_string();

        let mbc = match rom.get(0x147).copied().unwrap_or(0) {
            0x00 => MbcType::None,
            0x01..=0x03 => MbcType::Mbc1,
            0x0F..=0x13 => MbcType::Mbc3,
            0x19..=0x1E => MbcType::Mbc5,
            _ => MbcType::None,
        };

        let ram_size = match rom.get(0x149).copied().unwrap_or(0) {
            0x02 => 8 * 1024,
            0x03 => 32 * 1024,
            0x04 => 128 * 1024,
            0x05 => 64 * 1024,
            _ => 0,
        };

        Cartridge {
            rom,
            ram: vec![0; ram_size],
            mbc,
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
            title,
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            // ROM Bank 0 (fixed)
            0x0000..=0x3FFF => *self.rom.get(addr as usize).unwrap_or(&0xFF),

            // ROM Bank N (switchable)
            0x4000..=0x7FFF => {
                let offset = self.rom_bank * 0x4000 + (addr as usize - 0x4000);
                *self.rom.get(offset).unwrap_or(&0xFF)
            }

            // External RAM
            0xA000..=0xBFFF => {
                if self.ram_enabled && !self.ram.is_empty() {
                    let offset = self.ram_bank * 0x2000 + (addr as usize - 0xA000);
                    *self.ram.get(offset).unwrap_or(&0xFF)
                } else {
                    0xFF
                }
            }

            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match self.mbc {
            MbcType::None => {
                // RAM write only
                if (0xA000..=0xBFFF).contains(&addr) && !self.ram.is_empty() {
                    let offset = addr as usize - 0xA000;
                    if let Some(cell) = self.ram.get_mut(offset) {
                        *cell = val;
                    }
                }
            }
            MbcType::Mbc1 => self.write_mbc1(addr, val),
            MbcType::Mbc3 => self.write_mbc3(addr, val),
            MbcType::Mbc5 => self.write_mbc5(addr, val),
        }
    }

    fn write_mbc1(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
            0x2000..=0x3FFF => {
                let bank = (val & 0x1F) as usize;
                self.rom_bank = if bank == 0 { 1 } else { bank };
            }
            0x4000..=0x5FFF => {
                self.ram_bank = (val & 0x03) as usize;
            }
            0x6000..=0x7FFF => { /* Banking mode select — simplified */ }
            0xA000..=0xBFFF => {
                if self.ram_enabled && !self.ram.is_empty() {
                    let offset = self.ram_bank * 0x2000 + (addr as usize - 0xA000);
                    if let Some(cell) = self.ram.get_mut(offset) {
                        *cell = val;
                    }
                }
            }
            _ => {}
        }
    }

    fn write_mbc3(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
            0x2000..=0x3FFF => {
                let bank = (val & 0x7F) as usize;
                self.rom_bank = if bank == 0 { 1 } else { bank };
            }
            0x4000..=0x5FFF => {
                self.ram_bank = (val & 0x03) as usize;
            }
            0xA000..=0xBFFF => {
                if self.ram_enabled && !self.ram.is_empty() {
                    let offset = self.ram_bank * 0x2000 + (addr as usize - 0xA000);
                    if let Some(cell) = self.ram.get_mut(offset) {
                        *cell = val;
                    }
                }
            }
            _ => {}
        }
    }

    fn write_mbc5(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
            0x2000..=0x2FFF => {
                self.rom_bank = (self.rom_bank & 0x100) | val as usize;
            }
            0x3000..=0x3FFF => {
                self.rom_bank = (self.rom_bank & 0xFF) | ((val as usize & 1) << 8);
            }
            0x4000..=0x5FFF => {
                self.ram_bank = (val & 0x0F) as usize;
            }
            0xA000..=0xBFFF => {
                if self.ram_enabled && !self.ram.is_empty() {
                    let offset = self.ram_bank * 0x2000 + (addr as usize - 0xA000);
                    if let Some(cell) = self.ram.get_mut(offset) {
                        *cell = val;
                    }
                }
            }
            _ => {}
        }
    }
}
