use crate::bus::Bus;

/// Sharp SM83 CPU (often called "Game Boy Z80")
pub struct Cpu {
    /// Registers
    pub a: u8,
    pub f: u8, // Flags: Z N H C 0 0 0 0
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,

    /// Interrupt Master Enable
    pub ime: bool,
    /// IME enable scheduled (EI delays by one instruction)
    pub ime_scheduled: bool,
    /// CPU halted (waiting for interrupt)
    pub halted: bool,
}

// Flag bit positions
const FLAG_Z: u8 = 7;
const FLAG_N: u8 = 6;
const FLAG_H: u8 = 5;
const FLAG_C: u8 = 4;

impl Cpu {
    pub fn new() -> Self {
        // Post-boot ROM state (DMG)
        Cpu {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100, // Skip boot ROM
            ime: true,
            ime_scheduled: false,
            halted: false,
        }
    }

    // Flag helpers
    fn flag(&self, bit: u8) -> bool {
        self.f & (1 << bit) != 0
    }
    fn set_flag(&mut self, bit: u8, val: bool) {
        if val {
            self.f |= 1 << bit;
        } else {
            self.f &= !(1 << bit);
        }
    }

    pub fn flag_z(&self) -> bool { self.flag(FLAG_Z) }
    pub fn flag_n(&self) -> bool { self.flag(FLAG_N) }
    pub fn flag_h(&self) -> bool { self.flag(FLAG_H) }
    pub fn flag_c(&self) -> bool { self.flag(FLAG_C) }

    // 16-bit register pairs
    fn af(&self) -> u16 { (self.a as u16) << 8 | self.f as u16 }
    fn bc(&self) -> u16 { (self.b as u16) << 8 | self.c as u16 }
    fn de(&self) -> u16 { (self.d as u16) << 8 | self.e as u16 }
    fn hl(&self) -> u16 { (self.h as u16) << 8 | self.l as u16 }

    fn set_af(&mut self, v: u16) { self.a = (v >> 8) as u8; self.f = (v & 0xF0) as u8; }
    fn set_bc(&mut self, v: u16) { self.b = (v >> 8) as u8; self.c = v as u8; }
    fn set_de(&mut self, v: u16) { self.d = (v >> 8) as u8; self.e = v as u8; }
    fn set_hl(&mut self, v: u16) { self.h = (v >> 8) as u8; self.l = v as u8; }

    // Fetch byte at PC and advance
    fn fetch(&mut self, bus: &Bus) -> u8 {
        let v = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    // Fetch 16-bit value (little-endian)
    fn fetch16(&mut self, bus: &Bus) -> u16 {
        let lo = self.fetch(bus) as u16;
        let hi = self.fetch(bus) as u16;
        hi << 8 | lo
    }

    /// Handle interrupts. Returns cycles consumed (0 if no interrupt).
    fn handle_interrupts(&mut self, bus: &mut Bus) -> u32 {
        if !self.ime && !self.halted {
            return 0;
        }

        // IE & IF
        let triggered = bus.read(0xFFFF) & bus.read(0xFF0F) & 0x1F;
        if triggered == 0 {
            return 0;
        }

        self.halted = false;

        if !self.ime {
            return 0;
        }

        self.ime = false;

        // Priority: bit 0 (VBlank) to bit 4 (Joypad)
        let bit = triggered.trailing_zeros() as u8;
        // Clear IF flag
        let if_val = bus.read(0xFF0F);
        bus.write(0xFF0F, if_val & !(1 << bit));

        // Push PC and jump to interrupt vector
        self.sp = self.sp.wrapping_sub(2);
        bus.write(self.sp, self.pc as u8);
        bus.write(self.sp.wrapping_add(1), (self.pc >> 8) as u8);
        self.pc = 0x0040 + (bit as u16) * 8;

        20 // interrupt dispatch takes 20 cycles
    }

    /// Execute one instruction. Returns cycles consumed (T-cycles).
    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        // Handle pending EI
        if self.ime_scheduled {
            self.ime_scheduled = false;
            self.ime = true;
        }

        // Check interrupts
        let int_cycles = self.handle_interrupts(bus);
        if int_cycles > 0 {
            return int_cycles;
        }

        if self.halted {
            return 4;
        }

        let opcode = self.fetch(bus);
        self.execute(opcode, bus)
    }

    fn execute(&mut self, opcode: u8, bus: &mut Bus) -> u32 {
        match opcode {
            // NOP
            0x00 => 4,

            // LD BC,d16
            0x01 => { let v = self.fetch16(bus); self.set_bc(v); 12 }
            // LD (BC),A
            0x02 => { bus.write(self.bc(), self.a); 8 }
            // INC BC
            0x03 => { let v = self.bc().wrapping_add(1); self.set_bc(v); 8 }
            // INC B
            0x04 => { self.b = self.inc(self.b); 4 }
            // DEC B
            0x05 => { self.b = self.dec(self.b); 4 }
            // LD B,d8
            0x06 => { self.b = self.fetch(bus); 8 }
            // RLCA
            0x07 => {
                let c = self.a >> 7;
                self.a = (self.a << 1) | c;
                self.f = 0;
                self.set_flag(FLAG_C, c != 0);
                4
            }
            // LD (a16),SP
            0x08 => {
                let addr = self.fetch16(bus);
                bus.write(addr, self.sp as u8);
                bus.write(addr.wrapping_add(1), (self.sp >> 8) as u8);
                20
            }
            // ADD HL,BC
            0x09 => { self.add_hl(self.bc()); 8 }
            // LD A,(BC)
            0x0A => { self.a = bus.read(self.bc()); 8 }
            // DEC BC
            0x0B => { let v = self.bc().wrapping_sub(1); self.set_bc(v); 8 }
            // INC C
            0x0C => { self.c = self.inc(self.c); 4 }
            // DEC C
            0x0D => { self.c = self.dec(self.c); 4 }
            // LD C,d8
            0x0E => { self.c = self.fetch(bus); 8 }
            // RRCA
            0x0F => {
                let c = self.a & 1;
                self.a = (self.a >> 1) | (c << 7);
                self.f = 0;
                self.set_flag(FLAG_C, c != 0);
                4
            }

            // STOP
            0x10 => { self.fetch(bus); 4 }

            // LD DE,d16
            0x11 => { let v = self.fetch16(bus); self.set_de(v); 12 }
            // LD (DE),A
            0x12 => { bus.write(self.de(), self.a); 8 }
            // INC DE
            0x13 => { let v = self.de().wrapping_add(1); self.set_de(v); 8 }
            // INC D
            0x14 => { self.d = self.inc(self.d); 4 }
            // DEC D
            0x15 => { self.d = self.dec(self.d); 4 }
            // LD D,d8
            0x16 => { self.d = self.fetch(bus); 8 }
            // RLA
            0x17 => {
                let c = self.flag_c() as u8;
                let new_c = self.a >> 7;
                self.a = (self.a << 1) | c;
                self.f = 0;
                self.set_flag(FLAG_C, new_c != 0);
                4
            }
            // JR r8
            0x18 => { let r = self.fetch(bus) as i8; self.pc = self.pc.wrapping_add(r as u16); 12 }
            // ADD HL,DE
            0x19 => { self.add_hl(self.de()); 8 }
            // LD A,(DE)
            0x1A => { self.a = bus.read(self.de()); 8 }
            // DEC DE
            0x1B => { let v = self.de().wrapping_sub(1); self.set_de(v); 8 }
            // INC E
            0x1C => { self.e = self.inc(self.e); 4 }
            // DEC E
            0x1D => { self.e = self.dec(self.e); 4 }
            // LD E,d8
            0x1E => { self.e = self.fetch(bus); 8 }
            // RRA
            0x1F => {
                let c = self.flag_c() as u8;
                let new_c = self.a & 1;
                self.a = (self.a >> 1) | (c << 7);
                self.f = 0;
                self.set_flag(FLAG_C, new_c != 0);
                4
            }

            // JR NZ,r8
            0x20 => { let r = self.fetch(bus) as i8; if !self.flag_z() { self.pc = self.pc.wrapping_add(r as u16); 12 } else { 8 } }
            // LD HL,d16
            0x21 => { let v = self.fetch16(bus); self.set_hl(v); 12 }
            // LD (HL+),A
            0x22 => { let hl = self.hl(); bus.write(hl, self.a); self.set_hl(hl.wrapping_add(1)); 8 }
            // INC HL
            0x23 => { let v = self.hl().wrapping_add(1); self.set_hl(v); 8 }
            // INC H
            0x24 => { self.h = self.inc(self.h); 4 }
            // DEC H
            0x25 => { self.h = self.dec(self.h); 4 }
            // LD H,d8
            0x26 => { self.h = self.fetch(bus); 8 }
            // DAA
            0x27 => { self.daa(); 4 }
            // JR Z,r8
            0x28 => { let r = self.fetch(bus) as i8; if self.flag_z() { self.pc = self.pc.wrapping_add(r as u16); 12 } else { 8 } }
            // ADD HL,HL
            0x29 => { let hl = self.hl(); self.add_hl(hl); 8 }
            // LD A,(HL+)
            0x2A => { let hl = self.hl(); self.a = bus.read(hl); self.set_hl(hl.wrapping_add(1)); 8 }
            // DEC HL
            0x2B => { let v = self.hl().wrapping_sub(1); self.set_hl(v); 8 }
            // INC L
            0x2C => { self.l = self.inc(self.l); 4 }
            // DEC L
            0x2D => { self.l = self.dec(self.l); 4 }
            // LD L,d8
            0x2E => { self.l = self.fetch(bus); 8 }
            // CPL
            0x2F => {
                self.a = !self.a;
                self.set_flag(FLAG_N, true);
                self.set_flag(FLAG_H, true);
                4
            }

            // JR NC,r8
            0x30 => { let r = self.fetch(bus) as i8; if !self.flag_c() { self.pc = self.pc.wrapping_add(r as u16); 12 } else { 8 } }
            // LD SP,d16
            0x31 => { self.sp = self.fetch16(bus); 12 }
            // LD (HL-),A
            0x32 => { let hl = self.hl(); bus.write(hl, self.a); self.set_hl(hl.wrapping_sub(1)); 8 }
            // INC SP
            0x33 => { self.sp = self.sp.wrapping_add(1); 8 }
            // INC (HL)
            0x34 => { let hl = self.hl(); let v = self.inc(bus.read(hl)); bus.write(hl, v); 12 }
            // DEC (HL)
            0x35 => { let hl = self.hl(); let v = self.dec(bus.read(hl)); bus.write(hl, v); 12 }
            // LD (HL),d8
            0x36 => { let v = self.fetch(bus); bus.write(self.hl(), v); 12 }
            // SCF
            0x37 => {
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, true);
                4
            }
            // JR C,r8
            0x38 => { let r = self.fetch(bus) as i8; if self.flag_c() { self.pc = self.pc.wrapping_add(r as u16); 12 } else { 8 } }
            // ADD HL,SP
            0x39 => { self.add_hl(self.sp); 8 }
            // LD A,(HL-)
            0x3A => { let hl = self.hl(); self.a = bus.read(hl); self.set_hl(hl.wrapping_sub(1)); 8 }
            // DEC SP
            0x3B => { self.sp = self.sp.wrapping_sub(1); 8 }
            // INC A
            0x3C => { self.a = self.inc(self.a); 4 }
            // DEC A
            0x3D => { self.a = self.dec(self.a); 4 }
            // LD A,d8
            0x3E => { self.a = self.fetch(bus); 8 }
            // CCF
            0x3F => {
                let c = !self.flag_c();
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, false);
                self.set_flag(FLAG_C, c);
                4
            }

            // LD B,r  (0x40-0x47)
            0x40 => 4, // LD B,B
            0x41 => { self.b = self.c; 4 }
            0x42 => { self.b = self.d; 4 }
            0x43 => { self.b = self.e; 4 }
            0x44 => { self.b = self.h; 4 }
            0x45 => { self.b = self.l; 4 }
            0x46 => { self.b = bus.read(self.hl()); 8 }
            0x47 => { self.b = self.a; 4 }

            // LD C,r  (0x48-0x4F)
            0x48 => { self.c = self.b; 4 }
            0x49 => 4,
            0x4A => { self.c = self.d; 4 }
            0x4B => { self.c = self.e; 4 }
            0x4C => { self.c = self.h; 4 }
            0x4D => { self.c = self.l; 4 }
            0x4E => { self.c = bus.read(self.hl()); 8 }
            0x4F => { self.c = self.a; 4 }

            // LD D,r  (0x50-0x57)
            0x50 => { self.d = self.b; 4 }
            0x51 => { self.d = self.c; 4 }
            0x52 => 4,
            0x53 => { self.d = self.e; 4 }
            0x54 => { self.d = self.h; 4 }
            0x55 => { self.d = self.l; 4 }
            0x56 => { self.d = bus.read(self.hl()); 8 }
            0x57 => { self.d = self.a; 4 }

            // LD E,r  (0x58-0x5F)
            0x58 => { self.e = self.b; 4 }
            0x59 => { self.e = self.c; 4 }
            0x5A => { self.e = self.d; 4 }
            0x5B => 4,
            0x5C => { self.e = self.h; 4 }
            0x5D => { self.e = self.l; 4 }
            0x5E => { self.e = bus.read(self.hl()); 8 }
            0x5F => { self.e = self.a; 4 }

            // LD H,r  (0x60-0x67)
            0x60 => { self.h = self.b; 4 }
            0x61 => { self.h = self.c; 4 }
            0x62 => { self.h = self.d; 4 }
            0x63 => { self.h = self.e; 4 }
            0x64 => 4,
            0x65 => { self.h = self.l; 4 }
            0x66 => { self.h = bus.read(self.hl()); 8 }
            0x67 => { self.h = self.a; 4 }

            // LD L,r  (0x68-0x6F)
            0x68 => { self.l = self.b; 4 }
            0x69 => { self.l = self.c; 4 }
            0x6A => { self.l = self.d; 4 }
            0x6B => { self.l = self.e; 4 }
            0x6C => { self.l = self.h; 4 }
            0x6D => 4,
            0x6E => { self.l = bus.read(self.hl()); 8 }
            0x6F => { self.l = self.a; 4 }

            // LD (HL),r  (0x70-0x75, 0x77)
            0x70 => { bus.write(self.hl(), self.b); 8 }
            0x71 => { bus.write(self.hl(), self.c); 8 }
            0x72 => { bus.write(self.hl(), self.d); 8 }
            0x73 => { bus.write(self.hl(), self.e); 8 }
            0x74 => { bus.write(self.hl(), self.h); 8 }
            0x75 => { bus.write(self.hl(), self.l); 8 }
            // HALT
            0x76 => { self.halted = true; 4 }
            0x77 => { bus.write(self.hl(), self.a); 8 }

            // LD A,r  (0x78-0x7F)
            0x78 => { self.a = self.b; 4 }
            0x79 => { self.a = self.c; 4 }
            0x7A => { self.a = self.d; 4 }
            0x7B => { self.a = self.e; 4 }
            0x7C => { self.a = self.h; 4 }
            0x7D => { self.a = self.l; 4 }
            0x7E => { self.a = bus.read(self.hl()); 8 }
            0x7F => 4, // LD A,A

            // ADD A,r  (0x80-0x87)
            0x80 => { self.add(self.b); 4 }
            0x81 => { self.add(self.c); 4 }
            0x82 => { self.add(self.d); 4 }
            0x83 => { self.add(self.e); 4 }
            0x84 => { self.add(self.h); 4 }
            0x85 => { self.add(self.l); 4 }
            0x86 => { self.add(bus.read(self.hl())); 8 }
            0x87 => { self.add(self.a); 4 }

            // ADC A,r  (0x88-0x8F)
            0x88 => { self.adc(self.b); 4 }
            0x89 => { self.adc(self.c); 4 }
            0x8A => { self.adc(self.d); 4 }
            0x8B => { self.adc(self.e); 4 }
            0x8C => { self.adc(self.h); 4 }
            0x8D => { self.adc(self.l); 4 }
            0x8E => { self.adc(bus.read(self.hl())); 8 }
            0x8F => { self.adc(self.a); 4 }

            // SUB r  (0x90-0x97)
            0x90 => { self.sub(self.b); 4 }
            0x91 => { self.sub(self.c); 4 }
            0x92 => { self.sub(self.d); 4 }
            0x93 => { self.sub(self.e); 4 }
            0x94 => { self.sub(self.h); 4 }
            0x95 => { self.sub(self.l); 4 }
            0x96 => { self.sub(bus.read(self.hl())); 8 }
            0x97 => { self.sub(self.a); 4 }

            // SBC A,r  (0x98-0x9F)
            0x98 => { self.sbc(self.b); 4 }
            0x99 => { self.sbc(self.c); 4 }
            0x9A => { self.sbc(self.d); 4 }
            0x9B => { self.sbc(self.e); 4 }
            0x9C => { self.sbc(self.h); 4 }
            0x9D => { self.sbc(self.l); 4 }
            0x9E => { self.sbc(bus.read(self.hl())); 8 }
            0x9F => { self.sbc(self.a); 4 }

            // AND r  (0xA0-0xA7)
            0xA0 => { self.and(self.b); 4 }
            0xA1 => { self.and(self.c); 4 }
            0xA2 => { self.and(self.d); 4 }
            0xA3 => { self.and(self.e); 4 }
            0xA4 => { self.and(self.h); 4 }
            0xA5 => { self.and(self.l); 4 }
            0xA6 => { self.and(bus.read(self.hl())); 8 }
            0xA7 => { self.and(self.a); 4 }

            // XOR r  (0xA8-0xAF)
            0xA8 => { self.xor(self.b); 4 }
            0xA9 => { self.xor(self.c); 4 }
            0xAA => { self.xor(self.d); 4 }
            0xAB => { self.xor(self.e); 4 }
            0xAC => { self.xor(self.h); 4 }
            0xAD => { self.xor(self.l); 4 }
            0xAE => { self.xor(bus.read(self.hl())); 8 }
            0xAF => { self.xor(self.a); 4 }

            // OR r  (0xB0-0xB7)
            0xB0 => { self.or(self.b); 4 }
            0xB1 => { self.or(self.c); 4 }
            0xB2 => { self.or(self.d); 4 }
            0xB3 => { self.or(self.e); 4 }
            0xB4 => { self.or(self.h); 4 }
            0xB5 => { self.or(self.l); 4 }
            0xB6 => { self.or(bus.read(self.hl())); 8 }
            0xB7 => { self.or(self.a); 4 }

            // CP r  (0xB8-0xBF)
            0xB8 => { self.cp(self.b); 4 }
            0xB9 => { self.cp(self.c); 4 }
            0xBA => { self.cp(self.d); 4 }
            0xBB => { self.cp(self.e); 4 }
            0xBC => { self.cp(self.h); 4 }
            0xBD => { self.cp(self.l); 4 }
            0xBE => { self.cp(bus.read(self.hl())); 8 }
            0xBF => { self.cp(self.a); 4 }

            // RET NZ
            0xC0 => { if !self.flag_z() { self.pc = self.pop(bus); 20 } else { 8 } }
            // POP BC
            0xC1 => { let v = self.pop(bus); self.set_bc(v); 12 }
            // JP NZ,a16
            0xC2 => { let addr = self.fetch16(bus); if !self.flag_z() { self.pc = addr; 16 } else { 12 } }
            // JP a16
            0xC3 => { self.pc = self.fetch16(bus); 16 }
            // CALL NZ,a16
            0xC4 => { let addr = self.fetch16(bus); if !self.flag_z() { self.push(bus, self.pc); self.pc = addr; 24 } else { 12 } }
            // PUSH BC
            0xC5 => { self.push(bus, self.bc()); 16 }
            // ADD A,d8
            0xC6 => { let v = self.fetch(bus); self.add(v); 8 }
            // RST 00H
            0xC7 => { self.push(bus, self.pc); self.pc = 0x00; 16 }
            // RET Z
            0xC8 => { if self.flag_z() { self.pc = self.pop(bus); 20 } else { 8 } }
            // RET
            0xC9 => { self.pc = self.pop(bus); 16 }
            // JP Z,a16
            0xCA => { let addr = self.fetch16(bus); if self.flag_z() { self.pc = addr; 16 } else { 12 } }
            // CB prefix
            0xCB => { let op = self.fetch(bus); self.execute_cb(op, bus) }
            // CALL Z,a16
            0xCC => { let addr = self.fetch16(bus); if self.flag_z() { self.push(bus, self.pc); self.pc = addr; 24 } else { 12 } }
            // CALL a16
            0xCD => { let addr = self.fetch16(bus); self.push(bus, self.pc); self.pc = addr; 24 }
            // ADC A,d8
            0xCE => { let v = self.fetch(bus); self.adc(v); 8 }
            // RST 08H
            0xCF => { self.push(bus, self.pc); self.pc = 0x08; 16 }

            // RET NC
            0xD0 => { if !self.flag_c() { self.pc = self.pop(bus); 20 } else { 8 } }
            // POP DE
            0xD1 => { let v = self.pop(bus); self.set_de(v); 12 }
            // JP NC,a16
            0xD2 => { let addr = self.fetch16(bus); if !self.flag_c() { self.pc = addr; 16 } else { 12 } }
            // CALL NC,a16
            0xD4 => { let addr = self.fetch16(bus); if !self.flag_c() { self.push(bus, self.pc); self.pc = addr; 24 } else { 12 } }
            // PUSH DE
            0xD5 => { self.push(bus, self.de()); 16 }
            // SUB d8
            0xD6 => { let v = self.fetch(bus); self.sub(v); 8 }
            // RST 10H
            0xD7 => { self.push(bus, self.pc); self.pc = 0x10; 16 }
            // RET C
            0xD8 => { if self.flag_c() { self.pc = self.pop(bus); 20 } else { 8 } }
            // RETI
            0xD9 => { self.pc = self.pop(bus); self.ime = true; 16 }
            // JP C,a16
            0xDA => { let addr = self.fetch16(bus); if self.flag_c() { self.pc = addr; 16 } else { 12 } }
            // CALL C,a16
            0xDC => { let addr = self.fetch16(bus); if self.flag_c() { self.push(bus, self.pc); self.pc = addr; 24 } else { 12 } }
            // SBC A,d8
            0xDE => { let v = self.fetch(bus); self.sbc(v); 8 }
            // RST 18H
            0xDF => { self.push(bus, self.pc); self.pc = 0x18; 16 }

            // LDH (a8),A
            0xE0 => { let a = self.fetch(bus); bus.write(0xFF00 | a as u16, self.a); 12 }
            // POP HL
            0xE1 => { let v = self.pop(bus); self.set_hl(v); 12 }
            // LD (C),A
            0xE2 => { bus.write(0xFF00 | self.c as u16, self.a); 8 }
            // PUSH HL
            0xE5 => { self.push(bus, self.hl()); 16 }
            // AND d8
            0xE6 => { let v = self.fetch(bus); self.and(v); 8 }
            // RST 20H
            0xE7 => { self.push(bus, self.pc); self.pc = 0x20; 16 }
            // ADD SP,r8
            0xE8 => {
                let r = self.fetch(bus) as i8 as i16 as u16;
                let sp = self.sp;
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, (sp & 0xF) + (r & 0xF) > 0xF);
                self.set_flag(FLAG_C, (sp & 0xFF) + (r & 0xFF) > 0xFF);
                self.sp = sp.wrapping_add(r);
                16
            }
            // JP (HL)
            0xE9 => { self.pc = self.hl(); 4 }
            // LD (a16),A
            0xEA => { let addr = self.fetch16(bus); bus.write(addr, self.a); 16 }
            // XOR d8
            0xEE => { let v = self.fetch(bus); self.xor(v); 8 }
            // RST 28H
            0xEF => { self.push(bus, self.pc); self.pc = 0x28; 16 }

            // LDH A,(a8)
            0xF0 => { let a = self.fetch(bus); self.a = bus.read(0xFF00 | a as u16); 12 }
            // POP AF
            0xF1 => { let v = self.pop(bus); self.set_af(v); 12 }
            // LD A,(C)
            0xF2 => { self.a = bus.read(0xFF00 | self.c as u16); 8 }
            // DI
            0xF3 => { self.ime = false; 4 }
            // PUSH AF
            0xF5 => { self.push(bus, self.af()); 16 }
            // OR d8
            0xF6 => { let v = self.fetch(bus); self.or(v); 8 }
            // RST 30H
            0xF7 => { self.push(bus, self.pc); self.pc = 0x30; 16 }
            // LD HL,SP+r8
            0xF8 => {
                let r = self.fetch(bus) as i8 as i16 as u16;
                let sp = self.sp;
                self.set_flag(FLAG_Z, false);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, (sp & 0xF) + (r & 0xF) > 0xF);
                self.set_flag(FLAG_C, (sp & 0xFF) + (r & 0xFF) > 0xFF);
                self.set_hl(sp.wrapping_add(r));
                12
            }
            // LD SP,HL
            0xF9 => { self.sp = self.hl(); 8 }
            // LD A,(a16)
            0xFA => { let addr = self.fetch16(bus); self.a = bus.read(addr); 16 }
            // EI
            0xFB => { self.ime_scheduled = true; 4 }
            // CP d8
            0xFE => { let v = self.fetch(bus); self.cp(v); 8 }
            // RST 38H
            0xFF => { self.push(bus, self.pc); self.pc = 0x38; 16 }

            _ => { 4 } // Undefined opcodes: NOP behavior
        }
    }

    // CB-prefixed instructions
    fn execute_cb(&mut self, opcode: u8, bus: &mut Bus) -> u32 {
        let reg_idx = opcode & 0x07;
        let val = self.read_reg(reg_idx, bus);
        let is_hl = reg_idx == 6;
        let cycles = if is_hl { 16 } else { 8 };

        let result = match opcode >> 3 {
            // RLC
            0 => { let c = val >> 7; let r = (val << 1) | c; self.set_flag(FLAG_Z, r == 0); self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, false); self.set_flag(FLAG_C, c != 0); r }
            // RRC
            1 => { let c = val & 1; let r = (val >> 1) | (c << 7); self.set_flag(FLAG_Z, r == 0); self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, false); self.set_flag(FLAG_C, c != 0); r }
            // RL
            2 => { let c = self.flag_c() as u8; let new_c = val >> 7; let r = (val << 1) | c; self.set_flag(FLAG_Z, r == 0); self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, false); self.set_flag(FLAG_C, new_c != 0); r }
            // RR
            3 => { let c = self.flag_c() as u8; let new_c = val & 1; let r = (val >> 1) | (c << 7); self.set_flag(FLAG_Z, r == 0); self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, false); self.set_flag(FLAG_C, new_c != 0); r }
            // SLA
            4 => { let c = val >> 7; let r = val << 1; self.set_flag(FLAG_Z, r == 0); self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, false); self.set_flag(FLAG_C, c != 0); r }
            // SRA
            5 => { let c = val & 1; let r = (val >> 1) | (val & 0x80); self.set_flag(FLAG_Z, r == 0); self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, false); self.set_flag(FLAG_C, c != 0); r }
            // SWAP
            6 => { let r = (val >> 4) | (val << 4); self.set_flag(FLAG_Z, r == 0); self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, false); self.set_flag(FLAG_C, false); r }
            // SRL
            7 => { let c = val & 1; let r = val >> 1; self.set_flag(FLAG_Z, r == 0); self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, false); self.set_flag(FLAG_C, c != 0); r }
            // BIT
            8..=15 => {
                let bit = (opcode >> 3) - 8;
                self.set_flag(FLAG_Z, val & (1 << bit) == 0);
                self.set_flag(FLAG_N, false);
                self.set_flag(FLAG_H, true);
                return if is_hl { 12 } else { 8 };
            }
            // RES
            16..=23 => {
                let bit = (opcode >> 3) - 16;
                val & !(1 << bit)
            }
            // SET
            24..=31 => {
                let bit = (opcode >> 3) - 24;
                val | (1 << bit)
            }
            _ => unreachable!(),
        };

        self.write_reg(reg_idx, result, bus);
        cycles
    }

    fn read_reg(&self, idx: u8, bus: &Bus) -> u8 {
        match idx {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => bus.read(self.hl()),
            7 => self.a,
            _ => unreachable!(),
        }
    }

    fn write_reg(&mut self, idx: u8, val: u8, bus: &mut Bus) {
        match idx {
            0 => self.b = val,
            1 => self.c = val,
            2 => self.d = val,
            3 => self.e = val,
            4 => self.h = val,
            5 => self.l = val,
            6 => bus.write(self.hl(), val),
            7 => self.a = val,
            _ => unreachable!(),
        }
    }

    // ALU operations
    fn add(&mut self, val: u8) {
        let (result, carry) = self.a.overflowing_add(val);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (self.a & 0xF) + (val & 0xF) > 0xF);
        self.set_flag(FLAG_C, carry);
        self.a = result;
    }

    fn adc(&mut self, val: u8) {
        let c = self.flag_c() as u8;
        let result = self.a.wrapping_add(val).wrapping_add(c);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (self.a & 0xF) + (val & 0xF) + c > 0xF);
        self.set_flag(FLAG_C, (self.a as u16) + (val as u16) + (c as u16) > 0xFF);
        self.a = result;
    }

    fn sub(&mut self, val: u8) {
        let result = self.a.wrapping_sub(val);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (self.a & 0xF) < (val & 0xF));
        self.set_flag(FLAG_C, self.a < val);
        self.a = result;
    }

    fn sbc(&mut self, val: u8) {
        let c = self.flag_c() as u8;
        let result = self.a.wrapping_sub(val).wrapping_sub(c);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (self.a & 0xF) < (val & 0xF) + c);
        self.set_flag(FLAG_C, (self.a as u16) < (val as u16) + (c as u16));
        self.a = result;
    }

    fn and(&mut self, val: u8) {
        self.a &= val;
        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, true);
        self.set_flag(FLAG_C, false);
    }

    fn xor(&mut self, val: u8) {
        self.a ^= val;
        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, false);
    }

    fn or(&mut self, val: u8) {
        self.a |= val;
        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, false);
    }

    fn cp(&mut self, val: u8) {
        self.set_flag(FLAG_Z, self.a == val);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (self.a & 0xF) < (val & 0xF));
        self.set_flag(FLAG_C, self.a < val);
    }

    fn inc(&mut self, val: u8) -> u8 {
        let result = val.wrapping_add(1);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (val & 0xF) == 0xF);
        result
    }

    fn dec(&mut self, val: u8) -> u8 {
        let result = val.wrapping_sub(1);
        self.set_flag(FLAG_Z, result == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (val & 0xF) == 0);
        result
    }

    fn add_hl(&mut self, val: u16) {
        let hl = self.hl();
        let (result, carry) = hl.overflowing_add(val);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (hl & 0xFFF) + (val & 0xFFF) > 0xFFF);
        self.set_flag(FLAG_C, carry);
        self.set_hl(result);
    }

    fn daa(&mut self) {
        let mut adjust = 0u8;
        let mut carry = false;

        if self.flag_n() {
            if self.flag_c() { adjust |= 0x60; carry = true; }
            if self.flag_h() { adjust |= 0x06; }
            self.a = self.a.wrapping_sub(adjust);
        } else {
            if self.flag_c() || self.a > 0x99 { adjust |= 0x60; carry = true; }
            if self.flag_h() || (self.a & 0x0F) > 0x09 { adjust |= 0x06; }
            self.a = self.a.wrapping_add(adjust);
        }

        self.set_flag(FLAG_Z, self.a == 0);
        self.set_flag(FLAG_H, false);
        self.set_flag(FLAG_C, carry);
    }

    fn push(&mut self, bus: &mut Bus, val: u16) {
        self.sp = self.sp.wrapping_sub(2);
        bus.write(self.sp, val as u8);
        bus.write(self.sp.wrapping_add(1), (val >> 8) as u8);
    }

    fn pop(&mut self, bus: &Bus) -> u16 {
        let lo = bus.read(self.sp) as u16;
        let hi = bus.read(self.sp.wrapping_add(1)) as u16;
        self.sp = self.sp.wrapping_add(2);
        hi << 8 | lo
    }
}
