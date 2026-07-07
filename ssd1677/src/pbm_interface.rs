// In-memory mock transport for the SSD1677.
// Tracks the last command byte so that write_data after WriteRamBw/WriteRamRed
// goes to the correct RAM bank.

use core::cell::Cell;
use std::io::Write;
use crate::DisplayInterface;

pub const DISPLAY_WIDTH: usize = 800;
pub const DISPLAY_HEIGHT: usize = 480;
pub const DISPLAY_BYTES: usize = DISPLAY_WIDTH * DISPLAY_HEIGHT / 8;

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

pub struct PbmHardware {
    pub bw_ram: Cell<[u8; DISPLAY_BYTES]>,
    pub red_ram: Cell<[u8; DISPLAY_BYTES]>,
    pub screen: Cell<[u8; DISPLAY_BYTES]>,
}

impl PbmHardware {
    pub fn new() -> Self {
        Self {
            bw_ram: Cell::new([0x00; DISPLAY_BYTES]),
            red_ram: Cell::new([0x00; DISPLAY_BYTES]),
            screen: Cell::new([0xFF; DISPLAY_BYTES]), // start white
        }
    }
}

pub struct PbmInterface {
    hw: PbmHardware,
    last_cmd: u8,
    cmd_arg_count: u8,
    // RAM address state (mimics SSD1677 internal counters)
    x_start: u16,
    x_end: u16,
    y_start: u16,
    y_end: u16,
    x_counter: u16,
    y_counter: u16,
    x_inc: bool,
    y_dec: bool,
    // Refresh state tracking
    ctrl1: u8,
    frame_count: u64,
}

impl PbmInterface {
    pub fn new(hw: PbmHardware) -> Self {
        Self {
            hw,
            last_cmd: 0,
            cmd_arg_count: 0,
            x_start: 0,
            x_end: 799,
            y_start: 0,
            y_end: 479,
            x_counter: 0,
            y_counter: 0,
            x_inc: true,
            y_dec: true,
            ctrl1: 0,
            frame_count: 0,
        }
    }

    pub fn inner_ref(&self) -> &PbmHardware {
        &self.hw
    }

    fn write_ram_bank(&mut self, red: bool, data: &[u8]) {
        let ram = if red { &self.hw.red_ram } else { &self.hw.bw_ram };
        let ptr: *mut u8 = ram.as_ptr() as *mut u8;

        for &byte in data.iter() {
            let x = self.x_counter as usize;
            let y = self.y_counter as usize;
            let byte_col = x / 8;
            let bit_shift = x % 8; // how many bits from MSB to the X pixel
            let addr = y * DISPLAY_WIDTH / 8 + byte_col;

            if addr < DISPLAY_BYTES {
                if bit_shift == 0 {
                    // Byte-aligned: write entire byte
                    unsafe { *ptr.add(addr) = byte; }
                } else {
                    // Non-byte-aligned: split across two bytes.
                    // The incoming byte B maps such that B[7] lands at pixel X
                    // (bit position 7 - bit_shift within the current byte).
                    // Bits B[7..bit_shift] fill the remaining LSBits of current byte.
                    // Bits B[bit_shift-1..0] go to MSBits of the next byte.
                    let cur = unsafe { *ptr.add(addr) };
                    // How many bits fit in current byte (starting at bit_shift from MSB)
                    let bits_in_first = 8 - bit_shift; // e.g. 4 for bit_shift=4
                    // Upper `bits_in_first` bits of B go to lower `bits_in_first` bits of curr
                    let first_mask = (1u16 << bits_in_first) - 1; // e.g. 0x0F
                    let first_part = ((byte as u16) >> (8 - bits_in_first)) as u8;
                    let new_cur = (cur & !(first_mask as u8)) | first_part;
                    unsafe { *ptr.add(addr) = new_cur; }

                    // The remaining bits go to the next byte's upper bits
                    // Only write if the next byte is within the X window
                    let next_x = x + bits_in_first;
                    if next_x < self.x_end as usize + 1 && addr + 1 < DISPLAY_BYTES {
                        let nxt = unsafe { *ptr.add(addr + 1) };
                        let bits_in_second = bit_shift; // e.g. 4 for bit_shift=4
                        let second_mask = (0xFFu16 << (8 - bits_in_second)) & 0xFF;
                        let second_part = (byte << (8 - bits_in_second)) & (second_mask as u8);
                        let new_nxt = (nxt & !(second_mask as u8)) | second_part;
                        unsafe { *ptr.add(addr + 1) = new_nxt; }
                    }
                }
            }
            // Advance counter by 1 byte worth of pixels
            if self.x_inc {
                self.x_counter += 8;
            } else {
                self.x_counter = self.x_counter.wrapping_sub(8);
            }
            if self.x_counter > self.x_end || self.x_counter < self.x_start {
                self.x_counter = self.x_start;
                if self.y_dec {
                    self.y_counter = self.y_counter.wrapping_sub(1);
                } else {
                    self.y_counter += 1;
                }
            }
        }
    }

    fn auto_write_ram_bank(&mut self, red: bool, value: u8) {
        let ram = if red { &self.hw.red_ram } else { &self.hw.bw_ram };
        let ptr: *mut u8 = ram.as_ptr() as *mut u8;
        for i in 0..DISPLAY_BYTES {
            unsafe { *ptr.add(i) = value; }
        }
    }

    fn do_refresh(&mut self) {
        // Flash sequence for full refresh (hardware visual indicator)
        if self.ctrl1 == CTRL1_BYPASS_RED {
            self.write_solid_pbm(0xFF); // flash to black (P4: 1=black)
            self.write_solid_pbm(0x00); // flash to white (P4: 0=white)
        }

        let screen = &self.hw.screen;
        let bw = &self.hw.bw_ram;
        let red = &self.hw.red_ram;
        let sptr: *mut u8 = screen.as_ptr() as *mut u8;
        let bwptr: *const u8 = bw.as_ptr() as *const u8;
        let rdptr: *const u8 = red.as_ptr() as *const u8;

        // Lookup table: result bit for each (R, BW) pair
        // Index: bit 1 = R, bit 0 = BW
        // Table 6-5: (0,0)->Black(0), (0,1)->White(1), (1,0)->Black(0), (1,1)->White(1)
        const LUT: [u8; 4] = [0, 1, 0, 1];

        // Validate rendering invariant: Red must equal BW before refresh.
        for i in 0..DISPLAY_BYTES / 64 {
            let r = unsafe { *rdptr.add(i) };
            let b = unsafe { *bwptr.add(i) };
            if r != b {
                eprintln!("FAIL: Red!=BW at byte {}: Red={:02X}, BW={:02X}", i, r, b);
                break;
            }
        }

        if self.ctrl1 != CTRL1_BYPASS_RED {
            // Validate: Red should match the current screen before update.
            // If not, the previous Red write didn't complete correctly.
            for i in 0..DISPLAY_BYTES / 64 {
                let r = unsafe { *rdptr.add(i) };
                let s = unsafe { *sptr.add(i) };
                if r != s {
                    eprintln!("Red/Screen mismatch at byte {}: Red={:02X}, Screen={:02X}", i, r, s);
                    break;
                }
            }
        }

        for i in 0..DISPLAY_BYTES {
            let b = unsafe { *bwptr.add(i) };
            let r = if self.ctrl1 == CTRL1_BYPASS_RED { 0u8 } else { unsafe { *rdptr.add(i) } };
            let mut result: u8 = 0;
            for bit in 0..8 {
                let r_bit = (r >> bit) & 1;
                let b_bit = (b >> bit) & 1;
                let idx = (r_bit << 1) | b_bit;
                result |= LUT[idx as usize] << bit;
            }
            unsafe { *sptr.add(i) = result; }
        }

        // Write PBM output after every refresh
        self.write_pbm();

        // Simulate hardware buffer swap: after MasterActivation,
        // the controller swaps BW and Red RAM contents.
        core::mem::swap(&mut self.hw.bw_ram, &mut self.hw.red_ram);
    }

    fn write_solid_pbm(&mut self, fill: u8) {
        let _ = std::fs::create_dir_all("/tmp/xtx4_frames");
        let path = format!("/tmp/xtx4_frames/frame_{:04}.pbm", self.frame_count);
        if let Ok(mut f) = std::fs::File::create(&path) {
            let _ = f.write_all(b"P4\n480 800\n");
            let portrait = [fill; DISPLAY_BYTES];
            let _ = f.write_all(&portrait);
        }
        println!("frame_{:04}.pbm written (flash)", self.frame_count);
        self.frame_count += 1;
    }

    fn write_pbm(&mut self) {
        let pixel_buf = self.read_screen_buf();
        let _ = std::fs::create_dir_all("/tmp/xtx4_frames");
        let path = format!("/tmp/xtx4_frames/frame_{:04}.pbm", self.frame_count);
        if let Ok(mut f) = std::fs::File::create(&path) {
            let _ = f.write_all(b"P4\n480 800\n");
            let mut portrait = [0u8; DISPLAY_BYTES];
            for p_y in 0..(DISPLAY_WIDTH as usize) {
                for p_x in 0..(DISPLAY_HEIGHT as usize) {
                    let l_y = p_x;
                    let l_x = p_y;
                    let l_byte = l_y * (DISPLAY_WIDTH as usize / 8) + l_x / 8;
                    let l_bit = l_x % 8;
                    let p_byte = p_y * (DISPLAY_HEIGHT as usize / 8) + p_x / 8;
                    let p_bit = p_x % 8;
                    let raw_bit = (pixel_buf[l_byte] >> (7 - l_bit)) & 1;
                    let bit_val = 1 - raw_bit; // invert: 1=white->0, 0=black->1 in PBM
                    portrait[p_byte] |= bit_val << (7 - p_bit);
                }
            }
            let _ = f.write_all(&portrait);
        }
        println!("frame_{:04}.pbm written", self.frame_count);
        self.frame_count += 1;
    }

    fn read_screen_buf(&self) -> [u8; DISPLAY_BYTES] {
        let mut buf = [0u8; DISPLAY_BYTES];
        let ptr: *const u8 = self.hw.screen.as_ptr() as *const u8;
        for i in 0..DISPLAY_BYTES {
            buf[i] = unsafe { *ptr.add(i) };
        }
        buf
    }
}

impl DisplayInterface for PbmInterface {
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
                        self.x_counter = b as u16;
                    } else if self.cmd_arg_count == 1 {
                        self.x_counter |= (b as u16) << 8;
                    }
                    self.cmd_arg_count += 1;
                }
            }
            CMD_SET_RAM_Y_COUNTER => {
                for &b in data {
                    if self.cmd_arg_count == 0 {
                        self.y_counter = b as u16;
                    } else if self.cmd_arg_count == 1 {
                        self.y_counter |= (b as u16) << 8;
                    }
                    self.cmd_arg_count += 1;
                }
            }
            CMD_SET_RAM_X_RANGE => {
                for &b in data {
                    match self.cmd_arg_count {
                        0 => self.x_start = b as u16,
                        1 => self.x_start |= (b as u16) << 8,
                        2 => self.x_end = b as u16,
                        3 => self.x_end |= (b as u16) << 8,
                        _ => {}
                    }
                    self.cmd_arg_count += 1;
                }
            }
            CMD_SET_RAM_Y_RANGE => {
                for &b in data {
                    match self.cmd_arg_count {
                        0 => self.y_start = b as u16,
                        1 => self.y_start |= (b as u16) << 8,
                        2 => self.y_end = b as u16,
                        3 => self.y_end |= (b as u16) << 8,
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
                let y = self.y_counter as usize;
                let x_byte = (self.x_counter as usize) / 8;
                let addr = y * DISPLAY_WIDTH / 8 + x_byte;
                data[i] = if addr < DISPLAY_BYTES {
                    unsafe { *ptr.add(addr) }
                } else {
                    0
                };
                if self.x_inc {
                    self.x_counter += 8;
                } else {
                    self.x_counter = self.x_counter.wrapping_sub(8);
                }
                if self.x_counter > self.x_end || self.x_counter < self.x_start {
                    self.x_counter = self.x_start;
                    if self.y_dec {
                        self.y_counter = self.y_counter.wrapping_sub(1);
                    } else {
                        self.y_counter += 1;
                    }
                }
            }
        }
    }

    fn reset(&mut self) {
        self.last_cmd = 0;
        self.cmd_arg_count = 0;
        self.x_start = 0;
        self.x_end = DISPLAY_WIDTH as u16 - 1;
        self.y_start = 0;
        self.y_end = DISPLAY_HEIGHT as u16 - 1;
        self.x_counter = 0;
        self.y_counter = 0;
        self.x_inc = true;
        self.y_dec = false;
    }

    fn busy_high(&self) -> bool {
        false
    }
}
