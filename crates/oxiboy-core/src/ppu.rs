use crate::{SCREEN_WIDTH, SCREEN_HEIGHT};

/// Pixel Processing Unit — renders tiles, sprites, and background
pub struct Ppu {
    pub vram: [u8; 0x2000],      // 8KB Video RAM
    pub oam: [u8; 0xA0],         // Object Attribute Memory (40 sprites × 4 bytes)
    pub framebuffer: [u8; SCREEN_WIDTH * SCREEN_HEIGHT * 4], // RGBA

    // LCD registers
    pub lcdc: u8,  // 0xFF40 — LCD Control
    pub stat: u8,  // 0xFF41 — LCD Status
    pub scy: u8,   // 0xFF42 — Scroll Y
    pub scx: u8,   // 0xFF43 — Scroll X
    pub ly: u8,    // 0xFF44 — Current scanline
    pub lyc: u8,   // 0xFF45 — LY Compare
    pub bgp: u8,   // 0xFF47 — BG Palette
    pub obp0: u8,  // 0xFF48 — Object Palette 0
    pub obp1: u8,  // 0xFF49 — Object Palette 1
    pub wy: u8,    // 0xFF4A — Window Y
    pub wx: u8,    // 0xFF4B — Window X

    pub dma: u8,   // 0xFF46 — DMA Transfer

    // Internal state
    cycles: u32,
    pub frame_ready: bool,
    pub interrupt_flags: u8,
    window_line: u8,
}

// PPU modes
const MODE_HBLANK: u8 = 0;
const MODE_VBLANK: u8 = 1;
const MODE_OAM: u8 = 2;
const MODE_TRANSFER: u8 = 3;

// LCDC bits
const LCDC_ENABLE: u8 = 7;
const LCDC_WIN_MAP: u8 = 6;
const LCDC_WIN_ENABLE: u8 = 5;
const LCDC_TILE_DATA: u8 = 4;
const LCDC_BG_MAP: u8 = 3;
const LCDC_OBJ_SIZE: u8 = 2;
const LCDC_OBJ_ENABLE: u8 = 1;
const LCDC_BG_ENABLE: u8 = 0;

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            vram: [0; 0x2000],
            oam: [0; 0xA0],
            framebuffer: [0; SCREEN_WIDTH * SCREEN_HEIGHT * 4],
            lcdc: 0x91,
            stat: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            dma: 0,
            cycles: 0,
            frame_ready: false,
            interrupt_flags: 0,
            window_line: 0,
        }
    }

    fn lcd_enabled(&self) -> bool {
        self.lcdc & (1 << LCDC_ENABLE) != 0
    }

    fn mode(&self) -> u8 {
        self.stat & 0x03
    }

    fn set_mode(&mut self, mode: u8) {
        self.stat = (self.stat & 0xFC) | (mode & 0x03);
    }

    /// Step PPU by given T-cycles.
    pub fn step(&mut self, cycles: u32) {
        self.interrupt_flags = 0;
        if !self.lcd_enabled() {
            return;
        }

        self.cycles += cycles;

        match self.mode() {
            // OAM scan (80 cycles)
            MODE_OAM => {
                if self.cycles >= 80 {
                    self.cycles -= 80;
                    self.set_mode(MODE_TRANSFER);
                }
            }
            // Pixel transfer (172 cycles)
            MODE_TRANSFER => {
                if self.cycles >= 172 {
                    self.cycles -= 172;
                    self.render_scanline();
                    self.set_mode(MODE_HBLANK);
                    if self.stat & 0x08 != 0 {
                        self.interrupt_flags |= 0x02; // STAT interrupt
                    }
                }
            }
            // HBlank (204 cycles)
            MODE_HBLANK => {
                if self.cycles >= 204 {
                    self.cycles -= 204;
                    self.ly += 1;
                    self.check_lyc();

                    if self.ly >= 144 {
                        self.set_mode(MODE_VBLANK);
                        self.interrupt_flags |= 0x01; // VBlank interrupt
                        self.frame_ready = true;
                        if self.stat & 0x10 != 0 {
                            self.interrupt_flags |= 0x02;
                        }
                    } else {
                        self.set_mode(MODE_OAM);
                        if self.stat & 0x20 != 0 {
                            self.interrupt_flags |= 0x02;
                        }
                    }
                }
            }
            // VBlank (10 lines, 456 cycles each)
            MODE_VBLANK => {
                if self.cycles >= 456 {
                    self.cycles -= 456;
                    self.ly += 1;
                    self.check_lyc();

                    if self.ly > 153 {
                        self.ly = 0;
                        self.window_line = 0;
                        self.set_mode(MODE_OAM);
                        if self.stat & 0x20 != 0 {
                            self.interrupt_flags |= 0x02;
                        }
                    }
                }
            }
            _ => {}
        }

    }

    fn check_lyc(&mut self) {
        if self.ly == self.lyc {
            self.stat |= 0x04; // LYC=LY flag
            if self.stat & 0x40 != 0 {
                self.interrupt_flags |= 0x02;
            }
        } else {
            self.stat &= !0x04;
        }
    }

    fn render_scanline(&mut self) {
        if self.ly >= SCREEN_HEIGHT as u8 {
            return;
        }

        // Background
        if self.lcdc & (1 << LCDC_BG_ENABLE) != 0 {
            self.render_bg();
        } else {
            // Clear line to white
            let offset = self.ly as usize * SCREEN_WIDTH * 4;
            for x in 0..SCREEN_WIDTH {
                let i = offset + x * 4;
                self.framebuffer[i] = 255;
                self.framebuffer[i + 1] = 255;
                self.framebuffer[i + 2] = 255;
                self.framebuffer[i + 3] = 255;
            }
        }

        // Window
        if self.lcdc & (1 << LCDC_WIN_ENABLE) != 0 {
            self.render_window();
        }

        // Sprites
        if self.lcdc & (1 << LCDC_OBJ_ENABLE) != 0 {
            self.render_sprites();
        }
    }

    fn render_bg(&mut self) {
        let tile_data_base: u16 = if self.lcdc & (1 << LCDC_TILE_DATA) != 0 { 0x0000 } else { 0x0800 };
        let tile_map_base: u16 = if self.lcdc & (1 << LCDC_BG_MAP) != 0 { 0x1C00 } else { 0x1800 };
        let signed = self.lcdc & (1 << LCDC_TILE_DATA) == 0;

        let y = self.ly.wrapping_add(self.scy);
        let tile_row = (y / 8) as u16;
        let pixel_row = (y % 8) as u16;

        for x in 0..SCREEN_WIDTH as u8 {
            let sx = x.wrapping_add(self.scx);
            let tile_col = (sx / 8) as u16;
            let pixel_col = (sx % 8) as u16;

            let map_offset = tile_map_base + tile_row * 32 + tile_col;
            let tile_idx = self.vram[map_offset as usize];

            let tile_addr = if signed {
                let signed_idx = tile_idx as i8 as i16;
                (tile_data_base as i16 + (signed_idx + 128) * 16) as u16
            } else {
                tile_data_base + tile_idx as u16 * 16
            };

            let byte1 = self.vram[(tile_addr + pixel_row * 2) as usize];
            let byte2 = self.vram[(tile_addr + pixel_row * 2 + 1) as usize];

            let bit = 7 - pixel_col as u8;
            let color_id = ((byte2 >> bit) & 1) << 1 | ((byte1 >> bit) & 1);
            let color = self.palette_color(self.bgp, color_id);

            let fb_idx = (self.ly as usize * SCREEN_WIDTH + x as usize) * 4;
            self.framebuffer[fb_idx] = color;
            self.framebuffer[fb_idx + 1] = color;
            self.framebuffer[fb_idx + 2] = color;
            self.framebuffer[fb_idx + 3] = 255;
        }
    }

    fn render_window(&mut self) {
        if self.ly < self.wy {
            return;
        }

        let tile_data_base: u16 = if self.lcdc & (1 << LCDC_TILE_DATA) != 0 { 0x0000 } else { 0x0800 };
        let tile_map_base: u16 = if self.lcdc & (1 << LCDC_WIN_MAP) != 0 { 0x1C00 } else { 0x1800 };
        let signed = self.lcdc & (1 << LCDC_TILE_DATA) == 0;

        let wx = self.wx.wrapping_sub(7);
        if wx >= SCREEN_WIDTH as u8 {
            return;
        }

        let y = self.window_line;
        let tile_row = (y / 8) as u16;
        let pixel_row = (y % 8) as u16;

        for x in wx..SCREEN_WIDTH as u8 {
            let win_x = (x - wx) as u16;
            let tile_col = win_x / 8;
            let pixel_col = win_x % 8;

            let map_offset = tile_map_base + tile_row * 32 + tile_col;
            let tile_idx = self.vram[map_offset as usize];

            let tile_addr = if signed {
                let signed_idx = tile_idx as i8 as i16;
                (tile_data_base as i16 + (signed_idx + 128) * 16) as u16
            } else {
                tile_data_base + tile_idx as u16 * 16
            };

            let byte1 = self.vram[(tile_addr + pixel_row * 2) as usize];
            let byte2 = self.vram[(tile_addr + pixel_row * 2 + 1) as usize];

            let bit = 7 - pixel_col as u8;
            let color_id = ((byte2 >> bit) & 1) << 1 | ((byte1 >> bit) & 1);
            let color = self.palette_color(self.bgp, color_id);

            let fb_idx = (self.ly as usize * SCREEN_WIDTH + x as usize) * 4;
            self.framebuffer[fb_idx] = color;
            self.framebuffer[fb_idx + 1] = color;
            self.framebuffer[fb_idx + 2] = color;
            self.framebuffer[fb_idx + 3] = 255;
        }

        self.window_line += 1;
    }

    fn render_sprites(&mut self) {
        let tall = self.lcdc & (1 << LCDC_OBJ_SIZE) != 0;
        let height = if tall { 16 } else { 8 };

        // Collect visible sprites (max 10 per line)
        let mut sprites: Vec<(u8, u8, u8, u8, usize)> = Vec::new();
        for i in 0..40 {
            let offset = i * 4;
            let sy = self.oam[offset].wrapping_sub(16);
            let sx = self.oam[offset + 1].wrapping_sub(8);
            let tile = self.oam[offset + 2];
            let flags = self.oam[offset + 3];

            if self.ly >= sy && self.ly < sy.wrapping_add(height) {
                sprites.push((sx, sy, tile, flags, i));
                if sprites.len() >= 10 {
                    break;
                }
            }
        }

        // Draw in reverse order (lower index = higher priority)
        for &(sx, sy, tile, flags, _) in sprites.iter().rev() {
            let palette = if flags & 0x10 != 0 { self.obp1 } else { self.obp0 };
            let flip_x = flags & 0x20 != 0;
            let flip_y = flags & 0x40 != 0;
            let behind_bg = flags & 0x80 != 0;

            let mut row = self.ly.wrapping_sub(sy) as u16;
            if flip_y {
                row = (height as u16 - 1) - row;
            }

            let tile_idx = if tall { tile & 0xFE } else { tile };
            let tile_addr = tile_idx as u16 * 16 + row * 2;

            if tile_addr as usize + 1 >= self.vram.len() {
                continue;
            }

            let byte1 = self.vram[tile_addr as usize];
            let byte2 = self.vram[(tile_addr + 1) as usize];

            for px in 0..8u8 {
                let screen_x = sx.wrapping_add(px);
                if screen_x >= SCREEN_WIDTH as u8 {
                    continue;
                }

                let bit = if flip_x { px } else { 7 - px };
                let color_id = ((byte2 >> bit) & 1) << 1 | ((byte1 >> bit) & 1);

                if color_id == 0 {
                    continue; // Transparent
                }

                let fb_idx = (self.ly as usize * SCREEN_WIDTH + screen_x as usize) * 4;

                // Behind BG: only draw if BG pixel is color 0 (white)
                if behind_bg && self.framebuffer[fb_idx] != 255 {
                    continue;
                }

                let color = self.palette_color(palette, color_id);
                self.framebuffer[fb_idx] = color;
                self.framebuffer[fb_idx + 1] = color;
                self.framebuffer[fb_idx + 2] = color;
                self.framebuffer[fb_idx + 3] = 255;
            }
        }
    }

    fn palette_color(&self, palette: u8, color_id: u8) -> u8 {
        let shade = (palette >> (color_id * 2)) & 0x03;
        match shade {
            0 => 255,  // White
            1 => 170,  // Light gray
            2 => 85,   // Dark gray
            3 => 0,    // Black
            _ => 255,
        }
    }
}
