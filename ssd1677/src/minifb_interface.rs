// Minifb render backend for the SSD1677.
// Behaves identically to PbmInterface (RAM banks, LUT, buffer swap) but
// renders the result to a minifb window with ghosting and flash effects.

use core::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use minifb::Window;
use crate::DisplayInterface;

// ── Coordinate space constants ───────────────────────────────────────────
//
// Portrait space (user-facing): 480 wide × 800 tall.
// Landscape space (RAM layout):  800 wide × 480 tall.
//
// Suffix naming convention:
//   _p  = portrait coordinate
//   _l  = landscape coordinate
//   _px = in portrait X range (0..480)
//   _py = in portrait Y range (0..800)
//   _lx = in landscape X range (0..800)
//   _ly = in landscape Y range (0..480)

const PORTRAIT_W: usize = 480;
const PORTRAIT_H: usize = 800;

const LANDSCAPE_W: usize = PORTRAIT_H; // 800
const LANDSCAPE_H: usize = PORTRAIT_W; // 480
const LANDSCAPE_BYTES: usize = LANDSCAPE_W * LANDSCAPE_H / 8; // 48000

const FLASH_CYCLES: usize = 3;
const FLASH_PHASE_MS: u64 = 80;

const BLACK: u32 = 0x001C1C1A;
const WHITE: u32 = 0x00F0ECD8;

const CMD_WRITE_RAM_BW: u8 = 0x24;
const CMD_WRITE_RAM_RED: u8 = 0x26;
const CMD_AUTO_WRITE_BW: u8 = 0x46;
const CMD_AUTO_WRITE_RED: u8 = 0x47;
const CMD_READ_RAM: u8 = 0x27;
const CMD_SET_RAM_X_COUNTER: u8 = 0x4E;
const CMD_SET_RAM_Y_COUNTER: u8 = 0x4F;
const CMD_SET_RAM_X_RANGE: u8 = 0x44;
const CMD_SET_RAM_Y_RANGE: u8 = 0x45;
const CMD_DATA_ENTRY_MODE: u8 = 0x11;
const CMD_DISPLAY_UPDATE_CTRL1: u8 = 0x21;
const CMD_MASTER_ACTIVATION: u8 = 0x20;

const CTRL1_BYPASS_RED: u8 = 0x40;

const DARK_GHOST: u8 = 60;
const LIGHT_GHOST: u8 = 190;

pub struct MinifbHardware {
    pub bw_ram: Cell<[u8; LANDSCAPE_BYTES]>,
    pub red_ram: Cell<[u8; LANDSCAPE_BYTES]>,
    /// Portrait-space 480×800 framebuffer for flash phase (stored as 1bpp bytes).
    pub prev_fb: Cell<[u8; PORTRAIT_W * PORTRAIT_H / 8]>,
}

impl MinifbHardware {
    pub fn new() -> Self {
        Self {
            bw_ram: Cell::new([0x00; LANDSCAPE_BYTES]),
            red_ram: Cell::new([0x00; LANDSCAPE_BYTES]),
            prev_fb: Cell::new([0x00; PORTRAIT_W * PORTRAIT_H / 8]),
        }
    }
}

pub struct MinifbInterface {
    hw: MinifbHardware,
    window: Rc<RefCell<Window>>,
    ghost_buf_p: [u8; PORTRAIT_W * PORTRAIT_H],
    last_cmd: u8,
    cmd_arg_count: u8,
    // Landscape-space RAM address state
    x_start_l: u16,
    x_end_l: u16,
    y_start_l: u16,
    y_end_l: u16,
    x_counter_l: u16,
    y_counter_l: u16,
    x_inc: bool,
    y_dec: bool,
    ctrl1: u8,
    is_initialised: bool,
}

impl MinifbInterface {
    pub fn new(hw: MinifbHardware, window: Rc<RefCell<Window>>) -> Self {
        Self {
            hw,
            window,
            ghost_buf_p: [0; PORTRAIT_W * PORTRAIT_H],
            last_cmd: 0,
            cmd_arg_count: 0,
            x_start_l: 0,
            x_end_l: (LANDSCAPE_W - 1) as u16,
            y_start_l: 0,
            y_end_l: (LANDSCAPE_H - 1) as u16,
            x_counter_l: 0,
            y_counter_l: 0,
            x_inc: true,
            y_dec: true,
            ctrl1: 0,
            is_initialised: false,
        }
    }

    pub fn inner_ref(&self) -> &MinifbHardware { &self.hw }

    fn write_ram_bank(&mut self, red: bool, data: &[u8]) {
        let ram = if red { &self.hw.red_ram } else { &self.hw.bw_ram };
        let ptr: *mut u8 = ram.as_ptr() as *mut u8;

        for &byte in data.iter() {
            let x_l = self.x_counter_l as usize;
            let y_l = self.y_counter_l as usize;
            let byte_col = x_l / 8;
            let bit_shift = x_l % 8;
            let addr = y_l * LANDSCAPE_W / 8 + byte_col;

            if addr < LANDSCAPE_BYTES {
                if bit_shift == 0 {
                    unsafe { *ptr.add(addr) = byte; }
                } else {
                    let cur = unsafe { *ptr.add(addr) };
                    let bits_in_first = 8 - bit_shift;
                    let first_mask = (1u16 << bits_in_first) - 1;
                    let first_part = ((byte as u16) >> (8 - bits_in_first)) as u8;
                    unsafe { *ptr.add(addr) = (cur & !(first_mask as u8)) | first_part; }

                    let next_x_l = x_l + bits_in_first;
                    if next_x_l < self.x_end_l as usize + 1 && addr + 1 < LANDSCAPE_BYTES {
                        let nxt = unsafe { *ptr.add(addr + 1) };
                        let second_mask = (0xFFu16 << (8 - bit_shift)) & 0xFF;
                        let second_part = (byte << (8 - bit_shift)) & (second_mask as u8);
                        unsafe { *ptr.add(addr + 1) = (nxt & !(second_mask as u8)) | second_part; }
                    }
                }
            }
            if self.x_inc {
                self.x_counter_l += 8;
            } else {
                self.x_counter_l = self.x_counter_l.wrapping_sub(8);
            }
            if self.x_counter_l > self.x_end_l || self.x_counter_l < self.x_start_l {
                self.x_counter_l = self.x_start_l;
                if self.y_dec {
                    self.y_counter_l = self.y_counter_l.wrapping_sub(1);
                } else {
                    self.y_counter_l += 1;
                }
            }
        }
    }

    fn auto_write_ram_bank(&mut self, red: bool, value: u8) {
        let ram = if red { &self.hw.red_ram } else { &self.hw.bw_ram };
        let ptr: *mut u8 = ram.as_ptr() as *mut u8;
        for i in 0..LANDSCAPE_BYTES {
            unsafe { *ptr.add(i) = value; }
        }
    }

    /// Read landscape RAM, apply LUT, rotate to 480×800 portrait pixels.
    fn render_to_portrait(&self) -> Vec<u32> {
        let bw = &self.hw.bw_ram;
        let red = &self.hw.red_ram;
        let bwptr: *const u8 = bw.as_ptr() as *const u8;
        let rdptr: *const u8 = red.as_ptr() as *const u8;

        const LUT: [u8; 4] = [0, 1, 0, 1];

        // Step 1: walk landscape RAM, produce 800×480 raw pixels (landscape order)
        let mut raw_l = vec![0u32; LANDSCAPE_W * LANDSCAPE_H]; // indexed [ly * LANDSCAPE_W + lx]
        for ly in 0..LANDSCAPE_H {
            for lx in 0..LANDSCAPE_W {
                let byte_addr = ly * (LANDSCAPE_W / 8) + lx / 8;
                let bit = lx % 8;

                let bw_byte = if byte_addr < LANDSCAPE_BYTES {
                    unsafe { *bwptr.add(byte_addr) }
                } else {
                    0
                };
                let red_byte = if byte_addr < LANDSCAPE_BYTES {
                    unsafe { *rdptr.add(byte_addr) }
                } else {
                    0
                };

                let r_bit = if self.ctrl1 == CTRL1_BYPASS_RED {
                    0
                } else {
                    (red_byte >> (7 - bit)) & 1
                };
                let bw_bit = (bw_byte >> (7 - bit)) & 1;
                let idx = (r_bit << 1) | bw_bit;
                raw_l[ly * LANDSCAPE_W + lx] = if LUT[idx as usize] == 1 { WHITE } else { BLACK };
            }
        }

        // Step 2: rotate 800×480 landscape → 480×800 portrait (pixel buffer)
        let mut portrait = vec![0u32; PORTRAIT_W * PORTRAIT_H];
        for py in 0..PORTRAIT_H {
            for px in 0..PORTRAIT_W {
                // Portrait (px,py) ← Landscape (ly=px, lx=py)
                let ly = px;
                let lx = py;
                portrait[py * PORTRAIT_W + px] = raw_l[ly * LANDSCAPE_W + lx];
            }
        }
        portrait
    }

    fn read_prev_pixels(&self) -> Vec<u32> {
        let fb = &self.hw.prev_fb;
        let cells = fb.as_array_of_cells();
        let mut portrait = vec![0u32; PORTRAIT_W * PORTRAIT_H];
        for (i, byte) in cells.iter().enumerate() {
            for bit in 0..8 {
                let px = i * 8 + bit;
                let is_white = (byte.get() & (0x80 >> bit)) != 0;
                portrait[px] = if is_white { WHITE } else { BLACK };
            }
        }
        portrait
    }

    fn sleep_and_render(&mut self, ms: u64, pixels_p: &[u32]) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        self.window.borrow_mut().update_with_buffer(pixels_p, PORTRAIT_W, PORTRAIT_H).unwrap();
    }

    fn u8_to_pixel(v: u8) -> u32 {
        let t = v as f32 / 255.0;
        let r = (((BLACK >> 16) & 0xFF) as f32 * (1.0 - t) + ((WHITE >> 16) & 0xFF) as f32 * t) as u32;
        let g = (((BLACK >> 8) & 0xFF) as f32 * (1.0 - t) + ((WHITE >> 8) & 0xFF) as f32 * t) as u32;
        let b = ((BLACK & 0xFF) as f32 * (1.0 - t) + (WHITE & 0xFF) as f32 * t) as u32;
        (r << 16) | (g << 8) | b
    }

    fn render_ghost(&mut self) {
        let mut pixels_p = vec![0u32; PORTRAIT_W * PORTRAIT_H];
        for (i, &v) in self.ghost_buf_p.iter().enumerate() {
            pixels_p[i] = Self::u8_to_pixel(v);
        }
        self.window.borrow_mut().update_with_buffer(&pixels_p, PORTRAIT_W, PORTRAIT_H).unwrap();
    }

    fn store_prev(&mut self) {
        // Render current landscape RAM to portrait and store in prev_fb for flash phase.
        let portrait = self.render_to_portrait();
        let fb = &self.hw.prev_fb;
        let ptr: *mut u8 = fb.as_ptr() as *mut u8;

        for py in 0..PORTRAIT_H {
            for px in 0..PORTRAIT_W {
                let white = portrait[py * PORTRAIT_W + px] == WHITE;
                let byte_p = py * (PORTRAIT_W / 8) + px / 8;
                let bit_p = px % 8;
                if white {
                    unsafe { *ptr.add(byte_p) |= 0x80 >> bit_p; }
                } else {
                    unsafe { *ptr.add(byte_p) &= !(0x80 >> bit_p); }
                }
            }
        }
    }

    fn do_refresh(&mut self) {
        if !self.is_initialised {
            let pixels_p = vec![BLACK; PORTRAIT_W * PORTRAIT_H];
            self.sleep_and_render(FLASH_PHASE_MS * 2, &pixels_p);
            self.is_initialised = true;
        }

        if self.ctrl1 == CTRL1_BYPASS_RED {
            // Full refresh: flash sequence
            for _ in 0..FLASH_CYCLES {
                let prev_p = self.read_prev_pixels();
                self.sleep_and_render(FLASH_PHASE_MS, &prev_p);

                let black_p = vec![BLACK; PORTRAIT_W * PORTRAIT_H];
                self.sleep_and_render(FLASH_PHASE_MS, &black_p);

                let new_p = self.render_to_portrait();
                self.sleep_and_render(FLASH_PHASE_MS, &new_p);

                let white_p = vec![WHITE; PORTRAIT_W * PORTRAIT_H];
                self.sleep_and_render(FLASH_PHASE_MS, &white_p);
            }

            let pixels_p = self.render_to_portrait();
            self.sleep_and_render(0, &pixels_p);

            for (i, &p) in pixels_p.iter().enumerate() {
                self.ghost_buf_p[i] = if p == WHITE { 255 } else { 0 };
            }
        } else {
            // Partial refresh: ghosting effect
            let new_p = self.render_to_portrait();

            for (i, &new) in new_p.iter().enumerate() {
                let old_white = self.ghost_buf_p[i] > 127;
                let new_white = new == WHITE;
                self.ghost_buf_p[i] = if old_white == new_white {
                    let target: u8 = if new_white { 255 } else { 0 };
                    let current = self.ghost_buf_p[i] as i16;
                    (current + (target as i16 - current) / 4) as u8
                } else if old_white && !new_white {
                    DARK_GHOST
                } else {
                    LIGHT_GHOST
                };
            }

            self.render_ghost();
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Second-phase: resolve ghosting toward target.
            for (i, &new) in new_p.iter().enumerate() {
                let target: u8 = if new == WHITE { 255 } else { 0 };
                let current = self.ghost_buf_p[i] as i16;
                self.ghost_buf_p[i] = (current + (target as i16 - current) / 2) as u8;
            }
            self.render_ghost();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        self.store_prev();

        core::mem::swap(&mut self.hw.bw_ram, &mut self.hw.red_ram);
    }
}

impl DisplayInterface for MinifbInterface {
    fn write_command(&mut self, cmd: u8) {
        self.last_cmd = cmd;
        self.cmd_arg_count = 0;
        if cmd == CMD_MASTER_ACTIVATION {
            self.do_refresh();
        }
    }

    fn write_data(&mut self, data: &[u8]) {
        match self.last_cmd {
            CMD_WRITE_RAM_BW => self.write_ram_bank(false, data),
            CMD_WRITE_RAM_RED => self.write_ram_bank(true, data),
            CMD_AUTO_WRITE_BW => {
                for &b in data { self.auto_write_ram_bank(false, b); }
            }
            CMD_AUTO_WRITE_RED => {
                for &b in data { self.auto_write_ram_bank(true, b); }
            }
            CMD_DATA_ENTRY_MODE => {
                if let Some(&mode) = data.first() {
                    self.x_inc = (mode & 0x01) != 0;
                    self.y_dec = (mode & 0x02) == 0;
                }
            }
            CMD_SET_RAM_X_COUNTER => {
                for &b in data {
                    if self.cmd_arg_count == 0 {
                        self.x_counter_l = b as u16;
                    } else if self.cmd_arg_count == 1 {
                        self.x_counter_l |= (b as u16) << 8;
                    }
                    self.cmd_arg_count += 1;
                }
            }
            CMD_SET_RAM_Y_COUNTER => {
                for &b in data {
                    if self.cmd_arg_count == 0 {
                        self.y_counter_l = b as u16;
                    } else if self.cmd_arg_count == 1 {
                        self.y_counter_l |= (b as u16) << 8;
                    }
                    self.cmd_arg_count += 1;
                }
            }
            CMD_SET_RAM_X_RANGE => {
                for &b in data {
                    match self.cmd_arg_count {
                        0 => self.x_start_l = b as u16,
                        1 => self.x_start_l |= (b as u16) << 8,
                        2 => self.x_end_l = b as u16,
                        3 => self.x_end_l |= (b as u16) << 8,
                        _ => {}
                    }
                    self.cmd_arg_count += 1;
                }
            }
            CMD_SET_RAM_Y_RANGE => {
                for &b in data {
                    match self.cmd_arg_count {
                        0 => self.y_start_l = b as u16,
                        1 => self.y_start_l |= (b as u16) << 8,
                        2 => self.y_end_l = b as u16,
                        3 => self.y_end_l |= (b as u16) << 8,
                        _ => {}
                    }
                    self.cmd_arg_count += 1;
                }
            }
            CMD_DISPLAY_UPDATE_CTRL1 => {
                if let Some(&ctl) = data.first() {
                    self.ctrl1 = ctl;
                }
            }
            _ => {}
        }
    }

    fn read_data(&mut self, data: &mut [u8]) {
        if self.last_cmd == CMD_READ_RAM {
            let ptr: *const u8 = self.hw.bw_ram.as_ptr() as *const u8;
            for i in 0..data.len() {
                let y_l = self.y_counter_l as usize;
                let x_byte = (self.x_counter_l as usize) / 8;
                let addr = y_l * LANDSCAPE_W / 8 + x_byte;
                data[i] = if addr < LANDSCAPE_BYTES {
                    unsafe { *ptr.add(addr) }
                } else {
                    0
                };
                if self.x_inc {
                    self.x_counter_l += 8;
                } else {
                    self.x_counter_l = self.x_counter_l.wrapping_sub(8);
                }
                if self.x_counter_l > self.x_end_l || self.x_counter_l < self.x_start_l {
                    self.x_counter_l = self.x_start_l;
                    if self.y_dec {
                        self.y_counter_l = self.y_counter_l.wrapping_sub(1);
                    } else {
                        self.y_counter_l += 1;
                    }
                }
            }
        }
    }

    fn reset(&mut self) {
        self.last_cmd = 0;
        self.cmd_arg_count = 0;
        self.x_start_l = 0;
        self.x_end_l = (LANDSCAPE_W - 1) as u16;
        self.y_start_l = 0;
        self.y_end_l = (LANDSCAPE_H - 1) as u16;
        self.x_counter_l = 0;
        self.y_counter_l = 0;
        self.x_inc = true;
        self.y_dec = false;
    }

    fn busy_high(&self) -> bool {
        false
    }
}
